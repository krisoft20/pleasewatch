use crate::{
    jackett::Jackett,
    middleware::AuthUser,
    models::{ApiError, UserPublic},
    qbit::Qbit,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/admin/settings", get(handle_settings_get))
        .route("/api/admin/settings", post(handle_settings_post))
        .route("/api/admin/users", get(handle_users_list))
        .route("/api/admin/users/{id}/approve", post(handle_user_approve))
        .route("/api/admin/users/{id}/role", post(handle_user_role))
        .route("/api/admin/users/{id}", delete(handle_user_delete))
        .route("/api/admin/stats", get(handle_stats))
        .route("/api/admin/health", get(handle_health))
        .route("/api/admin/storage", get(handle_storage))
        .route("/api/admin/system", get(handle_system))
        .route("/api/admin/metrics", get(handle_metrics))
        .route("/api/admin/insights", get(handle_insights))
        .route("/api/admin/processing", get(handle_processing))
        .route("/api/admin/watch-stats", get(handle_watch_stats))
        .route("/api/admin/disk-usage", get(handle_disk_usage))
        .route("/api/admin/cleanup", post(handle_cleanup))
        .route("/api/admin/clean-downloads", post(handle_clean_downloads))
        .route(
            "/api/admin/detect-intros/{media_id}",
            post(handle_detect_intros),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Debug, Serialize)]
struct SettingsView {
    tmdb_api_key_set: bool,
    wyzie_api_key_set: bool,
    wyzie_key_count: usize,
    wyzie_keys_masked: Vec<String>,
    wyzie_keys_full: Vec<String>,
    omdb_api_key_set: bool,
    omdb_ready: bool,
    omdb_key_count: usize,
    omdb_keys_masked: Vec<String>,
    omdb_keys_full: Vec<String>,
    jackett_url: String,
    jackett_api_key_set: bool,
    prowlarr_url: String,
    prowlarr_api_key_set: bool,
    qbit_url: String,
    qbit_user: String,
    qbit_pass_set: bool,
    jackett_ready: bool,
    prowlarr_ready: bool,
    qbit_ready: bool,
    tmdb_ready: bool,
    wyzie_ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct SettingsUpdate {
    pub tmdb_api_key: Option<String>,
    pub wyzie_api_key: Option<String>,
    pub wyzie_key_add: Option<String>,
    pub wyzie_key_remove_mask: Option<String>,
    pub omdb_api_key: Option<String>,
    pub omdb_key_add: Option<String>,
    pub omdb_key_remove_mask: Option<String>,
    pub jackett_url: Option<String>,
    pub jackett_api_key: Option<String>,
    pub prowlarr_url: Option<String>,
    pub prowlarr_api_key: Option<String>,
    pub qbit_url: Option<String>,
    pub qbit_user: Option<String>,
    pub qbit_pass: Option<String>,
}

fn mask_wyzie_key(k: &str) -> String {
    let stripped = k.strip_prefix("wyzie-").unwrap_or(k);
    if stripped.len() < 8 {
        return format!("wyzie-{}", "*".repeat(stripped.len().max(4)));
    }
    let head = &stripped[..4];
    let tail = &stripped[stripped.len() - 3..];
    format!("wyzie-{head}****{tail}")
}

fn mask_omdb_key(k: &str) -> String {
    if k.len() < 6 {
        return "*".repeat(k.len().max(4));
    }
    let head = &k[..2];
    let tail = &k[k.len() - 2..];
    format!("{head}****{tail}")
}

fn setting_or_env(db: &crate::db::Database, key: &str, env: &str) -> String {
    db.get_setting(key)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var(env).unwrap_or_default())
}

async fn build_view(state: &Arc<AppState>) -> SettingsView {
    let (
        jackett_url,
        jackett_api_key,
        prowlarr_url,
        prowlarr_api_key,
        qbit_url,
        qbit_user,
        qbit_pass,
    ) = {
        let db = state.db.lock().await;
        (
            setting_or_env(&db, "jackett_url", "JACKETT_URL"),
            setting_or_env(&db, "jackett_api_key", "JACKETT_API_KEY"),
            setting_or_env(&db, "prowlarr_url", "PROWLARR_URL"),
            setting_or_env(&db, "prowlarr_api_key", "PROWLARR_API_KEY"),
            setting_or_env(&db, "qbit_url", "QBIT_URL"),
            setting_or_env(&db, "qbit_user", "QBIT_USER"),
            setting_or_env(&db, "qbit_pass", "QBIT_PASS"),
        )
    };

    let tmdb_key = state.tmdb_key().await;
    let wyzie_key = state.wyzie_key().await;
    let omdb_key = state.omdb_key().await;
    let wyzie_keys = crate::parse_wyzie_keys(&wyzie_key);
    let omdb_keys = crate::parse_wyzie_keys(&omdb_key);
    let wyzie_key_count = wyzie_keys.len();
    let omdb_key_count = omdb_keys.len();
    let wyzie_keys_masked: Vec<String> = wyzie_keys.iter().map(|k| mask_wyzie_key(k)).collect();
    let wyzie_keys_full: Vec<String> = wyzie_keys.iter().map(|k| k.to_string()).collect();
    let omdb_keys_masked: Vec<String> = omdb_keys.iter().map(|k| mask_omdb_key(k)).collect();
    let omdb_keys_full: Vec<String> = omdb_keys.iter().map(|k| k.to_string()).collect();
    let jackett_ready = state.jackett.lock().await.is_some();
    let prowlarr_ready = state.prowlarr.lock().await.is_some();
    let qbit_ready = state.qbit.lock().await.is_some();

    SettingsView {
        tmdb_api_key_set: !tmdb_key.is_empty(),
        wyzie_api_key_set: wyzie_key_count > 0,
        wyzie_key_count,
        wyzie_keys_masked,
        wyzie_keys_full,
        omdb_api_key_set: omdb_key_count > 0,
        omdb_ready: omdb_key_count > 0,
        omdb_key_count,
        omdb_keys_masked,
        omdb_keys_full,
        jackett_url,
        jackett_api_key_set: !jackett_api_key.is_empty(),
        prowlarr_url,
        prowlarr_api_key_set: !prowlarr_api_key.is_empty(),
        qbit_url,
        qbit_user,
        qbit_pass_set: !qbit_pass.is_empty(),
        jackett_ready,
        prowlarr_ready,
        qbit_ready,
        tmdb_ready: !tmdb_key.is_empty(),
        wyzie_ready: wyzie_key_count > 0,
    }
}

