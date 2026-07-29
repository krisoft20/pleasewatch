mod admin;
mod anilist;
mod auth;
mod books;
mod clips;
mod collection;
mod crypto;
mod db;
mod downloads;
mod ffmpeg;
mod intro;
mod jackett;
mod jackett_proxy;
mod lang;
mod log_buf;
mod manga;
mod manga_ck;
mod manga_mk;
mod media;
mod middleware;
mod models;
mod omdb;
mod party;
mod prowlarr;
mod prowlarr_proxy;
mod qbit;
mod rate_limit;
mod search;
mod stream;
mod subs;
mod tmdb;
mod vpn;
mod watch;
mod ws;

use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

pub struct AppState {
    pub db: Mutex<db::Database>,
    pub media_root: String,
    pub tmdb_api_key: Mutex<String>,
    pub wyzie_api_key: Mutex<String>,
    pub omdb_api_key: Mutex<String>,
    pub login_limiter: Mutex<rate_limit::RateLimiter>,
    pub qbit: Mutex<Option<qbit::Qbit>>,
    pub jackett: Mutex<Option<jackett::Jackett>>,
    pub prowlarr: Mutex<Option<prowlarr::Prowlarr>>,
    pub party: party::PartyManager,
    pub subs_busy: Mutex<std::collections::HashSet<String>>,
}

impl AppState {
    pub async fn tmdb_key(&self) -> String {
        self.tmdb_api_key.lock().await.clone()
    }
    pub async fn wyzie_key(&self) -> String {
        self.wyzie_api_key.lock().await.clone()
    }
    pub async fn wyzie_keys_rotated(&self) -> Vec<String> {
        let raw = self.wyzie_api_key.lock().await.clone();
        rotate_keys(parse_wyzie_keys(&raw))
    }
    pub async fn omdb_key(&self) -> String {
        self.omdb_api_key.lock().await.clone()
    }
    pub async fn omdb_keys_rotated(&self) -> Vec<String> {
        let raw = self.omdb_api_key.lock().await.clone();
        rotate_omdb(parse_wyzie_keys(&raw))
    }
}

