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
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use std::net::SocketAddr;
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

    let hash = match crypto::hash_password(&body.password) {
        Ok(h) => h,
        Err(_) => return error_resp(StatusCode::INTERNAL_SERVER_ERROR, "hash failed"),
    };

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
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    {
        let mut limiter = state.login_limiter.lock().await;
        if !limiter.check(addr.ip()) {
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
            eprintln!(
                "[auth] login miss for '{}' from {}",
                body.username,
                addr.ip()
            );
            return error_resp(StatusCode::UNAUTHORIZED, "invalid credentials");
        }
    };

    if !crypto::verify_password(&body.password, &user.password_hash) {
        eprintln!(
            "[auth] bad password for '{}' from {}",
            user.username,
            addr.ip()
        );
        return error_resp(StatusCode::UNAUTHORIZED, "invalid credentials");
    }

    if user.is_pending() {
        return error_resp(StatusCode::FORBIDDEN, "account pending admin approval");
    }

    let token = crypto::new_session_token();
    if db.create_session(&token, &user.id).is_err() {
        return error_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not create session",
        );
    }

    {
        let mut limiter = state.login_limiter.lock().await;
        limiter.reset(addr.ip());
    }

    let cookie = Cookie::build(("token", token))
        .path("/")
        .http_only(true)
        .secure(cookie_secure())
        .same_site(SameSite::Strict)
        .max_age(time::Duration::days(3650))
        .build();

    let public: UserPublic = user.into();
    (jar.add(cookie), Json(LoginResponse { user: public })).into_response()
}

async fn handle_logout(State(state): State<Arc<AppState>>, jar: CookieJar) -> impl IntoResponse {
    if let Some(c) = jar.get("token") {
        let token = c.value().to_string();
        let db = state.db.lock().await;
        let _ = db.delete_session(&token);
    }

    let cookie = Cookie::build(("token", ""))
        .path("/")
        .http_only(true)
        .secure(cookie_secure())
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

fn cookie_secure() -> bool {
    std::env::var("PUBLIC_BASE_URL")
        .map(|url| url.trim().starts_with("https://"))
        .unwrap_or(false)
}
