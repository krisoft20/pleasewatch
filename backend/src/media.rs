use crate::{
    middleware::AuthUser,
    models::{AddMediaRequest, ApiError, Media, MediaWithEpisodes},
    tmdb, AppState,
};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
pub struct ListQuery {
    pub mine: Option<bool>,
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/media", get(handle_list).post(handle_add))
        .route("/api/media/{id}", get(handle_get).delete(handle_delete))
        .route(
            "/api/media/by-tmdb/{kind}/{tmdb_id}",
            get(handle_get_by_tmdb),
        )
        .route(
            "/api/media/{id}/upload-local",
            post(handle_upload_local).layer(DefaultBodyLimit::max(50 * 1024 * 1024 * 1024)),
        )
        .route(
            "/api/episodes/{id}/file",
            axum::routing::delete(handle_episode_remove_file),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

async fn handle_get_by_tmdb(
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    if kind != "movie" && kind != "tv" {
        return err(StatusCode::BAD_REQUEST, "kind must be 'movie' or 'tv'");
    }
    let db = state.db.lock().await;
    let media = match db.find_media_by_tmdb(tmdb_id, &kind) {
        Ok(Some(m)) => m,
        Ok(None) => return err(StatusCode::NOT_FOUND, "not in library"),
        Err(e) => {
            eprintln!("[media] find by tmdb: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    };
    let episodes = if media.media_type == "tv" {
        db.list_episodes_for_media(&media.id).unwrap_or_default()
    } else {
        Vec::new()
    };
    drop(db);
    let subs_processing = subs_busy_for(&state, &media.id, &episodes).await;
    Json(MediaWithEpisodes {
        media,
        episodes,
        subs_processing,
    })
    .into_response()
}

async fn subs_busy_for(
    state: &Arc<AppState>,
    media_id: &str,
    episodes: &[crate::models::Episode],
) -> bool {
    let busy = state.subs_busy.lock().await;
    busy.contains(media_id) || episodes.iter().any(|e| busy.contains(&e.id))
}

async fn handle_list(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let res = if q.mine.unwrap_or(false) {
        db.list_media_by_user(&auth.id)
    } else {
        db.list_media_for_viewer(&auth.id)
    };
    match res {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            eprintln!("[media] list failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

async fn handle_get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;

    let media = match db.find_media_by_id(&id) {
        Ok(Some(m)) => m,
        Ok(None) => return err(StatusCode::NOT_FOUND, "not found"),
        Err(e) => {
            eprintln!("[media] find failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    };

    let episodes = if media.media_type == "tv" {
        db.list_episodes_for_media(&id).unwrap_or_default()
    } else {
        Vec::new()
    };

    drop(db);
    let subs_processing = subs_busy_for(&state, &media.id, &episodes).await;
    Json(MediaWithEpisodes {
        media,
        episodes,
        subs_processing,
    })
    .into_response()
}

async fn handle_add(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddMediaRequest>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return err(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    if body.media_type != "movie" && body.media_type != "tv" {
        return err(
            StatusCode::BAD_REQUEST,
            "media_type must be 'movie' or 'tv'",
        );
    }

    {
        let db = state.db.lock().await;
        if let Ok(Some(existing)) = db.find_media_by_tmdb(body.tmdb_id, &body.media_type) {
            return Json(existing).into_response();
        }
    }

    let detail = match body.media_type.as_str() {
        "movie" => tmdb::movie(&key, body.tmdb_id, "en-US").await,
        _ => tmdb::tv(&key, body.tmdb_id, "en-US").await,
    };

    let detail = match detail {
        Ok(Some(d)) => d,
        Ok(None) => return err(StatusCode::NOT_FOUND, "not found on tmdb"),
        Err(e) => {
            eprintln!("[media] tmdb fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "tmdb fetch failed");
        }
    };

    let year = detail.year.as_deref().and_then(|y| y.parse::<i32>().ok());
    let is_anime = detail.is_anime;
    let media = Media {
        id: uuid::Uuid::new_v4().to_string(),
        tmdb_id: Some(detail.tmdb_id),
        media_type: detail.media_type,
        title: detail.title,
        year,
        overview: detail.overview,
        poster_url: detail.poster_url,
        backdrop_url: detail.backdrop_url,
        file_path: None,
        duration: detail.runtime.map(|r| (r as i64) * 60),
        status: "pending".into(),
        added_by: Some(auth.id),
        added_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        activity_at: None,
        activity_label: None,
        is_anime,
        source_name: None,
    };

    let db = state.db.lock().await;
    if let Err(e) = db.create_media(&media) {
        eprintln!("[media] create failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "could not create");
    }

    println!(
        "[media] added '{}' ({}) by {}",
        media.title, media.media_type, auth.username
    );

    (StatusCode::CREATED, Json(media)).into_response()
}

async fn handle_delete(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let on_disk_dir = {
        let db = state.db.lock().await;
        match db.find_media_by_id(&id) {
            Ok(Some(m)) => {
                let is_owner = m.added_by.as_deref() == Some(auth.id.as_str());
                if !auth.is_admin() && !is_owner {
                    return err(StatusCode::FORBIDDEN, "not yours");
                }
                let type_dir = if m.media_type == "movie" {
                    "movies"
                } else if m.is_anime {
                    "anime"
                } else {
                    "series"
                };
                Some(
                    std::path::Path::new(&state.media_root)
                        .join(type_dir)
                        .join(&id),
                )
            }
            Ok(None) => return err(StatusCode::NOT_FOUND, "not found"),
            Err(e) => {
                eprintln!("[media] delete lookup: {e}");
                return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
            }
        }
    };

    let (sub_paths, hashes) = {
        let db = state.db.lock().await;
        let subs = db.subtitle_paths_for_media(&id).unwrap_or_default();
        let dls = db.downloads_for_media(&id).unwrap_or_default();
        let hs: Vec<String> = dls.into_iter().filter_map(|d| d.qbit_hash).collect();
        (subs, hs)
    };

    if !hashes.is_empty() {
        if let Some(qbit) = state.qbit.lock().await.clone() {
            for h in &hashes {
                if let Err(e) = qbit.delete(h, true).await {
                    eprintln!("[media] qbit delete {h} failed: {e}");
                }
            }
            println!("[media] purged {} torrent(s) for media {id}", hashes.len());
        }
    }

    let db = state.db.lock().await;
    let result = db.delete_media(&id);
    drop(db);

    match result {
        Ok(true) => {
            for p in sub_paths {
                let _ = std::fs::remove_file(&p);
            }
            if let Some(dir) = on_disk_dir {
                if let Err(e) = std::fs::remove_dir_all(&dir) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        eprintln!("[media] remove dir {} failed: {e}", dir.display());
                    }
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => err(StatusCode::NOT_FOUND, "not found"),
        Err(e) => {
            eprintln!("[media] delete failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "delete failed")
        }
    }
}

async fn handle_episode_remove_file(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let ep = match db.find_episode_by_id(&id) {
        Ok(Some(e)) => e,
        Ok(None) => return err(StatusCode::NOT_FOUND, "episode not found"),
        Err(e) => {
            eprintln!("[media] find episode: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    };
    let owner_ok = match db.find_media_by_id(&ep.media_id) {
        Ok(Some(m)) => m.added_by.as_deref() == Some(auth.id.as_str()),
        _ => false,
    };
    if !auth.is_admin() && !owner_ok {
        return err(StatusCode::FORBIDDEN, "not yours");
    }
    if let Some(p) = ep.file_path.as_deref() {
        let abs = if std::path::Path::new(p).is_absolute() {
            std::path::PathBuf::from(p)
        } else {
            std::path::Path::new(&state.media_root).join(p)
        };
        let _ = std::fs::remove_file(&abs);
        if let Some(parent) = abs.parent() {
            let _ = std::fs::remove_dir_all(parent.join("subs"));
            let _ = std::fs::remove_dir(parent);
        }
    }
    let upload_paths = db.subtitle_paths_for_owner(&ep.id).unwrap_or_default();
    if let Err(e) = db.delete_subtitles_by_owner(&ep.id) {
        eprintln!("[media] clear episode subtitles failed: {e}");
    }
    for p in upload_paths {
        let _ = std::fs::remove_file(&p);
    }
    if let Err(e) = db.clear_episode_file(&ep.id) {
        eprintln!("[media] clear episode failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    }
    StatusCode::NO_CONTENT.into_response()
}

fn safe_upload_filename(raw: &str) -> Option<String> {
    let leaf = raw.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    if leaf.is_empty() || matches!(leaf, "." | "..") {
        return None;
    }

    let mut safe = leaf
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect::<String>();
    safe.truncate(
        safe.char_indices()
            .nth(240)
            .map(|(idx, _)| idx)
            .unwrap_or(safe.len()),
    );
    let trimmed_len = safe.trim_end_matches([' ', '.']).len();
    safe.truncate(trimmed_len);
    if safe.is_empty() || matches!(safe.as_str(), "." | "..") {
        return None;
    }

    let stem = safe
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let numbered_device = stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9');
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered_device {
        safe.insert(0, '_');
    }

    Some(safe)
}

async fn handle_upload_local(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(media_id): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return err(StatusCode::FORBIDDEN, "admin only");
    }

    let media = {
        let db = state.db.lock().await;
        match db.find_media_by_id(&media_id).ok().flatten() {
            Some(m) => m,
            None => return err(StatusCode::NOT_FOUND, "media not found"),
        }
    };

    let staging_root = std::path::Path::new(&state.media_root).join("_uploads");
    let _ = std::fs::create_dir_all(&staging_root);
    let staging_dir = staging_root.join(uuid::Uuid::new_v4().to_string());
    if let Err(e) = std::fs::create_dir_all(&staging_dir) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("staging: {e}"));
    }

    let mut staged_file: Option<PathBuf> = None;
    let mut season_override: Option<i32> = None;
    let mut episode_override: Option<i32> = None;

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let fname = field.name().unwrap_or("").to_string();
        if fname == "season" {
            if let Ok(v) = field.text().await {
                season_override = v.parse().ok();
            }
            continue;
        }
        if fname == "episode" {
            if let Ok(v) = field.text().await {
                episode_override = v.parse().ok();
            }
            continue;
        }
        if fname != "file" {
            continue;
        }
        let original = field
            .file_name()
            .and_then(safe_upload_filename)
            .unwrap_or_else(|| format!("upload-{}.mkv", uuid::Uuid::new_v4()));
        let target = staging_dir.join(&original);
        let mut out = match tokio::fs::File::create(&target).await {
            Ok(f) => f,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("create: {e}"));
            }
        };
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(e) = out.write_all(&chunk).await {
                        let _ = std::fs::remove_dir_all(&staging_dir);
                        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("write: {e}"));
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    eprintln!("[upload] chunk read failed: {e}");
                    let _ = std::fs::remove_dir_all(&staging_dir);
                    return err(StatusCode::BAD_REQUEST, "upload aborted");
                }
            }
        }
        if let Err(e) = out.flush().await {
            eprintln!("[upload] flush: {e}");
        }
        drop(out);
        staged_file = Some(target);
    }

    let Some(src) = staged_file else {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return err(StatusCode::BAD_REQUEST, "no file uploaded");
    };

    let se = if media.media_type == "tv" {
        match (season_override, episode_override) {
            (Some(s), Some(e)) => Some((s, e)),
            _ => crate::downloads::parse_episode_from_path(&src),
        }
    } else {
        None
    };

    println!(
        "[upload] media={} file={} se={:?} by={}",
        media.id,
        src.display(),
        se,
        auth.username
    );

    let res = crate::downloads::ingest_local(&state, &media, &src, se).await;
    let _ = std::fs::remove_dir_all(&staging_dir);

    match res {
        Ok(owner_id) => Json(serde_json::json!({
            "ok": true,
            "owner_id": owner_id,
        }))
        .into_response(),
        Err(e) => {
            eprintln!("[upload] ingest failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, &e)
        }
    }
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_filename_never_keeps_a_client_path() {
        assert_eq!(
            safe_upload_filename("../../manage/bin/pleasewatch"),
            Some("pleasewatch".into())
        );
        assert_eq!(
            safe_upload_filename(r"C:\manage\bin\pleasewatch.exe"),
            Some("pleasewatch.exe".into())
        );
        assert_eq!(safe_upload_filename("/manage/.env"), Some(".env".into()));
    }

    #[test]
    fn upload_filename_handles_windows_special_names() {
        assert_eq!(
            safe_upload_filename("movie:S01E02?.mkv"),
            Some("movie_S01E02_.mkv".into())
        );
        assert_eq!(safe_upload_filename("CON.mkv"), Some("_CON.mkv".into()));
        assert_eq!(safe_upload_filename(".."), None);
        assert_eq!(safe_upload_filename("////"), None);
    }
}
