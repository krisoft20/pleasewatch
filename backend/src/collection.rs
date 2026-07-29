use crate::{db::CollectionError, middleware::AuthUser, models::ApiError, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

const KINDS: &[&str] = &["movie", "tv", "anime"];
const STATUSES: &[&str] = &["planned", "in_progress", "completed"];

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/collection", get(handle_list).post(handle_upsert))
        .route(
            "/api/collection/{kind}/{tmdb_id}",
            get(handle_get).patch(handle_patch).delete(handle_delete),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct ListQuery {
    kind: Option<String>,
    status: Option<String>,
    showcased: Option<bool>,
}

#[derive(Deserialize)]
struct UpsertBody {
    tmdb_id: i64,
    kind: String,
    title: String,
    year: Option<String>,
    poster_url: Option<String>,
    backdrop_url: Option<String>,
    status: String,
    showcased: Option<bool>,
}

#[derive(Deserialize)]
struct PatchBody {
    status: Option<String>,
    showcased: Option<bool>,
}

async fn handle_list(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> Response {
    if let Some(kind) = query.kind.as_deref() {
        if !valid_kind(kind) {
            return bad_kind();
        }
    }
    if let Some(status) = query.status.as_deref() {
        if !valid_status(status) {
            return bad_status();
        }
    }

    let db = state.db.lock().await;
    match db.list_collection(
        &auth.id,
        query.kind.as_deref(),
        query.status.as_deref(),
        query.showcased,
    ) {
        Ok(items) => Json(items).into_response(),
        Err(err) => {
            eprintln!("[collection] list failed: {err}");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

async fn handle_get(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
) -> Response {
    if !valid_kind(&kind) {
        return bad_kind();
    }
    if tmdb_id <= 0 {
        return api_error(StatusCode::BAD_REQUEST, "tmdb_id must be positive");
    }

    let db = state.db.lock().await;
    match db.get_collection_item(&auth.id, &kind, tmdb_id) {
        Ok(Some(item)) => Json(item).into_response(),
        Ok(None) => api_error(StatusCode::NOT_FOUND, "not in collection"),
        Err(err) => {
            eprintln!("[collection] get failed: {err}");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed")
        }
    }
}

async fn handle_upsert(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpsertBody>,
) -> Response {
    if body.tmdb_id <= 0 {
        return api_error(StatusCode::BAD_REQUEST, "tmdb_id must be positive");
    }
    if !valid_kind(&body.kind) {
        return bad_kind();
    }
    if !valid_status(&body.status) {
        return bad_status();
    }

    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > 300 {
        return api_error(StatusCode::BAD_REQUEST, "title must be 1-300 characters");
    }
    let year = match clean_optional(body.year, 16, "year") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let poster_url = match clean_optional(body.poster_url, 2048, "poster_url") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let backdrop_url = match clean_optional(body.backdrop_url, 2048, "backdrop_url") {
        Ok(value) => value,
        Err(response) => return response,
    };

    let db = state.db.lock().await;
    match db.upsert_collection_item(
        &auth.id,
        body.tmdb_id,
        &body.kind,
        title,
        year.as_deref(),
        poster_url.as_deref(),
        backdrop_url.as_deref(),
        &body.status,
        body.showcased,
    ) {
        Ok(item) => Json(item).into_response(),
        Err(err) => collection_error(err, "save failed"),
    }
}

async fn handle_patch(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
    Json(body): Json<PatchBody>,
) -> Response {
    if !valid_kind(&kind) {
        return bad_kind();
    }
    if tmdb_id <= 0 {
        return api_error(StatusCode::BAD_REQUEST, "tmdb_id must be positive");
    }
    if body.status.is_none() && body.showcased.is_none() {
        return api_error(StatusCode::BAD_REQUEST, "status or showcased is required");
    }
    if let Some(status) = body.status.as_deref() {
        if !valid_status(status) {
            return bad_status();
        }
    }

    let db = state.db.lock().await;
    match db.patch_collection_item(
        &auth.id,
        &kind,
        tmdb_id,
        body.status.as_deref(),
        body.showcased,
    ) {
        Ok(item) => Json(item).into_response(),
        Err(err) => collection_error(err, "save failed"),
    }
}

async fn handle_delete(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
) -> Response {
    if !valid_kind(&kind) {
        return bad_kind();
    }
    if tmdb_id <= 0 {
        return api_error(StatusCode::BAD_REQUEST, "tmdb_id must be positive");
    }

    let db = state.db.lock().await;
    match db.delete_collection_item(&auth.id, &kind, tmdb_id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => api_error(StatusCode::NOT_FOUND, "not in collection"),
        Err(err) => {
            eprintln!("[collection] delete failed: {err}");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "remove failed")
        }
    }
}

fn valid_kind(kind: &str) -> bool {
    KINDS.contains(&kind)
}

fn valid_status(status: &str) -> bool {
    STATUSES.contains(&status)
}

fn clean_optional(
    value: Option<String>,
    max_chars: usize,
    field: &str,
) -> Result<Option<String>, Response> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > max_chars {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            &format!("{field} is too long"),
        ));
    }
    Ok(Some(value.to_string()))
}

fn collection_error(err: CollectionError, fallback: &str) -> Response {
    match err {
        CollectionError::NotFound => api_error(StatusCode::NOT_FOUND, "not in collection"),
        CollectionError::ShowcaseLimit => {
            api_error(StatusCode::CONFLICT, "showcase is limited to five items")
        }
        CollectionError::ShowcaseRequiresCompletion => api_error(
            StatusCode::CONFLICT,
            "only completed items can be showcased",
        ),
        CollectionError::Database(err) => {
            eprintln!("[collection] {fallback}: {err}");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, fallback)
        }
    }
}

fn bad_kind() -> Response {
    api_error(
        StatusCode::BAD_REQUEST,
        "kind must be 'movie', 'tv', or 'anime'",
    )
}

fn bad_status() -> Response {
    api_error(
        StatusCode::BAD_REQUEST,
        "status must be 'planned', 'in_progress', or 'completed'",
    )
}

fn api_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ApiError {
            error: message.to_string(),
        }),
    )
        .into_response()
}
