use crate::live::LiveGame;
use serde::Deserialize;
use std::time::Duration;

const EVENT_PREFIX: &str = "https://livetv.sx";

pub fn index_url() -> String {
    std::env::var("LIVETV_INDEX_URL").unwrap_or_else(|_| "https://livetv.sx/enx/".to_string())
}

fn sniffer_url() -> String {
    std::env::var("SNIFFER_URL").unwrap_or_else(|_| "http://sniffer:3000".to_string())
}

fn sniffer_token() -> String {
    std::env::var("SNIFFER_TOKEN").unwrap_or_default()
}

pub fn enabled() -> bool {
    !sniffer_token().is_empty() && std::env::var("LIVETV_ENABLED").as_deref() != Ok("0")
}

fn filter_terms() -> Vec<String> {
    std::env::var("LIVETV_FILTER")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn is_livetv_url(url: &str) -> bool {
    url.starts_with(EVENT_PREFIX) || url.contains("livetv.sx/")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .unwrap_or_default()
}

async fn fetch_page(url: &str) -> Result<String, String> {
    let body =
        serde_json::json!({ "url": url, "gotoOptions": { "waitUntil": "domcontentloaded" } });
    let r = client()
        .post(format!(
            "{}/chromium/content?token={}",
            sniffer_url(),
            sniffer_token()
        ))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("content fetch: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("content status {}", r.status().as_u16()));
    }
    r.text().await.map_err(|e| format!("content read: {e}"))
}

#[derive(Debug, Deserialize)]
struct SniffHit {
    url: String,
}

#[derive(Debug, Deserialize)]
struct SniffOut {
    #[serde(default)]
    found: bool,
    #[serde(default)]
    hits: Vec<SniffHit>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SniffBody {
    Wrapped { data: SniffOut },
    Bare(SniffOut),
}

impl SniffBody {
    fn into_out(self) -> SniffOut {
        match self {
            SniffBody::Wrapped { data } => data,
            SniffBody::Bare(o) => o,
        }
    }
}

const SNIFF_JS: &str = include_str!("../assets/sniff.js");

pub async fn sniff(webplayer_url: &str, wait_ms: u64) -> Result<String, String> {
    let body = serde_json::json!({
        "code": SNIFF_JS,
        "context": { "url": webplayer_url, "waitMs": wait_ms }
    });
    let r = client()
        .post(format!(
            "{}/chromium/function?token={}",
            sniffer_url(),
            sniffer_token()
        ))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("sniff call: {e}"))?;
    let status = r.status();
    let text = r.text().await.map_err(|e| format!("sniff read: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "sniff status {} {}",
            status.as_u16(),
            &text[..text.len().min(120)]
        ));
    }
    let out = serde_json::from_str::<SniffBody>(&text)
        .map_err(|e| format!("sniff parse: {e}"))?
        .into_out();
    if !out.found || out.hits.is_empty() {
        return Err("no playlist".into());
    }
    Ok(out.hits[0].url.clone())
}

