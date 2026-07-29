use crate::AppState;
use axum::{
    body::Body,
    extract::{Path as AxPath, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Semaphore;

const OPEN_RANGE_CAP: u64 = 4 * 1024 * 1024;

static THUMB_SEM: tokio::sync::OnceCell<Semaphore> = tokio::sync::OnceCell::const_new();

async fn thumb_sem() -> &'static Semaphore {
    THUMB_SEM.get_or_init(|| async { Semaphore::new(2) }).await
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/stream/{id}", get(handle_stream))
        .route("/api/stream/{id}/audio-tracks", get(handle_audio_tracks))
        .route("/api/thumb/{id}", get(handle_thumb))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct StreamQuery {
    audio: Option<u32>,
}

async fn handle_stream(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Query(q): Query<StreamQuery>,
    headers: HeaderMap,
) -> Response {
    let path = match resolve_file(&state, &id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "no file").into_response(),
    };
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "file missing on disk").into_response();
    }

    if let Some(idx) = q.audio.filter(|n| *n > 0) {
        match ensure_audio_remux(&path, idx).await {
            Ok(cached) => return serve_with_range(&cached, &headers).await,
            Err(e) => {
                eprintln!("[stream] audio remux failed: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, "audio remux failed").into_response();
            }
        }
    }

    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let sibling = path.with_file_name(format!("{stem}_audio0.mp4"));
        if sibling.exists() {
            if audio_cache_fresh(&path, &sibling) {
                return serve_with_range(&sibling, &headers).await;
            }
            let _ = tokio::fs::remove_file(&sibling).await;
        }
    }

    serve_with_range(&path, &headers).await
}

static AUDIO_CACHE: OnceLock<Mutex<HashMap<String, Vec<AudioTrack>>>> = OnceLock::new();

#[derive(Clone, serde::Serialize)]
struct AudioTrack {
    index: u32,
    language: String,
    label: String,
    codec: String,
}

fn audio_track_label(n: usize, lang: &str, title: &str, codec: &str) -> String {
    let title_ascii = title.chars().any(|c| c.is_ascii_alphabetic());
    if !title.is_empty() && title_ascii && !crate::lang::looks_like_raw_code(title) {
        return title.to_string();
    }

    let name = lang_name(lang);
    let base = if name == "Unknown" {
        format!("audio {}", n + 1)
    } else {
        name
    };
    if codec.is_empty() {
        base
    } else {
        format!("{base} ({codec})")
    }
}

fn audio_track_lang(raw: &str, title: &str) -> String {
    crate::lang::canon_lang_code(raw)
        .map(str::to_string)
        .or_else(|| crate::lang::lang_code_from_label(title).map(str::to_string))
        .unwrap_or_else(|| {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
                "und".into()
            } else {
                trimmed.to_lowercase()
            }
        })
}

fn json_tag<'a>(tags: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    tags.as_object()?
        .iter()
        .find(|(k, v)| {
            k.eq_ignore_ascii_case(key) && v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)
        })
        .and_then(|(_, v)| v.as_str())
}

async fn handle_audio_tracks(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> Response {
    let path = match resolve_file(&state, &id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "no file").into_response(),
    };
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "file missing").into_response();
    }

    let cache = AUDIO_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cache_key = audio_cache_key(&path);
    if let Ok(g) = cache.lock() {
        if let Some(hit) = g.get(&cache_key) {
            return Json(hit.clone()).into_response();
        }
    }

    let out = match tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "a",
        ])
        .arg(&path)
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[stream] ffprobe spawn: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "probe failed").into_response();
        }
    };

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).unwrap_or_else(|_| serde_json::json!({"streams": []}));
    let mut tracks: Vec<AudioTrack> = Vec::new();
    if let Some(arr) = v.get("streams").and_then(|s| s.as_array()) {
        for (i, s) in arr.iter().enumerate() {
            let tags = s.get("tags").unwrap_or(&serde_json::Value::Null);
            let title = json_tag(tags, "title")
                .or_else(|| json_tag(tags, "track_name"))
                .unwrap_or("")
                .to_string();
            let raw_lang = json_tag(tags, "language")
                .or_else(|| json_tag(tags, "language_ietf"))
                .or_else(|| json_tag(tags, "language-ietf"))
                .unwrap_or("und");
            let lang = audio_track_lang(raw_lang, &title);
            let codec = s
                .get("codec_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let label = audio_track_label(i, &lang, &title, &codec);
            tracks.push(AudioTrack {
                index: i as u32,
                language: lang,
                label,
                codec,
            });
        }
    }
    if let Ok(mut g) = cache.lock() {
        g.insert(cache_key, tracks.clone());
    }
    Json(tracks).into_response()
}

#[derive(Deserialize)]
struct ThumbQuery {
    t: Option<i64>,
    v: Option<u32>,
}

