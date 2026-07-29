use crate::jackett::sanitize_query;
use crate::models::TorrentOption;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone)]
pub struct Prowlarr {
    client: Client,
    base: String,
    api_key: String,
}

#[derive(Deserialize)]
struct Hit {
    #[serde(rename = "title", default)]
    title: String,
    #[serde(rename = "indexer", default)]
    indexer: String,
    #[serde(rename = "indexerId", default)]
    indexer_id: i64,
    #[serde(rename = "magnetUrl", default)]
    magnet_url: Option<String>,
    #[serde(rename = "downloadUrl", default)]
    download_url: Option<String>,
    #[serde(rename = "guid", default)]
    guid: Option<String>,
    #[serde(rename = "size", default)]
    size: i64,
    #[serde(rename = "seeders", default)]
    seeders: i32,
    #[serde(rename = "leechers", default)]
    leechers: i32,
}

impl Prowlarr {
    pub fn new(base: impl Into<String>, api_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .expect("build prowlarr http client");
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        }
    }

    pub async fn ping(&self) -> Result<(), String> {
        let url = format!("{}/api/v1/system/status", self.base);
        let resp = match self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
        {
            Ok(r) => r,
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
        cats: &[i32],
        imdb_id: Option<&str>,
    ) -> Vec<TorrentOption> {
        let cleaned = sanitize_query(query);
        let url = format!("{}/api/v1/search", self.base);

        let mut params: Vec<(&str, String)> = vec![("query", cleaned)];
        for c in cats {
            params.push(("categories", c.to_string()));
        }
        let search_type = if imdb_id.is_some() {
            if cats.iter().any(|c| (2000..3000).contains(c)) {
                "movie"
            } else {
                "tvsearch"
            }
        } else {
            "search"
        };
        params.push(("type", search_type.into()));
        if let Some(id) = imdb_id {
            let trimmed = id.trim_start_matches("tt");
            params.push(("imdbId", trimmed.into()));
        }

        let resp = match self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .query(&params)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[prowlarr] request failed: {e}");
                return Vec::new();
            }
        };

        let status = resp.status();
        if !status.is_success() {
            eprintln!("[prowlarr] status {status}");
            return Vec::new();
        }

        let hits: Vec<Hit> = match resp.json().await {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[prowlarr] json parse failed: {e}");
                return Vec::new();
            }
        };

        let mut out = Vec::with_capacity(hits.len());
        for h in hits {
            let magnet = h
                .magnet_url
                .filter(|m| !m.is_empty())
                .or_else(|| h.download_url.filter(|d| !d.is_empty()));
            let Some(magnet) = magnet else { continue };

            let meta = crate::jackett::parse_title_metadata(&h.title);
            let title = crate::jackett::decode_html(&h.title);
            let provider_id = h.guid.unwrap_or_else(|| h.indexer_id.to_string());

            out.push(TorrentOption {
                provider: h.indexer,
                provider_id,
                title,
                magnet,
                quality: crate::jackett::extract_quality(&h.title),
                size: h.size,
                seeds: h.seeders,
                peers: h.leechers,
                audio: meta.audio,
                video_codec: meta.video_codec,
                subtitle_info: meta.subtitle_info,
                release_group: meta.release_group,
                tags: meta.tags,
                pref_score: 0.0,
                aggregator: "prowlarr".into(),
            });
        }
        out
    }
}

fn config_candidates() -> [&'static str; 3] {
    [
        "/manage/prowlarr-config/config.xml",
        "deploy/prowlarr-config/config.xml",
        "../deploy/prowlarr-config/config.xml",
    ]
}

fn read_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let i = xml.find(&open)? + open.len();
    let j = xml[i..].find(&close)? + i;
    Some(xml[i..j].trim().to_string())
}

pub fn read_api_key_from_config() -> Option<String> {
    for p in config_candidates() {
        if let Ok(s) = std::fs::read_to_string(Path::new(p)) {
            if let Some(k) = read_xml_tag(&s, "ApiKey") {
                if !k.is_empty() {
                    return Some(k);
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
        let current = read_xml_tag(&s, "UrlBase").unwrap_or_default();
        if current == "/prowlarr" {
            return false;
        }
        let new = if let Some(i) = s.find("<UrlBase>") {
            let end = s[i..].find("</UrlBase>").map(|e| i + e).unwrap_or(i);
            let after = end + "</UrlBase>".len();
            format!("{}<UrlBase>/prowlarr</UrlBase>{}", &s[..i], &s[after..])
        } else if let Some(i) = s.find("</Config>") {
            format!("{}  <UrlBase>/prowlarr</UrlBase>\n{}", &s[..i], &s[i..])
        } else {
            continue;
        };
        if std::fs::write(path, new).is_ok() {
            println!("[prowlarr] patched config.xml (urlbase=/prowlarr)");
            return true;
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
                "prowlarr",
            ])
            .output()
            .await;
        if matches!(out, Ok(o) if o.status.success()) {
            println!("[prowlarr] container restarted via compose");
            return;
        }
    }
    let _ = tokio::process::Command::new("docker")
        .args(["restart", "pleasewatch-prowlarr-1"])
        .output()
        .await;
    println!("[prowlarr] container restart attempted via name");
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
                db.get_setting("prowlarr_api_key")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            };
            let url = {
                let db = state.db.lock().await;
                db.get_setting("prowlarr_url")
                    .ok()
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| std::env::var("PROWLARR_URL").unwrap_or_default())
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
                let _ = db.set_setting("prowlarr_api_key", &disk_key);
            }
            {
                let mut slot = state.prowlarr.lock().await;
                *slot = Some(Prowlarr::new(&url, &disk_key));
            }
            println!("[prowlarr] auto-imported api key from config");
            return;
        }
    });
}
