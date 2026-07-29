use crate::middleware::AuthUser;
use crate::models::{ApiError, Clip};
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

const MAX_CLIP_SECS: f64 = 120.0;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/clips", post(handle_create).get(handle_list))
        .route("/api/clips/{id}", delete(handle_delete))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

pub fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/clips/{id}", get(handle_serve))
        .route("/clip/{id}", get(handle_embed))
}

#[derive(Deserialize)]
pub struct CreateClipBody {
    pub media_id: String,
    pub episode_id: Option<String>,
    pub start: f64,
    pub end: f64,
    pub subtitle_id: Option<String>,
}

async fn handle_create(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateClipBody>,
) -> Response {
    let duration = body.end - body.start;
    if duration <= 0.0 || duration > MAX_CLIP_SECS {
        return err_resp(StatusCode::BAD_REQUEST, "clip must be 0-120 seconds");
    }

    let stream_id = body.episode_id.as_deref().unwrap_or(&body.media_id);
    let file_rel = {
        let db = state.db.lock().await;
        if let Ok(Some(m)) = db.find_media_by_id(stream_id) {
            m.file_path
        } else if let Ok(Some(ep)) = db.find_episode_by_id(stream_id) {
            ep.file_path
        } else {
            None
        }
    };
    let Some(file_rel) = file_rel else {
        return err_resp(StatusCode::NOT_FOUND, "media not found");
    };
    let full = abs_media_path(&state.media_root, &file_rel);
    if !full.exists() {
        return err_resp(StatusCode::NOT_FOUND, "video file missing on disk");
    }

    let sub_path = if let Some(sid) = body.subtitle_id.as_deref() {
        let db = state.db.lock().await;
        match db.find_subtitle_by_id(sid) {
            Ok(Some(s)) => Some(s.file_path),
            _ => None,
        }
    } else {
        None
    };

    let clip_id = uuid::Uuid::new_v4().to_string();
    let clips_dir = PathBuf::from(&state.media_root).join("clips");
    if let Err(e) = std::fs::create_dir_all(&clips_dir) {
        eprintln!("[clips] mkdir: {e}");
        return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "mkdir failed");
    }
    let clip_path = clips_dir.join(format!("{clip_id}.mp4"));

    if let Err(e) = encode_clip(&full, &clip_path, body.start, duration, sub_path.as_deref()).await
    {
        eprintln!("[clips] encode failed: {e}");
        return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "encode failed");
    }

    let size = std::fs::metadata(&clip_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let rel = format!("clips/{clip_id}.mp4");

    let clip = Clip {
        id: clip_id.clone(),
        media_id: body.media_id,
        episode_id: body.episode_id,
        start_sec: body.start,
        end_sec: body.end,
        subtitle_id: body.subtitle_id,
        file_path: rel,
        file_size: Some(size),
        created_by: auth.id.clone(),
        created_at: String::new(),
    };
    {
        let db = state.db.lock().await;
        if let Err(e) = db.create_clip(&clip) {
            eprintln!("[clips] db insert: {e}");
            let _ = std::fs::remove_file(&clip_path);
            return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "db error");
        }
    }
    println!("[clips] created {clip_id} ({duration:.1}s, {size} bytes)");

    let resp = serde_json::json!({
        "id": clip_id,
        "url": format!("/api/clips/{clip_id}"),
        "share_url": format!("/clip/{clip_id}"),
        "start": body.start,
        "end": body.end,
        "duration": duration,
        "file_size": size,
    });
    Json(resp).into_response()
}

async fn encode_clip(
    src: &std::path::Path,
    dest: &std::path::Path,
    start: f64,
    duration: f64,
    sub_path: Option<&str>,
) -> Result<(), String> {
    let sub_tmp = if let Some(sp) = sub_path {
        let abs = std::path::Path::new(sp).to_path_buf();
        if !abs.exists() {
            None
        } else {
            let tmp = dest.with_extension("sub.vtt");
            let out = tokio::process::Command::new("ffmpeg")
                .args([
                    "-y",
                    "-ss",
                    &start.to_string(),
                    "-t",
                    &duration.to_string(),
                    "-i",
                ])
                .arg(&abs)
                .arg(&tmp)
                .output()
                .await
                .map_err(|e| format!("sub slice spawn: {e}"))?;
            if !out.status.success() {
                eprintln!(
                    "[clips] sub slice failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                None
            } else {
                Some(tmp)
            }
        }
    } else {
        None
    };

    let fast_seek = (start - 3.0).max(0.0);
    let slow_seek = start - fast_seek;

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.args(["-y", "-ss", &fast_seek.to_string(), "-i"])
        .arg(src)
        .args(["-ss", &slow_seek.to_string(), "-t", &duration.to_string()]);

    if let Some(ref tmp) = sub_tmp {
        let style = "FontName=Arial,Fontsize=18,Bold=1,\
                     PrimaryColour=&H00FFFFFF&,OutlineColour=&H00000000&,\
                     BorderStyle=1,Outline=1,Shadow=1,\
                     Alignment=2,MarginV=30";
        let filter = format!(
            "subtitles='{}':force_style='{style}'",
            tmp.to_string_lossy().replace('\\', "/").replace(':', "\\:")
        );
        cmd.args(["-vf", &filter]);
    }

    cmd.args([
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "20",
        "-profile:v",
        "high",
        "-level",
        "4.1",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
    ])
    .arg(dest);

    let out = cmd
        .output()
        .await
        .map_err(|e| format!("ffmpeg spawn: {e}"))?;
    if let Some(ref tmp) = sub_tmp {
        let _ = std::fs::remove_file(tmp);
    }
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail = stderr.lines().rev().take(8).collect::<Vec<_>>().join(" | ");
        return Err(tail);
    }
    Ok(())
}

