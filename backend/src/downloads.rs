use crate::{
    middleware::AuthUser,
    models::{ApiError, Download, DownloadRequest, DownloadStatus, TorrentOption},
    AppState,
};
use axum::{
    extract::{Path as AxPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const QBIT_CATEGORY: &str = "pleasewatch";
const POLL_INTERVAL_SECS: u64 = 5;
const HASH_RESOLVE_BACKOFF_MS: &[u64] = &[200, 400, 700, 1100, 1700, 2400];

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/torrents/search", get(handle_search))
        .route("/api/torrents/indexers", get(handle_indexers))
        .route("/api/downloads", get(handle_list).post(handle_create))
        .route("/api/downloads/{id}", delete(handle_cancel))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum SearchSource {
    Jackett,
    Prowlarr,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    kind: Option<String>,
    imdb: Option<String>,
    source: Option<SearchSource>,
}

async fn handle_search(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let mut jackett = state.jackett.lock().await.clone();
    let mut prowlarr = state.prowlarr.lock().await.clone();
    if jackett.is_none() && prowlarr.is_none() {
        return err(
            StatusCode::SERVICE_UNAVAILABLE,
            "no torrent indexer configured",
        );
    }

    match q.source {
        Some(SearchSource::Jackett) => prowlarr = None,
        Some(SearchSource::Prowlarr) => jackett = None,
        None => {}
    }

    if jackett.is_none() && prowlarr.is_none() {
        return Json::<Vec<TorrentOption>>(Vec::new()).into_response();
    }

    if q.q.trim().is_empty() {
        return Json::<Vec<TorrentOption>>(Vec::new()).into_response();
    }

    let cats: &[i32] = match q.kind.as_deref() {
        Some("movie") => crate::jackett::MOVIE_CATS,
        Some("anime") => crate::jackett::ANIME_PLUS_TV_CATS,
        Some("tv") => crate::jackett::TV_CATS,
        Some("book") => &[],
        _ => &[],
    };

    let indexers = if jackett.is_some() {
        resolve_indexers(&state, q.kind.as_deref()).await
    } else {
        Vec::new()
    };

    let is_book_kind = matches!(q.kind.as_deref(), Some("book"));
    let search_q = if is_book_kind {
        let lower = q.q.to_lowercase();
        if ["epub", "ebook", "mobi", "azw3", "pdf", "kindle"]
            .iter()
            .any(|m| lower.contains(m))
        {
            q.q.clone()
        } else {
            format!("{} epub", q.q)
        }
    } else {
        q.q.clone()
    };

    let title_search = dual_search(&jackett, &prowlarr, &search_q, &indexers, cats, None);
    let imdb_search = async {
        if let Some(imdb) = q.imdb.as_deref() {
            dual_search(&jackett, &prowlarr, &search_q, &indexers, cats, Some(imdb)).await
        } else {
            Vec::new()
        }
    };
    let (mut title_items, imdb_items) = tokio::join!(title_search, imdb_search);

    if is_book_kind && title_items.is_empty() && imdb_items.is_empty() {
        let tokens: Vec<&str> = q.q.split_whitespace().collect();
        if tokens.len() > 2 {
            let author = tokens[tokens.len() - 2..].join(" ");
            let retry_q = format!("{} epub", author);
            crate::pi!("[downloads] book retry with author-only: {retry_q}");
            title_items = dual_search(&jackett, &prowlarr, &retry_q, &indexers, cats, None).await;
        }
    }

    let parsed_q = crate::jackett::parse_title(&q.q);
    let expected_show = parsed_q.show.clone();
    let want_season = parsed_q.season;
    let want_episode = parsed_q.episode;
    let skip_episode_match = matches!(q.kind.as_deref(), Some("movie") | Some("book"));

    let marker_ok = |p: &crate::jackett::ParsedTitle| -> bool {
        match (want_season, want_episode) {
            (Some(s), Some(e)) => {
                (p.season == Some(s) && p.episode == Some(e)) || (p.season == Some(s) && p.is_pack)
            }
            (Some(s), None) => p.season == Some(s) && p.is_pack,
            _ => true,
        }
    };

    let mut seen = std::collections::HashSet::new();
    let mut items: Vec<TorrentOption> = Vec::with_capacity(title_items.len() + imdb_items.len());
    let mut fallback: Vec<TorrentOption> = Vec::new();

    for t in imdb_items {
        if !seen.insert(t.magnet.clone()) {
            continue;
        }
        if !skip_episode_match {
            let p = crate::jackett::parse_title(&t.title);
            if !marker_ok(&p) {
                continue;
            }
        }
        items.push(t);
    }

    for t in title_items {
        if !seen.insert(t.magnet.clone()) {
            continue;
        }
        if !skip_episode_match {
            let p = crate::jackett::parse_title(&t.title);
            if !marker_ok(&p) {
                continue;
            }
            if !expected_show.is_empty() && !crate::jackett::show_matches(&p.show, &expected_show) {
                fallback.push(t);
                continue;
            }
        }
        items.push(t);
    }

    if items.len() < 5 {
        items.extend(fallback);
    }

    let is_book = matches!(q.kind.as_deref(), Some("book"));
    if is_book {
        items.retain(|t| {
            let lower = t.title.to_lowercase();
            let audio = [
                "audiobook",
                "audio book",
                "graphic audio",
                "[audio]",
                "(audio)",
                ".m4b",
                ".mp3",
                ".m4a",
                "narrated by",
                "narrated",
                "unabridged",
            ];
            if audio.iter().any(|m| lower.contains(m)) {
                return false;
            }
            let book_marker = [
                "epub", "ebook", "e-book", "e book", ".mobi", "mobi8", ".azw3", "azw3", ".pdf",
                "pdf]", "pdf)", " pdf ", "kindle", "calibre", "(book", "[book",
            ];
            if !book_marker.iter().any(|m| lower.contains(m)) {
                return false;
            }
            if t.size > 200 * 1024 * 1024 {
                return false;
            }
            true
        });
    }

    struct Score {
        exact: bool,
        pack: bool,
        seeds: i32,
    }
    let mut scored: Vec<(TorrentOption, Score)> = items
        .into_iter()
        .map(|t| {
            let p = crate::jackett::parse_title(&t.title);
            let exact = match want_episode {
                Some(e) => p.episode == Some(e) || crate::jackett::episode_match(&t.title, e),
                None => false,
            };
            let pack = p.is_pack || crate::jackett::pack_match(&t.title);
            let seeds = t.seeds;
            (t, Score { exact, pack, seeds })
        })
        .collect();

    if want_episode.is_some() {
        scored.sort_by(|(_, a), (_, b)| {
            match (a.exact, b.exact) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            match (a.pack, b.pack) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => b.seeds.cmp(&a.seeds),
            }
        });
    } else if want_season.is_some() {
        scored.sort_by(|(_, a), (_, b)| match (a.pack, b.pack) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.seeds.cmp(&a.seeds),
        });
    } else {
        scored.sort_by(|(_, a), (_, b)| b.seeds.cmp(&a.seeds));
    }

    let mut items: Vec<TorrentOption> = scored.into_iter().map(|(t, _)| t).collect();
    items.truncate(120);

    if is_book {
        return Json(items).into_response();
    }

    let mut pref_map = std::collections::HashMap::<(String, String), f64>::new();
    {
        let db = state.db.lock().await;
        let admin_ids = db.list_admin_ids().unwrap_or_default();
        let is_admin = admin_ids.iter().any(|a| a == &auth.id);
        let mut who = vec![auth.id.clone()];
        if !is_admin {
            for a in &admin_ids {
                if a != &auth.id {
                    who.push(a.clone());
                }
            }
        }
        match db.get_torrent_prefs(&who) {
            Ok(rows) => {
                for (uid, kind, value, weighted) in rows {
                    let w = if uid == auth.id { 1.5 } else { 1.0 };
                    *pref_map.entry((kind, value)).or_insert(0.0) += weighted * w;
                }
                crate::pi!(
                    "[downloads] loaded {} pref entries for user={}",
                    pref_map.len(),
                    auth.id
                );
            }
            Err(e) => eprintln!("[downloads] pref load: {e}"),
        }
    }

    for it in items.iter_mut() {
        let attrs = crate::jackett::torrent_attrs(it);
        let mut score = 0.0;
        for (k, v) in attrs {
            if let Some(w) = pref_map.get(&(k, v)) {
                score += w;
            }
        }
        it.pref_score = if it.seeds >= 2 { score } else { 0.0 };
    }

    items.sort_by(|a, b| {
        let am = a.pref_score > 0.0;
        let bm = b.pref_score > 0.0;
        match (am, bm) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.seeds.cmp(&a.seeds),
        }
    });

    Json(items).into_response()
}