async fn handle_settings_get(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    Json(build_view(&state).await).into_response()
}

async fn handle_settings_post(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsUpdate>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    {
        let db = state.db.lock().await;
        for (k, v) in [
            ("tmdb_api_key", &body.tmdb_api_key),
            ("wyzie_api_key", &body.wyzie_api_key),
            ("omdb_api_key", &body.omdb_api_key),
            ("jackett_url", &body.jackett_url),
            ("jackett_api_key", &body.jackett_api_key),
            ("prowlarr_url", &body.prowlarr_url),
            ("prowlarr_api_key", &body.prowlarr_api_key),
            ("qbit_url", &body.qbit_url),
            ("qbit_user", &body.qbit_user),
            ("qbit_pass", &body.qbit_pass),
        ] {
            if let Some(val) = v.as_deref() {
                let _ = db.set_setting(k, val.trim());
            }
        }
    }

    if let Some(tmdb) = body.tmdb_api_key.as_deref() {
        let trimmed = tmdb.trim().to_string();
        if !trimmed.is_empty() {
            *state.tmdb_api_key.lock().await = trimmed;
        }
    }

    if let Some(wyzie) = body.wyzie_api_key.as_deref() {
        let trimmed = wyzie.trim().to_string();
        if !trimmed.is_empty() {
            *state.wyzie_api_key.lock().await = trimmed;
        }
    }

    if let Some(omdb) = body.omdb_api_key.as_deref() {
        let trimmed = omdb.trim().to_string();
        *state.omdb_api_key.lock().await = trimmed;
    }

    if body.wyzie_key_add.is_some() || body.wyzie_key_remove_mask.is_some() {
        let current = state.wyzie_api_key.lock().await.clone();
        let mut keys = crate::parse_wyzie_keys(&current);

        if let Some(add) = body.wyzie_key_add.as_deref() {
            let trimmed = add.trim();
            if !trimmed.is_empty() && !keys.iter().any(|k| k == trimmed) {
                keys.push(trimmed.to_string());
            }
        }
        if let Some(mask) = body.wyzie_key_remove_mask.as_deref() {
            keys.retain(|k| mask_wyzie_key(k) != mask);
        }

        let joined = keys.join(",");
        {
            let db = state.db.lock().await;
            let _ = db.set_setting("wyzie_api_key", &joined);
        }
        *state.wyzie_api_key.lock().await = joined;
    }

    if body.omdb_key_add.is_some() || body.omdb_key_remove_mask.is_some() {
        let current = state.omdb_api_key.lock().await.clone();
        let mut keys = crate::parse_wyzie_keys(&current);

        if let Some(add) = body.omdb_key_add.as_deref() {
            let trimmed = add.trim();
            if !trimmed.is_empty() && !keys.iter().any(|k| k == trimmed) {
                keys.push(trimmed.to_string());
            }
        }
        if let Some(mask) = body.omdb_key_remove_mask.as_deref() {
            keys.retain(|k| mask_omdb_key(k) != mask);
        }

        let joined = keys.join(",");
        {
            let db = state.db.lock().await;
            let _ = db.set_setting("omdb_api_key", &joined);
        }
        *state.omdb_api_key.lock().await = joined;
    }

    let (jackett_url, jackett_key, prowlarr_url, prowlarr_key, qbit_url, qbit_user, qbit_pass) = {
        let db = state.db.lock().await;
        (
            setting_or_env(&db, "jackett_url", "JACKETT_URL"),
            setting_or_env(&db, "jackett_api_key", "JACKETT_API_KEY"),
            setting_or_env(&db, "prowlarr_url", "PROWLARR_URL"),
            setting_or_env(&db, "prowlarr_api_key", "PROWLARR_API_KEY"),
            setting_or_env(&db, "qbit_url", "QBIT_URL"),
            setting_or_env(&db, "qbit_user", "QBIT_USER"),
            setting_or_env(&db, "qbit_pass", "QBIT_PASS"),
        )
    };

    {
        let mut slot = state.jackett.lock().await;
        *slot = if !jackett_url.is_empty() && !jackett_key.is_empty() {
            Some(Jackett::new(&jackett_url, &jackett_key))
        } else {
            None
        };
    }

    {
        let mut slot = state.prowlarr.lock().await;
        *slot = if !prowlarr_url.is_empty() && !prowlarr_key.is_empty() {
            Some(crate::prowlarr::Prowlarr::new(&prowlarr_url, &prowlarr_key))
        } else {
            None
        };
    }

    {
        let mut slot = state.qbit.lock().await;
        *slot = if !qbit_url.is_empty() {
            Some(Qbit::new(&qbit_url, &qbit_user, &qbit_pass))
        } else {
            None
        };
    }

    crate::pi!("[admin] settings updated by {}", auth.username);

    Json(build_view(&state).await).into_response()
}

