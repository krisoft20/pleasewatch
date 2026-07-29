use crate::middleware::AuthUser;
use crate::models::ApiError;
use crate::AppState;
use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMsg {
    Play {
        time: f64,
    },
    Pause {
        time: f64,
    },
    Seek {
        time: f64,
    },
    Chat {
        name: String,
        message: String,
    },
    StateSync {
        playing: bool,
        time: f64,
        participants: u32,
    },
    UserJoined {
        name: String,
        participants: u32,
    },
    UserLeft {
        name: String,
        participants: u32,
    },
    EpisodeSwitch {
        episode_id: String,
    },
}

pub struct Session {
    playing: bool,
    current_time: f64,
    participants: u32,
    tx: broadcast::Sender<String>,
}

#[derive(Clone, Default)]
pub struct PartyManager {
    inner: Arc<RwLock<HashMap<String, Session>>>,
}

impl PartyManager {
    pub fn new() -> Self {
        Self::default()
    }

    async fn ensure(&self, code: &str) {
        let mut g = self.inner.write().await;
        if !g.contains_key(code) {
            let (tx, _) = broadcast::channel(64);
            g.insert(
                code.to_string(),
                Session {
                    playing: false,
                    current_time: 0.0,
                    participants: 0,
                    tx,
                },
            );
        }
    }

    async fn join(&self, code: &str) -> Option<(broadcast::Receiver<String>, bool, f64, u32)> {
        let mut g = self.inner.write().await;
        let s = g.get_mut(code)?;
        s.participants += 1;
        Some((s.tx.subscribe(), s.playing, s.current_time, s.participants))
    }

    async fn leave(&self, code: &str) -> u32 {
        let mut g = self.inner.write().await;
        let Some(s) = g.get_mut(code) else {
            return 0;
        };
        s.participants = s.participants.saturating_sub(1);
        let n = s.participants;
        if n == 0 {
            g.remove(code);
        }
        n
    }

    async fn dispatch(&self, code: &str, msg: &WsMsg) {
        let mut g = self.inner.write().await;
        let Some(s) = g.get_mut(code) else {
            return;
        };
        match msg {
            WsMsg::Play { time } => {
                s.playing = true;
                s.current_time = *time;
            }
            WsMsg::Pause { time } => {
                s.playing = false;
                s.current_time = *time;
            }
            WsMsg::Seek { time } => {
                s.current_time = *time;
            }
            _ => {}
        }
        if let Ok(j) = serde_json::to_string(msg) {
            let _ = s.tx.send(j);
        }
    }
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/party", post(handle_create))
        .route("/api/party/{code}/episode", post(handle_set_episode))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

pub fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/party/{code}", get(handle_info))
        .route("/api/party/{code}/stream", get(handle_stream))
        .route("/api/party/{code}/subs", get(handle_subs))
        .route("/api/party/{code}/subs/{sub_id}", get(handle_sub_file))
        .route("/api/ws/party/{code}", get(handle_ws))
}

#[derive(Deserialize)]
struct CreateBody {
    media_id: String,
    episode_id: Option<String>,
}

#[derive(Serialize)]
struct CreateResp {
    code: String,
}

