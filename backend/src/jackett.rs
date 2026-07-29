use crate::models::TorrentOption;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone)]
pub struct Jackett {
    client: Client,
    base: String,
    api_key: String,
}

#[derive(Deserialize)]
struct Response {
    #[serde(rename = "Results")]
    results: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Tracker")]
    tracker: String,
    #[serde(rename = "TrackerId", default)]
    tracker_id: String,
    #[serde(rename = "MagnetUri")]
    magnet_uri: Option<String>,
    #[serde(rename = "Link")]
    link: Option<String>,
    #[serde(rename = "Size", default)]
    size: i64,
    #[serde(rename = "Seeders", default)]
    seeders: i32,
    #[serde(rename = "Peers", default)]
    peers: i32,
}

pub const TV_CATS: &[i32] = &[5000, 5010, 5030, 5040, 5045, 5050];
pub const MOVIE_CATS: &[i32] = &[2000, 2030, 2040, 2045, 2050];
pub const ANIME_PLUS_TV_CATS: &[i32] = &[5070, 5000, 5010, 5030, 5040, 5045, 5050];

pub fn sanitize_query(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    let mut prev_space = true;
    for c in q.chars() {
        if c == '\'' {
            continue;
        }
        if c.is_alphanumeric() {
            out.push(c);
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn config_candidates() -> [&'static str; 3] {
    [
        "/manage/jackett-config/Jackett/ServerConfig.json",
        "deploy/jackett-config/Jackett/ServerConfig.json",
        "../deploy/jackett-config/Jackett/ServerConfig.json",
    ]
}

pub fn read_api_key_from_config() -> Option<String> {
    for p in config_candidates() {
        if let Ok(s) = std::fs::read_to_string(Path::new(p)) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(k) = v.get("APIKey").and_then(|x| x.as_str()) {
                    if !k.is_empty() {
                        return Some(k.to_string());
                    }
                }
            }
        }
    }
    None
}

fn patch_config_if_needed() -> bool {
    for p in config_candidates() {
        let path = Path::new(p);
        let Ok(s) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&s) else {
            continue;
        };

        let mut dirty = false;
        if v.get("LocalBindAddress").and_then(|x| x.as_str()) != Some("*") {
            v["LocalBindAddress"] = serde_json::Value::String("*".into());
            dirty = true;
        }
        if v.get("BasePathOverride").and_then(|x| x.as_str()) != Some("/jackett") {
            v["BasePathOverride"] = serde_json::Value::String("/jackett".into());
            dirty = true;
        }

        if !dirty {
            return false;
        }
        if let Ok(out) = serde_json::to_string_pretty(&v) {
            if std::fs::write(path, out).is_ok() {
                println!("[jackett] patched ServerConfig.json (bind=*, base=/jackett)");
                return true;
            }
        }
        return false;
    }
    false
}

async fn restart_container() {
    let compose = std::env::var("PW_COMPOSE_FILE").unwrap_or_default();
    let host_dir =
        std::env::var("PW_HOST_DEPLOY_DIR").unwrap_or_else(|_| "/opt/pleasewatch/deploy".into());
    if !compose.is_empty() {
        let env_path = std::env::var("PW_DEPLOY_DIR")
            .map(|d| format!("{d}/.env"))
            .unwrap_or_else(|_| "/manage/.env".into());
        let out = tokio::process::Command::new("docker")
            .env("COMPOSE_PROJECT_DIR", &host_dir)
            .args([
                "compose",
                "--project-directory",
                &host_dir,
                "--env-file",
                &env_path,
                "-f",
                &compose,
                "restart",
                "jackett",
            ])
            .output()
            .await;
        if matches!(out, Ok(o) if o.status.success()) {
            println!("[jackett] container restarted via compose");
            return;
        }
    }
    let _ = tokio::process::Command::new("docker")
        .args(["restart", "pleasewatch-jackett-1"])
        .output()
        .await;
    println!("[jackett] container restart attempted via name");
}