async fn handle_users_list(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;
    let rows = match db.list_all_users() {
        Ok(r) => r,
        Err(e) => {
            crate::pe!("[admin] list users failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    };
    let public: Vec<UserPublic> = rows.into_iter().map(UserPublic::from).collect();
    Json(public).into_response()
}

async fn handle_user_approve(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;
    match db.approve_user(&id, &auth.id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "user not pending"),
        Err(e) => {
            crate::pe!("[admin] approve failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

#[derive(Debug, Deserialize)]
struct RoleUpdate {
    role: String,
}

async fn handle_user_role(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RoleUpdate>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    if !matches!(body.role.as_str(), "user" | "admin" | "pending") {
        return err(StatusCode::BAD_REQUEST, "bad role");
    }
    if id == auth.id && body.role != "admin" {
        return err(StatusCode::BAD_REQUEST, "can't demote yourself");
    }
    let db = state.db.lock().await;
    match db.set_user_role(&id, &body.role) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "user not found"),
        Err(e) => {
            crate::pe!("[admin] role update failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

async fn handle_user_delete(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    if id == auth.id {
        return err(StatusCode::BAD_REQUEST, "can't delete yourself");
    }
    let db = state.db.lock().await;
    match db.delete_user(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "not found"),
        Err(e) => {
            crate::pe!("[admin] delete failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

#[derive(Debug, Serialize)]
struct StatsView {
    users_total: i64,
    users_pending: i64,
    media_total: i64,
    downloads_active: i64,
}

async fn handle_stats(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;
    let users_total = db.count_users().unwrap_or(0);
    let users_pending = db
        .list_users_by_role("pending")
        .map(|v| v.len() as i64)
        .unwrap_or(0);
    let media_total = db.list_media().map(|v| v.len() as i64).unwrap_or(0);
    let downloads_active = db
        .list_active_downloads()
        .map(|v| v.len() as i64)
        .unwrap_or(0);
    Json(StatsView {
        users_total,
        users_pending,
        media_total,
        downloads_active,
    })
    .into_response()
}

#[derive(Debug, Serialize)]
struct HealthCheck {
    name: String,
    ok: bool,
    detail: Option<String>,
    latency_ms: Option<u128>,
}

async fn handle_health(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    let mut out: Vec<HealthCheck> = Vec::new();

    let tmdb_key = state.tmdb_key().await;
    if !tmdb_key.is_empty() {
        let start = std::time::Instant::now();
        let url = format!(
            "https://api.themoviedb.org/3/configuration?api_key={}",
            tmdb_key
        );
        let r = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap()
            .get(&url)
            .send()
            .await;
        let latency = start.elapsed().as_millis();
        match r {
            Ok(resp) if resp.status().is_success() => out.push(HealthCheck {
                name: "tmdb".into(),
                ok: true,
                detail: None,
                latency_ms: Some(latency),
            }),
            Ok(resp) => out.push(HealthCheck {
                name: "tmdb".into(),
                ok: false,
                detail: Some(format!("http {}", resp.status().as_u16())),
                latency_ms: Some(latency),
            }),
            Err(e) => out.push(HealthCheck {
                name: "tmdb".into(),
                ok: false,
                detail: Some(short_err(&e.to_string())),
                latency_ms: None,
            }),
        }
    } else {
        out.push(HealthCheck {
            name: "tmdb".into(),
            ok: false,
            detail: Some("api key not set".into()),
            latency_ms: None,
        });
    }

    let jackett = state.jackett.lock().await.clone();
    match jackett {
        None => out.push(HealthCheck {
            name: "jackett".into(),
            ok: false,
            detail: Some("not configured".into()),
            latency_ms: None,
        }),
        Some(j) => {
            let start = std::time::Instant::now();
            let res = j.search("ping", &[], &[], None).await;
            out.push(HealthCheck {
                name: "jackett".into(),
                ok: !res.is_empty() || start.elapsed().as_millis() < 8000,
                detail: None,
                latency_ms: Some(start.elapsed().as_millis()),
            });
        }
    }

    let prowlarr = state.prowlarr.lock().await.clone();
    match prowlarr {
        None => out.push(HealthCheck {
            name: "prowlarr".into(),
            ok: false,
            detail: Some("not configured".into()),
            latency_ms: None,
        }),
        Some(p) => {
            let start = std::time::Instant::now();
            let res = p.ping().await;
            let latency = start.elapsed().as_millis();
            out.push(HealthCheck {
                name: "prowlarr".into(),
                ok: res.is_ok(),
                detail: res.err().map(|e| short_err(&e)),
                latency_ms: Some(latency),
            });
        }
    }

    let qbit = state.qbit.lock().await.clone();
    match qbit {
        None => out.push(HealthCheck {
            name: "qbittorrent".into(),
            ok: false,
            detail: Some("not configured".into()),
            latency_ms: None,
        }),
        Some(q) => {
            let start = std::time::Instant::now();
            let ok = q.ping().await.is_ok();
            out.push(HealthCheck {
                name: "qbittorrent".into(),
                ok,
                detail: if ok {
                    None
                } else {
                    Some("login or unreachable".into())
                },
                latency_ms: Some(start.elapsed().as_millis()),
            });
        }
    }

    Json(out).into_response()
}

fn short_err(s: &str) -> String {
    s.split(':').next().unwrap_or(s).trim().to_string()
}

#[derive(Debug, Serialize)]
struct StorageView {
    library_bytes: u64,
    library_files: u64,
    app_bytes: u64,
    app_files: u64,
    media_root: String,
    free_bytes: Option<u64>,
    total_bytes: Option<u64>,
    used_bytes: Option<u64>,
    directories: Vec<StorageBucket>,
    file_types: Vec<StorageBucket>,
    inventory: StorageInventory,
    items: Vec<StorageMediaItem>,
}

#[derive(Debug, Serialize)]
struct StorageBucket {
    key: String,
    label: String,
    bytes: u64,
    files: u64,
}

#[derive(Debug, Serialize)]
struct StorageInventory {
    total_items: i64,
    movies: i64,
    series: i64,
    anime: i64,
    ready_items: i64,
    without_files: i64,
    total_episodes: i64,
    ready_episodes: i64,
    subtitle_tracks: i64,
}

#[derive(Debug, Serialize)]
struct StorageMediaItem {
    id: String,
    tmdb_id: Option<i64>,
    media_type: String,
    title: String,
    year: Option<i32>,
    poster_url: Option<String>,
    status: String,
    added_at: String,
    is_anime: bool,
    bytes: u64,
    files: u64,
    video_bytes: u64,
    video_files: u64,
    audio_bytes: u64,
    audio_files: u64,
    subtitle_bytes: u64,
    subtitle_files: u64,
    episode_total: i64,
    episode_ready: i64,
    has_files: bool,
    relative_path: String,
}

#[derive(Debug, Default, Clone)]
struct StorageScan {
    bytes: u64,
    files: u64,
    video_bytes: u64,
    video_files: u64,
    audio_bytes: u64,
    audio_files: u64,
    subtitle_bytes: u64,
    subtitle_files: u64,
    artwork_bytes: u64,
    artwork_files: u64,
    other_bytes: u64,
    other_files: u64,
}

impl StorageScan {
    fn add(&mut self, other: &StorageScan) {
        self.bytes += other.bytes;
        self.files += other.files;
        self.video_bytes += other.video_bytes;
        self.video_files += other.video_files;
        self.audio_bytes += other.audio_bytes;
        self.audio_files += other.audio_files;
        self.subtitle_bytes += other.subtitle_bytes;
        self.subtitle_files += other.subtitle_files;
        self.artwork_bytes += other.artwork_bytes;
        self.artwork_files += other.artwork_files;
        self.other_bytes += other.other_bytes;
        self.other_files += other.other_files;
    }
}

async fn handle_storage(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    let root = std::path::PathBuf::from(&state.media_root);
    let directory_defs = [
        ("movies", "Movies"),
        ("series", "TV series"),
        ("anime", "Anime"),
        ("_dl", "Download cache"),
        ("clips", "Clips"),
        ("thumbnails", "Thumbnails"),
        ("books", "Books"),
        ("manga", "Manga"),
    ];

    let mut scans = std::collections::HashMap::new();
    let mut media_scans = std::collections::HashMap::new();
    let mut directories = Vec::with_capacity(directory_defs.len() + 1);
    let mut app_scan = StorageScan::default();
    for (key, label) in directory_defs {
        let scan = if ["movies", "series", "anime"].contains(&key) {
            let (total, children) = scan_storage_children(&root.join(key));
            for (id, child_scan) in children {
                media_scans.insert(format!("{key}/{id}"), child_scan);
            }
            total
        } else {
            scan_storage(&root.join(key))
        };
        app_scan.add(&scan);
        directories.push(storage_bucket(key, label, &scan));
        scans.insert(key, scan);
    }

    let known: std::collections::HashSet<&str> =
        directory_defs.iter().map(|(key, _)| *key).collect();
    let mut other_scan = StorageScan::default();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            if entry
                .file_type()
                .map(|kind| kind.is_symlink())
                .unwrap_or(true)
            {
                continue;
            }
            let name = entry.file_name();
            if name
                .to_str()
                .map(|value| known.contains(value))
                .unwrap_or(false)
            {
                continue;
            }
            other_scan.add(&scan_storage(&entry.path()));
        }
    }
    app_scan.add(&other_scan);
    directories.push(storage_bucket("other", "Other app data", &other_scan));

    let mut library_scan = StorageScan::default();
    for key in ["movies", "series", "anime"] {
        if let Some(scan) = scans.get(key) {
            library_scan.add(scan);
        }
    }

    let (media, episodes_by_media, subtitle_tracks) = {
        let db = state.db.lock().await;
        let media = db.list_media().unwrap_or_default();
        let episodes = media
            .iter()
            .map(|item| {
                let rows = db.list_episodes_for_media(&item.id).unwrap_or_default();
                (item.id.clone(), rows)
            })
            .collect::<std::collections::HashMap<_, _>>();
        let subtitle_tracks = db.subtitle_stats().map(|stats| stats.0).unwrap_or(0);
        (media, episodes, subtitle_tracks)
    };

    let mut movies = 0i64;
    let mut series = 0i64;
    let mut anime = 0i64;
    let mut ready_items = 0i64;
    let mut total_episodes = 0i64;
    let mut ready_episodes = 0i64;
    let mut items = Vec::with_capacity(media.len());

    for item in media {
        let bucket = media_bucket(&item);
        if item.is_anime {
            anime += 1;
        } else if item.media_type == "movie" {
            movies += 1;
        } else {
            series += 1;
        }

        let relative_path = format!("{bucket}/{}", item.id);
        let scan = media_scans.get(&relative_path).cloned().unwrap_or_default();
        let episodes = episodes_by_media
            .get(&item.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let item_episode_total = episodes.len() as i64;
        let item_episode_ready = episodes
            .iter()
            .filter(|episode| {
                episode.status == "ready"
                    && episode
                        .file_path
                        .as_ref()
                        .map(|path| std::path::Path::new(path).exists())
                        .unwrap_or(false)
            })
            .count() as i64;
        let movie_file_ready = item
            .file_path
            .as_ref()
            .map(|path| std::path::Path::new(path).exists())
            .unwrap_or(false);
        let has_files = movie_file_ready || item_episode_ready > 0 || scan.video_files > 0;

        total_episodes += item_episode_total;
        ready_episodes += item_episode_ready;
        if has_files {
            ready_items += 1;
        }

        items.push(StorageMediaItem {
            id: item.id,
            tmdb_id: item.tmdb_id,
            media_type: item.media_type,
            title: item.title,
            year: item.year,
            poster_url: item.poster_url,
            status: item.status,
            added_at: item.added_at,
            is_anime: item.is_anime,
            bytes: scan.bytes,
            files: scan.files,
            video_bytes: scan.video_bytes,
            video_files: scan.video_files,
            audio_bytes: scan.audio_bytes,
            audio_files: scan.audio_files,
            subtitle_bytes: scan.subtitle_bytes,
            subtitle_files: scan.subtitle_files,
            episode_total: item_episode_total,
            episode_ready: item_episode_ready,
            has_files,
            relative_path,
        });
    }
    items.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    let (total_bytes, free_bytes) = disk_usage(&root);
    let used_bytes = total_bytes
        .zip(free_bytes)
        .map(|(total, free)| total.saturating_sub(free));
    let total_items = items.len() as i64;

    Json(StorageView {
        library_bytes: library_scan.bytes,
        library_files: library_scan.files,
        app_bytes: app_scan.bytes,
        app_files: app_scan.files,
        media_root: state.media_root.clone(),
        free_bytes,
        total_bytes,
        used_bytes,
        directories,
        file_types: storage_file_types(&library_scan),
        inventory: StorageInventory {
            total_items,
            movies,
            series,
            anime,
            ready_items,
            without_files: total_items.saturating_sub(ready_items),
            total_episodes,
            ready_episodes,
            subtitle_tracks,
        },
        items,
    })
    .into_response()
}

fn media_bucket(media: &crate::models::Media) -> &'static str {
    if media.is_anime {
        "anime"
    } else if media.media_type == "movie" {
        "movies"
    } else {
        "series"
    }
}

fn storage_bucket(key: &str, label: &str, scan: &StorageScan) -> StorageBucket {
    StorageBucket {
        key: key.to_string(),
        label: label.to_string(),
        bytes: scan.bytes,
        files: scan.files,
    }
}

fn storage_file_types(scan: &StorageScan) -> Vec<StorageBucket> {
    vec![
        StorageBucket {
            key: "video".into(),
            label: "Video".into(),
            bytes: scan.video_bytes,
            files: scan.video_files,
        },
        StorageBucket {
            key: "audio".into(),
            label: "Audio cache".into(),
            bytes: scan.audio_bytes,
            files: scan.audio_files,
        },
        StorageBucket {
            key: "subtitles".into(),
            label: "Subtitles".into(),
            bytes: scan.subtitle_bytes,
            files: scan.subtitle_files,
        },
        StorageBucket {
            key: "artwork".into(),
            label: "Artwork".into(),
            bytes: scan.artwork_bytes,
            files: scan.artwork_files,
        },
        StorageBucket {
            key: "other".into(),
            label: "Other".into(),
            bytes: scan.other_bytes,
            files: scan.other_files,
        },
    ]
}

fn scan_storage(path: &std::path::Path) -> StorageScan {
    let mut scan = StorageScan::default();
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        if current.is_file() {
            if let Ok(metadata) = current.metadata() {
                classify_storage_file(&current, metadata.len(), &mut scan);
            }
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(child);
            } else if kind.is_file() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                classify_storage_file(&child, metadata.len(), &mut scan);
            }
        }
    }
    scan
}

fn scan_storage_children(
    path: &std::path::Path,
) -> (StorageScan, std::collections::HashMap<String, StorageScan>) {
    let mut total = StorageScan::default();
    let mut children = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return (total, children);
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            let scan = scan_storage(&entry.path());
            total.add(&scan);
            if let Some(name) = entry.file_name().to_str() {
                children.insert(name.to_string(), scan);
            }
        } else if kind.is_file() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            classify_storage_file(&entry.path(), metadata.len(), &mut total);
        }
    }
    (total, children)
}

fn classify_storage_file(path: &std::path::Path, bytes: u64, scan: &mut StorageScan) {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    scan.bytes += bytes;
    scan.files += 1;
    if ["mp4", "mkv", "webm", "m4v", "mov", "avi"].contains(&extension.as_str()) {
        if stem.contains("_audio") {
            scan.audio_bytes += bytes;
            scan.audio_files += 1;
        } else {
            scan.video_bytes += bytes;
            scan.video_files += 1;
        }
    } else if ["vtt", "srt", "ass", "ssa", "sub"].contains(&extension.as_str()) {
        scan.subtitle_bytes += bytes;
        scan.subtitle_files += 1;
    } else if ["jpg", "jpeg", "png", "webp", "avif"].contains(&extension.as_str()) {
        scan.artwork_bytes += bytes;
        scan.artwork_files += 1;
    } else {
        scan.other_bytes += bytes;
        scan.other_files += 1;
    }
}

fn walk_size(path: &std::path::Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&p) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Ok(m) = e.metadata() {
                bytes += m.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

#[derive(Debug, Serialize)]
struct SystemMetrics {
    cpu_percent: f64,
    memory_total_bytes: u64,
    memory_used_bytes: u64,
    memory_percent: f64,
    uptime_seconds: u64,
    load_avg: [f64; 3],
    disk_total_bytes: u64,
    disk_used_bytes: u64,
    disk_free_bytes: u64,
}

async fn handle_system(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    Json(read_system_metrics(&state.media_root)).into_response()
}

#[cfg(unix)]
fn read_system_metrics(media_root: &str) -> SystemMetrics {
    let mem = read_meminfo();
    let load = read_loadavg();
    let uptime = read_uptime();
    let cpu = read_cpu_percent();
    let (disk_total, disk_free) = disk_usage(std::path::Path::new(media_root));
    let disk_total = disk_total.unwrap_or(0);
    let disk_free = disk_free.unwrap_or(0);
    let disk_used = disk_total.saturating_sub(disk_free);

    let memory_used_bytes = mem.0.saturating_sub(mem.1);
    let memory_percent = if mem.0 > 0 {
        memory_used_bytes as f64 / mem.0 as f64 * 100.0
    } else {
        0.0
    };

    SystemMetrics {
        cpu_percent: cpu,
        memory_total_bytes: mem.0,
        memory_used_bytes,
        memory_percent,
        uptime_seconds: uptime,
        load_avg: load,
        disk_total_bytes: disk_total,
        disk_used_bytes: disk_used,
        disk_free_bytes: disk_free,
    }
}

#[cfg(not(unix))]
fn read_system_metrics(_media_root: &str) -> SystemMetrics {
    SystemMetrics {
        cpu_percent: 0.0,
        memory_total_bytes: 0,
        memory_used_bytes: 0,
        memory_percent: 0.0,
        uptime_seconds: 0,
        load_avg: [0.0; 3],
        disk_total_bytes: 0,
        disk_used_bytes: 0,
        disk_free_bytes: 0,
    }
}

#[cfg(unix)]
fn read_meminfo() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in s.lines() {
        let mut it = line.split_whitespace();
        let key = it.next().unwrap_or("");
        let val: u64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        match key {
            "MemTotal:" => total = val * 1024,
            "MemAvailable:" => avail = val * 1024,
            _ => {}
        }
    }
    (total, avail)
}

#[cfg(unix)]
fn read_loadavg() -> [f64; 3] {
    let s = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let mut it = s.split_whitespace();
    [
        it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
    ]
}

#[cfg(unix)]
fn read_uptime() -> u64 {
    let s = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    s.split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0)
}

#[cfg(unix)]
fn read_cpu_percent() -> f64 {
    fn read_total_idle() -> Option<(u64, u64)> {
        let s = std::fs::read_to_string("/proc/stat").ok()?;
        let line = s.lines().next()?;
        let parts: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|x| x.parse().ok())
            .collect();
        if parts.len() < 4 {
            return None;
        }
        let total: u64 = parts.iter().sum();
        let idle = parts[3] + parts.get(4).copied().unwrap_or(0);
        Some((total, idle))
    }
    let Some((t1, i1)) = read_total_idle() else {
        return 0.0;
    };
    std::thread::sleep(std::time::Duration::from_millis(100));
    let Some((t2, i2)) = read_total_idle() else {
        return 0.0;
    };
    let dt = t2.saturating_sub(t1);
    let di = i2.saturating_sub(i1);
    if dt == 0 {
        0.0
    } else {
        (1.0 - di as f64 / dt as f64) * 100.0
    }
}

#[cfg(unix)]
fn disk_usage(path: &std::path::Path) -> (Option<u64>, Option<u64>) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let cpath = match CString::new(path.as_os_str().as_bytes()) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(cpath.as_ptr(), &mut stat) != 0 {
            return (None, None);
        }
        let total = stat.f_blocks as u64 * stat.f_frsize as u64;
        let free = stat.f_bavail as u64 * stat.f_frsize as u64;
        (Some(total), Some(free))
    }
}

