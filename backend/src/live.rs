use crate::{middleware::AuthUser, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/128.0 Safari/537.36";

const SCHEDULE_TTL: Duration = Duration::from_secs(300);
const TOKEN_TTL: Duration = Duration::from_secs(4 * 3600);

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/api/live/hls/{token}/master.m3u8", get(hls_master))
        .route("/api/live/hls/{token}/p/{hex}", get(hls_proxy));

    let authed = Router::new()
        .route("/api/live/schedule", get(list_schedule))
        .route("/api/live/resolve", post(resolve_handler))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ));

    public.merge(authed)
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveGame {
    pub id: String,
    pub sport: String,
    pub league: String,
    pub label: String,
    pub away: String,
    pub home: String,
    pub status: String,
    pub start_hint: Option<String>,
    pub starts_in: Option<i64>,
    pub source_url: String,
    pub espn: Option<crate::espn::EspnMatch>,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .http1_only()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("build live client")
}

struct ScheduleCache {
    fetched_at: Instant,
    games: Vec<LiveGame>,
}
static SCHEDULE: LazyLock<RwLock<Option<ScheduleCache>>> = LazyLock::new(|| RwLock::new(None));

static REFRESHING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
static LAST_ASKED: LazyLock<RwLock<Option<Instant>>> = LazyLock::new(|| RwLock::new(None));

async fn refresh_schedule() -> Result<Vec<LiveGame>, String> {
    let t0 = Instant::now();
    let games = fetch_all_indexes().await?;
    eprintln!(
        "[live] schedule refreshed in {:.1}s: {} games",
        t0.elapsed().as_secs_f32(),
        games.len()
    );
    *SCHEDULE.write().await = Some(ScheduleCache {
        fetched_at: Instant::now(),
        games: games.clone(),
    });
    Ok(games)
}

async fn cached_games() -> Option<(Vec<LiveGame>, Duration)> {
    SCHEDULE
        .read()
        .await
        .as_ref()
        .map(|c| (c.games.clone(), c.fetched_at.elapsed()))
}

async fn get_schedule() -> Vec<LiveGame> {
    *LAST_ASKED.write().await = Some(Instant::now());
    match cached_games().await {
        Some((games, age)) if age < SCHEDULE_TTL => games,
        Some((games, _)) => {
            tokio::spawn(async {
                let Ok(_guard) = REFRESHING.try_lock() else {
                    return;
                };
                if let Err(e) = refresh_schedule().await {
                    eprintln!("[live] background refresh failed: {e}");
                }
            });
            games
        }
        None => {
            let _guard = REFRESHING.lock().await;
            if let Some((games, age)) = cached_games().await {
                if age < SCHEDULE_TTL {
                    return games;
                }
            }
            refresh_schedule().await.unwrap_or_else(|e| {
                eprintln!("[live] schedule fetch failed: {e}");
                Vec::new()
            })
        }
    }
}

pub fn spawn_refresher() {
    let every = std::env::var("LIVE_REFRESH_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n >= 30)
        .unwrap_or(60);
    let idle_after = Duration::from_secs(15 * 60);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(every));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let asked = *LAST_ASKED.read().await;
            let idle = match asked {
                Some(t) => t.elapsed() > idle_after,
                None => SCHEDULE.read().await.is_some(),
            };
            if idle {
                continue;
            }
            let Ok(_guard) = REFRESHING.try_lock() else {
                continue;
            };
            if let Err(e) = refresh_schedule().await {
                eprintln!("[live] scheduled refresh failed: {e}");
            }
        }
    });
}

