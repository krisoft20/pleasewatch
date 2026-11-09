use crate::{middleware::AuthUser, models::ApiError, AppState};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

const KINDS: &[&str] = &["book", "movie", "tv", "manga"];
const COVER_PREFIXES: &[&str] = &[
    "/",
    "data:image/",
    "https://image.tmdb.org/",
    "https://covers.openlibrary.org/",
];

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/shares", get(handle_inbox).post(handle_send))
        .route("/api/shares/users", get(handle_users))
        .route("/api/shares/seen", post(handle_seen))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct SendBody {
    to: String,
    kind: String,
    key: String,
    title: String,
    cover_url: Option<String>,
    note: Option<String>,
}

fn fail(code: StatusCode, msg: &str) -> Response {
    (code, Json(ApiError { error: msg.into() })).into_response()
}

async fn handle_inbox(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let db = state.db.lock().await;
    match db.list_shares(&auth.id) {
        Ok(items) => {
            let unread = items.iter().filter(|s| !s.seen).count();
            Json(serde_json::json!({ "items": items, "unread": unread })).into_response()
        }
        Err(e) => {
            eprintln!("[shares] inbox failed: {e}");
            fail(StatusCode::INTERNAL_SERVER_ERROR, "inbox failed")
        }
    }
}

async fn handle_users(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let db = state.db.lock().await;
    let users = match db.list_share_targets(&auth.id) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[shares] user list failed: {e}");
            return fail(StatusCode::INTERNAL_SERVER_ERROR, "list failed");
        }
    };
    let out: Vec<_> = users
        .into_iter()
        .map(|(id, username)| serde_json::json!({ "id": id, "username": username }))
        .collect();
    Json(out).into_response()
}

async fn handle_send(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<SendBody>,
) -> Response {
    if !KINDS.contains(&b.kind.as_str()) {
        return fail(StatusCode::BAD_REQUEST, "bad kind");
    }
    let key = b.key.trim();
    let title = b.title.trim();
    if key.is_empty() || key.len() > 64 || title.is_empty() || title.chars().count() > 300 {
        return fail(StatusCode::BAD_REQUEST, "bad key or title");
    }
    if b.to == auth.id {
        return fail(StatusCode::BAD_REQUEST, "can't share with yourself");
    }
    let cover = b
        .cover_url
        .as_deref()
        .filter(|c| c.len() <= 4096 && !c.starts_with("//"))
        .filter(|c| COVER_PREFIXES.iter().any(|p| c.starts_with(p)));
    let note = b.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.chars().count() > 200) {
        return fail(StatusCode::BAD_REQUEST, "note too long");
    }

    let db = state.db.lock().await;
    match db.list_share_targets(&auth.id) {
        Ok(targets) if targets.iter().any(|(id, _)| *id == b.to) => {}
        Ok(_) => return fail(StatusCode::NOT_FOUND, "no such user"),
        Err(e) => {
            eprintln!("[shares] target lookup failed: {e}");
            return fail(StatusCode::INTERNAL_SERVER_ERROR, "send failed");
        }
    }
    if let Err(e) = db.create_share(&auth.id, &b.to, &b.kind, key, title, cover, note) {
        eprintln!("[shares] insert failed: {e}");
        return fail(StatusCode::INTERNAL_SERVER_ERROR, "send failed");
    }
    println!("[shares] {} sent {} '{}'", auth.username, b.kind, title);
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_seen(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let n = db.mark_shares_seen(&auth.id).unwrap_or(0);
    Json(serde_json::json!({ "ok": true, "marked": n }))
}