pub fn spawn_auto_setup(state: std::sync::Arc<crate::AppState>) {
    tokio::spawn(async move {
        let mut patched_once = false;
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            if !patched_once && patch_config_if_needed() {
                patched_once = true;
                restart_container().await;
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            }

            let saved_key = {
                let db = state.db.lock().await;
                db.get_setting("jackett_api_key")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            };
            let url = {
                let db = state.db.lock().await;
                db.get_setting("jackett_url")
                    .ok()
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| std::env::var("JACKETT_URL").unwrap_or_default())
            };
            if url.is_empty() {
                continue;
            }

            let Some(disk_key) = read_api_key_from_config() else {
                continue;
            };
            if disk_key == saved_key {
                return;
            }

            {
                let db = state.db.lock().await;
                let _ = db.set_setting("jackett_api_key", &disk_key);
            }
            {
                let mut slot = state.jackett.lock().await;
                *slot = Some(Jackett::new(&url, &disk_key));
            }
            println!("[jackett] auto-imported api key from config");
            return;
        }
    });
}

impl Jackett {
    pub fn new(base: &str, api_key: &str) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("build jackett http client");
        Self {
            client,
            base: base.trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        }
    }

    pub async fn ping(&self) -> Result<(), String> {
        let url = format!("{}/api/v2.0/indexers", self.base);
        let resp = match self
            .client
            .get(url)
            .query(&[("apikey", self.api_key.as_str()), ("configured", "true")])
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => return Err(e.to_string()),
        };

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("http {}", resp.status().as_u16()))
        }
    }

    pub async fn search(
        &self,
        query: &str,
        indexers: &[String],
        cats: &[i32],
        imdb_id: Option<&str>,
    ) -> Vec<TorrentOption> {
        let cleaned = sanitize_query(query);
        let query = cleaned.as_str();
        let use_all = indexers.is_empty() || indexers.iter().any(|i| i == "all");

        if use_all {
            return self
                .search_one("all", query, cats, imdb_id)
                .await
                .unwrap_or_default();
        }

        let mut tasks = tokio::task::JoinSet::new();
        for indexer in indexers {
            let me = self.clone();
            let q = query.to_string();
            let idx = indexer.clone();
            let cats = cats.to_vec();
            let imdb = imdb_id.map(|s| s.to_string());
            tasks.spawn(async move {
                me.search_one(&idx, &q, &cats, imdb.as_deref())
                    .await
                    .unwrap_or_else(|e| {
                        eprintln!("[jackett] '{idx}' failed: {e}");
                        Vec::new()
                    })
            });
        }

        let mut all = Vec::new();
        while let Some(Ok(part)) = tasks.join_next().await {
            all.extend(part);
        }
        all
    }

    async fn search_one(
        &self,
        indexer: &str,
        query: &str,
        cats: &[i32],
        imdb_id: Option<&str>,
    ) -> reqwest::Result<Vec<TorrentOption>> {
        let url = format!("{}/api/v2.0/indexers/{}/results", self.base, indexer);

        let mut params: Vec<(&str, String)> = vec![
            ("apikey", self.api_key.clone()),
            ("Query", query.to_string()),
        ];
        if let Some(id) = imdb_id {
            params.push(("imdbid", id.to_string()));
        }
        for c in cats {
            params.push(("Category[]", c.to_string()));
        }

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json::<Response>()
            .await?;

        let mut out = Vec::with_capacity(resp.results.len());
        for h in resp.results {
            let magnet = h
                .magnet_uri
                .filter(|m| !m.is_empty())
                .or_else(|| h.link.filter(|l| !l.is_empty()));
            let Some(magnet) = magnet else { continue };

            let meta = parse_title_metadata(&h.title);
            let title = decode_html(&h.title);

            out.push(TorrentOption {
                provider: h.tracker,
                provider_id: h.tracker_id,
                title,
                magnet,
                quality: extract_quality(&h.title),
                size: h.size,
                seeds: h.seeders,
                peers: h.peers,
                audio: meta.audio,
                video_codec: meta.video_codec,
                subtitle_info: meta.subtitle_info,
                release_group: meta.release_group,
                tags: meta.tags,
                pref_score: 0.0,
                aggregator: "jackett".into(),
            });
        }
        Ok(out)
    }

    pub async fn list_indexers(&self) -> reqwest::Result<Vec<String>> {
        let url = format!("{}/api/v2.0/indexers", self.base);
        let resp = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_str()), ("configured", "true")])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let parsed: serde_json::Value = match serde_json::from_str(&resp) {
            Ok(v) => v,
            Err(_) => return Ok(Vec::new()),
        };

        if let Some(arr) = parsed.as_array() {
            return Ok(arr
                .iter()
                .filter_map(|v| v.get("id").and_then(|x| x.as_str()).map(String::from))
                .collect());
        }
        if let Some(arr) = parsed
            .get("Indexers")
            .or_else(|| parsed.get("indexers"))
            .and_then(|v| v.as_array())
        {
            return Ok(arr
                .iter()
                .filter_map(|v| {
                    v.get("id")
                        .or_else(|| v.get("ID"))
                        .or_else(|| v.get("Id"))
                        .and_then(|x| x.as_str())
                        .map(String::from)
                })
                .collect());
        }
        Ok(Vec::new())
    }
}