static WYZIE_RR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static OMDB_RR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn parse_wyzie_keys(raw: &str) -> Vec<String> {
    raw.split(|c: char| matches!(c, ',' | ';' | '\n' | '\r' | ' ' | '\t'))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn rotate_keys(keys: Vec<String>) -> Vec<String> {
    if keys.len() < 2 {
        return keys;
    }
    let start = WYZIE_RR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % keys.len();
    let mut out = Vec::with_capacity(keys.len());
    for i in 0..keys.len() {
        out.push(keys[(start + i) % keys.len()].clone());
    }
    out
}

fn rotate_omdb(keys: Vec<String>) -> Vec<String> {
    if keys.len() < 2 {
        return keys;
    }
    let start = OMDB_RR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % keys.len();
    let mut out = Vec::with_capacity(keys.len());
    for i in 0..keys.len() {
        out.push(keys[(start + i) % keys.len()].clone());
    }
    out
}

#[tokio::main]
async fn main() {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or("data/pleasewatch.db".into());
    let database = db::Database::open(db_path).expect("failed to open database");
    database.migrate().expect("failed to run migrations");

    let media_root = std::env::var("MEDIA_ROOT").unwrap_or_else(|_| "media".to_string());

    let tmdb_api_key = database
        .get_setting("tmdb_api_key")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("TMDB_API_KEY").unwrap_or_default());

    if tmdb_api_key.is_empty() {
        crate::pi!("[pleasewatch] TMDB_API_KEY not set, set it in /admin -> settings or via the onboarding flow");
    }

    let setting_or_env = |key: &str, env: &str| -> String {
        database
            .get_setting(key)
            .ok()
            .flatten()
            .unwrap_or_else(|| std::env::var(env).unwrap_or_default())
    };

    let qbit = {
        let url = setting_or_env("qbit_url", "QBIT_URL");
        let user = setting_or_env("qbit_user", "QBIT_USER");
        let pass = setting_or_env("qbit_pass", "QBIT_PASS");
        if url.is_empty() {
            crate::pi!("[pleasewatch] qbit not configured (set in admin settings or env)");
            None
        } else {
            Some(qbit::Qbit::new(&url, &user, &pass))
        }
    };

    let jackett = {
        let url = setting_or_env("jackett_url", "JACKETT_URL");
        let key = setting_or_env("jackett_api_key", "JACKETT_API_KEY");
        if url.is_empty() || key.is_empty() {
            crate::pi!("[pleasewatch] jackett not configured (set in admin settings or env)");
            None
        } else {
            Some(jackett::Jackett::new(&url, &key))
        }
    };

    let prowlarr = {
        let url = setting_or_env("prowlarr_url", "PROWLARR_URL");
        let key = setting_or_env("prowlarr_api_key", "PROWLARR_API_KEY");
        if url.is_empty() || key.is_empty() {
            crate::pi!("[pleasewatch] prowlarr not configured (set in admin settings or env)");
            None
        } else {
            Some(prowlarr::Prowlarr::new(&url, &key))
        }
    };

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("seed-admin") => {
            seed_admin(&database);
            return;
        }
        Some("approve-user") => {
            let username = args.get(2).map(|s| s.as_str()).unwrap_or("");
            approve_user(&database, username);
            return;
        }
        Some("import-env") => {
            let path = args.get(2).map(String::as_str).unwrap_or(".env");
            import_env(&database, path);
            return;
        }
        _ => {}
    }

    let wyzie_api_key = setting_or_env("wyzie_api_key", "WYZIE_API_KEY");
    if wyzie_api_key.is_empty() {
        crate::pi!("[pleasewatch] WYZIE_API_KEY not set, sub search will return 503. claim free at https://sub.wyzie.io/redeem");
    }

    let omdb_api_key = setting_or_env("omdb_api_key", "OMDB_API_KEY");
    if omdb_api_key.is_empty() {
        crate::pi!("[pleasewatch] OMDB_API_KEY not set, anime season detection disabled. claim free at https://www.omdbapi.com/apikey.aspx");
    }

    let state = Arc::new(AppState {
        db: Mutex::new(database),
        media_root,
        tmdb_api_key: Mutex::new(tmdb_api_key),
        wyzie_api_key: Mutex::new(wyzie_api_key),
        omdb_api_key: Mutex::new(omdb_api_key),
        login_limiter: Mutex::new(rate_limit::RateLimiter::new(5, 300)),
        qbit: Mutex::new(qbit),
        jackett: Mutex::new(jackett),
        prowlarr: Mutex::new(prowlarr),
        party: party::PartyManager::new(),
        subs_busy: Mutex::new(std::collections::HashSet::new()),
    });

    downloads::spawn_poller(state.clone());
    qbit::spawn_auto_setup(state.clone());
    qbit::spawn_dht_watchdog(state.clone());
    jackett::spawn_auto_setup(state.clone());
    prowlarr::spawn_auto_setup(state.clone());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_default();
    let mut app = Router::new()
        .merge(auth::routes(state.clone()))
        .merge(media::routes(state.clone()))
        .merge(search::routes(state.clone()))
        .merge(downloads::routes(state.clone()))
        .merge(admin::routes(state.clone()))
        .merge(jackett_proxy::routes(state.clone()))
        .merge(prowlarr_proxy::routes(state.clone()))
        .merge(vpn::routes(state.clone()))
        .merge(stream::routes(state.clone()))
        .merge(subs::routes(state.clone()))
        .merge(watch::routes(state.clone()))
        .merge(manga::routes(state.clone()))
        .merge(books::routes(state.clone()))
        .merge(collection::routes(state.clone()))
        .merge(clips::routes(state.clone()))
        .merge(clips::public_routes())
        .merge(party::routes(state.clone()))
        .merge(party::public_routes())
        .merge(ws::routes())
        .with_state(state)
        .layer(cors);

    if !static_dir.is_empty() && std::path::Path::new(&static_dir).exists() {
        let index = format!("{}/index.html", static_dir);
        let serve = ServeDir::new(&static_dir).fallback(ServeFile::new(&index));
        app = app.fallback_service(serve);
        crate::pi!("[pleasewatch] serving static from {}", static_dir);
    }

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);

    crate::pi!("[pleasewatch] listening on {} [build-mark-rs-6]", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

fn seed_admin(db: &db::Database) {
    use std::io::Write;

    let count = db.count_users().unwrap_or(0);
    if count > 0 {
        let admins = db.list_users_by_role("admin").unwrap_or_default();
        if !admins.is_empty() {
            crate::pi!("[pleasewatch] admin already exists: {}", admins[0].username);
            return;
        }
    }

    print!("username: ");
    std::io::stdout().flush().ok();
    let mut username = String::new();
    std::io::stdin().read_line(&mut username).ok();
    let username = username.trim();

    print!("email: ");
    std::io::stdout().flush().ok();
    let mut email = String::new();
    std::io::stdin().read_line(&mut email).ok();
    let email = email.trim();

    print!("password: ");
    std::io::stdout().flush().ok();
    let mut password = String::new();
    std::io::stdin().read_line(&mut password).ok();
    let password = password.trim();

    let id = uuid::Uuid::new_v4().to_string();
    let hash = match crypto::hash_password(password) {
        Ok(h) => h,
        Err(e) => {
            crate::pe!("[pleasewatch] hash failed: {}", e);
            return;
        }
    };

    if let Err(e) = db.create_user(&id, username, email, &hash, "admin") {
        crate::pe!("[pleasewatch] create failed: {}", e);
        return;
    }

    crate::pi!("[pleasewatch] admin '{}' created", username);
}

fn import_env(db: &db::Database, path: &str) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            crate::pe!("[pleasewatch] cannot read {path}: {e}");
            return;
        }
    };

    let mappings = [
        ("JACKETT_URL", "jackett_url"),
        ("JACKETT_API_KEY", "jackett_api_key"),
        ("QBIT_URL", "qbit_url"),
        ("QBIT_USER", "qbit_user"),
        ("QBIT_PASS", "qbit_pass"),
        ("JACKETT_INDEXERS_MOVIE", "jackett_indexers_movie"),
        ("JACKETT_INDEXERS_TV", "jackett_indexers_tv"),
        ("JACKETT_INDEXERS_ANIME", "jackett_indexers_anime"),
        ("JACKETT_INDEXERS_BOOK", "jackett_indexers_book"),
        ("JACKETT_INDEXERS", "jackett_indexers"),
        ("PROWLARR_URL", "prowlarr_url"),
        ("PROWLARR_API_KEY", "prowlarr_api_key"),
    ];

    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, val)) = trimmed.split_once('=') else {
            continue;
        };
        let val = val.trim().trim_matches('"').trim_matches('\'');
        if val.is_empty() {
            continue;
        }
        for (env_key, setting_key) in &mappings {
            if key == *env_key {
                if let Err(e) = db.set_setting(setting_key, val) {
                    crate::pe!("[pleasewatch] save {setting_key} failed: {e}");
                } else {
                    crate::pi!("[pleasewatch] imported {setting_key}");
                    count += 1;
                }
                break;
            }
        }
    }
    crate::pi!("[pleasewatch] {count} settings imported. restart the server.");
}

fn approve_user(db: &db::Database, username: &str) {
    if username.is_empty() {
        crate::pe!("[pleasewatch] usage: approve-user <username>");
        return;
    }
    match db.find_user_by_username(username) {
        Ok(Some(u)) => match db.approve_user(&u.id, "cli") {
            Ok(true) => crate::pi!("[pleasewatch] approved '{}'", u.username),
            Ok(false) => crate::pi!(
                "[pleasewatch] '{}' was not pending (role={})",
                u.username,
                u.role
            ),
            Err(e) => crate::pe!("[pleasewatch] approve failed: {}", e),
        },
        Ok(None) => crate::pe!("[pleasewatch] no user '{}'", username),
        Err(e) => crate::pe!("[pleasewatch] db error: {}", e),
    }
}
