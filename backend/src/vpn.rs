use crate::{middleware::AuthUser, models::ApiError, AppState};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/admin/vpn", get(handle_status).post(handle_save))
        .route("/api/admin/vpn/disable", post(handle_disable))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Debug, Serialize)]
struct VpnStatus {
    enabled: bool,
    provider: String,
    countries: String,
    has_key: bool,
    addresses: String,
    container_state: Option<String>,
    public_ip: Option<String>,
}

async fn handle_status(Extension(auth): Extension<AuthUser>) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    let env = read_env();
    let enabled = override_yml().exists();
    let pip = if enabled { gluetun_ip().await } else { None };

    Json(VpnStatus {
        enabled,
        provider: env.get("VPN_PROVIDER").cloned().unwrap_or_default(),
        countries: env.get("VPN_COUNTRIES").cloned().unwrap_or_default(),
        addresses: env.get("WIREGUARD_ADDRESSES").cloned().unwrap_or_default(),
        has_key: env
            .get("WIREGUARD_PRIVATE_KEY")
            .map(|v| !v.is_empty())
            .unwrap_or(false),
        container_state: docker_state("pleasewatch-gluetun-1"),
        public_ip: pip,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
struct VpnSave {
    provider: String,
    wireguard_private_key: String,
    wireguard_addresses: String,
    countries: Option<String>,
}

async fn handle_save(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<VpnSave>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    let key = body.wireguard_private_key.trim();
    let addr = body.wireguard_addresses.trim();
    if key.is_empty() || addr.is_empty() {
        return err(StatusCode::BAD_REQUEST, "wireguard key + address required");
    }

    let mut env = read_env();
    env.insert("VPN_PROVIDER".into(), body.provider.trim().to_string());
    env.insert("VPN_TYPE".into(), "wireguard".into());
    env.insert("WIREGUARD_PRIVATE_KEY".into(), key.into());
    env.insert("WIREGUARD_ADDRESSES".into(), addr.into());
    env.insert(
        "VPN_COUNTRIES".into(),
        body.countries.unwrap_or_default().trim().to_string(),
    );

    if let Err(e) = write_env(&env) {
        eprintln!("[vpn] write env: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "could not write env");
    }
    if let Err(e) = std::fs::write(override_yml(), OVERRIDE_YML) {
        eprintln!("[vpn] write override: {e}");
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not write override",
        );
    }
    let _ = write_gluetun_auth();

    swap_qbit_url(&state, "http://gluetun:8080").await;

    if let Err(e) = compose(
        "up",
        &["-d", "--force-recreate"],
        &["gluetun", "qbittorrent"],
    ) {
        eprintln!("[vpn] compose up failed: {e}");
        return err(StatusCode::BAD_GATEWAY, "compose restart failed");
    }

    println!("[vpn] enabled via {} for {}", body.provider, auth.username);
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_disable(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    let mut env = read_env();
    for k in [
        "VPN_PROVIDER",
        "VPN_TYPE",
        "WIREGUARD_PRIVATE_KEY",
        "WIREGUARD_ADDRESSES",
        "VPN_COUNTRIES",
        "OPENVPN_USER",
        "OPENVPN_PASSWORD",
    ] {
        env.insert(k.into(), String::new());
    }
    if let Err(e) = write_env(&env) {
        eprintln!("[vpn] write env: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "could not write env");
    }
    let _ = std::fs::remove_file(override_yml());

    swap_qbit_url(&state, "http://qbittorrent:8080").await;

    let _ = compose("stop", &[], &["gluetun"]);
    let _ = compose("up", &["-d", "--force-recreate"], &["qbittorrent"]);

    println!("[vpn] disabled by {}", auth.username);
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn swap_qbit_url(state: &Arc<AppState>, url: &str) {
    {
        let db = state.db.lock().await;
        let _ = db.set_setting("qbit_url", url);
    }
    let mut slot = state.qbit.lock().await;
    if let Some(q) = slot.as_ref() {
        let user = q.user.clone();
        let pass = q.pass.clone();
        *slot = Some(crate::qbit::Qbit::new(url, &user, &pass));
    }
}

fn deploy_dir() -> PathBuf {
    PathBuf::from(std::env::var("PW_DEPLOY_DIR").unwrap_or_else(|_| "/manage".into()))
}
fn env_file() -> PathBuf {
    deploy_dir().join(".env")
}
fn override_yml() -> PathBuf {
    deploy_dir().join("docker-compose.override.yml")
}
fn compose_yml() -> String {
    std::env::var("PW_COMPOSE_FILE").unwrap_or_else(|_| "/manage/docker-compose.simple.yml".into())
}

fn read_env() -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(s) = std::fs::read_to_string(env_file()) else {
        return out;
    };
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            out.insert(k.into(), v.into());
        }
    }
    out
}

fn write_env(env: &std::collections::BTreeMap<String, String>) -> std::io::Result<()> {
    let path = env_file();
    let original = std::fs::read_to_string(&path).unwrap_or_default();
    let mut seen = std::collections::HashSet::<String>::new();
    let mut buf = String::new();

    for line in original.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            buf.push_str(line);
            buf.push('\n');
            continue;
        }
        match t.split_once('=') {
            Some((k, _)) if env.contains_key(k) => {
                buf.push_str(k);
                buf.push('=');
                buf.push_str(&env[k]);
                buf.push('\n');
                seen.insert(k.into());
            }
            _ => {
                buf.push_str(line);
                buf.push('\n');
            }
        }
    }
    for (k, v) in env {
        if !seen.contains(k) {
            buf.push_str(k);
            buf.push('=');
            buf.push_str(v);
            buf.push('\n');
        }
    }
    std::fs::write(&path, buf)
}

