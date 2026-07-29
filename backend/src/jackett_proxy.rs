use crate::{middleware::AuthUser, AppState};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Extension, Router,
};
use std::sync::Arc;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/jackett", any(proxy))
        .route("/jackett/{*rest}", any(proxy))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

fn skip_header(name: &str) -> bool {
    matches!(
        name,
        "connection" | "host" | "content-length" | "transfer-encoding" | "upgrade"
    )
}

async fn proxy(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Response {
    if !auth.is_admin() {
        return (StatusCode::FORBIDDEN, "admin only").into_response();
    }

    let upstream = {
        let db = state.db.lock().await;
        db.get_setting("jackett_url")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| std::env::var("JACKETT_URL").unwrap_or_default())
    };
    if upstream.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE, "jackett not configured").into_response();
    }

    let path_q = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let target = format!("{}{}", upstream.trim_end_matches('/'), path_q);
    let method = req.method().clone();
    let in_headers = req.headers().clone();

    let body = match axum::body::to_bytes(req.into_body(), 64 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "body too large").into_response(),
    };

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("build proxy client");

    let mut rb = client.request(method, &target);
    for (n, v) in in_headers.iter() {
        if !skip_header(&n.as_str().to_ascii_lowercase()) {
            rb = rb.header(n, v);
        }
    }

    let upstream_resp = match rb.body(body.to_vec()).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[jackett-proxy] {e}");
            return (StatusCode::BAD_GATEWAY, "jackett unreachable").into_response();
        }
    };

    let status = upstream_resp.status();
    let mut out = Response::builder().status(status.as_u16());
    for (n, v) in upstream_resp.headers().iter() {
        if skip_header(&n.as_str().to_ascii_lowercase()) {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            HeaderName::from_bytes(n.as_str().as_bytes()),
            HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out = out.header(hn, hv);
        }
    }

    let bytes = upstream_resp.bytes().await.unwrap_or_default();
    out.body(Body::from(bytes)).expect("build response")
}