async fn dual_search(
    jackett: &Option<crate::jackett::Jackett>,
    prowlarr: &Option<crate::prowlarr::Prowlarr>,
    query: &str,
    indexers: &[String],
    cats: &[i32],
    imdb_id: Option<&str>,
) -> Vec<TorrentOption> {
    let (jr, pr) = tokio::join!(
        async {
            match jackett {
                Some(j) => j.search(query, indexers, cats, imdb_id).await,
                None => Vec::new(),
            }
        },
        async {
            match prowlarr {
                Some(p) => p.search(query, cats, imdb_id).await,
                None => Vec::new(),
            }
        },
    );
    let mut out = jr;
    out.extend(pr);
    out
}

pub async fn resolve_indexers(state: &Arc<AppState>, kind: Option<&str>) -> Vec<String> {
    let key = match kind {
        Some("movie") => "jackett_indexers_movie",
        Some("anime") => "jackett_indexers_anime",
        Some("tv") => "jackett_indexers_tv",
        Some("book") => "jackett_indexers_book",
        _ => "jackett_indexers",
    };

    let from_db = {
        let db = state.db.lock().await;
        db.get_setting(key).ok().flatten()
    };

    let raw = from_db
        .or_else(|| std::env::var(key.to_uppercase()).ok())
        .or_else(|| std::env::var("JACKETT_INDEXERS").ok())
        .unwrap_or_default();

    let configured: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if !configured.is_empty() {
        return configured;
    }

    let jackett = state.jackett.lock().await.clone();
    if let Some(j) = jackett {
        if let Ok(found) = j.list_indexers().await {
            if !found.is_empty() {
                crate::pi!(
                    "[downloads] auto-discovered {} indexers from jackett",
                    found.len()
                );
                return found;
            }
        }
    }

    Vec::new()
}

async fn handle_indexers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let jackett = state.jackett.lock().await.clone();
    let Some(jackett) = jackett else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "jackett not configured");
    };
    match jackett.list_indexers().await {
        Ok(ids) => Json::<Vec<String>>(ids).into_response(),
        Err(_) => Json::<Vec<String>>(Vec::new()).into_response(),
    }
}

