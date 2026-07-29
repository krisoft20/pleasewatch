use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::process::Command;

#[derive(Clone)]
pub struct Qbit {
    client: Client,
    pub base: String,
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub hash: String,
    pub name: String,
    pub progress: f64,
    pub state: String,
    pub save_path: String,
    pub size: i64,
    pub content_path: String,
    #[serde(default)]
    pub num_seeds: i64,
    #[serde(default)]
    pub num_leechs: i64,
    #[serde(default)]
    pub dlspeed: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TorrentFile {
    pub name: String,
    #[serde(default)]
    pub size: i64,
    pub progress: f64,
}

impl Qbit {
    pub fn new(base: &str, user: &str, pass: &str) -> Self {
        let client = Client::builder()
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("build qbit http client");
        Self {
            client,
            base: base.trim_end_matches('/').to_string(),
            user: user.into(),
            pass: pass.into(),
        }
    }

    pub fn is_complete(t: &Torrent) -> bool {
        t.progress >= 1.0
            || matches!(
                t.state.as_str(),
                "uploading" | "pausedUP" | "stalledUP" | "queuedUP" | "forcedUP" | "checkingUP"
            )
    }

    async fn login(&self) -> reqwest::Result<()> {
        let resp = self
            .client
            .post(format!("{}/api/v2/auth/login", self.base))
            .form(&[("username", &self.user), ("password", &self.pass)])
            .send()
            .await?;
        let body = resp.text().await?;
        if !body.to_lowercase().contains("ok") {
            eprintln!("[qbit] login response: {}", body);
        }
        Ok(())
    }

    async fn authed<F>(&self, build: F) -> reqwest::Result<reqwest::Response>
    where
        F: Fn(&Client) -> reqwest::RequestBuilder,
    {
        let r = build(&self.client).send().await?;
        if r.status() == StatusCode::FORBIDDEN {
            self.login().await?;
            return build(&self.client).send().await;
        }
        Ok(r)
    }

    pub async fn ping(&self) -> reqwest::Result<()> {
        self.login().await?;
        let v = self
            .client
            .get(format!("{}/api/v2/app/version", self.base))
            .send()
            .await?;
        if !v.status().is_success() {
            return Err(v.error_for_status().unwrap_err());
        }
        Ok(())
    }

    pub async fn try_login_with(&self, user: &str, pass: &str) -> bool {
        let r = match self
            .client
            .post(format!("{}/api/v2/auth/login", self.base))
            .form(&[("username", user), ("password", pass)])
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => return false,
        };
        let status = r.status();
        let got_cookie = r
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .any(|v| v.to_str().map(|s| s.contains("QBT_SID")).unwrap_or(false));
        let body = r.text().await.unwrap_or_default();
        status.is_success() && (got_cookie || body.to_lowercase().contains("ok"))
    }

    pub async fn set_password(&self, new_password: &str) -> Result<(), String> {
        let prefs = serde_json::json!({ "web_ui_password": new_password });
        let body = format!("json={}", urlencode(&prefs.to_string()));
        let resp = self
            .client
            .post(format!("{}/api/v2/app/setPreferences", self.base))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Referer", &self.base)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("send: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("setPreferences {} body: {}", status, body.trim()));
        }
        Ok(())
    }
}

pub fn spawn_auto_setup(state: std::sync::Arc<crate::AppState>) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;

        for attempt in 1..=12 {
            let mut qbit = match state.qbit.lock().await.clone() {
                Some(q) => q,
                None => return,
            };

            if qbit.ping().await.is_ok() {
                println!("[qbit] connected with stored credentials");
                return;
            }

            let Some(temp) = find_temp_password() else {
                if attempt == 1 {
                    println!("[qbit] auth failed and no temp password in logs. set the password manually in /admin or qbit's web ui.");
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                continue;
            };

            println!("[qbit] found temp password in logs, claiming admin...");
            if !qbit.try_login_with(&qbit.user, &temp).await {
                eprintln!("[qbit] temp password rejected by qbit");
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                continue;
            }

            if qbit.pass.len() < 6 {
                let generated = random_password(20);
                if let Err(e) = persist_qbit_pass(&generated) {
                    eprintln!("[qbit] could not persist QBIT_PASS to .env: {e}");
                    return;
                }
                println!("[qbit] generated QBIT_PASS and wrote to .env");
                qbit.pass = generated.clone();
                {
                    let db = state.db.lock().await;
                    let _ = db.set_setting("qbit_pass", &generated);
                }
                {
                    let mut slot = state.qbit.lock().await;
                    if let Some(q) = slot.as_mut() {
                        q.pass = generated;
                    }
                }
            }

            if let Err(e) = qbit.set_password(&qbit.pass).await {
                eprintln!("[qbit] could not set permanent password: {e}");
                return;
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            match qbit.ping().await {
                Ok(_) => {
                    println!("[qbit] auto-setup complete, password persisted in QBIT_PASS");
                    return;
                }
                Err(e) => {
                    eprintln!("[qbit] post-setup ping failed: {e}");
                    return;
                }
            }
        }
    });
}

