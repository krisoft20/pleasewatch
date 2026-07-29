use crate::{
    crypto,
    middleware::AuthUser,
    models::{
        ApiError, ApiMessage, LoginRequest, LoginResponse, RegisterRequest, RegisterResponse, Role,
        UserPublic,
    },
    AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/api/auth/register", post(handle_register))
        .route("/api/auth/login", post(handle_login))
        .route("/api/auth/logout", post(handle_logout));

    let authed = Router::new()
        .route("/api/auth/me", get(handle_me))
        .route("/api/auth/check-admin", get(handle_check_admin))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ));

    public.merge(authed)
}

async fn handle_register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    if body.username.len() < 3 || body.username.len() > 32 {
        return error_resp(StatusCode::BAD_REQUEST, "username must be 3-32 chars");
    }
    if body.password.len() < 4 {
        return error_resp(StatusCode::BAD_REQUEST, "password must be at least 4 chars");
    }
    // TODO: actual email validation, this just checks for @
    if !body.email.contains('@') {
        return error_resp(StatusCode::BAD_REQUEST, "invalid email");
    }

    let db = state.db.lock().await;

    if db
        .find_user_by_username(&body.username)
        .ok()
        .flatten()
        .is_some()
    {
        return (
            StatusCode::CONFLICT,
            Json(ApiError {
                error: "username already taken".into(),
            }),
        )
            .into_response();
    }

    drop(db);

    let password = body.password.clone();
    let hash = match tokio::task::spawn_blocking(move || crypto::hash_password(&password)).await {
        Ok(Ok(h)) => h,
        Ok(Err(_)) => return error_resp(StatusCode::INTERNAL_SERVER_ERROR, "hash failed"),
        Err(e) => {
            eprintln!("[auth] password worker failed: {e}");
            return error_resp(StatusCode::INTERNAL_SERVER_ERROR, "hash failed");
        }
    };

    let db = state.db.lock().await;
    if db
        .find_user_by_username(&body.username)
        .ok()
        .flatten()
        .is_some()
    {
        return (
            StatusCode::CONFLICT,
            Json(ApiError {
                error: "username already taken".into(),
            }),
        )
            .into_response();
    }

    let role = match db.count_users().unwrap_or(1) {
        0 => Role::Admin,
        _ => Role::Pending,
    };

    let id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = db.create_user(&id, &body.username, &body.email, &hash, role.as_str()) {
        eprintln!("[auth] create_user failed: {e}");
        return error_resp(StatusCode::INTERNAL_SERVER_ERROR, "could not create user");
    }

    let message = match role {
        Role::Admin => "admin account created, you can sign in now.",
        _ => "account created, waiting for admin approval",
    };

    (
        StatusCode::CREATED,
        Json(RegisterResponse {
            message: message.into(),
            role: role.as_str().into(),
        }),
    )
        .into_response()
}

async fn handle_login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers, addr.ip(), state.trust_proxy);

    {
        let mut limiter = state.login_limiter.lock().await;
        if !limiter.check(ip) {
            return error_resp(
                StatusCode::TOO_MANY_REQUESTS,
                "too many attempts, try again later",
            );
        }
    }

    let db = state.db.lock().await;

    let user = match db.find_user_by_username(&body.username).ok().flatten() {
        Some(u) => u,
        None => {
            eprintln!("[auth] login miss for '{}' from {}", body.username, ip);
            return error_resp(StatusCode::UNAUTHORIZED, "invalid credentials");
        }
    };

    drop(db);

    let password = body.password;
    let hash = user.password_hash.clone();
    let password_ok = match tokio::task::spawn_blocking(move || {
        crypto::verify_password(&password, &hash)
    })
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("[auth] password worker failed: {e}");
            return error_resp(StatusCode::INTERNAL_SERVER_ERROR, "login failed");
        }
    };

    if !password_ok {
        eprintln!("[auth] bad password for '{}' from {}", user.username, ip);
        return error_resp(StatusCode::UNAUTHORIZED, "invalid credentials");
    }

    if user.is_pending() {
        return error_resp(StatusCode::FORBIDDEN, "account pending admin approval");
    }

    let token = crypto::new_session_token();
    {
        let db = state.db.lock().await;
        if db.create_session(&token, &user.id).is_err() {
            return error_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not create session",
            );
        }
    }

    {
        let mut limiter = state.login_limiter.lock().await;
        limiter.reset(ip);
    }

    let cookie = Cookie::build(("token", token))
        .path("/")
        .http_only(true)
        .secure(cookie_secure(&headers))
        .same_site(SameSite::Strict)
        .max_age(time::Duration::days(3650))
        .build();

    let public: UserPublic = user.into();
    (jar.add(cookie), Json(LoginResponse { user: public })).into_response()
}

async fn handle_logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(c) = jar.get("token") {
        let token = c.value().to_string();
        let db = state.db.lock().await;
        let _ = db.delete_session(&token);
    }

    let cookie = Cookie::build(("token", ""))
        .path("/")
        .http_only(true)
        .secure(cookie_secure(&headers))
        .max_age(time::Duration::ZERO)
        .build();

    (
        jar.remove(cookie),
        Json(ApiMessage {
            message: "logged out".into(),
        }),
    )
        .into_response()
}

async fn handle_me(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;

    match db.find_user_by_id(&auth.id).ok().flatten() {
        Some(u) => Json(UserPublic::from(u)).into_response(),
        None => error_resp(StatusCode::NOT_FOUND, "user not found"),
    }
}

async fn handle_check_admin(Extension(auth): Extension<AuthUser>) -> StatusCode {
    if auth.is_admin() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::FORBIDDEN
    }
}

fn error_resp(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}

fn client_ip(headers: &HeaderMap, peer: IpAddr, trust_proxy: bool) -> IpAddr {
    if !trust_proxy {
        return peer;
    }

    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(peer)
}

fn cookie_secure(headers: &HeaderMap) -> bool {
    if headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        == Some("https")
    {
        return true;
    }

    std::env::var("PUBLIC_BASE_URL")
        .map(|url| url.trim().starts_with("https://"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn secure_cookie_uses_forwarded_https() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        assert!(cookie_secure(&headers));
    }

    #[test]
    fn forwarded_ip_needs_trusted_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.8, 172.18.0.3"),
        );
        let peer = "172.18.0.4".parse().unwrap();
        let forwarded: IpAddr = "203.0.113.8".parse().unwrap();

        assert_eq!(client_ip(&headers, peer, true), forwarded);
        assert_eq!(client_ip(&headers, peer, false), peer);
    }

    #[test]
    fn invalid_forwarded_ip_uses_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("not-an-ip"));
        let peer = "172.18.0.4".parse().unwrap();

        assert_eq!(client_ip(&headers, peer, true), peer);
    }
}