async fn handle_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    let rows = match db.list_downloads() {
        Ok(r) => r,
        Err(e) => {
            crate::pe!("[downloads] list failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    };
    drop(db);

    let qbit = state.qbit.lock().await.clone();

    let mut out: Vec<DownloadStatus> = Vec::with_capacity(rows.len());
    for d in rows {
        let (progress, qstate) = match (&qbit, d.qbit_hash.as_deref()) {
            (Some(q), Some(h)) => match q.get(h).await {
                Ok(Some(t)) => (t.progress, Some(t.state)),
                _ => (if d.status == "complete" { 1.0 } else { 0.0 }, None),
            },
            _ => (if d.status == "complete" { 1.0 } else { 0.0 }, None),
        };
        out.push(DownloadStatus {
            download: d,
            progress,
            state: qstate,
        });
    }

    Json(out).into_response()
}

async fn handle_create(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<DownloadRequest>,
) -> impl IntoResponse {
    let qbit = state.qbit.lock().await.clone();
    let Some(qbit) = qbit else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "qbit not configured");
    };
    if !body.magnet.starts_with("magnet:") && !body.magnet.starts_with("http") {
        return err(StatusCode::BAD_REQUEST, "invalid magnet or torrent url");
    }

    let media_id = match resolve_media_id(&state, &body, &auth).await {
        Ok(id) => id,
        Err(msg) => return err(StatusCode::BAD_REQUEST, &msg),
    };

    let episode_id = match (body.episode_id.clone(), body.season, body.episode) {
        (Some(eid), _, _) => Some(eid),
        (None, Some(s), Some(e)) => {
            let looks_like_pack = body
                .title
                .as_deref()
                .map(|t| {
                    let p = crate::jackett::parse_title(t);
                    p.is_pack && p.episode.is_none()
                })
                .unwrap_or(false);
            if looks_like_pack {
                None
            } else {
                let db = state.db.lock().await;
                match db.find_or_create_episode(&media_id, s, e) {
                    Ok(ep) => Some(ep.id),
                    Err(err) => {
                        crate::pe!("[downloads] episode upsert failed: {err}");
                        None
                    }
                }
            }
        }
        _ => None,
    };

    let id = uuid::Uuid::new_v4().to_string();
    let save_path = format!("{}/_dl/{}", state.media_root, id);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let row = Download {
        id: id.clone(),
        media_id: media_id.clone(),
        episode_id,
        magnet: body.magnet.clone(),
        qbit_hash: extract_hash(&body.magnet),
        status: "queued".into(),
        save_path: save_path.clone(),
        title: body.title.clone(),
        requested_by: Some(auth.id.clone()),
        created_at: now,
        completed_at: None,
    };

    {
        let db = state.db.lock().await;
        if let Err(e) = db.create_download(&row) {
            crate::pe!("[downloads] db insert failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    }

    if let Err(e) = qbit
        .add_magnet(&body.magnet, QBIT_CATEGORY, &save_path)
        .await
    {
        crate::pe!("[downloads] qbit add failed: {e}");
        let db = state.db.lock().await;
        let _ = db.delete_download(&id);
        return err(StatusCode::BAD_GATEWAY, &format!("qbit: {e}"));
    }

    {
        let db = state.db.lock().await;
        let _ = db.set_download_status(&id, "downloading");
        if let Some(t) = &body.torrent {
            let attrs = crate::jackett::torrent_attrs(t);
            if !attrs.is_empty() {
                let _ = db.bump_torrent_prefs(&auth.id, &attrs);
            }
        }
    }

    if row.qbit_hash.is_none() {
        let qbit_bg = qbit.clone();
        let state_bg = state.clone();
        let id_bg = id.clone();
        let save_path_bg = save_path.clone();
        tokio::spawn(async move {
            for delay in HASH_RESOLVE_BACKOFF_MS {
                tokio::time::sleep(std::time::Duration::from_millis(*delay)).await;
                if let Some(h) = lookup_hash_by_path(&qbit_bg, &save_path_bg).await {
                    let db = state_bg.db.lock().await;
                    let _ = db.set_download_hash(&id_bg, &h);
                    crate::pi!("[downloads] resolved hash for {id_bg} (+{delay}ms)");
                    return;
                }
            }
            crate::pe!("[downloads] could not resolve hash for {id_bg} within backoff window");
        });
    }

    crate::pi!(
        "[downloads] queued '{}' for media {}",
        row.title.as_deref().unwrap_or("?"),
        media_id
    );

    (StatusCode::CREATED, Json(row)).into_response()
}

async fn resolve_media_id(
    state: &Arc<AppState>,
    body: &DownloadRequest,
    auth: &AuthUser,
) -> Result<String, String> {
    if let Some(id) = body.media_id.as_deref() {
        let db = state.db.lock().await;
        if db.find_media_by_id(id).ok().flatten().is_some() {
            return Ok(id.to_string());
        }
        return Err("media not found".into());
    }

    let tmdb_id = body
        .tmdb_id
        .ok_or_else(|| "tmdb_id or media_id required".to_string())?;
    let kind = body
        .media_type
        .as_deref()
        .ok_or_else(|| "media_type required".to_string())?;
    if kind != "movie" && kind != "tv" {
        return Err("media_type must be 'movie' or 'tv'".into());
    }

    {
        let db = state.db.lock().await;
        if let Ok(Some(existing)) = db.find_media_by_tmdb(tmdb_id, kind) {
            return Ok(existing.id);
        }
    }

    let key = state.tmdb_key().await;
    if key.is_empty() {
        return Err("tmdb not configured, cannot auto-create media".into());
    }
    let detail = match kind {
        "movie" => crate::tmdb::movie(&key, tmdb_id, "en-US").await,
        _ => crate::tmdb::tv(&key, tmdb_id, "en-US").await,
    };
    let detail = match detail {
        Ok(Some(d)) => d,
        Ok(None) => return Err("not found on tmdb".into()),
        Err(e) => {
            crate::pe!("[downloads] tmdb fetch: {e}");
            return Err("tmdb fetch failed".into());
        }
    };

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let is_anime = kind == "tv"
        && detail
            .genres
            .iter()
            .any(|g| g.eq_ignore_ascii_case("animation"));
    let media = crate::models::Media {
        id: new_id.clone(),
        tmdb_id: Some(detail.tmdb_id),
        media_type: kind.to_string(),
        title: detail.title.clone(),
        year: detail.year.as_ref().and_then(|y| y.parse::<i32>().ok()),
        overview: detail.overview.clone(),
        poster_url: detail.poster_url.clone(),
        backdrop_url: detail.backdrop_url.clone(),
        file_path: None,
        duration: None,
        status: "pending".into(),
        added_by: Some(auth.id.clone()),
        added_at: now,
        activity_at: None,
        activity_label: None,
        is_anime,
        source_name: None,
    };

    {
        let db = state.db.lock().await;
        if let Err(e) = db.create_media(&media) {
            crate::pe!("[downloads] create_media failed: {e}");
            return Err("could not create media row".into());
        }
    }

    crate::pi!("[downloads] auto-created media for tmdb {tmdb_id}");
    Ok(new_id)
}

async fn handle_cancel(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let row = {
        let db = state.db.lock().await;
        match db.find_download(&id) {
            Ok(Some(d)) => d,
            Ok(None) => return err(StatusCode::NOT_FOUND, "not found"),
            Err(e) => {
                crate::pe!("[downloads] cancel lookup failed: {e}");
                return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
            }
        }
    };

    let is_owner = row.requested_by.as_deref() == Some(auth.id.as_str());
    if !auth.is_admin() && !is_owner {
        return err(StatusCode::FORBIDDEN, "not yours");
    }

    if let Some(hash) = &row.qbit_hash {
        if let Some(qbit) = state.qbit.lock().await.clone() {
            let _ = qbit.delete(hash, true).await;
        }
    }

    {
        let db = state.db.lock().await;
        let _ = db.set_download_status(&id, "cancelled");
    }

    StatusCode::NO_CONTENT.into_response()
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}

pub fn extract_hash(magnet: &str) -> Option<String> {
    let xt = magnet.split('&').find(|p| p.starts_with("xt=urn:btih:"))?;
    let raw = xt.trim_start_matches("xt=urn:btih:");
    let h = raw.split('&').next()?.to_lowercase();
    if h.len() == 40 || h.len() == 32 {
        Some(h)
    } else {
        None
    }
}

pub fn spawn_poller(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(e) = poll_once(&state).await {
                crate::pe!("[downloads] poll error: {e}");
            }
        }
    });
}