#[cfg(not(unix))]
fn disk_usage(_path: &std::path::Path) -> (Option<u64>, Option<u64>) {
    (None, None)
}

#[derive(Debug, Serialize)]
struct AppMetrics {
    media: MediaCounts,
    episodes: EpisodeCounts,
    downloads: DownloadCounts,
    subtitles: SubtitleStats,
    watch: WatchSummary,
    users: UserCounts,
}

#[derive(Debug, Serialize)]
struct MediaCounts {
    total: i64,
    ready: i64,
    error: i64,
}
#[derive(Debug, Serialize)]
struct EpisodeCounts {
    total: i64,
    ready: i64,
}
#[derive(Debug, Serialize)]
struct DownloadCounts {
    active: i64,
    errored: i64,
}
#[derive(Debug, Serialize)]
struct SubtitleStats {
    total: i64,
    by_language: Vec<LangCount>,
    by_source: Vec<SourceCount>,
}
#[derive(Debug, Serialize)]
struct LangCount {
    language: String,
    count: i64,
}
#[derive(Debug, Serialize)]
struct SourceCount {
    source: String,
    count: i64,
}
#[derive(Debug, Serialize)]
struct WatchSummary {
    total_records: i64,
    completed_records: i64,
    active_last_24h: i64,
}
#[derive(Debug, Serialize)]
struct UserCounts {
    total: i64,
    admin: i64,
    pending: i64,
}

