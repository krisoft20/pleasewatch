use crate::models::TorrentOption;
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

const BASE: &str = "https://bitsearch.to";

static CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0")
            .build()
            .expect("build bitsearch client")
    })
}

pub fn enabled() -> bool {
    std::env::var("BITSEARCH_ENABLED").as_deref() != Ok("0")
}

pub async fn search(query: &str) -> Vec<TorrentOption> {
    let words: Vec<&str> = query
        .split_whitespace()
        .filter(|w| !matches!(*w, "PL" | "GER" | "ENG"))
        .collect();
    let full = words.join(" ");
    let bare = words
        .iter()
        .filter(|w| {
            let l = w.to_lowercase();
            let rest = l.strip_prefix('s').unwrap_or("");
            !(rest.starts_with(|c: char| c.is_ascii_digit())
                && rest.chars().all(|c| c.is_ascii_digit() || c == 'e'))
        })
        .copied()
        .collect::<Vec<_>>()
        .join(" ");

    let started = std::time::Instant::now();
    let (a, b) = if bare.is_empty() || bare == full {
        (fetch(&full).await, Vec::new())
    } else {
        tokio::join!(fetch(&full), fetch(&bare))
    };

    let mut out = a;
    for t in b {
        if !out.iter().any(|o| o.provider_id == t.provider_id) {
            out.push(t);
        }
    }
    crate::pi!(
        "[bitsearch] {} results in {:.1}s for {full:?}",
        out.len(),
        started.elapsed().as_secs_f32()
    );
    out
}

async fn fetch(q: &str) -> Vec<TorrentOption> {
    if q.trim().is_empty() {
        return Vec::new();
    }
    let mut url = reqwest::Url::parse(&format!("{BASE}/search")).unwrap();
    url.query_pairs_mut()
        .append_pair("q", q)
        .append_pair("sort", "seeders");

    let html = match client().get(url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        Ok(r) => {
            crate::pe!("[bitsearch] http {} for {q:?}", r.status());
            return Vec::new();
        }
        Err(e) => {
            crate::pe!("[bitsearch] {q:?}: {e}");
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    for card in html.split("<a href=\"/torrent/").skip(1) {
        let Some((title, rest)) = card.split_once("</a>") else {
            continue;
        };
        let title = unescape(title.split_once('>').map(|x| x.1).unwrap_or("").trim());
        let title = strip_site_tag(&title).to_string();
        let Some(m) = rest.find("href=\"magnet:") else {
            continue;
        };
        let magnet = unescape(rest[m + 6..].split('"').next().unwrap_or(""));
        let Some(hash) = between(&magnet, "btih:", "&") else {
            continue;
        };
        if title.is_empty() {
            continue;
        }

        let size = between(rest, "fa-download\"></i>", "</span>")
            .map(|s| s.replace("<span>", ""))
            .and_then(|s| crate::books::parse_size(s.trim()))
            .unwrap_or(0);
        let count = |label: &str| -> i32 {
            let Some(at) = rest.find(label) else { return 0 };
            let head = &rest[..at];
            let Some(open) = head.rfind("font-medium\">") else {
                return 0;
            };
            head[open + 13..]
                .split('<')
                .next()
                .and_then(|n| n.trim().replace(',', "").parse().ok())
                .unwrap_or(0)
        };

        let meta = crate::jackett::parse_title_metadata(&title);
        out.push(TorrentOption {
            provider: "bitsearch".into(),
            provider_id: hash.to_lowercase(),
            quality: crate::jackett::extract_quality(&title),
            magnet: format!("magnet:?xt=urn:btih:{hash}&dn={}", urlencode(&title))
                + &trackers(&magnet),
            title,
            size,
            seeds: count("<span>seeders"),
            peers: count("<span>leechers"),
            audio: meta.audio,
            video_codec: meta.video_codec,
            subtitle_info: meta.subtitle_info,
            release_group: meta.release_group,
            tags: meta.tags,
            pref_score: 0.0,
            aggregator: "bits".into(),
        });
    }
    out
}

fn strip_site_tag(t: &str) -> &str {
    let mut t = t.trim();
    while t.starts_with('[') {
        match t.find(']') {
            Some(i) => t = t[i + 1..].trim_start(),
            None => break,
        }
    }
    t
}

fn trackers(magnet: &str) -> String {
    magnet
        .split('&')
        .filter(|p| p.starts_with("tr="))
        .map(|p| format!("&{p}"))
        .collect()
}

fn urlencode(s: &str) -> String {
    let mut u = reqwest::Url::parse("http://x/").unwrap();
    u.query_pairs_mut().append_pair("x", s);
    u.query().unwrap_or("x=")[2..].to_string()
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&#x3D;", "=")
        .replace("&#x27;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let a = s.find(start)? + start.len();
    let b = s[a..].find(end)? + a;
    Some(&s[a..b])
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn live_polish_search() {
        let hits = super::search("Eksperyment Odsiadka S01E01 PL").await;
        for h in &hits {
            println!(
                "{} | {}b | S{} P{} | {}",
                h.title,
                h.size,
                h.seeds,
                h.peers,
                &h.magnet[..60]
            );
        }
        assert!(!hits.is_empty());
    }
}