async fn poll_once(state: &Arc<AppState>) -> Result<(), String> {
    let qbit = match state.qbit.lock().await.clone() {
        Some(q) => q,
        None => return Ok(()),
    };

    let active = {
        let db = state.db.lock().await;
        db.list_active_downloads().map_err(|e| e.to_string())?
    };

    for mut row in active {
        if row.qbit_hash.is_none() {
            if let Some(h) = lookup_hash_by_path(&qbit, &row.save_path).await {
                let db = state.db.lock().await;
                let _ = db.set_download_hash(&row.id, &h);
                row.qbit_hash = Some(h);
            } else {
                continue;
            }
        }

        let hash = row.qbit_hash.as_ref().unwrap();
        let info = match qbit.get(hash).await {
            Ok(Some(t)) => t,
            Ok(None) => continue,
            Err(e) => {
                crate::pe!("[downloads] qbit get failed: {e}");
                continue;
            }
        };

        let pack_complete = crate::qbit::Qbit::is_complete(&info);

        let media = {
            let db = state.db.lock().await;
            match db.find_media_by_id(&row.media_id).ok().flatten() {
                Some(m) => m,
                None => {
                    crate::pe!("[downloads] media {} vanished", row.media_id);
                    continue;
                }
            }
        };

        let type_dir = if media.media_type == "movie" {
            "movies"
        } else if media.is_anime {
            "anime"
        } else {
            "series"
        };
        let show_dir = format!("{}/{}/{}", state.media_root, type_dir, &row.media_id);
        let _ = std::fs::create_dir_all(&show_dir);

        let is_series = media.media_type == "tv";
        let is_pack = is_series && row.episode_id.is_none();

        if !is_pack && !pack_complete {
            continue;
        }

        let mut landed_any = false;

        let pack_file_progress: std::collections::HashMap<String, f64> = if is_pack {
            qbit.files(hash)
                .await
                .ok()
                .map(|fs| {
                    fs.into_iter()
                        .map(|f| {
                            let bn = std::path::Path::new(&f.name)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                            (bn, f.progress)
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

        if is_pack {
            let videos = find_episode_files(Path::new(&info.content_path));

            for (src, season, episode) in videos {
                let basename = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let file_done = pack_file_progress.get(&basename).copied().unwrap_or(0.0) >= 1.0;
                if !file_done {
                    continue;
                }
                let (ep_id, already_ready) = {
                    let db = state.db.lock().await;
                    match db.find_or_create_episode(&row.media_id, season, episode) {
                        Ok(ep) => {
                            let ready = ep.status == "ready"
                                && ep
                                    .file_path
                                    .as_deref()
                                    .map(|p| Path::new(p).exists())
                                    .unwrap_or(false);
                            (ep.id, ready)
                        }
                        Err(e) => {
                            crate::pe!("[downloads] episode upsert failed: {e}");
                            continue;
                        }
                    }
                };
                if already_ready {
                    landed_any = true;
                    continue;
                }
                crate::pi!("[downloads] S{season:02}E{episode:02} landing from {basename}");
                {
                    let db = state.db.lock().await;
                    let _ = db.update_episode_status(&ep_id, "processing");
                }

                let ep_dir = format!("{}/S{:02}E{:02}", show_dir, season, episode);
                let dest = format!("{ep_dir}/video.mp4");
                let landed = land_video(
                    Path::new(&src.to_string_lossy().to_string()),
                    Path::new(&dest),
                );
                if let Err(e) = landed {
                    crate::pe!("[downloads] land S{season:02}E{episode:02} failed: {e}");
                    continue;
                }

                let src_probe = crate::ffmpeg::probe_media(&src).ok();
                let probe = crate::ffmpeg::probe_media(Path::new(&dest)).ok();

                if let Some((s, e)) = crate::intro::detect_from_chapters(&src).await {
                    let db = state.db.lock().await;
                    if db.update_episode_intro(&ep_id, s, e).is_ok() {
                        crate::pi!(
                            "[intro] S{season:02}E{episode:02} from chapter at land: {s}s-{e}s"
                        );
                    }
                }

                if let Some(cs) = crate::intro::detect_credits_from_chapters(&src).await {
                    let db = state.db.lock().await;
                    if db.update_episode_credits(&ep_id, cs).is_ok() {
                        crate::pi!(
                            "[credits] S{season:02}E{episode:02} from chapter at land: {cs}s"
                        );
                    }
                }

                let source_name = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| strip_ext(s).to_string());

                {
                    let db = state.db.lock().await;
                    let _ = db.update_episode_ready(&ep_id, &dest);
                    if let Some(p) = probe {
                        let _ = db.update_episode_duration(&ep_id, p.duration_secs);
                    }
                    if let Some(sn) = &source_name {
                        let _ = db.update_episode_source(&ep_id, sn);
                    }
                }
                landed_any = true;
                crate::pi!("[downloads] S{season:02}E{episode:02} ready -> {dest}, subs continue in background");
                spawn_audio_pregen(dest.clone());

                state.subs_busy.lock().await.insert(ep_id.clone());
                let subs_dir = format!("{ep_dir}/subs");
                if let Some(p) = src_probe.as_ref() {
                    persist_subtitles(
                        state,
                        &ep_id,
                        &p.subtitle_tracks,
                        &src,
                        Path::new(&subs_dir),
                    )
                    .await;
                }
                if let Some(parent) = src.parent() {
                    import_sidecar_subs(
                        state,
                        &ep_id,
                        parent,
                        Path::new(&ep_dir),
                        Some((season, episode)),
                    )
                    .await;
                }
                if let Some(tid) = media.tmdb_id {
                    let existing = {
                        let db = state.db.lock().await;
                        db.list_subtitles_for_owner(&ep_id).unwrap_or_default()
                    };
                    let langs_needed: Vec<&str> = ["en", "pl"]
                        .iter()
                        .copied()
                        .filter(|l| !existing.iter().any(|s| sub_lang_matches(&s.language, l)))
                        .collect();
                    if !langs_needed.is_empty() {
                        crate::pi!("[downloads] S{season:02}E{episode:02} fetching wyzie for {:?} (have {} other tracks)", langs_needed, existing.len());
                        crate::subs::auto_fetch_for_owner(
                            state,
                            &ep_id,
                            tid,
                            Some(season),
                            Some(episode),
                            &langs_needed,
                        )
                        .await;
                    } else {
                        crate::pi!(
                            "[downloads] S{season:02}E{episode:02} already has en+pl, skip wyzie"
                        );
                    }
                }
                state.subs_busy.lock().await.remove(&ep_id);

                let s_intro = state.clone();
                let mid_intro = row.media_id.clone();
                let season_intro = season;
                tokio::spawn(async move {
                    let saved =
                        crate::intro::detect_for_season(&s_intro, &mid_intro, season_intro).await;
                    if saved > 0 {
                        crate::pi!(
                            "[intro] S{season_intro:02} per-ep refresh: {saved} intro markers"
                        );
                    }
                });
            }
        } else {
            let Some(src) = pick_video_file(Path::new(&info.content_path)) else {
                crate::pe!("[downloads] no video file in {}", info.content_path);
                continue;
            };

            let (ep_id_opt, dest_dir, ep_se) = if let Some(eid) = &row.episode_id {
                let ep = {
                    let db = state.db.lock().await;
                    db.find_episode_by_id(eid).ok().flatten()
                };
                let Some(ep) = ep else {
                    crate::pe!("[downloads] episode {eid} missing");
                    continue;
                };
                {
                    let db = state.db.lock().await;
                    let _ = db.update_episode_status(eid, "processing");
                }
                (
                    Some(eid.clone()),
                    format!("{}/S{:02}E{:02}", show_dir, ep.season, ep.episode),
                    Some((ep.season, ep.episode)),
                )
            } else {
                (None, show_dir.clone(), None)
            };

            let dest = format!("{dest_dir}/video.mp4");
            if let Err(e) = land_video(&src, Path::new(&dest)) {
                crate::pe!("[downloads] land '{}' failed: {e}", info.name);
                continue;
            }

            let src_probe = crate::ffmpeg::probe_media(&src).ok();
            let probe = crate::ffmpeg::probe_media(Path::new(&dest)).ok();
            let owner_id = ep_id_opt.clone().unwrap_or_else(|| row.media_id.clone());

            if let Some(eid) = &ep_id_opt {
                if let Some((s, e)) = crate::intro::detect_from_chapters(&src).await {
                    let db = state.db.lock().await;
                    if db.update_episode_intro(eid, s, e).is_ok() {
                        crate::pi!("[intro] {eid} from chapter at land: {s}s-{e}s");
                    }
                }
                if let Some(cs) = crate::intro::detect_credits_from_chapters(&src).await {
                    let db = state.db.lock().await;
                    if db.update_episode_credits(eid, cs).is_ok() {
                        crate::pi!("[credits] {eid} from chapter at land: {cs}s");
                    }
                }
            }

            let source_name = src
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| strip_ext(s).to_string());

            {
                let db = state.db.lock().await;
                match &ep_id_opt {
                    Some(eid) => {
                        let _ = db.update_episode_ready(eid, &dest);
                        if let Some(p) = &probe {
                            let _ = db.update_episode_duration(eid, p.duration_secs);
                        }
                        if let Some(sn) = &source_name {
                            let _ = db.update_episode_source(eid, sn);
                        }
                    }
                    None => {
                        let _ = db.update_media_ready(&row.media_id, &dest);
                        if let Some(p) = &probe {
                            let _ = db.update_media_duration(&row.media_id, p.duration_secs);
                        }
                        if let Some(sn) = &source_name {
                            let _ = db.update_media_source(&row.media_id, sn);
                        }
                    }
                }
            }
            landed_any = true;
            crate::pi!(
                "[downloads] '{}' ready -> {dest}, subs continue in background",
                info.name
            );
            spawn_audio_pregen(dest.clone());

            state.subs_busy.lock().await.insert(owner_id.clone());
            let subs_dir = format!("{dest_dir}/subs");
            if let Some(p) = src_probe.as_ref() {
                persist_subtitles(
                    state,
                    &owner_id,
                    &p.subtitle_tracks,
                    &src,
                    Path::new(&subs_dir),
                )
                .await;
            }
            if let Some(parent) = src.parent() {
                import_sidecar_subs(state, &owner_id, parent, Path::new(&dest_dir), ep_se).await;
            }
            if let Some(tid) = media.tmdb_id {
                let existing = {
                    let db = state.db.lock().await;
                    db.list_subtitles_for_owner(&owner_id).unwrap_or_default()
                };
                let langs_needed: Vec<&str> = ["en", "pl"]
                    .iter()
                    .copied()
                    .filter(|l| !existing.iter().any(|s| sub_lang_matches(&s.language, l)))
                    .collect();
                if !langs_needed.is_empty() {
                    let (s_n, e_n) = ep_se.map_or((None, None), |(s, e)| (Some(s), Some(e)));
                    crate::pi!(
                        "[downloads] {owner_id} fetching wyzie for {:?} (have {} other tracks)",
                        langs_needed,
                        existing.len()
                    );
                    crate::subs::auto_fetch_for_owner(
                        state,
                        &owner_id,
                        tid,
                        s_n,
                        e_n,
                        &langs_needed,
                    )
                    .await;
                }
            }
            state.subs_busy.lock().await.remove(&owner_id);
        }

        if !landed_any && pack_complete {
            crate::pe!("[downloads] no video files matched for download {}", row.id);
            continue;
        }

        if landed_any && is_series {
            let db = state.db.lock().await;
            let _ = db.update_media_status(&row.media_id, "ready");
        }

        let videos_done = if is_pack {
            let video_progress: Vec<f64> = pack_file_progress
                .iter()
                .filter(|(bn, _)| is_video_basename(bn))
                .map(|(_, p)| *p)
                .collect();
            !video_progress.is_empty() && video_progress.iter().all(|p| *p >= 1.0)
        } else {
            pack_complete
        };

        if !videos_done {
            continue;
        }

        if is_series {
            let s = state.clone();
            let mid = row.media_id.clone();
            tokio::spawn(async move {
                let saved = crate::intro::detect_for_media(&s, &mid).await;
                if saved > 0 {
                    crate::pi!("[intro] auto-detected {saved} intro markers for {mid}");
                }
            });
        }

        {
            let db = state.db.lock().await;
            let _ = db.set_download_status(&row.id, "complete");
        }

        let _ = qbit.set_upload_limit(hash, 1).await;

        if let Err(e) = qbit.delete(hash, true).await {
            crate::pe!("[downloads] qbit cleanup failed: {e}");
        } else {
            crate::pi!("[downloads] purged torrent {hash} (videos all landed)");
        }
    }

    Ok(())
}

fn sub_lang_matches(sub_lang: &str, want: &str) -> bool {
    let s = sub_lang.to_lowercase();
    let w = want.to_lowercase();
    if s == w {
        return true;
    }
    match w.as_str() {
        "en" => matches!(s.as_str(), "eng" | "english"),
        "pl" => matches!(s.as_str(), "pol" | "polish"),
        "de" => matches!(s.as_str(), "ger" | "deu" | "german"),
        "fr" => matches!(s.as_str(), "fre" | "fra" | "french"),
        "es" => matches!(s.as_str(), "spa" | "spanish"),
        "it" => matches!(s.as_str(), "ita" | "italian"),
        "pt" => matches!(s.as_str(), "por" | "portuguese"),
        "ru" => matches!(s.as_str(), "rus" | "russian"),
        "ja" => matches!(s.as_str(), "jpn" | "japanese"),
        _ => false,
    }
}

fn is_video_basename(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.rsplit_once('.').map(|(_, e)| e),
        Some("mkv")
            | Some("mp4")
            | Some("avi")
            | Some("webm")
            | Some("mov")
            | Some("m4v")
            | Some("ts")
            | Some("flv")
            | Some("wmv")
    )
}

fn strip_ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if name.len() - i <= 5 => &name[..i],
        _ => name,
    }
}