async fn handle_create(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateBody>,
) -> Response {
    let id = uuid::Uuid::new_v4().to_string();
    let code = id
        .replace('-', "")
        .chars()
        .take(6)
        .collect::<String>()
        .to_uppercase();
    {
        let db = state.db.lock().await;
        if let Err(e) = db.create_party(
            &id,
            &code,
            &auth.id,
            &body.media_id,
            body.episode_id.as_deref(),
        ) {
            eprintln!("[party] db insert: {e}");
            return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    }
    state.party.ensure(&code).await;
    println!(
        "[party] created {code} host={} media={}",
        auth.id, body.media_id
    );
    Json(CreateResp { code }).into_response()
}

#[derive(Deserialize)]
struct SetEpisodeBody {
    episode_id: String,
}

async fn handle_set_episode(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(code): Path<String>,
    Json(body): Json<SetEpisodeBody>,
) -> Response {
    let host = {
        let db = state.db.lock().await;
        match db.find_party_by_code(&code) {
            Ok(Some((_, _, _, host))) => host,
            _ => return err_resp(StatusCode::NOT_FOUND, "session not found"),
        }
    };
    if host != auth.id {
        return err_resp(StatusCode::FORBIDDEN, "host only");
    }
    {
        let db = state.db.lock().await;
        if let Err(e) = db.update_party_episode(&code, &body.episode_id) {
            eprintln!("[party] update ep: {e}");
            return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    }
    let msg = WsMsg::EpisodeSwitch {
        episode_id: body.episode_id.clone(),
    };
    state.party.dispatch(&code, &msg).await;
    println!("[party] {code} ep -> {}", body.episode_id);
    Json(serde_json::json!({ "ok": true })).into_response()
}

#[derive(Serialize)]
struct InfoResp {
    code: String,
    media_id: String,
    episode_id: Option<String>,
    media_title: String,
    media_type: String,
    poster_url: Option<String>,
    episode_title: Option<String>,
    episode_season: Option<i32>,
    episode_number: Option<i32>,
    participants: u32,
    stream_id: String,
}

async fn handle_info(State(state): State<Arc<AppState>>, Path(code): Path<String>) -> Response {
    let (sid, media_id, ep_id, _host) = {
        let db = state.db.lock().await;
        match db.find_party_by_code(&code) {
            Ok(Some(t)) => t,
            _ => return err_resp(StatusCode::NOT_FOUND, "session not found"),
        }
    };
    let _ = sid;

    let (media, ep) = {
        let db = state.db.lock().await;
        let lookup_id = ep_id.as_deref().unwrap_or(&media_id);
        let m = if let Ok(Some(m)) = db.find_media_by_id(&media_id) {
            Some(m)
        } else if let Ok(Some(ep)) = db.find_episode_by_id(&media_id) {
            db.find_media_by_id(&ep.media_id).ok().flatten()
        } else {
            None
        };
        let e = if ep_id.is_some() {
            db.find_episode_by_id(lookup_id).ok().flatten()
        } else {
            None
        };
        (m, e)
    };

    let Some(m) = media else {
        return err_resp(StatusCode::NOT_FOUND, "media not found");
    };
    let participants = {
        let g = state.party.inner.read().await;
        g.get(&code).map(|s| s.participants).unwrap_or(0)
    };

    let stream_id = ep_id.clone().unwrap_or_else(|| media_id.clone());
    let resp = InfoResp {
        code: code.clone(),
        media_id: media_id.clone(),
        episode_id: ep_id,
        media_title: m.title,
        media_type: m.media_type,
        poster_url: m.poster_url,
        episode_title: ep.as_ref().and_then(|e| e.title.clone()),
        episode_season: ep.as_ref().map(|e| e.season),
        episode_number: ep.as_ref().map(|e| e.episode),
        participants,
        stream_id,
    };
    Json(resp).into_response()
}

#[derive(Deserialize)]
struct WsQuery {
    name: Option<String>,
}

async fn handle_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Query(q): Query<WsQuery>,
) -> Response {
    let exists = {
        let db = state.db.lock().await;
        db.find_party_by_code(&code).ok().flatten().is_some()
    };
    if !exists {
        return err_resp(StatusCode::NOT_FOUND, "session not found");
    }
    let name = q.name.unwrap_or_else(|| "Guest".into());
    ws.on_upgrade(move |s| pump(s, state, code, name))
}

async fn pump(mut socket: WebSocket, state: Arc<AppState>, code: String, name: String) {
    state.party.ensure(&code).await;
    let Some((mut brx, playing, time, participants)) = state.party.join(&code).await else {
        let _ = socket.send(Message::Close(None)).await;
        return;
    };

    let sync = WsMsg::StateSync {
        playing,
        time,
        participants,
    };
    if let Ok(j) = serde_json::to_string(&sync) {
        let _ = socket.send(Message::Text(j.into())).await;
    }

    let join_msg = WsMsg::UserJoined {
        name: name.clone(),
        participants,
    };
    state.party.dispatch(&code, &join_msg).await;

    loop {
        tokio::select! {
            broadcast = brx.recv() => {
                let Ok(j) = broadcast else { continue; };
                if socket.send(Message::Text(j.into())).await.is_err() { break; }
            }
            client = socket.recv() => {
                match client {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(m) = serde_json::from_str::<WsMsg>(&t) {
                            match &m {
                                WsMsg::Play { .. } | WsMsg::Pause { .. } | WsMsg::Seek { .. } | WsMsg::Chat { .. } => {
                                    state.party.dispatch(&code, &m).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }

    let remaining = state.party.leave(&code).await;
    if remaining > 0 {
        let leave = WsMsg::UserLeft {
            name,
            participants: remaining,
        };
        state.party.dispatch(&code, &leave).await;
    } else {
        let id = {
            let db = state.db.lock().await;
            db.find_party_by_code(&code)
                .ok()
                .flatten()
                .map(|(id, _, _, _)| id)
        };
        if let Some(id) = id {
            let db = state.db.lock().await;
            let _ = db.deactivate_party(&id);
        }
    }
}

async fn handle_stream(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    headers: HeaderMap,
) -> Response {
    let stream_id = {
        let db = state.db.lock().await;
        match db.find_party_by_code(&code) {
            Ok(Some((_, media_id, ep_id, _))) => ep_id.unwrap_or(media_id),
            _ => return err_resp(StatusCode::NOT_FOUND, "session not found"),
        }
    };

    let file_rel = {
        let db = state.db.lock().await;
        if let Ok(Some(m)) = db.find_media_by_id(&stream_id) {
            m.file_path
        } else if let Ok(Some(ep)) = db.find_episode_by_id(&stream_id) {
            ep.file_path
        } else {
            None
        }
    };
    let Some(rel) = file_rel else {
        return err_resp(StatusCode::NOT_FOUND, "no file");
    };
    let abs = abs_path(&state.media_root, &rel);
    if !abs.exists() {
        return err_resp(StatusCode::NOT_FOUND, "file missing");
    }

    serve_with_range(&abs, &headers).await
}

async fn handle_subs(State(state): State<Arc<AppState>>, Path(code): Path<String>) -> Response {
    let owner = {
        let db = state.db.lock().await;
        match db.find_party_by_code(&code) {
            Ok(Some((_, media_id, ep_id, _))) => ep_id.unwrap_or(media_id),
            _ => return err_resp(StatusCode::NOT_FOUND, "session not found"),
        }
    };
    let subs = {
        let db = state.db.lock().await;
        db.list_subtitles_for_owner(&owner).unwrap_or_default()
    };
    Json(subs).into_response()
}

async fn handle_sub_file(
    State(state): State<Arc<AppState>>,
    Path((code, sub_id)): Path<(String, String)>,
) -> Response {
    let ok = {
        let db = state.db.lock().await;
        db.find_party_by_code(&code).ok().flatten().is_some()
    };
    if !ok {
        return err_resp(StatusCode::NOT_FOUND, "session not found");
    }

    let sub = {
        let db = state.db.lock().await;
        db.find_subtitle_by_id(&sub_id).ok().flatten()
    };
    let Some(s) = sub else {
        return err_resp(StatusCode::NOT_FOUND, "sub not found");
    };

    let abs = abs_path(&state.media_root, &s.file_path);
    let data = match tokio::fs::read(&abs).await {
        Ok(d) => d,
        Err(_) => return err_resp(StatusCode::NOT_FOUND, "sub missing"),
    };
    let ct = match s.format.as_str() {
        "vtt" => "text/vtt; charset=utf-8",
        "ass" | "ssa" => "text/x-ssa; charset=utf-8",
        "srt" => "application/x-subrip; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ct)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(data))
        .unwrap()
}

fn abs_path(root: &str, rel: &str) -> PathBuf {
    if std::path::Path::new(rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        std::path::Path::new(root).join(rel)
    }
}

async fn serve_with_range(path: &std::path::Path, headers: &HeaderMap) -> Response {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "stat"),
    };
    let size = meta.len();
    let ctype = "video/mp4";

    if let Some(rh) = headers.get(header::RANGE).and_then(|h| h.to_str().ok()) {
        if let Some((start, end)) = parse_range(rh, size) {
            let chunk = end - start + 1;
            let mut file = match tokio::fs::File::open(path).await {
                Ok(f) => f,
                Err(_) => return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "open"),
            };
            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "seek");
            }
            let stream = tokio_util::io::ReaderStream::new(file.take(chunk));
            let body = Body::from_stream(stream);
            let cr = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
                .unwrap_or_else(|_| HeaderValue::from_static(""));
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, ctype)
                .header(header::CONTENT_LENGTH, chunk)
                .header(header::CONTENT_RANGE, cr)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "private, max-age=86400")
                .body(body)
                .unwrap();
        }
    }

    let file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "open"),
    };
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = Body::from_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ctype)
        .header(header::CONTENT_LENGTH, size)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=86400")
        .body(body)
        .unwrap()
}

fn parse_range(s: &str, total: u64) -> Option<(u64, u64)> {
    let rest = s.strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    let start: u64 = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        total.saturating_sub(n)
    } else {
        a.parse().ok()?
    };
    let end: u64 = if b.is_empty() {
        total - 1
    } else {
        let n: u64 = b.parse().ok()?;
        n.min(total - 1)
    };
    if start > end || end >= total {
        return None;
    }
    Some((start, end))
}

fn err_resp(status: StatusCode, msg: &str) -> Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