async fn handle_metrics(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;

    let (m_total, m_ready, m_error) = db.count_media_by_status().unwrap_or((0, 0, 0));
    let (ep_total, ep_ready) = db.count_episodes().unwrap_or((0, 0));
    let (dl_active, dl_errored) = db.count_downloads().unwrap_or((0, 0));
    let (sub_total, sub_langs, sub_srcs) = db.subtitle_stats().unwrap_or((0, vec![], vec![]));
    let (wt_total, wt_completed, wt_active) = db.watch_summary().unwrap_or((0, 0, 0));
    let users_total = db.count_users().unwrap_or(0);
    let users_admin = db
        .list_users_by_role("admin")
        .map(|v| v.len() as i64)
        .unwrap_or(0);
    let users_pending = db
        .list_users_by_role("pending")
        .map(|v| v.len() as i64)
        .unwrap_or(0);

    Json(AppMetrics {
        media: MediaCounts {
            total: m_total,
            ready: m_ready,
            error: m_error,
        },
        episodes: EpisodeCounts {
            total: ep_total,
            ready: ep_ready,
        },
        downloads: DownloadCounts {
            active: dl_active,
            errored: dl_errored,
        },
        subtitles: SubtitleStats {
            total: sub_total,
            by_language: sub_langs
                .into_iter()
                .map(|(language, count)| LangCount { language, count })
                .collect(),
            by_source: sub_srcs
                .into_iter()
                .map(|(source, count)| SourceCount { source, count })
                .collect(),
        },
        watch: WatchSummary {
            total_records: wt_total,
            completed_records: wt_completed,
            active_last_24h: wt_active,
        },
        users: UserCounts {
            total: users_total,
            admin: users_admin,
            pending: users_pending,
        },
    })
    .into_response()
}