fn walk_sidecars(root: &Path) -> Vec<PathBuf> {
    let exts = ["srt", "vtt", "ass", "ssa", "sub", "txt"];
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if !exts.contains(&ext.as_str()) {
                continue;
            }
            if ext == "txt" || ext == "sub" {
                let head = std::fs::read(&p).unwrap_or_default();
                if !crate::ffmpeg::looks_like_microdvd(&head) {
                    continue;
                }
            }
            found.push(p);
        }
    }
    found
}

fn land_video(src: &Path, dest_mp4: &Path) -> Result<(), String> {
    if let Some(parent) = dest_mp4.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "mp4" {
        if std::fs::hard_link(src, dest_mp4).is_err() {
            if let Err(e) = std::fs::copy(src, dest_mp4) {
                let _ = std::fs::remove_file(dest_mp4);
                return Err(format!("copy mp4: {e}"));
            }
        }
    } else {
        if let Err(e) = crate::ffmpeg::remux_to_mp4(src, dest_mp4) {
            let _ = std::fs::remove_file(dest_mp4);
            return Err(e);
        }
    }

    let size = std::fs::metadata(dest_mp4).map(|m| m.len()).unwrap_or(0);
    if size < 1_000_000 {
        let _ = std::fs::remove_file(dest_mp4);
        return Err(format!("output too small ({size} bytes)"));
    }
    drop_audio_cache(dest_mp4);
    Ok(())
}

fn drop_audio_cache(video: &Path) {
    let Some(parent) = video.parent() else { return };
    let Some(stem) = video.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let prefix = format!("{stem}_audio");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with(&prefix) && name.ends_with(".mp4") {
            let _ = std::fs::remove_file(p);
        }
    }
}

