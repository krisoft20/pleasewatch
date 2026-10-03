use crate::models::TorrentOption;
use reqwest::Client;
use serde_json::{json, Value};
use sha2::Digest;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;

const BASE: &str = "https://ext.to";
const SESSION: &str = "pw-ext";

static CLIENT: OnceLock<Client> = OnceLock::new();
static SESSION_LOCK: OnceLock<Mutex<bool>> = OnceLock::new();

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(100))
            .build()
            .expect("build flaresolverr client")
    })
}

fn flare_url() -> String {
    std::env::var("FLARESOLVERR_URL").unwrap_or_else(|_| "http://flaresolverr:8191".to_string())
}

pub fn enabled() -> bool {
    std::env::var("EXT_ENABLED").as_deref() != Ok("0")
}

async fn flare(payload: Value) -> Result<String, String> {
    let resp = client()
        .post(format!("{}/v1", flare_url()))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    if v["status"] != "ok" {
        return Err(v["message"]
            .as_str()
            .unwrap_or("flaresolverr error")
            .to_string());
    }
    let sol = &v["solution"];
    if let Some(s) = sol["status"].as_i64() {
        if s != 200 {
            return Err(format!("http {s}"));
        }
    }
    Ok(sol["response"].as_str().unwrap_or_default().to_string())
}

async fn session_req(ready: &mut bool, mut payload: Value) -> Result<String, String> {
    if !*ready {
        let _ = flare(json!({ "cmd": "sessions.create", "session": SESSION })).await;
        *ready = true;
    }
    payload["session"] = SESSION.into();
    payload["session_ttl_minutes"] = 30.into();
    payload["maxTimeout"] = 60000.into();
    let r = flare(payload).await;
    if r.is_err() {
        let _ = flare(json!({ "cmd": "sessions.destroy", "session": SESSION })).await;
        *ready = false;
    }
    r
}

pub async fn search(query: &str, imdb: Option<&str>) -> Vec<TorrentOption> {
    let mut url = reqwest::Url::parse(&format!("{BASE}/browse/")).unwrap();
    match imdb {
        Some(id) => {
            let p = crate::jackett::parse_title(query);
            let marker = match (p.season, p.episode) {
                (Some(s), Some(e)) => format!("s{s:02}e{e:02}"),
                (Some(s), None) => format!("s{s:02}"),
                _ => String::new(),
            };
            url.query_pairs_mut().append_pair("imdb_id", id);
            if !marker.is_empty() {
                url.query_pairs_mut().append_pair("q", &marker);
            }
        }
        None => {
            url.query_pairs_mut().append_pair("q", query.trim());
        }
    }

    let started = std::time::Instant::now();
    let html = {
        let mut ready = SESSION_LOCK.get_or_init(|| Mutex::new(false)).lock().await;
        match session_req(
            &mut ready,
            json!({ "cmd": "request.get", "url": url.as_str() }),
        )
        .await
        {
            Ok(h) => h,
            Err(e) => {
                crate::pe!("[ext] search failed: {e}");
                return Vec::new();
            }
        }
    };

    let Some(table) = html.split("search-table").nth(1) else {
        crate::pi!("[ext] no results table for {url}");
        return Vec::new();
    };

    let mut out = Vec::new();
    for row in table.split("<tr>").skip(1) {
        let Some(at) = row.find("class=\"torrent-title-link\"") else {
            continue;
        };
        let Some(href) = row[..at].rfind("href=\"/") else {
            continue;
        };
        let slug = row[href + 7..at]
            .trim_end()
            .trim_end_matches('"')
            .trim_end_matches('/');
        let Some(id) = slug.rsplit('-').next().filter(|s| s.parse::<u64>().is_ok()) else {
            continue;
        };

        let rest = &row[at..];
        let title = between(rest, "<b>", "</b>")
            .map(strip_tags)
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let size = between(row, "Size</span>", "</span>")
            .and_then(|s| crate::books::parse_size(&strip_tags(s)))
            .unwrap_or(0);
        let num = |label: &str| {
            between(row, label, "</span>")
                .map(|s| strip_tags(s).replace(',', ""))
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(0)
        };
        let source = between(row, "/static/img/source/", ".")
            .map(|s| format!("ext/{s}"))
            .unwrap_or_else(|| "ext.to".into());

        let meta = crate::jackett::parse_title_metadata(&title);
        out.push(TorrentOption {
            provider: source,
            provider_id: id.to_string(),
            quality: crate::jackett::extract_quality(&title),
            title,
            magnet: format!("ext:{slug}"),
            size,
            seeds: num("Seeds</span>"),
            peers: num("Leechs</span>"),
            audio: meta.audio,
            video_codec: meta.video_codec,
            subtitle_info: meta.subtitle_info,
            release_group: meta.release_group,
            tags: meta.tags,
            pref_score: 0.0,
            aggregator: "ext".into(),
        });
    }
    crate::pi!(
        "[ext] {} results in {:.1}s for {url}",
        out.len(),
        started.elapsed().as_secs_f32()
    );
    out
}