pub(crate) fn decode_html(s: &str) -> String {
    s.replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

pub struct ParsedTitle {
    pub show: String,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub is_pack: bool,
}

pub fn parse_title(raw: &str) -> ParsedTitle {
    let norm = raw.replace('.', " ").replace('-', " ").replace('_', " ");
    let lower = norm.to_lowercase();
    let bytes = lower.as_bytes();
    let n = bytes.len();

    let mut marker: Option<usize> = None;
    let mut season: Option<i32> = None;
    let mut episode: Option<i32> = None;
    let mut is_pack = false;

    let mut i = 0;
    while i < n {
        let at_word = i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'(';
        if at_word && bytes[i] == b's' && i + 1 < n && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < n && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let s = lower[i + 1..j].parse::<i32>().unwrap_or(0);

            if j < n && bytes[j] == b'e' && j + 1 < n && bytes[j + 1].is_ascii_digit() {
                let mut k = j + 1;
                while k < n && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                let e = lower[j + 1..k].parse::<i32>().unwrap_or(0);
                marker = Some(i);
                season = Some(s);
                episode = Some(e);
                break;
            }
            if j == n || bytes[j] == b' ' {
                if marker.is_none() {
                    marker = Some(i);
                    season = Some(s);
                    is_pack = true;
                }
            }
        }

        if at_word
            && i + 1 < n
            && bytes[i].is_ascii_digit()
            && bytes[i + 1] == b'x'
            && i + 2 < n
            && bytes[i + 2].is_ascii_digit()
        {
            let s_start = i;
            let mut s_end = i;
            while s_end < n && bytes[s_end].is_ascii_digit() {
                s_end += 1;
            }
            let e_start = s_end + 1;
            let mut e_end = e_start;
            while e_end < n && bytes[e_end].is_ascii_digit() {
                e_end += 1;
            }
            if let (Ok(s), Ok(e)) = (
                lower[s_start..s_end].parse::<i32>(),
                lower[e_start..e_end].parse::<i32>(),
            ) {
                marker = Some(i);
                season = Some(s);
                episode = Some(e);
                break;
            }
        }

        i += 1;
    }

    if marker.is_none() {
        if let Some(p) = lower.find("season ") {
            let tail = &lower[p + 7..];
            let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(s) = digits.parse::<i32>() {
                marker = Some(p);
                season = Some(s);
                is_pack = true;
                if let Some(ep) = lower[p..].find("episode ") {
                    let after = &lower[p + ep + 8..];
                    let edigits: String =
                        after.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(e) = edigits.parse::<i32>() {
                        episode = Some(e);
                        is_pack = false;
                    }
                }
            }
        }
    }

    if lower.contains("complete") || lower.contains("batch") {
        is_pack = true;
    }

    let show = match marker {
        Some(pos) => clean_show(&lower[..pos]),
        None => clean_show(&lower),
    };

    ParsedTitle {
        show,
        season,
        episode,
        is_pack,
    }
}