async fn import_sidecar_subs(
    state: &Arc<AppState>,
    owner_id: &str,
    src_root: &Path,
    dest_dir: &Path,
    se: Option<(i32, i32)>,
) {
    let dest_subs = dest_dir.join("subs");
    let _ = std::fs::create_dir_all(&dest_subs);

    let mut found = walk_sidecars(src_root);
    if found.is_empty() {
        if let Some(up) = src_root.parent() {
            found = walk_sidecars(up);
        }
    }

    let mut sidecar_idx = 0usize;
    for src in found {
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("sub");
        let lower = stem.to_lowercase();

        if let Some((s, e)) = se {
            let needle = format!("s{:02}e{:02}", s, e);
            if !lower.contains(&needle) {
                continue;
            }
        }

        let (lang, label) = sub_lang_from_stem(stem, sidecar_idx);
        sidecar_idx += 1;
        let src_ext = src
            .extension()
            .and_then(|x| x.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "vtt".into());
        let target_ext = match src_ext.as_str() {
            "vtt" | "ass" | "ssa" => src_ext.clone(),
            _ => "vtt".to_string(),
        };
        let dest = dest_subs.join(format!("{}_sidecar_{}.{}", lang, stem, target_ext));

        {
            let db = state.db.lock().await;
            if db.subtitle_exists(owner_id, &lang, &label).unwrap_or(false) {
                continue;
            }
        }

        let ok = crate::ffmpeg::convert_sub(&src, &dest, &lang);
        if !ok {
            crate::pe!("[downloads] sidecar copy failed: {}", src.display());
            continue;
        }

        let sub = crate::models::Subtitle {
            id: uuid::Uuid::new_v4().to_string(),
            owner_id: owner_id.to_string(),
            language: lang,
            label,
            format: target_ext,
            file_path: dest.to_string_lossy().into_owned(),
            is_default: false,
            media_id: None,
        };
        let db = state.db.lock().await;
        if let Err(e) = db.create_subtitle(&sub) {
            crate::pe!("[downloads] sidecar db insert failed: {e}");
        }
    }
}

fn sub_lang_from_stem(stem: &str, n: usize) -> (String, String) {
    let last = stem.rsplit('.').next().unwrap_or("").to_lowercase();
    match last.as_str() {
        "en" | "eng" | "english" => ("eng".into(), "English".into()),
        "pl" | "pol" | "polish" => ("pol".into(), "Polish".into()),
        "es" | "spa" | "spanish" => ("spa".into(), "Spanish".into()),
        "fr" | "fre" | "fra" | "french" => ("fre".into(), "French".into()),
        "de" | "ger" | "deu" | "german" => ("ger".into(), "German".into()),
        "it" | "ita" | "italian" => ("ita".into(), "Italian".into()),
        "pt" | "por" | "portuguese" => ("por".into(), "Portuguese".into()),
        "ru" | "rus" | "russian" => ("rus".into(), "Russian".into()),
        "ja" | "jpn" | "japanese" => ("jpn".into(), "Japanese".into()),
        "ko" | "kor" | "korean" => ("kor".into(), "Korean".into()),
        "zh" | "chi" | "zho" | "chinese" => ("chi".into(), "Chinese".into()),
        "ar" | "ara" | "arabic" => ("ara".into(), "Arabic".into()),
        "tr" | "tur" | "turkish" => ("tur".into(), "Turkish".into()),
        "nl" | "dut" | "nld" | "dutch" => ("dut".into(), "Dutch".into()),
        "sv" | "swe" | "swedish" => ("swe".into(), "Swedish".into()),
        _ => ("und".into(), format!("subtitle {}", n + 1)),
    }
}

fn saved_sub_label(track: &crate::ffmpeg::SubtitleTrack, n: usize) -> String {
    let raw = crate::ffmpeg::fmt_sub_label(track);
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();

    if !trimmed.is_empty()
        && !lower.starts_with("unknown")
        && !crate::lang::looks_like_raw_code(trimmed)
    {
        return trimmed
            .replace("(Forced)", "(forced)")
            .replace("(SDH)", "(sdh)");
    }

    let lang = crate::lang::lang_name(&track.language);
    let base = if lang == "Unknown" {
        format!("subtitle {}", n + 1)
    } else {
        lang
    };
    if track.forced {
        format!("{base} (forced)")
    } else if track.hearing_impaired {
        format!("{base} (sdh)")
    } else {
        base
    }
}

async fn persist_subtitles(
    state: &Arc<AppState>,
    owner_id: &str,
    tracks: &[crate::ffmpeg::SubtitleTrack],
    video_path: &Path,
    subs_dir: &Path,
) {
    if tracks.is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(subs_dir);

    let text_tracks: Vec<crate::ffmpeg::SubtitleTrack> = tracks
        .iter()
        .filter(|t| crate::ffmpeg::is_text_sub(&t.codec))
        .cloned()
        .collect();
    let pgs_tracks: Vec<crate::ffmpeg::SubtitleTrack> = tracks
        .iter()
        .filter(|t| crate::ffmpeg::is_pgs_sub(&t.codec))
        .cloned()
        .collect();

    let extracted = crate::ffmpeg::extract_all_subtitles(video_path, &text_tracks, subs_dir);
    for (i, sub_path) in extracted {
        let track = &text_tracks[i];
        let (lang, label) = if track.language == "und" || track.language.is_empty() {
            match crate::subs::detect_lang_from_file(&sub_path) {
                Some(code) => {
                    let canon = match code {
                        "eng" => "eng",
                        "pol" => "pol",
                        _ => code,
                    };
                    crate::pi!(
                        "[subs] track {} und -> detected {} from content",
                        track.index,
                        canon
                    );
                    let mut t = track.clone();
                    t.language = canon.to_string();
                    (canon.to_string(), saved_sub_label(&t, i))
                }
                None => {
                    crate::pi!(
                        "[subs] track {} und, content detection inconclusive",
                        track.index
                    );
                    (track.language.clone(), saved_sub_label(track, i))
                }
            }
        } else {
            (track.language.clone(), saved_sub_label(track, i))
        };
        save_track_sub(state, owner_id, &lang, &label, &sub_path, i == 0).await;
    }

    for (idx, track) in pgs_tracks.iter().enumerate() {
        let lang_ietf = match track.language.as_str() {
            "en" | "eng" => "en",
            "pl" | "pol" => "pl",
            "de" | "ger" | "deu" => "de",
            "fr" | "fre" | "fra" => "fr",
            "es" | "spa" => "es",
            "it" | "ita" => "it",
            "pt" | "por" => "pt",
            "ru" | "rus" => "ru",
            "ja" | "jpn" => "ja",
            other => other,
        };
        let stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video");
        let sup_path = subs_dir.join(format!("{stem}.t{}.{lang_ietf}.sup", track.index));
        if let Err(e) = crate::ffmpeg::extract_pgs_track(video_path, track.index, &sup_path) {
            crate::pe!("[ocr] extract pgs track {} failed: {e}", track.index);
            continue;
        }
        crate::pi!("[ocr] pgs track {} -> {}", track.index, sup_path.display());
        let srt_path = match crate::ffmpeg::ocr_pgs_to_srt(&sup_path, lang_ietf) {
            Ok(p) => p,
            Err(e) => {
                crate::pe!("[ocr] pgsrip {lang_ietf} failed: {e}");
                continue;
            }
        };
        let vtt_path = subs_dir.join(format!("{lang_ietf}_{}.vtt", track.index));
        if !crate::ffmpeg::convert_sub(&srt_path, &vtt_path, lang_ietf) {
            crate::pe!("[ocr] srt->vtt convert failed for {}", srt_path.display());
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(&vtt_path) {
            let flat = flatten_vtt_cues(&raw);
            let _ = std::fs::write(&vtt_path, flat);
        }
        let _ = std::fs::remove_file(&sup_path);
        let _ = std::fs::remove_file(&srt_path);
        let label = format!("{} (ocr)", saved_sub_label(track, text_tracks.len() + idx));
        save_track_sub(
            state,
            owner_id,
            &track.language,
            &label,
            &vtt_path,
            idx == 0 && text_tracks.is_empty(),
        )
        .await;
        crate::pi!("[ocr] {lang_ietf} track {} ready", track.index);
    }
}

const SUB_LINE_TARGET: usize = 42;

fn wrap_cue(line: &str) -> String {
    let s = line.trim();
    let len = s.chars().count();
    if len <= SUB_LINE_TARGET {
        return s.to_string();
    }

    let mid = len / 2;
    let mut best_byte: Option<usize> = None;
    let mut best_cost = i64::MAX;

    let mut char_idx = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if ch == ' ' {
            let dist = if char_idx > mid {
                char_idx - mid
            } else {
                mid - char_idx
            } as i64;
            let prev = s[..byte_idx].chars().next_back().unwrap_or(' ');
            let bonus = match prev {
                '.' | '!' | '?' => -30,
                ',' | ';' | ':' => -15,
                _ => 0,
            };
            let cost = dist + bonus;
            if cost < best_cost {
                best_cost = cost;
                best_byte = Some(byte_idx);
            }
        }
        char_idx += 1;
    }

    match best_byte {
        Some(i) => {
            let left = s[..i].trim_end();
            let right = s[i + 1..].trim_start();
            let lc = left.chars().count();
            let rc = right.chars().count();
            if lc > SUB_LINE_TARGET || rc > SUB_LINE_TARGET {
                let left_w = wrap_cue(left);
                let right_w = wrap_cue(right);
                return format!("{left_w}\n{right_w}");
            }
            format!("{left}\n{right}")
        }
        None => s.to_string(),
    }
}