fn random_password(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn persist_qbit_pass(pw: &str) -> std::io::Result<()> {
    let path = std::env::var("PW_DEPLOY_DIR")
        .map(|d| format!("{d}/.env"))
        .unwrap_or_else(|_| "/manage/.env".into());
    let p = std::path::PathBuf::from(&path);
    let original = std::fs::read_to_string(&p).unwrap_or_default();
    let mut out = String::new();
    let mut found = false;
    for line in original.lines() {
        if line.starts_with("QBIT_PASS=") {
            out.push_str("QBIT_PASS=");
            out.push_str(pw);
            out.push('\n');
            found = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !found {
        out.push_str("QBIT_PASS=");
        out.push_str(pw);
        out.push('\n');
    }
    std::fs::write(&p, out)
}

fn find_temp_password() -> Option<String> {
    const MARKER: &str = "temporary password is provided for this session:";
    let names = ["pleasewatch-qbittorrent-1", "qbittorrent"];
    for name in names {
        let out = Command::new("docker").args(["logs", name]).output().ok()?;
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let mut last: Option<String> = None;
        for line in combined.lines() {
            if let Some(pos) = line.find(MARKER) {
                let tail = line[pos + MARKER.len()..].trim();
                if !tail.is_empty() {
                    last = Some(tail.to_string());
                }
            }
        }
        if last.is_some() {
            return last;
        }
    }
    None
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

impl Qbit {
    pub async fn add_magnet(
        &self,
        magnet: &str,
        category: &str,
        save_path: &str,
    ) -> Result<(), String> {
        self.login()
            .await
            .map_err(|e| format!("login failed: {e}"))?;
        let url = format!("{}/api/v2/torrents/add", self.base);
        let send = || async {
            let form = reqwest::multipart::Form::new()
                .text("urls", magnet.to_string())
                .text("category", category.to_string())
                .text("savepath", save_path.to_string())
                .text("sequentialDownload", "true");
            self.client.post(&url).multipart(form).send().await
        };

        let mut r = send().await.map_err(|e| e.to_string())?;
        if r.status() == StatusCode::FORBIDDEN {
            self.login()
                .await
                .map_err(|e| format!("re-login failed: {e}"))?;
            r = send().await.map_err(|e| e.to_string())?;
        }

        let status = r.status();
        if status == StatusCode::CONFLICT {
            eprintln!("[qbit] add returned 409, treating as already-present");
            return Ok(());
        }
        if status.is_success() {
            return Ok(());
        }
        let body = r.text().await.unwrap_or_default();
        Err(format!(
            "qbit add returned {} ({})",
            status.as_u16(),
            body.trim()
        ))
    }

    pub async fn list(&self, category: Option<&str>) -> reqwest::Result<Vec<Torrent>> {
        let mut url = format!("{}/api/v2/torrents/info", self.base);
        if let Some(c) = category {
            url = format!("{url}?category={c}");
        }
        let r = self.authed(|c| c.get(&url)).await?.error_for_status()?;
        r.json().await
    }

    pub async fn get(&self, hash: &str) -> reqwest::Result<Option<Torrent>> {
        let url = format!("{}/api/v2/torrents/info?hashes={hash}", self.base);
        let r = self.authed(|c| c.get(&url)).await?.error_for_status()?;
        let mut v: Vec<Torrent> = r.json().await?;
        Ok(v.pop())
    }

    pub async fn files(&self, hash: &str) -> reqwest::Result<Vec<TorrentFile>> {
        let url = format!("{}/api/v2/torrents/files?hash={hash}", self.base);
        let r = self.authed(|c| c.get(&url)).await?.error_for_status()?;
        r.json().await
    }

    pub async fn set_upload_limit(&self, hash: &str, bytes_per_sec: i64) -> reqwest::Result<()> {
        let limit = bytes_per_sec.to_string();
        self.authed(|c| {
            c.post(format!("{}/api/v2/torrents/setUploadLimit", self.base))
                .form(&[("hashes", hash), ("limit", limit.as_str())])
        })
        .await?
        .error_for_status()?;
        Ok(())
    }

    pub async fn delete(&self, hash: &str, delete_files: bool) -> reqwest::Result<()> {
        let flag = if delete_files { "true" } else { "false" };
        self.authed(|c| {
            c.post(format!("{}/api/v2/torrents/delete", self.base))
                .form(&[("hashes", hash), ("deleteFiles", flag)])
        })
        .await?
        .error_for_status()?;
        Ok(())
    }

    pub async fn dht_nodes(&self) -> reqwest::Result<i64> {
        let url = format!("{}/api/v2/transfer/info", self.base);
        let r = self.authed(|c| c.get(&url)).await?.error_for_status()?;
        let v: serde_json::Value = r.json().await?;
        Ok(v.get("dht_nodes").and_then(|n| n.as_i64()).unwrap_or(-1))
    }
}

pub fn spawn_dht_watchdog(state: std::sync::Arc<crate::AppState>) {
    tokio::spawn(async move {
        let mut strikes: u32 = 0;
        let mut last_restart: Option<std::time::Instant> = None;
        const COOLDOWN: std::time::Duration = std::time::Duration::from_secs(900);
        const TICK: std::time::Duration = std::time::Duration::from_secs(300);
        const STRIKES_TO_RESTART: u32 = 2;

        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        loop {
            let qbit = match state.qbit.lock().await.clone() {
                Some(q) => q,
                None => {
                    tokio::time::sleep(TICK).await;
                    continue;
                }
            };

            let dht = match qbit.dht_nodes().await {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("[dht-watchdog] qbit transfer/info failed: {e}");
                    tokio::time::sleep(TICK).await;
                    continue;
                }
            };

            let active = match qbit.list(Some("pleasewatch")).await {
                Ok(ts) => ts.iter().filter(|t| is_active(&t.state)).count(),
                Err(e) => {
                    eprintln!("[dht-watchdog] qbit list failed: {e}");
                    tokio::time::sleep(TICK).await;
                    continue;
                }
            };

            println!("[dht-watchdog] dht={dht} active={active}");

            if dht > 0 || active == 0 {
                strikes = 0;
            } else {
                strikes += 1;
                println!("[dht-watchdog] dht=0 with {active} active torrents, strike {strikes}/{STRIKES_TO_RESTART}");

                let in_cooldown = last_restart
                    .map(|t| t.elapsed() < COOLDOWN)
                    .unwrap_or(false);

                if strikes >= STRIKES_TO_RESTART && !in_cooldown {
                    println!("[dht-watchdog] restarting gluetun + qbittorrent");
                    for name in ["pleasewatch-gluetun-1", "pleasewatch-qbittorrent-1"] {
                        match Command::new("docker").args(["restart", name]).output() {
                            Ok(o) if o.status.success() => {
                                println!("[dht-watchdog] restarted {name}")
                            }
                            Ok(o) => eprintln!(
                                "[dht-watchdog] docker restart {name} exited {}: {}",
                                o.status,
                                String::from_utf8_lossy(&o.stderr).trim()
                            ),
                            Err(e) => eprintln!(
                                "[dht-watchdog] could not spawn docker restart {name}: {e}"
                            ),
                        }
                    }
                    last_restart = Some(std::time::Instant::now());
                    strikes = 0;
                } else if strikes >= STRIKES_TO_RESTART {
                    let secs = last_restart.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                    println!(
                        "[dht-watchdog] skipping, last restart was {secs}s ago (cooldown {}s)",
                        COOLDOWN.as_secs()
                    );
                }
            }

            tokio::time::sleep(TICK).await;
        }
    });
}

fn is_active(state: &str) -> bool {
    matches!(
        state,
        "metaDL" | "downloading" | "stalledDL" | "queuedDL" | "forcedDL"
    )
}