async fn handle_thumb(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Query(q): Query<ThumbQuery>,
) -> Response {
    let ts = q.t.unwrap_or(0);
    let width = q.v.unwrap_or(480).clamp(180, 1280);
    match ensure_thumb(&state, &id, ts, width).await {
        Ok(data) => img_response(data),
        Err(ThumbErr::NoFile) => (StatusCode::NOT_FOUND, "no file").into_response(),
        Err(ThumbErr::Failed) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "thumb failed").into_response()
        }
    }
}

pub enum ThumbErr {
    NoFile,
    Failed,
}

pub async fn ensure_thumb(
    state: &Arc<AppState>,
    id: &str,
    ts: i64,
    width: u32,
) -> Result<Vec<u8>, ThumbErr> {
    let path = resolve_file(state, id).await.ok_or(ThumbErr::NoFile)?;
    if !path.exists() {
        return Err(ThumbErr::NoFile);
    }

    let cache_dir = std::path::Path::new(&state.media_root)
        .join("thumbs")
        .join(id);
    let cache_path = cache_dir.join(format!("{ts}_w{width}.jpg"));
    if cache_path.exists() {
        if let Ok(data) = tokio::fs::read(&cache_path).await {
            return Ok(data);
        }
    }

    let _permit = thumb_sem()
        .await
        .acquire()
        .await
        .map_err(|_| ThumbErr::Failed)?;

    if cache_path.exists() {
        if let Ok(data) = tokio::fs::read(&cache_path).await {
            return Ok(data);
        }
    }

    let scale = format!("scale={width}:-2");
    let out = tokio::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-analyzeduration",
            "500000",
            "-probesize",
            "500000",
            "-fflags",
            "+nobuffer+fastseek",
            "-ss",
            &ts.to_string(),
        ])
        .arg("-i")
        .arg(&path)
        .args([
            "-an",
            "-sn",
            "-dn",
            "-frames:v",
            "1",
            "-q:v",
            "5",
            "-vf",
            &scale,
            "-f",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .await
        .map_err(|_| ThumbErr::Failed)?;
    if !out.status.success() {
        return Err(ThumbErr::Failed);
    }
    let data = out.stdout;
    let dir_clone = cache_dir.clone();
    let path_clone = cache_path.clone();
    let data_clone = data.clone();
    tokio::spawn(async move {
        let _ = tokio::fs::create_dir_all(&dir_clone).await;
        let _ = tokio::fs::write(path_clone, data_clone).await;
    });
    Ok(data)
}

pub async fn default_audio_browser_safe(src: &std::path::Path) -> bool {
    let out = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "a:0",
        ])
        .arg(src)
        .output()
        .await;
    let Ok(o) = out else { return true };
    let codec = serde_json::from_slice::<serde_json::Value>(&o.stdout)
        .ok()
        .and_then(|v| v.get("streams").and_then(|s| s.as_array().cloned()))
        .and_then(|a| a.first().cloned())
        .and_then(|s| {
            s.get("codec_name")
                .and_then(|c| c.as_str().map(|x| x.to_string()))
        })
        .unwrap_or_default();
    matches!(codec.as_str(), "aac" | "mp3" | "opus" | "flac")
}

pub async fn audio_track_count(src: &std::path::Path) -> u32 {
    let out = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "a",
        ])
        .arg(src)
        .output()
        .await;
    let Ok(o) = out else { return 0 };
    let v: serde_json::Value =
        serde_json::from_slice(&o.stdout).unwrap_or_else(|_| serde_json::json!({"streams": []}));
    v.get("streams")
        .and_then(|s| s.as_array())
        .map(|a| a.len() as u32)
        .unwrap_or(0)
}