#[derive(Debug, Serialize)]
struct Insight {
    severity: String,
    title: String,
    detail: String,
}

async fn handle_insights(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let mut out: Vec<Insight> = Vec::new();
    let db = state.db.lock().await;

    let pending = db
        .list_users_by_role("pending")
        .map(|v| v.len())
        .unwrap_or(0);
    if pending > 0 {
        out.push(Insight {
            severity: "info".into(),
            title: format!(
                "{pending} user{} waiting for approval",
                if pending == 1 { "" } else { "s" }
            ),
            detail: "head over to the pending tab to approve.".into(),
        });
    }

    if state.tmdb_key().await.is_empty() {
        out.push(Insight {
            severity: "warning".into(),
            title: "tmdb api key missing".into(),
            detail: "search and metadata are disabled until set.".into(),
        });
    }

    if state.jackett.lock().await.is_none() {
        out.push(Insight {
            severity: "warning".into(),
            title: "jackett not configured".into(),
            detail: "set the api key in settings to enable torrent search.".into(),
        });
    }
    if state.qbit.lock().await.is_none() {
        out.push(Insight {
            severity: "warning".into(),
            title: "qbittorrent not configured".into(),
            detail: "downloads will fail until qbit is wired up in settings.".into(),
        });
    }

    Json(serde_json::json!({ "insights": out })).into_response()
}