fn clean_show(s: &str) -> String {
    let stripped: String = s
        .chars()
        .filter_map(|c| match c {
            '\'' => None,
            '(' | ')' | '[' | ']' | '{' | '}' | ':' | ',' | '!' | '?' | '&' => Some(' '),
            _ => Some(c),
        })
        .collect();
    stripped
        .split_whitespace()
        .filter(|w| !(w.len() == 4 && w.chars().all(|c| c.is_ascii_digit())))
        .filter(|w| !matches!(*w, "and" | "the"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn show_matches(parsed: &str, expected: &str) -> bool {
    let exp_clean = clean_show(&expected.to_lowercase());
    let par_clean = clean_show(parsed);
    let exp_words: Vec<&str> = exp_clean.split_whitespace().collect();
    let par_words: Vec<&str> = par_clean.split_whitespace().collect();
    if exp_words.is_empty() || par_words.len() < exp_words.len() {
        return false;
    }
    par_words
        .windows(exp_words.len())
        .any(|w| w == exp_words.as_slice())
}

pub fn episode_match(title: &str, episode: i32) -> bool {
    let t = title.to_lowercase();
    let e2 = format!("{episode:02}");
    t.contains(&format!("e{e2}"))
        || t.contains(&format!("episode {episode}"))
        || t.contains(&format!("episode {e2}"))
        || t.contains(&format!(" - {e2} "))
        || t.contains(&format!(" - {e2}."))
        || t.contains(&format!(" - {e2}v"))
        || t.contains(&format!(" - {episode} "))
}

pub fn torrent_attrs(t: &crate::models::TorrentOption) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if !t.provider.is_empty() {
        out.push(("indexer".into(), t.provider.to_lowercase()));
    }
    if let Some(g) = &t.release_group {
        if !g.is_empty() {
            out.push(("release_group".into(), g.to_lowercase()));
        }
    }
    if let Some(c) = &t.video_codec {
        if !c.is_empty() {
            out.push(("codec".into(), c.to_lowercase()));
        }
    }
    if let Some(q) = &t.quality {
        if !q.is_empty() {
            out.push(("resolution".into(), q.to_lowercase()));
        }
    }
    for a in &t.audio {
        let v = a.to_lowercase();
        if !v.is_empty() {
            out.push(("audio".into(), v));
        }
    }
    let lower = t.title.to_lowercase();
    let source = if lower.contains("bluray") || lower.contains("blu-ray") {
        Some("bluray")
    } else if lower.contains("web-dl") || lower.contains("webdl") {
        Some("webdl")
    } else if lower.contains("webrip") {
        Some("webrip")
    } else if lower.contains("hdtv") {
        Some("hdtv")
    } else {
        None
    };
    if let Some(s) = source {
        out.push(("source".into(), s.into()));
    }
    if lower.contains("10bit") || lower.contains("10-bit") {
        out.push(("bit_depth".into(), "10bit".into()));
    }
    if lower.contains("hdr") {
        out.push(("hdr".into(), "hdr".into()));
    }
    out
}

pub fn pack_match(title: &str) -> bool {
    let t = title.to_lowercase();
    t.contains("complete")
        || t.contains("batch")
        || t.contains("season ")
        || t.contains("(01-")
        || t.contains("(00-")
        || t.contains(" 01-")
}

pub(crate) fn extract_quality(title: &str) -> Option<String> {
    let t = title.to_lowercase();
    if t.contains("2160p") || t.contains("4k") || t.contains("uhd") {
        Some("2160p".into())
    } else if t.contains("1080p") {
        Some("1080p".into())
    } else if t.contains("720p") {
        Some("720p".into())
    } else if t.contains("480p") {
        Some("480p".into())
    } else {
        None
    }
}

pub(crate) struct TitleMeta {
    pub audio: Vec<String>,
    pub video_codec: Option<String>,
    pub subtitle_info: Option<String>,
    pub release_group: Option<String>,
    pub tags: Vec<String>,
}

pub(crate) fn parse_title_metadata(title: &str) -> TitleMeta {
    let t = title.to_lowercase();
    let mut audio: Vec<String> = Vec::new();
    let mut tags: Vec<String> = Vec::new();

    if t.contains("dual audio") || t.contains("dual-audio") || t.contains("[dual audio]") {
        audio.push("Dual Audio".into());
    } else if t.contains("multi audio") || t.contains("multi-audio") {
        audio.push("Multi Audio".into());
    } else {
        if t.contains("japanese") || (t.contains("jpn") && !t.contains("jpng")) {
            audio.push("Japanese".into());
        }
        if t.contains("english") || t.contains("[eng]") || t.contains("(eng)") {
            audio.push("English".into());
        }
    }

    if t.contains("flac") {
        tags.push("FLAC".into());
    }
    if t.contains("opus") {
        tags.push("Opus".into());
    }
    if t.contains("eac3") || t.contains("e-ac-3") || t.contains("ddp") || t.contains("dd+") {
        tags.push("EAC3".into());
    } else if t.contains("ac3") || t.contains("dd5.1") || t.contains("dd 5.1") {
        tags.push("AC3".into());
    }
    if t.contains("atmos") {
        tags.push("Atmos".into());
    }
    if t.contains("dts-hd") || t.contains("dts hd") {
        tags.push("DTS-HD".into());
    } else if t.contains("dts") && !t.contains("dts-hd") {
        tags.push("DTS".into());
    }
    if t.contains("aac") {
        tags.push("AAC".into());
    }

    let video_codec = if t.contains("hevc")
        || t.contains("x265")
        || t.contains("h.265")
        || t.contains("h265")
    {
        Some("HEVC".into())
    } else if t.contains("x264") || t.contains("h.264") || t.contains("h264") || t.contains("avc") {
        Some("x264".into())
    } else if t.contains("av1") {
        Some("AV1".into())
    } else {
        None
    };

    if t.contains("10bit") || t.contains("10-bit") || t.contains("hi10p") || t.contains("hi10") {
        tags.push("10bit".into());
    }

    if t.contains("dolby vision") || (t.contains("dv") && t.contains("hdr")) {
        tags.push("DV".into());
    }
    if t.contains("hdr10+") {
        tags.push("HDR10+".into());
    } else if t.contains("hdr10") {
        tags.push("HDR10".into());
    } else if t.contains("hdr") && !tags.iter().any(|x| x.starts_with("HDR") || x == "DV") {
        tags.push("HDR".into());
    }

    let mut subtitle_info =
        if t.contains("multi-subs") || t.contains("multi subs") || t.contains("[multi-sub]") {
            Some("Multi Subs".into())
        } else if t.contains("eng-subs")
            || t.contains("eng sub")
            || t.contains("english sub")
            || t.contains("[engsub]")
        {
            Some("Eng Subs".into())
        } else if t.contains("subbed") || t.contains("[sub]") || t.contains("softsub") {
            Some("Subbed".into())
        } else if t.contains("hardsub") {
            Some("Hardsub".into())
        } else {
            None
        };

    if t.contains("bluray") || t.contains("blu-ray") || t.contains("bdremux") {
        tags.push("BluRay".into());
    } else if t.contains("web-dl") || t.contains("webdl") {
        tags.push("WEB-DL".into());
    } else if t.contains("webrip") || t.contains("web rip") {
        tags.push("WEBRip".into());
    } else if t.contains("bdrip") || t.contains("brrip") {
        tags.push("BDRip".into());
    } else if t.contains("hdtv") {
        tags.push("HDTV".into());
    }
    if t.contains("remux") && !tags.iter().any(|x| x == "BluRay") {
        tags.push("Remux".into());
    }
    if t.contains("batch") || t.contains("complete") {
        tags.push("Batch".into());
    }

    let release_group = if title.starts_with('[') {
        title.find(']').map(|end| title[1..end].to_string())
    } else {
        None
    };

    if let Some(group) = &release_group {
        match group.to_lowercase().as_str() {
            "subsplease" | "asw" | "yameii" | "commie" | "gg" | "horriblesubs"
            | "horrible subs" => {
                if audio.is_empty() {
                    audio.push("Japanese".into());
                }
                if subtitle_info.is_none() {
                    subtitle_info = Some("Eng Subs".into());
                }
            }
            "erai-raws" => {
                if audio.is_empty() {
                    audio.push("Japanese".into());
                }
                if subtitle_info.is_none() {
                    subtitle_info = Some("Multi Subs".into());
                }
            }
            "ember" | "cleo" | "toonshub" | "tenrai-sensei" | "anime time" | "animetime" => {
                if audio.is_empty() {
                    audio.push("Dual Audio".into());
                }
                if matches!(group.to_lowercase().as_str(), "ember" | "cleo" | "toonshub")
                    && subtitle_info.is_none()
                {
                    subtitle_info = Some("Multi Subs".into());
                }
            }
            "kaidubs" | "kai" => {
                if audio.is_empty() {
                    audio.push("English Dub".into());
                }
            }
            _ => {}
        }
    }

    TitleMeta {
        audio,
        video_codec,
        subtitle_info,
        release_group,
        tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::get, Router};

    async fn server(status: StatusCode) -> String {
        let app = Router::new().route("/api/v2.0/indexers", get(move || async move { status }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn ping_uses_http_status() {
        let ok = server(StatusCode::OK).await;
        assert!(Jackett::new(&ok, "key").ping().await.is_ok());

        let denied = server(StatusCode::UNAUTHORIZED).await;
        let err = Jackett::new(&denied, "bad").ping().await.unwrap_err();
        assert_eq!(err, "http 401");
    }
}