fn flatten_vtt_cues(vtt: &str) -> String {
    let mut out = String::with_capacity(vtt.len());
    let mut buf: Vec<&str> = Vec::new();
    let mut in_cue = false;

    let flush = |out: &mut String, buf: &mut Vec<&str>| {
        if buf.is_empty() {
            return;
        }
        let joined = buf.iter().map(|s| s.trim()).collect::<Vec<_>>().join(" ");
        let wrapped = wrap_cue(joined.trim());
        out.push_str(&wrapped);
        out.push('\n');
        buf.clear();
    };

    for line in vtt.lines() {
        if line.contains("-->") {
            flush(&mut out, &mut buf);
            out.push_str(line);
            out.push('\n');
            in_cue = true;
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut out, &mut buf);
            out.push('\n');
            in_cue = false;
            continue;
        }
        if in_cue {
            buf.push(line);
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    flush(&mut out, &mut buf);
    out
}

async fn save_track_sub(
    state: &Arc<AppState>,
    owner_id: &str,
    lang: &str,
    label: &str,
    file: &Path,
    is_default: bool,
) {
    let db = state.db.lock().await;
    if db.subtitle_exists(owner_id, lang, label).unwrap_or(false) {
        return;
    }
    let sub = crate::models::Subtitle {
        id: uuid::Uuid::new_v4().to_string(),
        owner_id: owner_id.to_string(),
        language: lang.to_string(),
        label: label.to_string(),
        format: "vtt".into(),
        file_path: file.to_string_lossy().into_owned(),
        is_default,
        media_id: None,
    };
    if let Err(e) = db.create_subtitle(&sub) {
        crate::pe!("[downloads] save subtitle failed: {e}");
    }
}

const SKIP_DIRS: &[&str] = &[
    "featurettes",
    "featurette",
    "extras",
    "extra",
    "bonus",
    "specials",
    "special",
    "behind the scenes",
    "deleted scenes",
    "interviews",
    "samples",
    "sample",
];

fn is_video_ext(ext: &str) -> bool {
    matches!(ext, "mkv" | "mp4" | "avi" | "mov" | "webm" | "m4v")
}

fn find_episode_files(start: &Path) -> Vec<(PathBuf, i32, i32)> {
    let mut out: Vec<(PathBuf, i32, i32)> = Vec::new();

    if start.is_file() {
        if let Some((s, e)) = parse_episode_from_path(start) {
            out.push((start.to_path_buf(), s, e));
        }
        return out;
    }

    let mut stack = vec![start.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if SKIP_DIRS.iter().any(|s| name == *s) {
                    continue;
                }
                stack.push(p);
                continue;
            }
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if !is_video_ext(&ext) {
                continue;
            }
            if let Some((s, e)) = parse_episode_from_path(&p) {
                out.push((p, s, e));
            }
        }
    }
    out.sort_by_key(|(_, s, e)| (*s, *e));
    out
}

pub fn parse_episode_from_path(p: &Path) -> Option<(i32, i32)> {
    let filename = p.file_name()?.to_str()?.to_lowercase();
    let bytes = filename.as_bytes();
    let n = bytes.len();

    let mut i = 0;
    while i < n {
        if bytes[i] == b's' && i + 1 < n && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < n && bytes[j].is_ascii_digit() && j - i <= 3 {
                j += 1;
            }
            if j < n && bytes[j] == b'e' && j + 1 < n && bytes[j + 1].is_ascii_digit() {
                let mut k = j + 1;
                while k < n && bytes[k].is_ascii_digit() && k - j <= 3 {
                    k += 1;
                }
                let s: i32 = filename[i + 1..j].parse().ok()?;
                let e: i32 = filename[j + 1..k].parse().ok()?;
                if s > 0 && e > 0 {
                    return Some((s, e));
                }
            }
        }
        i += 1;
    }

    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < n && bytes[j].is_ascii_digit() && j - i < 2 {
                j += 1;
            }
            if j < n && bytes[j] == b'x' && j + 1 < n && bytes[j + 1].is_ascii_digit() {
                let mut k = j + 1;
                while k < n && bytes[k].is_ascii_digit() && k - j <= 3 {
                    k += 1;
                }
                if k - j >= 2 && k - j <= 3 {
                    let prev_ok =
                        i == 0 || matches!(bytes[i - 1], b'.' | b' ' | b'_' | b'-' | b'/' | b'\\');
                    let next_ok =
                        k >= n || matches!(bytes[k], b'.' | b' ' | b'_' | b'-' | b'/' | b'\\');
                    if prev_ok && next_ok {
                        let s: i32 = filename[i..j].parse().ok()?;
                        let e: i32 = filename[j + 1..k].parse().ok()?;
                        if s > 0 && e > 0 && s <= 50 && e <= 99 {
                            return Some((s, e));
                        }
                    }
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }

    if filename.starts_with('[') || filename.contains("] ") {
        if let Some(ep) = parse_anime_episode(&filename) {
            let season = parent_season_hint(p).unwrap_or(1);
            return Some((season, ep));
        }
    }

    None
}