pub async fn ensure_audio_remux(src: &std::path::Path, idx: u32) -> Result<PathBuf, String> {
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let cached = src.with_file_name(format!("{stem}_audio{idx}.mp4"));
    if cached.exists() {
        if audio_cache_fresh(src, &cached) {
            return Ok(cached);
        }
        let _ = tokio::fs::remove_file(&cached).await;
    }

    let probe_out = tokio::process::Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_streams"])
        .arg(src)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let streams = serde_json::from_slice::<serde_json::Value>(&probe_out.stdout)
        .ok()
        .and_then(|v| v.get("streams").and_then(|s| s.as_array().cloned()))
        .unwrap_or_default();
    let codec = streams
        .iter()
        .filter(|s| s.get("codec_type").and_then(|t| t.as_str()) == Some("audio"))
        .nth(idx as usize)
        .and_then(|s| {
            s.get("codec_name")
                .and_then(|c| c.as_str().map(|x| x.to_string()))
        })
        .unwrap_or_default();
    let vcodec = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(|t| t.as_str()) == Some("video"))
        .and_then(|s| {
            s.get("codec_name")
                .and_then(|c| c.as_str().map(|x| x.to_string()))
        })
        .unwrap_or_default();
    let safe = matches!(codec.as_str(), "aac" | "mp3" | "opus" | "flac");
    let is_hevc = matches!(vcodec.as_str(), "hevc" | "h265");

    let src_owned = src.to_path_buf();
    let cached_owned = cached.clone();
    let codec_for_err = codec.clone();
    let out = tokio::task::spawn_blocking(move || {
        let threads = crate::ffmpeg::smart_threads().to_string();
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.args(["-y", "-threads", &threads, "-i"])
            .arg(&src_owned)
            .args([
                "-map",
                "0:v:0",
                "-map",
                &format!("0:a:{idx}"),
                "-c:v",
                "copy",
            ]);
        if is_hevc {
            cmd.args(["-tag:v", "hvc1"]);
        }
        if safe {
            cmd.args(["-c:a", "copy"]);
        } else {
            cmd.args([
                "-c:a", "aac", "-b:a", "192k", "-ac", "2", "-threads", &threads,
            ]);
        }
        cmd.args(["-sn", "-movflags", "+faststart"])
            .arg(&cached_owned);
        let source = src_owned.display().to_string();
        crate::ffmpeg::throttled_tracked(cmd, Some(("audio-alt-track", &source)))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if !out.status.success() {
        eprintln!("[ffmpeg] audio '{codec_for_err}' remux failed");
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(cached)
}

fn audio_cache_key(path: &Path) -> String {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return path.to_string_lossy().into_owned(),
    };
    let stamp = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}:{}:{stamp}", path.to_string_lossy(), meta.len())
}

fn audio_cache_fresh(src: &Path, cached: &Path) -> bool {
    let src_meta = match std::fs::metadata(src) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let cached_meta = match std::fs::metadata(cached) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if cached_meta.len() < 1_000_000 {
        return false;
    }
    match (src_meta.modified(), cached_meta.modified()) {
        (Ok(src_time), Ok(cached_time)) => cached_time >= src_time,
        _ => false,
    }
}

pub(crate) async fn resolve_file(state: &Arc<AppState>, id: &str) -> Option<PathBuf> {
    let rel = {
        let db = state.db.lock().await;
        if let Ok(Some(m)) = db.find_media_by_id(id) {
            m.file_path
        } else if let Ok(Some(ep)) = db.find_episode_by_id(id) {
            ep.file_path
        } else {
            None
        }
    };
    let rel = rel?;
    Some(if std::path::Path::new(&rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        std::path::Path::new(&state.media_root).join(rel)
    })
}

async fn serve_with_range(path: &std::path::Path, headers: &HeaderMap) -> Response {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "stat failed").into_response(),
    };
    let size = meta.len();
    let ctype = ctype_for(path);

    if let Some(rh) = headers.get(header::RANGE).and_then(|h| h.to_str().ok()) {
        if let Some((start, end)) = parse_range(rh, size) {
            return serve_partial(path, start, end, size, ctype).await;
        }
    }

    let file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "open failed").into_response(),
    };
    let stream = tokio_util::io::ReaderStream::with_capacity(file, 256 * 1024);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ctype)
        .header(header::CONTENT_LENGTH, size)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=86400, immutable")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "Range")
        .header(
            "Access-Control-Expose-Headers",
            "Content-Range, Content-Length, Accept-Ranges",
        )
        .body(body)
        .unwrap()
}

async fn serve_partial(
    path: &std::path::Path,
    start: u64,
    end: u64,
    size: u64,
    ctype: &str,
) -> Response {
    let chunk = end - start + 1;
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "open failed").into_response(),
    };
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "seek failed").into_response();
    }
    let stream = tokio_util::io::ReaderStream::with_capacity(file.take(chunk), 256 * 1024);
    let body = Body::from_stream(stream);

    let cr = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
        .unwrap_or_else(|_| HeaderValue::from_static(""));
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, ctype)
        .header(header::CONTENT_LENGTH, chunk)
        .header(header::CONTENT_RANGE, cr)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=86400, immutable")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "Range")
        .header(
            "Access-Control-Expose-Headers",
            "Content-Range, Content-Length, Accept-Ranges",
        )
        .body(body)
        .unwrap()
}

fn parse_range(raw: &str, size: u64) -> Option<(u64, u64)> {
    let s = raw.strip_prefix("bytes=")?;
    let (a, b) = s.split_once('-')?;
    let start: u64 = if a.is_empty() {
        let suffix: u64 = b.parse().ok()?;
        size.saturating_sub(suffix)
    } else {
        a.parse().ok()?
    };
    let end: u64 = if b.is_empty() {
        let cap = start.saturating_add(OPEN_RANGE_CAP).saturating_sub(1);
        cap.min(size.saturating_sub(1))
    } else {
        b.parse().ok()?
    };
    if start > end || start >= size {
        return None;
    }
    Some((start, end.min(size - 1)))
}

fn ctype_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn img_response(data: Vec<u8>) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=86400, immutable")
        .body(Body::from(data))
        .unwrap()
}

fn lang_name(code: &str) -> String {
    crate::lang::lang_name(code)
}