const OVERRIDE_YML: &str = r#"services:
  gluetun:
    image: qmcgaw/gluetun:latest
    cap_add: [NET_ADMIN]
    devices:
      - /dev/net/tun:/dev/net/tun
    networks: [pwnet]
    volumes:
      - ./gluetun-config:/gluetun
    ports:
      - "${QBIT_BIND:-127.0.0.1:8080}:8080"
    environment:
      - VPN_SERVICE_PROVIDER=${VPN_PROVIDER}
      - VPN_TYPE=${VPN_TYPE:-wireguard}
      - WIREGUARD_PRIVATE_KEY=${WIREGUARD_PRIVATE_KEY:-}
      - WIREGUARD_ADDRESSES=${WIREGUARD_ADDRESSES:-}
      - SERVER_COUNTRIES=${VPN_COUNTRIES:-}
      - SERVER_CITIES=${SERVER_CITIES:-}
      - OPENVPN_USER=${OPENVPN_USER:-}
      - OPENVPN_PASSWORD=${OPENVPN_PASSWORD:-}
      - WIREGUARD_MTU=${WIREGUARD_MTU:-1320}
      - TZ=${TZ:-UTC}
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "-qO-", "https://ipinfo.io/ip"]
      interval: 60s
      timeout: 15s
      retries: 5
  qbittorrent:
    network_mode: "service:gluetun"
    networks: !reset null
    ports: !reset []
    depends_on:
      gluetun:
        condition: service_healthy
  pleasewatch:
    environment:
      - QBIT_URL=http://gluetun:8080
"#;

fn write_gluetun_auth() -> std::io::Result<()> {
    let dir = deploy_dir().join("gluetun-config").join("auth");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("config.toml"),
        r#"[[roles]]
name = "pleasewatch"
routes = ["GET /v1/publicip/ip", "GET /v1/vpn/status"]
auth = "none"
"#,
    )
}

fn compose(verb: &str, flags: &[&str], services: &[&str]) -> Result<(), String> {
    let host_dir =
        std::env::var("PW_HOST_DEPLOY_DIR").unwrap_or_else(|_| "/opt/pleasewatch/deploy".into());
    let mut c = Command::new("docker");
    c.env("COMPOSE_PROJECT_DIR", &host_dir);
    c.arg("compose")
        .arg("--project-directory")
        .arg(&host_dir)
        .arg("--env-file")
        .arg(env_file())
        .arg("-f")
        .arg(compose_yml());
    if override_yml().exists() {
        c.arg("-f").arg(override_yml());
    }
    c.arg(verb).args(flags).args(services);
    let out = c.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn docker_state(name: &str) -> Option<String> {
    let out = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Status}}", name])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().into())
}

async fn gluetun_ip() -> Option<String> {
    let r = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()?
        .get("http://gluetun:8000/v1/publicip/ip")
        .send()
        .await
        .ok()?;
    let v: serde_json::Value = r.json().await.ok()?;
    v.get("public_ip")
        .and_then(|x| x.as_str())
        .map(String::from)
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