fn parse_anime_episode(filename: &str) -> Option<i32> {
    let bytes = filename.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if matches!(bytes[i], b' ' | b'-' | b'_') {
            let mut j = i;
            while j < n && matches!(bytes[j], b' ' | b'-' | b'_') {
                j += 1;
            }
            let mut k = j;
            while k < n && bytes[k].is_ascii_digit() && k - j <= 3 {
                k += 1;
            }
            if k > j && k - j >= 2 && k < n {
                let after = bytes[k];
                if matches!(after, b' ' | b'.' | b'(' | b'[' | b'v') {
                    let ep: i32 = filename[j..k].parse().ok()?;
                    if ep > 0 && ep < 999 {
                        return Some(ep);
                    }
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    None
}

fn parent_season_hint(p: &Path) -> Option<i32> {
    let parent = p.parent()?;
    let name = parent.file_name()?.to_str()?.to_lowercase();
    let bytes = name.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b's' && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() && j - i <= 3 {
                j += 1;
            }
            return name[i + 1..j].parse().ok();
        }
        i += 1;
    }
    None
}

async fn lookup_hash_by_path(qbit: &crate::qbit::Qbit, save_path: &str) -> Option<String> {
    let list = qbit.list(Some(QBIT_CATEGORY)).await.ok()?;
    list.into_iter()
        .find(|t| t.save_path.starts_with(save_path) || save_path.starts_with(&t.save_path))
        .map(|t| t.hash)
}

fn pick_video_file(start: &Path) -> Option<PathBuf> {
    if start.is_file() {
        let ext = start
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if is_video_ext(&ext) {
            return Some(start.to_path_buf());
        }
    }

    let mut best: Option<(PathBuf, u64)> = None;
    let mut stack = vec![start.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if SKIP_DIRS.iter().any(|s| name == *s) {
                    continue;
                }
                stack.push(p);
                continue;
            }
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if !is_video_ext(&ext) {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            if best.as_ref().map(|b| size > b.1).unwrap_or(true) {
                best = Some((p, size));
            }
        }
    }
    best.map(|(p, _)| p)
}

pub async fn ingest_local(
    state: &Arc<AppState>,
    media: &crate::models::Media,
    src: &Path,
    se: Option<(i32, i32)>,
) -> Result<String, String> {
    let type_dir = if media.media_type == "movie" {
        "movies"
    } else if media.is_anime {
        "anime"
    } else {
        "series"
    };
    let show_dir = format!("{}/{}/{}", state.media_root, type_dir, &media.id);
    let _ = std::fs::create_dir_all(&show_dir);

    let is_tv = media.media_type == "tv";

    let (owner_id, dest_dir, ep_se) = if is_tv {
        let (season, episode) = se.ok_or_else(|| "could not detect season/episode".to_string())?;
        let ep_id = {
            let db = state.db.lock().await;
            db.find_or_create_episode(&media.id, season, episode)
                .map(|ep| ep.id)
                .map_err(|e| format!("episode upsert: {e}"))?
        };
        {
            let db = state.db.lock().await;
            let _ = db.update_episode_status(&ep_id, "processing");
        }
        (
            ep_id,
            format!("{show_dir}/S{season:02}E{episode:02}"),
            Some((season, episode)),
        )
    } else {
        (media.id.clone(), show_dir.clone(), None)
    };

    let dest = format!("{dest_dir}/video.mp4");
    if let Err(e) = land_video(src, Path::new(&dest)) {
        return Err(format!("land: {e}"));
    }

    let src_probe = crate::ffmpeg::probe_media(src).ok();
    let probe = crate::ffmpeg::probe_media(Path::new(&dest)).ok();

    if is_tv {
        if let Some((a, b)) = crate::intro::detect_from_chapters(src).await {
            let db = state.db.lock().await;
            let _ = db.update_episode_intro(&owner_id, a, b);
        }
        if let Some(cs) = crate::intro::detect_credits_from_chapters(src).await {
            let db = state.db.lock().await;
            let _ = db.update_episode_credits(&owner_id, cs);
        }
    }

    let source_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| strip_ext(s).to_string());

    {
        let db = state.db.lock().await;
        if is_tv {
            let _ = db.update_episode_ready(&owner_id, &dest);
            if let Some(p) = &probe {
                let _ = db.update_episode_duration(&owner_id, p.duration_secs);
            }
            if let Some(sn) = &source_name {
                let _ = db.update_episode_source(&owner_id, sn);
            }
        } else {
            let _ = db.update_media_ready(&media.id, &dest);
            if let Some(p) = &probe {
                let _ = db.update_media_duration(&media.id, p.duration_secs);
            }
            if let Some(sn) = &source_name {
                let _ = db.update_media_source(&media.id, sn);
            }
        }
    }

    if is_tv {
        let db = state.db.lock().await;
        let _ = db.update_media_status(&media.id, "ready");
    }

    spawn_audio_pregen(dest);

    state.subs_busy.lock().await.insert(owner_id.clone());
    let subs_dir = format!("{dest_dir}/subs");
    if let Some(p) = src_probe.as_ref() {
        persist_subtitles(
            state,
            &owner_id,
            &p.subtitle_tracks,
            src,
            Path::new(&subs_dir),
        )
        .await;
    }
    if let Some(parent) = src.parent() {
        import_sidecar_subs(state, &owner_id, parent, Path::new(&dest_dir), ep_se).await;
    }
    if let Some(tid) = media.tmdb_id {
        let have = {
            let db = state.db.lock().await;
            db.list_subtitles_for_owner(&owner_id)
                .map(|v| v.len())
                .unwrap_or(0)
        };
        if have == 0 {
            let (s_n, e_n) = ep_se.map_or((None, None), |(s, e)| (Some(s), Some(e)));
            crate::subs::auto_fetch_for_owner(state, &owner_id, tid, s_n, e_n, &["en", "pl"]).await;
        }
    }
    state.subs_busy.lock().await.remove(&owner_id);

    Ok(owner_id)
}

fn spawn_audio_pregen(dest: String) {
    tokio::spawn(async move {
        let path = std::path::Path::new(&dest);
        let count = crate::stream::audio_track_count(path).await;
        if count == 0 {
            return;
        }

        if !crate::stream::default_audio_browser_safe(path).await {
            match crate::stream::ensure_audio_remux(path, 0).await {
                Ok(p) => crate::pi!("[downloads] default audio remuxed -> {}", p.display()),
                Err(e) => crate::pe!("[downloads] default audio remux failed: {e}"),
            }
        }

        if count <= 1 {
            return;
        }
        crate::pi!(
            "[downloads] pre-gen {} alt audio tracks for {dest}",
            count - 1
        );
        for idx in 1..count {
            match crate::stream::ensure_audio_remux(path, idx).await {
                Ok(p) => crate::pi!("[downloads] alt audio {idx} cached -> {}", p.display()),
                Err(e) => crate::pe!("[downloads] alt audio {idx} failed: {e}"),
            }
        }
    });
}