async fn handle_processing(Extension(auth): Extension<AuthUser>) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let jobs = crate::ffmpeg::list_processing_jobs();
    Json(jobs).into_response()
}

#[derive(Debug, Serialize)]
struct WatchStats {
    total_watch_seconds: i64,
    total_completed_episodes: i64,
    leaderboard: Vec<LeaderboardRow>,
}
#[derive(Debug, Serialize)]
struct LeaderboardRow {
    user_id: String,
    username: String,
    watch_seconds: i64,
    completed_episodes: i64,
}

async fn handle_watch_stats(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let (total_secs, total_done, rows) = {
        let db = state.db.lock().await;
        match db.watch_stats_aggregate() {
            Ok(r) => r,
            Err(e) => {
                crate::pe!("[admin] watch_stats failed: {e}");
                return err(StatusCode::INTERNAL_SERVER_ERROR, "stats failed");
            }
        }
    };
    let leaderboard: Vec<LeaderboardRow> = rows
        .into_iter()
        .map(|(user_id, username, secs, done)| LeaderboardRow {
            user_id,
            username,
            watch_seconds: secs,
            completed_episodes: done,
        })
        .collect();
    Json(WatchStats {
        total_watch_seconds: total_secs,
        total_completed_episodes: total_done,
        leaderboard,
    })
    .into_response()
}

