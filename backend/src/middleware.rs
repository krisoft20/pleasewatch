use crate::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub role: String,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == crate::models::Role::Admin.as_str()
    }
}

pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = read_token(&jar, &req).ok_or(StatusCode::UNAUTHORIZED)?;

    let db = state.db.lock().await;
    let user = match db.find_user_by_session(&token) {
        Ok(Some(u)) => u,
        _ => return Err(StatusCode::UNAUTHORIZED),
    };
    drop(db);

    if user.is_pending() {
        return Err(StatusCode::FORBIDDEN);
    }

    req.extensions_mut().insert(AuthUser {
        id: user.id,
        username: user.username,
        role: user.role,
    });

    Ok(next.run(req).await)
}

fn read_token(jar: &CookieJar, req: &Request) -> Option<String> {
    if let Some(c) = jar.get("token") {
        return Some(c.value().to_string());
    }
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.to_string())
}
