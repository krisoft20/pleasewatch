use crate::{middleware::AuthUser, models::ApiError, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

const WATCH_TICK_SECONDS: i64 = 15;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/watch/save", post(handle_save))
        .route("/api/watch/get", get(handle_get))
        .route("/api/watch/mark", post(handle_mark))
        .route("/api/watch/tick", post(handle_tick))
        .route("/api/watch/continue", get(handle_continue))
        .route("/api/watch/summary", get(handle_summary))
        .route("/api/watch/dismiss/{media_id}", post(handle_dismiss))
        .route("/api/media/{id}/progress", get(handle_list_for_media))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct SaveBody {
    media_id: String,
    episode_id: Option<String>,
    position: i64,
    duration: i64,
}

async fn handle_save(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<SaveBody>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    if let Err(e) = db.upsert_progress(
        &auth.id,
        &b.media_id,
        b.episode_id.as_deref(),
        b.position,
        b.duration,
    ) {
        eprintln!("[watch] save failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct GetQuery {
    media_id: String,
    episode_id: Option<String>,
}

async fn handle_get(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(q): Query<GetQuery>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_progress(&auth.id, &q.media_id, q.episode_id.as_deref()) {
        Ok(p) => Json(p).into_response(),
        Err(e) => {
            eprintln!("[watch] get failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed")
        }
    }
}

#[derive(Deserialize)]
struct MarkBody {
    media_id: String,
    episode_id: Option<String>,
    watched: bool,
    duration: Option<i64>,
}

#[derive(Deserialize)]
struct TickBody {
    media_id: String,
    position: f64,
    duration: f64,
}

async fn handle_mark(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<MarkBody>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let dur = b.duration.unwrap_or(1);
    if let Err(e) = db.mark_watched(
        &auth.id,
        &b.media_id,
        b.episode_id.as_deref(),
        dur,
        b.watched,
    ) {
        eprintln!("[watch] mark failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "mark failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_tick(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<TickBody>,
) -> impl IntoResponse {
    if b.media_id.is_empty() || b.duration <= 0.0 || b.position <= 0.0 {
        return Json(serde_json::json!({ "ok": true })).into_response();
    }
    let db = state.db.lock().await;
    if let Err(e) = db.add_watch_seconds(&auth.id, WATCH_TICK_SECONDS) {
        eprintln!("[watch] tick failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "tick failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_continue(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let raw = {
        let db = state.db.lock().await;
        match db.list_continue_watching(&auth.id, 12) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("[watch] continue list failed: {e}");
                return err(StatusCode::INTERNAL_SERVER_ERROR, "list failed");
            }
        }
    };
    let mut items = Vec::with_capacity(raw.len());
    for item in raw {
        let stream_id = item.episode_id.as_deref().unwrap_or(&item.media_id);
        let Some(path) = crate::stream::resolve_file(&state, stream_id).await else {
            continue;
        };
        if tokio::fs::metadata(path).await.is_ok() {
            items.push(item);
        }
    }
    let tmdb_key = state.tmdb_key().await;
    if tmdb_key.is_empty() {
        return Json(items).into_response();
    }
    let mut out = items;
    let mut set = tokio::task::JoinSet::new();
    for (i, item) in out.iter().enumerate() {
        if item.episode_still_url.is_some() {
            continue;
        }
        if let (Some(tid), Some(s), Some(e)) =
            (item.tmdb_id, item.episode_season, item.episode_number)
        {
            let key = tmdb_key.clone();
            let ep_id = item.episode_id.clone();
            set.spawn(async move {
                let eps = crate::tmdb::season(&key, tid, s, "en-US")
                    .await
                    .ok()
                    .flatten()?;
                let still = eps.into_iter().find(|x| x.episode_number == e)?.still_url?;
                Some((i, ep_id, still))
            });
        }
    }
    let mut fetched = Vec::new();
    while let Some(res) = set.join_next().await {
        if let Ok(Some((i, ep_id, still))) = res {
            out[i].episode_still_url = Some(still.clone());
            if let Some(eid) = ep_id {
                fetched.push((eid, still));
            }
        }
    }
    if !fetched.is_empty() {
        let db = state.db.lock().await;
        for (eid, url) in fetched {
            let _ = db.update_episode_still(&eid, &url);
        }
    }
    Json(out).into_response()
}

async fn handle_summary(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.list_progress_summary(&auth.id) {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            eprintln!("[watch] summary failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

async fn handle_dismiss(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(media_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    if let Err(e) = db.dismiss_progress(&auth.id, &media_id) {
        eprintln!("[watch] dismiss failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "dismiss failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_list_for_media(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(media_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.list_progress_for_media(&auth.id, &media_id) {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            eprintln!("[watch] list-for-media failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