fn decode_entities(s: &str) -> String {
    s.replace("&ndash;", "-")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => {
                if depth > 0 {
                    depth -= 1
                }
            }
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn teams_from_slug(slug: &str) -> (String, String) {
    let cleaned = slug.trim_matches('_');
    let parts: Vec<&str> = cleaned.split('_').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return (title_case(cleaned), String::new());
    }
    let mid = parts.len() / 2;
    (
        title_case(&parts[..mid].join(" ")),
        title_case(&parts[mid..].join(" ")),
    )
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_sport(raw: &str) -> String {
    let k = raw.to_ascii_lowercase();
    match k.as_str() {
        "football" | "futsal" => "soccer",
        "american football" => "football",
        "ice hockey" | "hockey" | "bandy" => "hockey",
        "basketball" => "basketball",
        "baseball" => "baseball",
        "boxing" | "mma" | "wrestling" | "martial arts" => "combat",
        "motorsport" | "moto racing" | "auto racing" | "formula 1" => "motor",
        _ => return k.replace(' ', "-"),
    }
    .to_string()
}

fn sport_and_league(alt: &str) -> (String, String) {
    let parts: Vec<&str> = alt
        .split('.')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    let sport = parts
        .first()
        .map(|s| normalize_sport(s))
        .unwrap_or_else(|| "other".to_string());
    let league = parts.last().map(|s| s.to_string()).unwrap_or_default();
    (sport, league)
}

fn split_evdesc(d: &str) -> (String, String) {
    match (d.find('('), d.rfind(')')) {
        (Some(a), Some(b)) if b > a => (d[..a].trim().to_string(), d[a + 1..b].trim().to_string()),
        _ => (d.trim().to_string(), String::new()),
    }
}

pub fn parse_index(html: &str, terms: &[String]) -> Vec<LiveGame> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let needle = "href=\"/enx/eventinfo/";
    let mut pos = 0;
    while let Some(i) = html[pos..].find(needle) {
        let abs = pos + i;
        pos = abs + needle.len();
        let after = &html[pos..];
        let Some(qe) = after.find('"') else { continue };
        let path = &after[..qe];
        let full = format!("{EVENT_PREFIX}/enx/eventinfo/{path}");
        if !seen.insert(full.clone()) {
            continue;
        }

        let back = &html[abs.saturating_sub(700)..abs];
        let alt = back
            .rfind("alt=\"")
            .map(|a| {
                let rest = &back[a + 5..];
                rest.find('"')
                    .map(|e| rest[..e].to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let (sport, league_from_alt) = sport_and_league(&decode_entities(&alt));

        let block = &after[qe..after.len().min(qe + 900)];
        let anchor_text = block
            .find('>')
            .and_then(|s| {
                block[s + 1..]
                    .find("</a>")
                    .map(|e| squeeze(&strip_tags(&block[s + 1..s + 1 + e])))
            })
            .unwrap_or_default();
        let evdesc = block
            .find("class=\"evdesc\">")
            .map(|s| {
                let rest = &block[s + 15..];
                let e = rest.find("</span>").unwrap_or(rest.len());
                squeeze(&strip_tags(&rest[..e]))
            })
            .unwrap_or_default();
        let (time, where_) = split_evdesc(&evdesc);
        let league = if league_from_alt.is_empty() {
            where_.rsplit('.').next().unwrap_or("").trim().to_string()
        } else {
            league_from_alt
        };

        if !terms.is_empty() {
            let hay = format!("{} {} {}", where_, league, sport).to_ascii_lowercase();
            if !terms.iter().any(|t| hay.contains(t.as_str())) {
                continue;
            }
        }

        let slug = path
            .split_once('_')
            .map(|(_, s)| s)
            .unwrap_or("")
            .trim_matches('/');
        let (mut away, mut home) = teams_from_slug(slug);
        if let Some((a, b)) = anchor_text.split_once('-') {
            let (a, b) = (a.trim(), b.trim());
            if !a.is_empty() && !b.is_empty() {
                away = a.to_string();
                home = b.to_string();
            }
        }

        let live = block.contains("class=\"live\"")
            || back.ends_with("class=\"live\" ")
            || html[abs.saturating_sub(60)..abs].contains("class=\"live\"");

        let label = if away.is_empty() {
            squeeze(&anchor_text)
        } else {
            format!("{away} vs {home}")
        };

        out.push(LiveGame {
            id: format!(
                "ltv-{}",
                path.split('_').next().unwrap_or(path).trim_matches('/')
            ),
            sport,
            league,
            label,
            away,
            home,
            status: if live {
                "live".into()
            } else {
                "scheduled".into()
            },
            start_hint: if time.is_empty() { None } else { Some(time) },
            starts_in: None,
            source_url: full,
            espn: None,
        });
    }
    out
}

pub fn parse_mirrors(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = "/webplayer.php?t=ifr";
    let mut pos = 0;
    while let Some(i) = html[pos..].find(needle) {
        let abs = pos + i;
        pos = abs + needle.len();
        let rest = &html[abs..];
        let end = rest
            .find(|c| c == '"' || c == '\'' || c == '>' || c == ' ')
            .unwrap_or(rest.len());
        let url = format!("{EVENT_PREFIX}{}", decode_entities(&rest[..end]));
        if !out.contains(&url) {
            out.push(url);
        }
    }
    out
}

pub fn link_kinds(html: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let needle = "webplayer";
    let mut pos = 0;
    while let Some(i) = html[pos..].find(needle) {
        let abs = pos + i;
        pos = abs + needle.len();
        let rest = &html[abs..html.len().min(abs + 60)];
        if let Some(t) = rest.split_once("t=") {
            let kind: String =
                t.1.chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
            if !kind.is_empty() {
                out.insert(kind.to_ascii_lowercase());
            }
        }
    }
    out
}

pub fn parse_youtube_ids(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let needle = "webplayer2.php?t=youtube";
    let mut pos = 0;
    while let Some(i) = html[pos..].find(needle) {
        let abs = pos + i;
        pos = abs + needle.len();
        let rest = &html[abs..html.len().min(abs + 200)];
        let Some((_, after)) = rest.split_once("c=") else {
            continue;
        };
        let id: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if id.len() == 11 && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

pub async fn fetch_index() -> Result<Vec<LiveGame>, String> {
    let html = fetch_page(&index_url()).await?;
    let games = parse_index(&html, &filter_terms());
    eprintln!(
        "[livetv] index: {} bytes -> {} games",
        html.len(),
        games.len()
    );
    Ok(games)
}

pub async fn resolve(event_url: &str) -> Result<(String, String), String> {
    let html = fetch_page(event_url).await?;
    let mirrors = parse_mirrors(&html);
    if mirrors.is_empty() {
        let kinds = link_kinds(&html);
        eprintln!("[livetv] no ifr mirror; link kinds: {kinds:?}");
        if let Some(id) = parse_youtube_ids(&html).into_iter().next() {
            return Err(format!("youtube:{id}"));
        }
        if kinds.contains("youtube") {
            return Err("youtube only".into());
        }
        if kinds.contains("acestream") {
            return Err("p2p only".into());
        }
        return Err("dead stream".into());
    }
    let tried = mirrors.len().min(3);
    let mut set = tokio::task::JoinSet::new();
    for m in mirrors.into_iter().take(tried) {
        set.spawn(async move {
            let r = sniff(&m, 18000).await;
            (m, r)
        });
    }
    let mut last = String::from("no mirror worked");
    while let Some(joined) = set.join_next().await {
        let Ok((m, res)) = joined else { continue };
        match res {
            Ok(playlist) => {
                eprintln!("[livetv] {} gave a playlist", short(&m));
                set.abort_all();
                return Ok((playlist, format!("{EVENT_PREFIX}/")));
            }
            Err(e) => {
                eprintln!("[livetv] {} failed: {e}", short(&m));
                last = e;
            }
        }
    }
    Err(if last == "no playlist" {
        "dead stream".into()
    } else {
        last
    })
}

fn short(mirror: &str) -> String {
    mirror
        .split_once("c=")
        .map(|(_, r)| format!("mirror c={}", r.split('&').next().unwrap_or("?")))
        .unwrap_or_else(|| "mirror".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/livetv_index.html");

    #[test]
    fn splits_sport_and_league_from_icon_alt() {
        let (s, l) = sport_and_league("Football. Poland. Ekstraklasa");
        assert_eq!(s, "soccer");
        assert_eq!(l, "Ekstraklasa");
        assert_eq!(sport_and_league("Ice Hockey. Sweden. SHL").0, "hockey");
        assert_eq!(
            sport_and_league("American Football. USA. NFL").0,
            "football"
        );
        assert_eq!(
            sport_and_league("Handball. Germany. Bundesliga").0,
            "handball"
        );
    }

    #[test]
    fn splits_time_from_evdesc() {
        let (t, w) = split_evdesc("14:00 (England. Premier League)");
        assert_eq!(t, "14:00");
        assert_eq!(w, "England. Premier League");
    }

    #[test]
    fn teams_come_from_slug_when_anchor_is_a_score() {
        let (a, b) = teams_from_slug("telstar_cambuur");
        assert_eq!(a, "Telstar");
        assert_eq!(b, "Cambuur");
    }

    #[test]
    fn parses_events_off_the_real_index() {
        let games = parse_index(FIXTURE, &[]);
        assert!(!games.is_empty(), "fixture yielded nothing");
        let ev = games.iter().find(|g| g.source_url.contains("everton"));
        let ev = ev.expect("everton event missing");
        assert_eq!(ev.league, "Premier League");
        assert_eq!(ev.sport, "soccer");
        assert!(ev.label.contains("Everton"), "label was {}", ev.label);
        assert!(ev.id.starts_with("ltv-"));
    }

    #[test]
    fn filter_keeps_only_matching_leagues() {
        let all = parse_index(FIXTURE, &[]);
        let nl = parse_index(FIXTURE, &["netherlands".to_string()]);
        assert!(nl.len() < all.len(), "filter did not narrow anything");
        assert!(nl
            .iter()
            .all(|g| g.source_url.contains("telstar") || g.league.contains("Eredivisie")));
    }

    #[test]
    fn accepts_both_browserless_response_shapes() {
        let wrapped = r##"{"data":{"found":true,"hits":[{"url":"https://x/a.m3u8"}]},"type":"application/json"}"##;
        let bare = r##"{"found":true,"hits":[{"url":"https://x/b.m3u8"}]}"##;
        for (raw, want) in [(wrapped, "https://x/a.m3u8"), (bare, "https://x/b.m3u8")] {
            let out = serde_json::from_str::<SniffBody>(raw)
                .expect("parse")
                .into_out();
            assert!(out.found);
            assert_eq!(out.hits[0].url, want);
        }
    }

    #[test]
    fn picks_youtube_ids_off_an_event() {
        let html = r##"<a href="/webplayer2.php?t=youtube&amp;c=Pys12Z5Z4G8&amp;lang=en&amp;eid=1">yt</a>
                       <a href="/webplayer2.php?t=youtube&amp;c=Pys12Z5Z4G8&amp;lang=en">dup</a>
                       <a href="/webplayer2.php?t=acestream&amp;c=deadbeefdeadbeef">ace</a>"##;
        assert_eq!(parse_youtube_ids(html), vec!["Pys12Z5Z4G8".to_string()]);
        assert!(
            parse_youtube_ids(r##"<a href="/webplayer2.php?t=acestream&c=deadbeef">x</a>"##)
                .is_empty()
        );
    }

    #[test]
    fn link_kinds_sees_what_we_cannot_play() {
        let html = r##"<a href="/webplayer2.php?t=youtube&amp;c=abc">yt</a>
                       <a href="/webplayer2.php?t=acestream&amp;c=dead">ace</a>"##;
        let k = link_kinds(html);
        assert!(k.contains("youtube"));
        assert!(k.contains("acestream"));
        assert!(!k.contains("ifr"));
        assert!(
            parse_mirrors(html).is_empty(),
            "youtube is not a sniffable mirror"
        );
    }

    #[test]
    fn mirrors_are_absolute_and_deduped() {
        let html = r##"<a href="/webplayer.php?t=ifr&amp;c=1&amp;eid=9">a</a>
                       <a href="/webplayer.php?t=ifr&amp;c=1&amp;eid=9">dup</a>
                       <a href="/webplayer2.php?t=acestream&amp;c=deadbeef">ace</a>
                       <a href="/webplayer.php?t=ifr&amp;c=2&amp;eid=9">b</a>"##;
        let m = parse_mirrors(html);
        assert_eq!(m.len(), 2, "got {m:?}");
        assert!(m[0].starts_with("https://livetv.sx/webplayer.php?t=ifr&c=1"));
        assert!(m.iter().all(|u| !u.contains("acestream")));
    }
}