pub async fn resolve(placeholder: &str) -> Result<String, String> {
    let slug = placeholder.strip_prefix("ext:").unwrap_or(placeholder);
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("bad ext id".into());
    }
    let id: u64 = slug
        .rsplit('-')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or("bad ext id")?;
    let page = format!("{BASE}/{slug}/");

    let mut ready = SESSION_LOCK.get_or_init(|| Mutex::new(false)).lock().await;
    let html = session_req(&mut ready, json!({ "cmd": "request.get", "url": page })).await?;
    let token = between(&html, "pageToken = '", "'").ok_or("no page token")?;
    let csrf = between(&html, "csrfToken = '", "'").ok_or("no csrf token")?;

    let ts = chrono::Utc::now().timestamp();
    let hmac = format!("{:x}", sha2::Sha256::digest(format!("{id}|{ts}|{token}")));
    let mut form = reqwest::Url::parse("http://x/").unwrap();
    form.query_pairs_mut()
        .append_pair("torrent_id", &id.to_string())
        .append_pair("action", "get_magnet")
        .append_pair("timestamp", &ts.to_string())
        .append_pair("hmac", &hmac)
        .append_pair("sessid", csrf);

    let body = session_req(
        &mut ready,
        json!({
            "cmd": "request.post",
            "url": format!("{BASE}/ajax/getTorrentMagnet.php"),
            "postData": form.query().unwrap_or_default(),
        }),
    )
    .await?;

    let raw = strip_tags(&body).replace("&lt;", "<").replace("&gt;", ">");
    let raw = match (raw.find('{'), raw.rfind('}')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => return Err("no json in magnet response".into()),
    };
    let v: Value = serde_json::from_str(raw).map_err(|e| format!("bad magnet json: {e}"))?;
    if v["success"] != true {
        return Err(v["error"]
            .as_str()
            .unwrap_or("magnet lookup failed")
            .to_string());
    }
    if let Some(m) = v["magnet"].as_str().filter(|m| m.starts_with("magnet:")) {
        return Ok(m.to_string());
    }
    match v["hash"].as_str() {
        Some(h) => Ok(format!("magnet:?xt=urn:btih:{h}")),
        None => Err("no magnet in response".into()),
    }
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(&s[i..j])
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn live_search_and_resolve() {
        let hits = super::search("The Mentalist S01E01", Some("tt1196946")).await;
        assert!(!hits.is_empty(), "no hits");
        for h in hits.iter().take(5) {
            println!(
                "{} | {} | {}b | S{} P{} | {}",
                h.provider, h.title, h.size, h.seeds, h.peers, h.magnet
            );
        }
        let m = super::resolve(&hits[0].magnet).await.expect("resolve");
        println!("{m}");
        assert!(m.starts_with("magnet:?xt=urn:btih:"));
    }
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
    crate::jackett::decode_html(out.trim())
}