async fn handle_list(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Response {
    if !auth.is_admin() {
        return err_resp(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;
    match db.list_clips(200) {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => {
            eprintln!("[clips] list: {e}");
            err_resp(StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

async fn handle_delete(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if !auth.is_admin() {
        return err_resp(StatusCode::FORBIDDEN, "admin only");
    }
    let db = state.db.lock().await;
    match db.delete_clip(&id) {
        Ok(Some(rel)) => {
            let abs = abs_media_path(&state.media_root, &rel);
            let _ = std::fs::remove_file(&abs);
            (StatusCode::NO_CONTENT).into_response()
        }
        Ok(None) => err_resp(StatusCode::NOT_FOUND, "not found"),
        Err(e) => {
            eprintln!("[clips] delete: {e}");
            err_resp(StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

async fn handle_serve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let clean = id.strip_suffix(".mp4").unwrap_or(&id);
    let rel = {
        let db = state.db.lock().await;
        match db.find_clip_by_id(clean) {
            Ok(Some(c)) => c.file_path,
            _ => return err_resp(StatusCode::NOT_FOUND, "not found"),
        }
    };
    let abs = abs_media_path(&state.media_root, &rel);
    if !abs.exists() {
        return err_resp(StatusCode::NOT_FOUND, "file missing");
    }
    serve_with_range(&abs, &headers).await
}

async fn handle_embed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let bot_markers = [
        "Discordbot",
        "Twitterbot",
        "Slackbot",
        "facebookexternalhit",
        "LinkedInBot",
        "WhatsApp",
        "TelegramBot",
    ];
    let is_bot = bot_markers.iter().any(|m| ua.contains(m));

    if !is_bot {
        let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".into());
        let index = std::path::Path::new(&static_dir).join("index.html");
        return match std::fs::read_to_string(&index) {
            Ok(html) => Html(html).into_response(),
            Err(_) => err_resp(StatusCode::INTERNAL_SERVER_ERROR, "no index.html"),
        };
    }

    let clip = {
        let db = state.db.lock().await;
        db.find_clip_by_id(&id).ok().flatten()
    };
    let Some(c) = clip else {
        return err_resp(StatusCode::NOT_FOUND, "clip not found");
    };

    let base = public_base(&headers);
    let video_url = format!("{base}/api/clips/{}", c.id);
    let dur = (c.end_sec - c.start_sec).round() as i64;

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta property="og:type" content="video.other" />
<meta property="og:title" content="pleasewatch - {dur}s clip" />
<meta property="og:video" content="{video_url}" />
<meta property="og:video:secure_url" content="{video_url}" />
<meta property="og:video:type" content="video/mp4" />
<meta property="og:video:width" content="1280" />
<meta property="og:video:height" content="720" />
<meta name="twitter:card" content="player" />
<meta name="twitter:player" content="{video_url}" />
<meta name="twitter:player:width" content="1280" />
<meta name="twitter:player:height" content="720" />
</head>
<body></body>
</html>"#
    );
    Html(html).into_response()
}

fn abs_media_path(root: &str, rel: &str) -> PathBuf {
    if std::path::Path::new(rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        std::path::Path::new(root).join(rel)
    }
}

fn public_base(headers: &HeaderMap) -> String {
    match std::env::var("PUBLIC_BASE_URL") {
        Ok(v) if !v.trim().is_empty() => return v.trim_end_matches('/').to_string(),
        _ => {}
    }
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:3000");
    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{host}")
}

async fn serve_with_range(path: &std::path::Path, headers: &HeaderMap) -> Response {
    use axum::http::HeaderValue;
    use tokio::io::AsyncSeekExt;

    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return err_resp(StatusCode::INTERNAL_SERVER_ERROR, "stat failed"),
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
            use tokio::io::AsyncReadExt;
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
                .header(header::CACHE_CONTROL, "public, max-age=86400, immutable")
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
        .header(header::CACHE_CONTROL, "public, max-age=86400, immutable")
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
        let parsed: u64 = b.parse().ok()?;
        parsed.min(total - 1)
    };
    if start > end || end >= total {
        return None;
    }
    Some((start, end))
}

fn err_resp(status: StatusCode, msg: &str) -> Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