#[derive(Debug, Serialize)]
struct DiskUsageView {
    total_disk: u64,
    used_disk: u64,
    free_disk: u64,
    movies_size: u64,
    series_size: u64,
    anime_size: u64,
    media_processed: u64,
    video_size: u64,
    audio_remux_size: u64,
    embedded_subs_size: u64,
    downloads_cache: u64,
    shared_subs: u64,
    clips: u64,
    thumbnails: u64,
    total_media: i64,
}

async fn handle_disk_usage(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let root = std::path::PathBuf::from(&state.media_root);
    let movies = scan_storage(&root.join("movies"));
    let series = scan_storage(&root.join("series"));
    let anime = scan_storage(&root.join("anime"));
    let mut library = StorageScan::default();
    library.add(&movies);
    library.add(&series);
    library.add(&anime);
    let (dl_bytes, _) = walk_size(&root.join("_dl"));
    let (clips_bytes, _) = walk_size(&root.join("clips"));
    let (thumbs_bytes, _) = walk_size(&root.join("thumbnails"));
    let (total, free) = disk_usage(&root);
    let total = total.unwrap_or(0);
    let free = free.unwrap_or(0);

    let total_media = {
        let db = state.db.lock().await;
        db.list_media().map(|v| v.len() as i64).unwrap_or(0)
    };

    Json(DiskUsageView {
        total_disk: total,
        used_disk: total.saturating_sub(free),
        free_disk: free,
        movies_size: movies.bytes,
        series_size: series.bytes,
        anime_size: anime.bytes,
        media_processed: library.bytes,
        video_size: library.video_bytes,
        audio_remux_size: library.audio_bytes,
        embedded_subs_size: 0,
        downloads_cache: dl_bytes,
        shared_subs: library.subtitle_bytes,
        clips: clips_bytes,
        thumbnails: thumbs_bytes,
        total_media,
    })
    .into_response()
}

async fn handle_cleanup(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;
    let removed = db.purge_dead_downloads().unwrap_or(0);
    Json(serde_json::json!({ "removed": removed })).into_response()
}

#[derive(Debug, Serialize)]
struct CleanResult {
    cleaned_bytes: u64,
    cleaned_downloads: i64,
}

async fn handle_clean_downloads(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let dl_root = std::path::PathBuf::from(&state.media_root).join("_dl");
    let (bytes, _) = walk_size(&dl_root);
    let mut count = 0i64;
    if dl_root.exists() {
        if let Ok(entries) = std::fs::read_dir(&dl_root) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && std::fs::remove_dir_all(&p).is_ok() {
                    count += 1;
                }
            }
        }
    }
    Json(CleanResult {
        cleaned_bytes: bytes,
        cleaned_downloads: count,
    })
    .into_response()
}

async fn handle_detect_intros(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(media_id): Path<String>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }
    let s = state.clone();
    let mid = media_id.clone();
    tokio::spawn(async move {
        let saved = crate::intro::detect_for_media(&s, &mid).await;
        crate::pi!("[admin] intro detection for {mid}: saved {saved} markers");
    });
    Json(serde_json::json!({ "ok": true })).into_response()
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