fn index_sources() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::env::var("LIVE_INDEX_URLS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|pair| {
            let (k, url) = pair.split_once('=')?;
            let (k, url) = (k.trim(), url.trim());
            if k.is_empty() || !url.starts_with("http") {
                return None;
            }
            Some((k.to_ascii_lowercase(), url.to_string()))
        })
        .collect();
    if out.is_empty() {
        if let Ok(url) = std::env::var("LIVE_CFB_INDEX_URL") {
            if url.starts_with("http") {
                out.push(("cfb".into(), url));
            }
        }
    }
    out
}
fn allowed_origins() -> Vec<String> {
    std::env::var("LIVE_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
fn source_allowed(url: &str) -> bool {
    let allow = allowed_origins();
    if allow.is_empty() {
        return false;
    }
    allow.iter().any(|prefix| url.starts_with(prefix))
}

async fn fetch_all_indexes() -> Result<Vec<LiveGame>, String> {
    let sources = index_sources();
    if sources.is_empty() && !crate::livetv::enabled() {
        return Err("LIVE_INDEX_URLS not set".into());
    }
    let livetv_task = if crate::livetv::enabled() {
        Some(tokio::spawn(async { crate::livetv::fetch_index().await }))
    } else {
        None
    };
    let mut set = tokio::task::JoinSet::new();
    for (sport, url) in sources {
        set.spawn(async move {
            let html = match client().get(&url).send().await {
                Ok(r) => r.text().await.unwrap_or_default(),
                Err(e) => {
                    eprintln!("[live] index {sport} fetch failed: {e}");
                    String::new()
                }
            };
            let games = parse_index(&html, &sport);
            eprintln!(
                "[live] index {sport}: {} bytes -> {} games",
                html.len(),
                games.len()
            );
            games
        });
    }
    let mut merged: HashMap<String, LiveGame> = HashMap::new();
    while let Some(res) = set.join_next().await {
        let Ok(games) = res else { continue };
        for g in games {
            let named = !sport_from_href(&g.source_url, "").is_empty();
            match merged.get(&g.id) {
                Some(prev) if !named || !sport_from_href(&prev.source_url, "").is_empty() => {}
                _ => {
                    merged.insert(g.id.clone(), g);
                }
            }
        }
    }
    if let Some(task) = livetv_task {
        match task.await {
            Ok(Ok(games)) => {
                for g in games {
                    merged.entry(g.id.clone()).or_insert(g);
                }
            }
            Ok(Err(e)) => eprintln!("[livetv] index failed: {e}"),
            Err(e) => eprintln!("[livetv] index task died: {e}"),
        }
    }
    let mut out: Vec<LiveGame> = merged.into_values().collect();
    out.sort_by(|a, b| a.sport.cmp(&b.sport).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

fn parse_index(html: &str, sport: &str) -> Vec<LiveGame> {
    let mut out = Vec::new();
    let marker = "f1-podium--item match-card";
    let mut cursor = 0;
    while let Some(i) = html[cursor..].find(marker) {
        let start = cursor + i;
        let end = html[start..]
            .find("</li>")
            .map(|e| start + e)
            .unwrap_or(html.len());
        let block = &html[start..end];
        cursor = end + 5;

        let href = match between(block, "href=\"", "\"") {
            Some(h) => h,
            None => continue,
        };
        if !href.starts_with("http") {
            continue;
        }
        let label = between(block, "d-md-inline", "</span>")
            .and_then(|s| s.split_once('>').map(|(_, r)| collapse_ws(&strip_tags(r))))
            .map(|s| decode_entities(&s))
            .unwrap_or_default();
        let league = between(block, "f1-podium--rank", "</span>")
            .and_then(|s| s.split_once('>').map(|(_, r)| collapse_ws(&strip_tags(r))))
            .map(|s| decode_entities(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| sport.to_ascii_uppercase());
        let hint = extract_status_hint(block).map(|s| collapse_ws(&strip_tags(&s)));
        let status = match hint.as_deref().map(str::to_ascii_lowercase).as_deref() {
            Some("live") => "live",
            Some("final") | Some("ended") | Some("ft") => "final",
            _ => "scheduled",
        }
        .to_string();
        let starts_in = hint.as_deref().and_then(parse_starts_in);

        let (away, home) = split_teams(&label);
        let slug = href.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        if label.is_empty() || slug.is_empty() {
            continue;
        }
        out.push(LiveGame {
            id: slug.to_string(),
            sport: sport_from_href(&href, sport),
            league,
            label,
            away,
            home,
            status,
            start_hint: hint,
            starts_in,
            source_url: href,
            espn: None,
        });
    }
    out
}

fn sport_from_href(href: &str, fallback: &str) -> String {
    let path = href.split_once("://").map(|(_, r)| r).unwrap_or(href);
    let seg = path
        .split('/')
        .skip(1)
        .find(|s| !s.is_empty())
        .unwrap_or("");
    let ok = !seg.is_empty()
        && seg != "stream"
        && seg.len() <= 32
        && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if ok {
        seg.to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

fn parse_starts_in(hint: &str) -> Option<i64> {
    let h = hint.to_ascii_lowercase();
    if !h.contains("from now") {
        return None;
    }
    let n: i64 = h.split_whitespace().next()?.parse().ok()?;
    if h.contains("minute") {
        Some(n)
    } else if h.contains("hour") {
        Some(n * 60)
    } else if h.contains("day") {
        Some(n * 60 * 24)
    } else {
        None
    }
}

fn extract_status_hint(block: &str) -> Option<String> {
    let start = block.find("SaatZamanBilgisi")?;
    let after = &block[start..];
    let open_end = after.find('>')?;
    let mut i = open_end + 1;
    let mut depth: i32 = 1;
    let mut out = String::new();
    while i < after.len() {
        if let Some(tag_start) = after[i..].find('<') {
            out.push_str(&after[i..i + tag_start]);
            i += tag_start;
            let tag_end = i + after[i..].find('>').unwrap_or(after.len() - i);
            let tag = &after[i..tag_end + 1];
            if tag.starts_with("</") {
                depth -= 1;
                if depth == 0 {
                    return Some(out);
                }
            } else if !tag.ends_with("/>") {
                depth += 1;
            }
            out.push_str(tag);
            i = tag_end + 1;
        } else {
            out.push_str(&after[i..]);
            break;
        }
    }
    Some(out)
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn decode_entities(s: &str) -> String {
    let named = s
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    let mut out = String::with_capacity(named.len());
    let mut rest = named.as_str();
    while let Some(i) = rest.find("&#") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        match tail.find(';').and_then(|e| {
            let n: u32 = tail[..e].parse().ok()?;
            char::from_u32(n).map(|c| (c, e))
        }) {
            Some((c, e)) => {
                out.push(c);
                rest = &tail[e + 1..];
            }
            None => {
                out.push_str("&#");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out.replace("&amp;", "&")
}

fn between(s: &str, a: &str, b: &str) -> Option<String> {
    let i = s.find(a)? + a.len();
    let rest = &s[i..];
    let j = rest.find(b)?;
    Some(rest[..j].to_string())
}
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn split_teams(s: &str) -> (String, String) {
    if let Some((a, h)) = s.split_once(" vs ") {
        (a.trim().to_string(), h.trim().to_string())
    } else {
        (String::new(), s.trim().to_string())
    }
}

struct ResolvedChain {
    master_url: String,
    embed_url: String,
}

async fn resolve_master(source_url: &str) -> Result<ResolvedChain, String> {
    let c = client();
    let mut page = String::new();
    let mut found = None;
    for _ in 0..2 {
        page = c
            .get(source_url)
            .send()
            .await
            .map_err(|e| format!("source fetch: {e}"))?
            .text()
            .await
            .map_err(|e| format!("source read: {e}"))?;
        found = extract_embed_iframe(&page);
        if found.is_some() {
            break;
        }
    }
    let embed = found.ok_or_else(|| {
        eprintln!(
            "[live] no player iframe ({} bytes, marker {})",
            page.len(),
            page.contains("wp_player")
        );
        "dead stream".to_string()
    })?;

    let embed_html = c
        .get(&embed)
        .header("Referer", source_url)
        .send()
        .await
        .map_err(|e| format!("embed fetch: {e}"))?
        .text()
        .await
        .map_err(|e| format!("embed read: {e}"))?;

    let b64 = extract_loader_b64(&embed_html)
        .ok_or_else(|| "no base64 loader in embed page".to_string())?;
    let loader = decode_b64_str(&b64).ok_or_else(|| "loader b64 decode failed".to_string())?;
    if !loader.starts_with("http") {
        return Err("loader is not an http url".into());
    }
    let referer = origin_of(&embed);
    probe_variant(&c, &loader, &referer).await?;
    Ok(ResolvedChain {
        master_url: loader,
        embed_url: embed,
    })
}

async fn probe_variant(c: &reqwest::Client, loader: &str, referer: &str) -> Result<(), String> {
    let text = c
        .get(loader)
        .header("Referer", referer)
        .send()
        .await
        .map_err(|e| format!("loader fetch: {e}"))?
        .text()
        .await
        .map_err(|e| format!("loader read: {e}"))?;
    if !text.contains("#EXT-X-STREAM-INF") {
        return Ok(());
    }
    let Some(first) = first_uri(&text) else {
        return Err("dead stream".into());
    };
    let abs = join_url(loader, &first);
    let status = c
        .get(&abs)
        .header("Referer", referer)
        .send()
        .await
        .map_err(|e| format!("variant probe: {e}"))?
        .status();
    if status.is_success() {
        Ok(())
    } else {
        eprintln!("[live] dead variant {} {}", status.as_u16(), host_of(&abs));
        Err("dead stream".into())
    }
}

fn first_uri(playlist: &str) -> Option<String> {
    playlist
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
}

fn join_url(base: &str, line: &str) -> String {
    if line.starts_with("http://") || line.starts_with("https://") {
        return line.to_string();
    }
    if line.starts_with('/') {
        return format!("{}{}", origin_of(base).trim_end_matches('/'), line);
    }
    let dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    format!("{dir}/{line}")
}

fn extract_embed_iframe(html: &str) -> Option<String> {
    let mut fallback: Option<String> = None;
    let mut pos = 0;
    while let Some(i) = html[pos..].find("<iframe") {
        let abs = pos + i;
        let end = html[abs..]
            .find('>')
            .map(|e| abs + e + 1)
            .unwrap_or(html.len());
        let tag = &html[abs..end];
        pos = end;
        let Some(src) = tag_attr(tag, "src") else {
            continue;
        };
        if !src.starts_with("http") {
            continue;
        }
        if tag.contains("wp_player") {
            return Some(src);
        }
        if fallback.is_none() && (src.contains("embed") || src.contains("player")) {
            fallback = Some(src);
        }
    }
    fallback
}

fn tag_attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let mut pos = 0;
    while let Some(i) = tag[pos..].find(&key) {
        let abs = pos + i;
        let boundary = abs == 0 || tag.as_bytes()[abs - 1].is_ascii_whitespace();
        pos = abs + key.len();
        if !boundary {
            continue;
        }
        let rest = &tag[pos..];
        let q = *rest.as_bytes().first()?;
        if q == b'"' || q == b'\'' {
            let close = rest[1..].find(q as char)? + 1;
            return Some(rest[1..close].to_string());
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        if end > 0 {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn extract_loader_b64(html: &str) -> Option<String> {
    let needle = "aHR0c";
    let mut pos = 0;
    while let Some(i) = html[pos..].find(needle) {
        let abs = pos + i;
        let quote = if abs > 0 { html.as_bytes()[abs - 1] } else { 0 };
        if quote != b'"' && quote != b'\'' && quote != b'`' {
            pos = abs + needle.len();
            continue;
        }
        let close = quote as char;
        let rest = &html[abs..];
        let end = match rest.find(close) {
            Some(e) => e,
            None => break,
        };
        let cand = &rest[..end];
        if cand.len() > 24
            && cand
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
        {
            return Some(cand.to_string());
        }
        pos = abs + end;
    }
    None
}

fn decode_b64_str(s: &str) -> Option<String> {
    let bytes = decode_b64(s)?;
    String::from_utf8(bytes).ok()
}
fn decode_b64(s: &str) -> Option<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let stripped = clean.trim_end_matches('=');
    let mut out = Vec::with_capacity(stripped.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in stripped.chars() {
        let v: u32 = match c {
            'A'..='Z' => (c as u32) - 'A' as u32,
            'a'..='z' => (c as u32) - 'a' as u32 + 26,
            '0'..='9' => (c as u32) - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

#[derive(Clone)]
pub(crate) struct StreamToken {
    pub(crate) master_url: String,
    pub(crate) referer: String,
    created_at: Instant,
}
static TOKENS: LazyLock<RwLock<HashMap<String, StreamToken>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub(crate) async fn issue_token(master_url: String, referer: String) -> String {
    prune_tokens().await;
    let id = uuid::Uuid::new_v4().to_string();
    TOKENS.write().await.insert(
        id.clone(),
        StreamToken {
            master_url,
            referer,
            created_at: Instant::now(),
        },
    );
    id
}
pub(crate) async fn load_token(id: &str) -> Option<StreamToken> {
    let tks = TOKENS.read().await;
    let t = tks.get(id)?.clone();
    if t.created_at.elapsed() > TOKEN_TTL {
        return None;
    }
    Some(t)
}
async fn prune_tokens() {
    TOKENS
        .write()
        .await
        .retain(|_, t| t.created_at.elapsed() < TOKEN_TTL);
}

#[derive(Serialize)]
struct ScheduleResp {
    games: Vec<LiveGame>,
    fetched_at: String,
}

async fn list_schedule(
    Extension(_auth): Extension<AuthUser>,
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut games = get_schedule().await;
    crate::espn::enrich(&mut games).await;
    Json(ScheduleResp {
        games,
        fetched_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[derive(Deserialize)]
struct ResolveReq {
    source_url: String,
}

#[derive(Serialize)]
struct ResolveResp {
    token: String,
    master_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    youtube: Option<String>,
}

async fn resolve_handler(
    Extension(_auth): Extension<AuthUser>,
    State(_state): State<Arc<AppState>>,
    Json(body): Json<ResolveReq>,
) -> Response {
    if !source_allowed(&body.source_url) {
        return (StatusCode::BAD_REQUEST, "unsupported source").into_response();
    }
    if crate::livetv::is_livetv_url(&body.source_url) {
        return match crate::livetv::resolve(&body.source_url).await {
            Ok((master, referer)) => {
                eprintln!("[livetv] resolved: master_host={}", host_of(&master));
                let token = issue_token(master, referer).await;
                let master_url = format!("/api/live/hls/{token}/master.m3u8");
                Json(ResolveResp {
                    token,
                    master_url,
                    youtube: None,
                })
                .into_response()
            }
            Err(e) if e.starts_with("youtube:") => {
                let id = e.trim_start_matches("youtube:").to_string();
                eprintln!("[livetv] youtube-only event, handing off id {id}");
                Json(ResolveResp {
                    token: String::new(),
                    master_url: String::new(),
                    youtube: Some(id),
                })
                .into_response()
            }
            Err(e) => {
                eprintln!("[livetv] resolve failed: {e}");
                let msg: String = match e.as_str() {
                    "dead stream" | "youtube only" | "p2p only" => e,
                    _ => "resolve failed".to_string(),
                };
                (StatusCode::BAD_GATEWAY, msg).into_response()
            }
        };
    }
    let chain = match resolve_master(&body.source_url).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live] resolve failed: {e}");
            let msg = if e == "dead stream" {
                "dead stream"
            } else {
                "resolve failed"
            };
            return (StatusCode::BAD_GATEWAY, msg).into_response();
        }
    };
    let referer = origin_of(&chain.embed_url);
    eprintln!(
        "[live] resolved token: master_host={} referer={}",
        host_of(&chain.master_url),
        referer
    );
    let token = issue_token(chain.master_url, referer).await;
    let master_url = format!("/api/live/hls/{token}/master.m3u8");
    Json(ResolveResp {
        token,
        master_url,
        youtube: None,
    })
    .into_response()
}

async fn hls_master(Path(token): Path<String>) -> Response {
    let Some(t) = load_token(&token).await else {
        eprintln!("[live] master: unknown token {}", short_tok(&token));
        return (StatusCode::NOT_FOUND, "token expired").into_response();
    };
    eprintln!(
        "[live] master {} -> {} (ref {})",
        short_tok(&token),
        host_of(&t.master_url),
        t.referer
    );
    proxy_playlist(&token, &t.master_url, &t.referer).await
}

async fn hls_proxy(Path((token, hex)): Path<(String, String)>) -> Response {
    let Some(t) = load_token(&token).await else {
        eprintln!("[live] proxy: unknown token {}", short_tok(&token));
        return (StatusCode::NOT_FOUND, "token expired").into_response();
    };
    let Some(upstream) = hex_decode_url(&hex) else {
        return (StatusCode::BAD_REQUEST, "bad url hex").into_response();
    };

    let c = client();
    let resp = match c.get(&upstream).header("Referer", &t.referer).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[live] upstream fetch {}: {e}", host_of(&upstream));
            return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
        }
    };
    let status = resp.status();
    let final_url = resp.url().to_string();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let headers = resp.headers().clone();

    let body = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[live] upstream body {}: {e}", host_of(&upstream));
            return (StatusCode::BAD_GATEWAY, "upstream body").into_response();
        }
    };
    if is_playlist_body(&body) {
        let text = String::from_utf8_lossy(&body);
        let seg_count = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .count();
        eprintln!(
            "[live] variant {} {} -> {} segs",
            status.as_u16(),
            host_of(&upstream),
            seg_count
        );
        let rewritten = rewrite_playlist(&text, &final_url, &token);
        return (
            status,
            [("content-type", "application/vnd.apple.mpegurl")],
            rewritten,
        )
            .into_response();
    }

    let mut out = Response::builder().status(status.as_u16());
    for (n, v) in headers.iter() {
        let name = n.as_str().to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "content-type" | "cache-control" | "accept-ranges"
        ) {
            out = out.header(n, v);
        }
    }
    if !ct.contains("video") && !ct.contains("mp2t") {
        out = out.header("content-type", "video/mp2t");
    }
    if status.as_u16() >= 400 {
        eprintln!(
            "[live] segment {} {}{} ct={ct} body={}",
            status.as_u16(),
            host_of(&upstream),
            path_of(&upstream),
            String::from_utf8_lossy(&body[..body.len().min(120)])
        );
    }
    out.body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn is_playlist_body(body: &[u8]) -> bool {
    body.iter()
        .position(|b| !b.is_ascii_whitespace() && *b != 0xEF && *b != 0xBB && *b != 0xBF)
        .is_some_and(|i| body[i..].starts_with(b"#EXTM3U"))
}

fn path_of(url: &str) -> String {
    url.split_once("://")
        .and_then(|(_, r)| r.split_once('/').map(|(_, p)| format!("/{p}")))
        .unwrap_or_default()
}

fn short_tok(t: &str) -> &str {
    t.get(..8).unwrap_or(t)
}

async fn proxy_playlist(token: &str, url: &str, referer: &str) -> Response {
    let c = client();
    let resp = match c.get(url).header("Referer", referer).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[live] playlist fetch: {e}");
            return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
        }
    };
    let base = resp.url().to_string();
    let text = resp.text().await.unwrap_or_default();
    let rewritten = rewrite_playlist(&text, &base, token);
    (
        [("content-type", "application/vnd.apple.mpegurl")],
        rewritten,
    )
        .into_response()
}

fn rewrite_playlist(text: &str, base_url: &str, token: &str) -> String {
    let origin = origin_of(base_url);
    let dir = base_url
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();
    let proxied = |uri: &str| {
        let abs = if uri.starts_with("http://") || uri.starts_with("https://") {
            uri.to_string()
        } else if uri.starts_with('/') {
            format!("{}{}", origin.trim_end_matches('/'), uri)
        } else {
            format!("{dir}/{uri}")
        };
        format!("/api/live/hls/{token}/p/{}", hex_encode(abs.as_bytes()))
    };
    let mut out = String::with_capacity(text.len());
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            out.push('\n');
            continue;
        }
        if line.starts_with('#') {
            match line.find("URI=\"") {
                Some(i) => {
                    let start = i + 5;
                    let end = line[start..]
                        .find('"')
                        .map(|e| start + e)
                        .unwrap_or(line.len());
                    out.push_str(&line[..start]);
                    out.push_str(&proxied(&line[start..end]));
                    out.push_str(&line[end..]);
                }
                None => out.push_str(line),
            }
            out.push('\n');
            continue;
        }
        out.push_str(&proxied(line));
        out.push('\n');
    }
    out
}

pub(crate) fn origin_of(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((host, _)) = rest.split_once('/') {
            return format!("{scheme}://{host}/");
        }
        return format!("{scheme}://{rest}/");
    }
    String::new()
}

fn host_of(url: &str) -> String {
    url.split_once("://")
        .and_then(|(_, r)| r.split_once('/').map(|(h, _)| h.to_string()))
        .unwrap_or_default()
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}
fn hex_decode_url(s: &str) -> Option<String> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks(2) {
        let hi = hex_val(chunk[0])?;
        let lo = hex_val(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    String::from_utf8(out).ok()
}
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEAD_PAGE: &str = r##"
        <script>window.changeStream = function (streamId){
          document.getElementById('wp_player').src='https://gooz.example.net/new-stream-embed/' + streamId + '?3newa=22'
        }</script>
        <div class="embed-responsive embed-responsive-16by9"></div>
        <img src="https://a.espncdn.com/combiner/i?img=%2Fteamlogos%2Fmlb%2F500%2Fcin.png&w=65">
    "##;

    const LIVE_PAGE: &str = r##"
        <script>window.changeStream = function (streamId){
          document.getElementById('wp_player').src='https://gooz.example.net/new-stream-embed/' + streamId + '?3newa=22'
        }</script>
        <img src="https://a.espncdn.com/combiner/i?img=%2Fteamlogos%2Fcfb%2F500%2F99.png&w=65">
        <iframe  class="embed-responsive-item" id="wp_player" frameborder=0 height=360px
          width=100% src="https://gooz.example.net/new-stream-embed/55953?ad=111" allowfullscreen></iframe>
    "##;

    #[test]
    fn playlist_sniff_goes_by_body_not_type() {
        let pl = r"#EXTM3U
#EXT-X-TARGETDURATION:6
";
        assert!(is_playlist_body(pl.as_bytes()));

        let mut bom = vec![0xEFu8, 0xBB, 0xBF];
        bom.extend_from_slice(b"  #EXTM3U");
        assert!(is_playlist_body(&bom));

        let err = r"<html>
<head><title>404 Not Found</title></head>
<body>";
        assert!(!is_playlist_body(err.as_bytes()));
        assert!(!is_playlist_body(b""));
    }

    #[test]
    fn embed_iframe_found_on_live_page() {
        assert_eq!(
            extract_embed_iframe(LIVE_PAGE).as_deref(),
            Some("https://gooz.example.net/new-stream-embed/55953?ad=111")
        );
    }

    #[test]
    fn dead_page_yields_no_embed_not_a_logo() {
        assert_eq!(extract_embed_iframe(DEAD_PAGE), None);
    }

    #[test]
    fn tag_attr_handles_both_quotes_and_bare() {
        let t = r##"<iframe id='wp_player' src="https://e.net/a" data-src="https://nope/">"##;
        assert_eq!(tag_attr(t, "src").as_deref(), Some("https://e.net/a"));
        assert_eq!(tag_attr(t, "id").as_deref(), Some("wp_player"));
        assert_eq!(
            tag_attr("<iframe src=https://e.net/b >", "src").as_deref(),
            Some("https://e.net/b")
        );
    }

    #[test]
    fn b64_decodes_known_loader() {
        let s = "aHR0cHM6Ly9leGFtcGxlLm5ldC9wbGF5bGlzdC81NTg2MS9sb2FkLXBsYXlsaXN0";
        let v = decode_b64_str(s).unwrap();
        assert_eq!(v, "https://example.net/playlist/55861/load-playlist");
    }

    #[test]
    fn extract_hint_strips_nested_span() {
        let block = r##"<span class="f1-podium--time f1-label SaatZamanBilgisi btn-live "><span style="color: #930f0d;font-weight:bold;">LIVE</span></span>"##;
        let raw = extract_status_hint(block).unwrap();
        assert_eq!(collapse_ws(&strip_tags(&raw)), "LIVE");
    }

    #[test]
    fn label_with_entities_decodes() {
        let html = r##"<li class="f1-podium--item match-card ">
            <a href="https://example.test/cfb/x-y/1">
              <span class="d-md-inline "> William &amp; Mary vs Delaware State </span>
              <span class="SaatZamanBilgisi btn-live "><span>LIVE</span></span>
            </a></li>"##;
        let g = parse_index(html, "cfb");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].label, "William & Mary vs Delaware State");
        assert_eq!(g[0].away, "William & Mary");
        assert_eq!(g[0].home, "Delaware State");
        assert_eq!(g[0].status, "live");
    }

    #[test]
    fn loader_extractor_accepts_single_quotes() {
        let html =
            "var loader = 'aHR0cHM6Ly9leGFtcGxlLm5ldC9wbGF5bGlzdC81NTg3OC9sb2FkLXBsYXlsaXN0';";
        let got = extract_loader_b64(html).unwrap();
        assert!(got.starts_with("aHR0"));
        assert_eq!(
            decode_b64_str(&got).unwrap(),
            "https://example.net/playlist/55878/load-playlist"
        );
    }

    #[test]
    fn parses_match_card_row() {
        let html = r##"<li data-league="19510-cfb" class="f1-podium--item match-card ">
            <a href="https://example.test/cfb/baylor-bears-auburn-tigers/43531392" class="x">
              <span class="f1-podium--rank">CFB</span>
              <span class="d-md-inline "> Auburn vs Baylor </span>
              <span class="SaatZamanBilgisi btn-live " > LIVE </span>
            </a>
          </li>"##;
        let games = parse_index(html, "cfb");
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, "43531392");
        assert_eq!(games[0].league, "CFB");
        assert_eq!(games[0].away, "Auburn");
        assert_eq!(games[0].home, "Baylor");
        assert_eq!(games[0].status, "live");
    }

    #[test]
    fn combat_card_without_numeric_id_survives() {
        let html = r##"<li class="f1-podium--item match-card ">
            <a href="https://example.test/stream/ufc/topuria-vs-volkanovski-live-stream">
              <span class="f1-podium--rank">UFC</span>
              <span class="d-md-inline "> Ilia Topuria vs Alexander Volkanovski </span>
              <span class="SaatZamanBilgisi "> 2 days from now </span>
            </a></li>"##;
        let g = parse_index(html, "mma");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].id, "topuria-vs-volkanovski-live-stream");
        assert_eq!(g[0].league, "UFC");
        assert_eq!(g[0].status, "scheduled");
        assert_eq!(g[0].starts_in, Some(2880));
    }

    #[test]
    fn numeric_entities_decode() {
        assert_eq!(
            decode_entities("Dana White&#039;s &amp; Co"),
            "Dana White's & Co"
        );
        assert_eq!(decode_entities("a &#x; b"), "a &#x; b");
    }

    #[test]
    fn sport_comes_from_the_link_not_the_page() {
        assert_eq!(
            sport_from_href("https://x.test/premier-league/arsenal-spurs/99", "top"),
            "premier-league"
        );
        assert_eq!(
            sport_from_href("https://x.test/cfb/lsu-clemson/1", "top"),
            "cfb"
        );
        assert_eq!(
            sport_from_href("https://x.test/stream/ufc/fight-night", "mma"),
            "mma"
        );
        assert_eq!(sport_from_href("nonsense", "cfb"), "cfb");
    }

    #[test]
    fn homepage_and_league_index_agree_on_one_id() {
        let card = |host_path: &str| {
            format!(
                r##"<li class="f1-podium--item match-card ">
                <a href="https://x.test{host_path}">
                  <span class="f1-podium--rank">EPL</span>
                  <span class="d-md-inline "> Arsenal vs Spurs </span>
                  <span class="SaatZamanBilgisi "> LIVE </span>
                </a></li>"##
            )
        };
        let from_home = parse_index(&card("/premier-league/arsenal-spurs/77"), "top");
        let from_league = parse_index(&card("/premier-league/arsenal-spurs/77"), "premier-league");
        assert_eq!(from_home[0].id, from_league[0].id);
        assert_eq!(from_home[0].sport, "premier-league");
    }

    #[test]
    fn starts_in_reads_relative_hints() {
        assert_eq!(parse_starts_in("9 minutes from now"), Some(9));
        assert_eq!(parse_starts_in("1 hour from now"), Some(60));
        assert_eq!(parse_starts_in("LIVE"), None);
    }

    #[test]
    fn index_sources_parses_pairs() {
        std::env::set_var(
            "LIVE_INDEX_URLS",
            "cfb=https://a.test/c, nfl = https://a.test/n ,junk",
        );
        let got = index_sources();
        std::env::remove_var("LIVE_INDEX_URLS");
        assert_eq!(
            got,
            vec![
                ("cfb".to_string(), "https://a.test/c".to_string()),
                ("nfl".to_string(), "https://a.test/n".to_string())
            ]
        );
    }

    #[test]
    fn playlist_rewrites_absolute_and_relative() {
        let m = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=4000000\nhttps://a.b/c/d.m3u8\n#EXTINF:6,\nseg.ts\n";
        let out = rewrite_playlist(m, "https://x.y/root/master.m3u8", "TOK");
        assert!(out.contains("/api/live/hls/TOK/p/"));
        assert!(out.contains(&hex_encode(b"https://x.y/root/seg.ts")));
    }

    #[test]
    fn playlist_rewrites_tag_uris() {
        let m = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",URI=\"96kbps/list.m3u8\",DEFAULT=YES\n#EXT-X-MAP:URI=\"/x/init.mp4\"\n#EXT-X-TARGETDURATION:3\n";
        let out = rewrite_playlist(m, "https://x.y/root/master.m3u8", "TOK");
        assert!(out.contains(&format!(
            "URI=\"/api/live/hls/TOK/p/{}\",DEFAULT=YES",
            hex_encode(b"https://x.y/root/96kbps/list.m3u8")
        )));
        assert!(out.contains(&format!(
            "#EXT-X-MAP:URI=\"/api/live/hls/TOK/p/{}\"",
            hex_encode(b"https://x.y/x/init.mp4")
        )));
        assert!(out.contains("#EXT-X-TARGETDURATION:3\n"));
    }

    #[test]
    fn first_uri_skips_tags_and_joins_relative() {
        let master = "#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1
sub/caxi
";
        let u = first_uri(master).unwrap();
        assert_eq!(u, "sub/caxi");
        assert_eq!(
            join_url("https://a.test/playlist/1/load-playlist", &u),
            "https://a.test/playlist/1/sub/caxi"
        );
        assert_eq!(
            join_url("https://a.test/playlist/1/load-playlist", "/x/y"),
            "https://a.test/x/y"
        );
    }

    #[test]
    fn hex_roundtrip() {
        let src = "https://example.org/hello?x=1&y=2";
        let enc = hex_encode(src.as_bytes());
        assert_eq!(hex_decode_url(&enc).unwrap(), src);
    }
}
