use crate::AppState;
use axum::{
    body::Body,
    extract::{Multipart, Path as AxPath, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

pub fn detect_lang_from_file(path: &Path) -> Option<&'static str> {
    let raw = std::fs::read_to_string(path).ok()?;
    detect_lang_from_text(&raw)
}

pub fn detect_lang_from_text(raw: &str) -> Option<&'static str> {
    let mut body = String::with_capacity(raw.len() / 2);
    for line in raw.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if l.starts_with("WEBVTT") {
            continue;
        }
        if l.starts_with("NOTE") {
            continue;
        }
        if l.starts_with("STYLE") {
            continue;
        }
        if l.contains("-->") {
            continue;
        }
        if l.chars().all(|c| {
            c.is_ascii_digit()
                || c == '.'
                || c == ':'
                || c == ','
                || c == ' '
                || c == '-'
                || c == '>'
        }) {
            continue;
        }
        body.push_str(&l.to_lowercase());
        body.push(' ');
    }

    let mut en = 0u32;
    let mut pl = 0u32;
    let mut total = 0u32;
    for word in body.split(|c: char| !c.is_alphabetic() && c != '\'') {
        if word.is_empty() {
            continue;
        }
        total += 1;
        if EN_MARKERS.contains(&word) {
            en += 1;
        } else if PL_MARKERS.contains(&word) {
            pl += 1;
        }
    }

    if total < 30 {
        return None;
    }

    if pl >= 4 && pl as f32 >= en as f32 * 1.5 {
        return Some("pol");
    }
    if en >= 4 && en as f32 >= pl as f32 * 1.5 {
        return Some("eng");
    }
    None
}

fn wyzie_source_name(code: &str) -> Option<&'static str> {
    match code.to_lowercase().as_str() {
        "bravo" => Some("Subf2m"),
        "charlie" => Some("OpenSubtitles"),
        "foxtrot" => Some("Jimaku"),
        "india" => Some("YIFY"),
        "juliet" => Some("Ajatt-Tools"),
        "lima" => Some("IndexSubtitle"),
        "mike" => Some("Mike"),
        "november" => Some("November"),
        _ => None,
    }
}

const EN_MARKERS: &[&str] = &[
    "the", "and", "you", "that", "have", "for", "this", "with", "but", "not", "what", "your",
    "they", "from", "would", "there", "could", "about", "their", "which", "she", "him", "his",
    "her", "who", "why", "how", "when", "are", "was", "were", "been",
];

const PL_MARKERS: &[&str] = &[
    "się",
    "jest",
    "może",
    "tylko",
    "czy",
    "być",
    "tego",
    "jeszcze",
    "który",
    "która",
    "tutaj",
    "wszystko",
    "naprawdę",
    "powiedzieć",
    "wiem",
    "robić",
    "dlaczego",
    "musisz",
    "jak",
    "nie",
    "tak",
    "ale",
    "więc",
    "żeby",
    "gdzie",
    "kiedy",
    "muszę",
    "chcę",
    "wszyscy",
    "nigdy",
    "zawsze",
    "trochę",
    "bardzo",
    "moja",
    "swoje",
    "miałem",
];

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/subtitles/{owner_id}", get(list))
        .route("/api/subtitle/{id}", get(serve).delete(remove))
        .route("/api/subtitle/upload", post(upload))
        .route("/api/subtitle/{id}/sync", post(sync_ffsubsync))
        .route("/api/subtitle/{id}/sync_alass", post(sync_alass))
        .route("/api/subtitle/{id}/sync_whisper", post(sync_whisper))
        .route("/api/subtitles/search/{owner_id}", get(search_wyzie))
        .route("/api/subtitles/translate/{owner_id}", post(ai_translate))
        .route("/api/subtitle/fetch", post(fetch_from_url))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

async fn list(
    State(state): State<Arc<AppState>>,
    AxPath(owner_id): AxPath<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.list_subtitles_for_owner(&owner_id) {
        Ok(mut subs) => {
            for s in subs.iter_mut() {
                if crate::lang::looks_like_raw_code(&s.label)
                    && (s.label.eq_ignore_ascii_case(&s.language)
                        || crate::lang::lang_name(&s.label) != s.label)
                {
                    let lang_for_lookup = if !s.language.is_empty() {
                        &s.language
                    } else {
                        &s.label
                    };
                    s.label = crate::lang::lang_name(lang_for_lookup);
                }
            }
            Json(subs).into_response()
        }
        Err(e) => {
            crate::pe!("[subs] list failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

async fn upload(State(state): State<Arc<AppState>>, mut multipart: Multipart) -> Response {
    let mut owner_id: Option<String> = None;
    let mut language = String::from("und");
    let mut label = String::new();
    let mut is_default = false;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "owner_id" => owner_id = field.text().await.ok(),
            "language" => {
                if let Ok(v) = field.text().await {
                    language = v;
                }
            }
            "label" => {
                if let Ok(v) = field.text().await {
                    label = v;
                }
            }
            "is_default" => {
                if let Ok(v) = field.text().await {
                    is_default = v == "true" || v == "1";
                }
            }
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                file_bytes = field.bytes().await.ok().map(|b| b.to_vec());
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let Some(owner_id) = owner_id.filter(|s| !s.is_empty()) else {
        return (StatusCode::BAD_REQUEST, "owner_id required").into_response();
    };
    let Some(bytes) = file_bytes.filter(|b| !b.is_empty()) else {
        return (StatusCode::BAD_REQUEST, "file required").into_response();
    };

    let src_ext = filename
        .as_deref()
        .and_then(|n| n.rsplit('.').next())
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| matches!(s.as_str(), "srt" | "vtt" | "ass" | "ssa"))
        .unwrap_or_else(|| "srt".into());

    let target_ext = if src_ext == "srt" {
        "vtt"
    } else {
        src_ext.as_str()
    }
    .to_string();

    if label.is_empty() {
        label = lang_label(&language).to_string();
    }

    let sub_id = uuid::Uuid::new_v4().to_string();
    let upload_dir = std::path::Path::new(&state.media_root).join("uploads");
    if let Err(e) = std::fs::create_dir_all(&upload_dir) {
        crate::pe!("[subs] mkdir uploads: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "mkdir failed").into_response();
    }
    let staging = upload_dir.join(format!("{sub_id}.in.{src_ext}"));
    if let Err(e) = std::fs::write(&staging, &bytes) {
        crate::pe!("[subs] staging write: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    let dest = upload_dir.join(format!("{sub_id}.{target_ext}"));
    let ok = crate::ffmpeg::convert_sub(&staging, &dest, &language);
    let _ = std::fs::remove_file(&staging);
    if !ok {
        return (StatusCode::UNPROCESSABLE_ENTITY, "convert failed").into_response();
    }

    let sub = crate::models::Subtitle {
        id: sub_id,
        owner_id,
        language,
        label,
        format: target_ext,
        file_path: dest.to_string_lossy().into_owned(),
        is_default,
        media_id: None,
    };
    let db = state.db.lock().await;
    if let Err(e) = db.create_subtitle(&sub) {
        crate::pe!("[subs] db insert: {e}");
        let _ = std::fs::remove_file(&dest);
        return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
    }
    Json(sub).into_response()
}

async fn remove(State(state): State<Arc<AppState>>, AxPath(id): AxPath<String>) -> Response {
    let path = {
        let db = state.db.lock().await;
        match db.delete_subtitle(&id) {
            Ok(p) => p,
            Err(e) => {
                crate::pe!("[subs] delete failed: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
            }
        }
    };
    let Some(rel) = path else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let p = if std::path::Path::new(&rel).is_absolute() {
        std::path::PathBuf::from(&rel)
    } else {
        std::path::Path::new(&state.media_root).join(&rel)
    };
    let _ = std::fs::remove_file(&p);
    StatusCode::NO_CONTENT.into_response()
}

fn wyzie_encoding_for_lang(lang: &str) -> &'static str {
    match lang {
        "pol" | "pl" | "ces" | "cze" | "cs" | "slk" | "slo" | "sk" | "hun" | "hu" | "hrv"
        | "hr" | "scc" | "ron" | "rum" | "ro" => "Windows-1250",
        "rus" | "ru" | "ukr" | "uk" | "bul" | "bg" | "srp" | "sr" | "bel" | "be" => "Windows-1251",
        "ell" | "gre" | "el" => "Windows-1253",
        "tur" | "tr" => "Windows-1254",
        "heb" | "he" | "iw" => "Windows-1255",
        "ara" | "ar" => "Windows-1256",
        "tha" | "th" => "Windows-874",
        "vie" | "vi" => "Windows-1258",
        "jpn" | "ja" => "Shift_JIS",
        "kor" | "ko" => "EUC-KR",
        "chi" | "zho" | "zh" => "GB18030",
        _ => "UTF-8",
    }
}

fn rewrite_wyzie_encoding(url: &str, lang: &str) -> String {
    if url.contains("/translate") || url.contains("target=") {
        return url.to_string();
    }
    let enc = wyzie_encoding_for_lang(lang);
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, q),
        None => return format!("{url}?encoding={enc}"),
    };
    let mut kept: Vec<String> = query
        .split('&')
        .filter(|p| !p.starts_with("encoding="))
        .map(|p| p.to_string())
        .collect();
    kept.push(format!("encoding={enc}"));
    format!("{base}?{}", kept.join("&"))
}

pub fn lang_label(code: &str) -> &'static str {
    match code.to_ascii_lowercase().as_str() {
        "eng" | "en" => "English",
        "pol" | "pl" => "Polish",
        "ger" | "de" | "deu" => "German",
        "fre" | "fr" | "fra" => "French",
        "spa" | "es" => "Spanish",
        "ita" | "it" => "Italian",
        "por" | "pt" => "Portuguese",
        "rus" | "ru" => "Russian",
        "jpn" | "ja" => "Japanese",
        "kor" | "ko" => "Korean",
        "chi" | "zho" | "zh" => "Chinese",
        "ara" | "ar" => "Arabic",
        "tur" | "tr" => "Turkish",
        "dut" | "nld" | "nl" => "Dutch",
        "swe" | "sv" => "Swedish",
        "nor" | "nob" | "nno" | "no" => "Norwegian",
        "dan" | "da" => "Danish",
        "fin" | "fi" => "Finnish",
        "ces" | "cze" | "cs" => "Czech",
        "slk" | "slo" | "sk" => "Slovak",
        "slv" | "sl" => "Slovenian",
        "hun" | "hu" => "Hungarian",
        "rom" | "ron" | "rum" | "ro" => "Romanian",
        "bul" | "bg" => "Bulgarian",
        "hrv" | "hr" => "Croatian",
        "srp" | "sr" => "Serbian",
        "ukr" | "uk" => "Ukrainian",
        "gre" | "ell" | "el" => "Greek",
        "heb" | "he" => "Hebrew",
        "hin" | "hi" => "Hindi",
        "tha" | "th" => "Thai",
        "vie" | "vi" => "Vietnamese",
        "ind" | "id" => "Indonesian",
        "msa" | "may" | "ms" => "Malay",
        "tgl" | "fil" => "Filipino",
        "cat" | "ca" => "Catalan",
        "baq" | "eus" | "eu" => "Basque",
        "glg" | "gl" => "Galician",
        "fas" | "per" | "fa" => "Persian",
        "est" | "et" => "Estonian",
        "lav" | "lv" => "Latvian",
        "lit" | "lt" => "Lithuanian",
        "aze" | "az" => "Azerbaijani",
        "ben" | "bn" => "Bengali",
        "tam" | "ta" => "Tamil",
        "tel" | "te" => "Telugu",
        "mal" | "ml" => "Malayalam",
        "mar" | "mr" => "Marathi",
        "urd" | "ur" => "Urdu",
        "und" => "Unknown",
        _ => "Unknown",
    }
}

pub fn pretty_label(code: &str, raw_label: &str) -> String {
    let base = lang_label(code);
    let raw = raw_label.trim();

    if raw.is_empty() {
        return if base == "Unknown" {
            "subtitle".to_string()
        } else {
            base.to_string()
        };
    }

    let raw_lower = raw.to_lowercase();
    if matches!(raw_lower.as_str(), "unknown" | "und" | "undefined") {
        return if base == "Unknown" {
            "subtitle".into()
        } else {
            base.into()
        };
    }

    if matches!(code.to_ascii_lowercase().as_str(), "chi" | "zho" | "zh") {
        if raw_lower.contains("simplif") {
            return "Simplified Chinese".into();
        }
        if raw_lower.contains("traditi") {
            return "Traditional Chinese".into();
        }
    }

    if base != "Unknown" {
        if raw_lower.contains("sdh") || raw_lower.contains("hearing") {
            return format!("{base} (SDH)");
        }
        if raw_lower == "cc" || raw_lower.starts_with("cc ") || raw_lower.ends_with(" cc") {
            return format!("{base} (CC)");
        }
        if raw_lower.contains("forced") {
            return format!("{base} (forced)");
        }
        if raw_lower.contains(&base.to_lowercase()) {
            return raw.to_string();
        }
        return format!("{base}, {raw}");
    }

    if raw_lower.contains("sdh") || raw_lower.contains("hearing") {
        return "English (SDH)".into();
    }
    if raw_lower == "cc" {
        return "English (CC)".into();
    }

    raw.to_string()
}

async fn sync_ffsubsync(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> Response {
    run_sync_tool(&state, &id, SyncTool::Ffsubsync).await
}

async fn sync_alass(State(state): State<Arc<AppState>>, AxPath(id): AxPath<String>) -> Response {
    run_sync_tool(&state, &id, SyncTool::Alass).await
}

async fn sync_whisper(State(state): State<Arc<AppState>>, AxPath(id): AxPath<String>) -> Response {
    run_sync_tool(&state, &id, SyncTool::Whisper).await
}

#[derive(Clone, Copy)]
enum SyncTool {
    Ffsubsync,
    Alass,
    Whisper,
}

impl SyncTool {
    fn bin(&self) -> &'static str {
        match self {
            SyncTool::Ffsubsync => "ffsubsync",
            SyncTool::Alass => "alass",
            SyncTool::Whisper => "pw-whisper-sync",
        }
    }
    fn tag(&self) -> &'static str {
        match self {
            SyncTool::Ffsubsync => "ffsubsync",
            SyncTool::Alass => "alass",
            SyncTool::Whisper => "whisper",
        }
    }
    fn needs_srt_input(&self) -> bool {
        matches!(self, SyncTool::Alass | SyncTool::Whisper)
    }
}

async fn run_sync_tool(state: &Arc<AppState>, id: &str, tool: SyncTool) -> Response {
    let (owner_id, sub_path) = {
        let db = state.db.lock().await;
        match db.find_subtitle_owner(id) {
            Ok(Some(v)) => v,
            Ok(None) => return (StatusCode::NOT_FOUND, "subtitle not found").into_response(),
            Err(e) => {
                crate::pe!("[subs] sync lookup: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
            }
        }
    };

    let video_path = match resolve_video(state, &owner_id).await {
        Some(p) => p,
        None => return (StatusCode::CONFLICT, "video file not found").into_response(),
    };

    let abs_sub = absolutise(&state.media_root, &sub_path);
    if !abs_sub.exists() {
        return (StatusCode::NOT_FOUND, "subtitle file missing on disk").into_response();
    }

    let tag = tool.tag();
    let bin = tool.bin();

    let dest_ext = abs_sub
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("vtt")
        .to_string();

    let mut cleanup: Vec<std::path::PathBuf> = Vec::new();
    let input_for_tool = if tool.needs_srt_input() && dest_ext != "srt" {
        let tmp_in = abs_sub.with_extension(format!("sync.{}.in.srt", tag));
        if !crate::ffmpeg::convert_sub(&abs_sub, &tmp_in, "und") {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "pre-convert to srt failed",
            )
                .into_response();
        }
        cleanup.push(tmp_in.clone());
        tmp_in
    } else {
        abs_sub.clone()
    };

    let tmp = abs_sub.with_extension(format!("sync.{}.tmp", tag));

    let bin_owned = bin.to_string();
    let video_clone = video_path.clone();
    let sub_clone = input_for_tool.clone();
    let tmp_clone = tmp.clone();
    let result = tokio::task::spawn_blocking(move || match tool {
        SyncTool::Ffsubsync => std::process::Command::new(&bin_owned)
            .arg(&video_clone)
            .arg("-i")
            .arg(&sub_clone)
            .arg("-o")
            .arg(&tmp_clone)
            .output(),
        SyncTool::Alass | SyncTool::Whisper => std::process::Command::new(&bin_owned)
            .arg(&video_clone)
            .arg(&sub_clone)
            .arg(&tmp_clone)
            .output(),
    })
    .await;

    let drop_tmps = |extra: &[&std::path::Path]| {
        for p in &cleanup {
            let _ = std::fs::remove_file(p);
        }
        for p in extra {
            let _ = std::fs::remove_file(p);
        }
    };

    let out = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            crate::pe!("[subs] {tag} spawn: {e}");
            drop_tmps(&[]);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("{tag} not installed"),
            )
                .into_response();
        }
        Err(e) => {
            crate::pe!("[subs] {tag} join: {e}");
            drop_tmps(&[]);
            return (StatusCode::INTERNAL_SERVER_ERROR, "join error").into_response();
        }
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        crate::pe!("[subs] {tag} fail: {stderr}");
        drop_tmps(&[&tmp]);
        return (StatusCode::UNPROCESSABLE_ENTITY, format!("{tag} failed")).into_response();
    }

    if std::fs::metadata(&tmp)
        .map(|m| m.len() < 32)
        .unwrap_or(true)
    {
        drop_tmps(&[&tmp]);
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "sync produced empty output",
        )
            .into_response();
    }

    let final_dest = abs_sub.clone();

    if dest_ext == "srt" {
        if let Err(e) = std::fs::rename(&tmp, &final_dest) {
            crate::pe!("[subs] {tag} rename: {e}");
            drop_tmps(&[&tmp]);
            return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
        }
    } else {
        let ok = crate::ffmpeg::convert_sub(&tmp, &final_dest, "und");
        drop_tmps(&[&tmp]);
        if !ok {
            return (StatusCode::INTERNAL_SERVER_ERROR, "post-convert failed").into_response();
        }
    }

    drop_tmps(&[]);
    crate::pi!("[subs] {tag} ok: {}", final_dest.display());
    StatusCode::NO_CONTENT.into_response()
}

async fn resolve_video(state: &Arc<AppState>, owner_id: &str) -> Option<std::path::PathBuf> {
    let db = state.db.lock().await;
    if let Ok(Some(ep)) = db.find_episode_by_id(owner_id) {
        if let Some(p) = ep.file_path {
            return Some(absolutise(&state.media_root, &p));
        }
    }
    if let Ok(Some(m)) = db.find_media_by_id(owner_id) {
        if let Some(p) = m.file_path {
            return Some(absolutise(&state.media_root, &p));
        }
    }
    None
}

fn absolutise(root: &str, p: &str) -> std::path::PathBuf {
    if std::path::Path::new(p).is_absolute() {
        std::path::PathBuf::from(p)
    } else {
        std::path::Path::new(root).join(p)
    }
}

async fn serve(State(state): State<Arc<AppState>>, AxPath(id): AxPath<String>) -> Response {
    let path = {
        let db = state.db.lock().await;
        db.find_subtitle_path(&id).ok().flatten()
    };
    let Some(rel) = path else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    let p = if std::path::Path::new(&rel).is_absolute() {
        std::path::PathBuf::from(&rel)
    } else {
        std::path::Path::new(&state.media_root).join(&rel)
    };

    let bytes = match tokio::fs::read(&p).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::NOT_FOUND, "missing on disk").into_response(),
    };
    let ctype = match p.extension().and_then(|e| e.to_str()) {
        Some("vtt") => "text/vtt; charset=utf-8",
        Some("srt") => "application/x-subrip; charset=utf-8",
        Some("ass") | Some("ssa") => "text/x-ssa; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    };
    Response::builder()
        .header(header::CONTENT_TYPE, ctype)
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .body(Body::from(bytes))
        .unwrap()
}

#[derive(Deserialize)]
struct WyzieQuery {
    language: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct WyzieEntry {
    id: Option<String>,
    display: Option<String>,
    language: Option<String>,
    encoding: Option<String>,
    format: Option<String>,
    media: Option<String>,
    source: Option<String>,
    url: String,
    release: Option<String>,
    origin: Option<String>,
    #[serde(rename = "fileName")]
    file_name: Option<String>,
    #[serde(rename = "downloadCount", default)]
    download_count: Option<i64>,
    #[serde(rename = "matchedRelease")]
    matched_release: Option<String>,
    #[serde(rename = "isHearingImpaired", default)]
    hearing_impaired: bool,
}

pub async fn auto_fetch_for_owner(
    state: &Arc<AppState>,
    owner_id: &str,
    tmdb_id: i64,
    season: Option<i32>,
    episode: Option<i32>,
    langs: &[&str],
) -> usize {
    let keys = state.wyzie_keys_rotated().await;
    if keys.is_empty() {
        crate::pe!("[subs] auto-fetch: no wyzie key");
        return 0;
    }

    let upload_dir = std::path::Path::new(&state.media_root).join("uploads");
    if let Err(e) = std::fs::create_dir_all(&upload_dir) {
        crate::pe!("[subs] auto-fetch mkdir: {e}");
        return 0;
    }

    let existing = {
        let db = state.db.lock().await;
        db.list_subtitles_for_owner(owner_id).unwrap_or_default()
    };
    let already_have = |lang: &str| existing.iter().any(|s| s.language == lang);

    let mut saved = 0usize;
    for (i, lang) in langs.iter().enumerate() {
        if already_have(lang) {
            continue;
        }

        let mut entries: Vec<WyzieEntry> =
            match wyzie_search(&keys, tmdb_id, season, episode, lang).await {
                Some(v) => v,
                None => continue,
            };
        entries.sort_by_key(|e| -e.download_count.unwrap_or(0));
        let candidates: Vec<WyzieEntry> = entries
            .iter()
            .filter(|e| !e.hearing_impaired)
            .chain(entries.iter().filter(|e| e.hearing_impaired))
            .take(5)
            .cloned()
            .collect();
        if candidates.is_empty() {
            crate::pi!("[subs] auto-fetch {lang}: no entries for tmdb={tmdb_id}");
            continue;
        }

        let cli = reqwest::Client::new();
        let mut got: Option<(Vec<u8>, WyzieEntry)> = None;
        for (cand_idx, pick) in candidates.iter().enumerate() {
            let fetch_url = rewrite_wyzie_encoding(&pick.url, lang);
            match cli.get(&fetch_url).send().await {
                Ok(r) => {
                    let status = r.status();
                    if !status.is_success() {
                        crate::pe!(
                            "[subs] auto-fetch {lang} cand {cand_idx}: HTTP {status}, trying next"
                        );
                        continue;
                    }
                    match r.bytes().await {
                        Ok(b) if !b.is_empty() => {
                            got = Some((b.to_vec(), pick.clone()));
                            break;
                        }
                        Ok(_) => {
                            crate::pe!("[subs] auto-fetch {lang} cand {cand_idx}: empty body");
                            continue;
                        }
                        Err(e) => {
                            crate::pe!("[subs] auto-fetch {lang} cand {cand_idx} body: {e}");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    crate::pe!("[subs] auto-fetch {lang} cand {cand_idx} req: {e}");
                    continue;
                }
            }
        }
        let (bytes, pick) = match got {
            Some(v) => v,
            None => {
                crate::pe!(
                    "[subs] auto-fetch {lang}: all {} candidates failed",
                    candidates.len()
                );
                continue;
            }
        };

        let raw_fmt = pick
            .format
            .as_deref()
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "srt" | "vtt" | "ass" | "ssa"))
            .unwrap_or_else(|| "srt".into());

        let sub_id = uuid::Uuid::new_v4().to_string();
        let staging = upload_dir.join(format!("{sub_id}.in.{raw_fmt}"));
        if std::fs::write(&staging, &bytes).is_err() {
            continue;
        }

        let dest = upload_dir.join(format!("{sub_id}.vtt"));
        let ok = crate::ffmpeg::convert_sub(&staging, &dest, lang);
        let _ = std::fs::remove_file(&staging);
        if !ok {
            continue;
        }

        let sub = crate::models::Subtitle {
            id: sub_id,
            owner_id: owner_id.to_string(),
            language: (*lang).into(),
            label: match pick.source.as_deref().and_then(wyzie_source_name) {
                Some(src) => format!("{} (auto, {src})", lang_label(lang)),
                None => format!("{} (auto)", lang_label(lang)),
            },
            format: "vtt".into(),
            file_path: dest.to_string_lossy().into_owned(),
            is_default: i == 0,
            media_id: None,
        };
        let db = state.db.lock().await;
        if let Err(e) = db.create_subtitle(&sub) {
            crate::pe!("[subs] auto-fetch db: {e}");
            let _ = std::fs::remove_file(&dest);
            continue;
        }
        drop(db);
        crate::pi!("[subs] auto-fetched {lang} for owner={owner_id}");

        let synced = sync_fetched(state, owner_id, &dest, lang).await;
        if synced {
            crate::pi!("[subs] {lang} synced ok");
        } else {
            crate::pe!("[subs] {lang} sync skipped/failed, keeping unsynced");
        }
        saved += 1;
    }
    saved
}

async fn sync_fetched(
    state: &Arc<AppState>,
    owner_id: &str,
    fetched: &std::path::Path,
    lang: &str,
) -> bool {
    if let Some(reference) = pick_reference(state, owner_id, fetched).await {
        crate::pi!("[sync] {lang} via alass against {}", reference.display());
        if run_alass(&reference, fetched).await {
            return true;
        }
        crate::pe!("[sync] alass failed, falling back to ffsubsync");
    }

    let video = match resolve_video(state, owner_id).await {
        Some(v) => v,
        None => {
            crate::pe!("[sync] no video file for owner {owner_id}");
            return false;
        }
    };
    crate::pi!("[sync] {lang} via ffsubsync against {}", video.display());
    run_ffsubsync(&video, fetched).await
}

async fn pick_reference(
    state: &Arc<AppState>,
    owner_id: &str,
    exclude: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let exclude_str = exclude.to_string_lossy().into_owned();
    let subs = {
        let db = state.db.lock().await;
        db.list_subtitles_for_owner(owner_id).ok()?
    };
    for s in subs {
        if s.label.contains("(auto)") {
            continue;
        }
        if s.file_path == exclude_str {
            continue;
        }
        let abs = absolutise(&state.media_root, &s.file_path);
        if abs.exists() {
            return Some(abs);
        }
    }
    None
}

async fn run_alass(reference: &std::path::Path, target: &std::path::Path) -> bool {
    let tmp_in = target.with_extension("alass.in.srt");
    if !crate::ffmpeg::convert_sub(target, &tmp_in, "und") {
        return false;
    }
    let ref_srt = target.with_extension("alass.ref.srt");
    if !crate::ffmpeg::convert_sub(reference, &ref_srt, "und") {
        let _ = std::fs::remove_file(&tmp_in);
        return false;
    }
    let tmp_out = target.with_extension("alass.out.srt");

    let ref_size = std::fs::metadata(&ref_srt).map(|m| m.len()).unwrap_or(0);
    let in_size = std::fs::metadata(&tmp_in).map(|m| m.len()).unwrap_or(0);
    crate::pi!(
        "[alass] ref={} ({} bytes) in={} ({} bytes)",
        ref_srt.display(),
        ref_size,
        tmp_in.display(),
        in_size
    );

    let ref_owned = ref_srt.clone();
    let in_owned = tmp_in.clone();
    let out_owned = tmp_out.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("alass");
        cmd.stdin(std::process::Stdio::null())
            .arg(&ref_owned)
            .arg(&in_owned)
            .arg(&out_owned);
        crate::ffmpeg::throttled(cmd)
    })
    .await;

    let _ = std::fs::remove_file(&tmp_in);
    let _ = std::fs::remove_file(&ref_srt);

    if let Ok(Ok(o)) = &result {
        let stderr = String::from_utf8_lossy(&o.stderr);
        for line in stderr.lines() {
            let line = line.trim();
            if line.is_empty() || line.contains("/s ") || line.contains('%') {
                continue;
            }
            crate::pe!("[alass:err] {line}");
        }
        let stdout = String::from_utf8_lossy(&o.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if !line.is_empty() {
                crate::pi!("[alass:out] {line}");
            }
        }
        if !o.status.success() {
            crate::pe!("[alass] exit {}", o.status);
        }
    } else if let Ok(Err(e)) = &result {
        crate::pe!("[alass] spawn: {e}");
    }

    let ok = matches!(&result, Ok(Ok(o)) if o.status.success())
        && crate::ffmpeg::convert_sub(&tmp_out, target, "und");
    let _ = std::fs::remove_file(&tmp_out);
    ok
}

async fn run_ffsubsync(video: &std::path::Path, target: &std::path::Path) -> bool {
    let tmp_in = target.with_extension("ffsubsync.in.srt");
    if !crate::ffmpeg::convert_sub(target, &tmp_in, "und") {
        return false;
    }
    let tmp_out = target.with_extension("ffsubsync.out.srt");

    let video_owned = video.to_path_buf();
    let in_owned = tmp_in.clone();
    let out_owned = tmp_out.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("ffsubsync");
        cmd.arg(&video_owned)
            .arg("-i")
            .arg(&in_owned)
            .arg("-o")
            .arg(&out_owned);
        crate::ffmpeg::throttled(cmd)
    })
    .await;

    let _ = std::fs::remove_file(&tmp_in);

    if let Ok(Ok(o)) = &result {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if !line.trim().is_empty() {
                crate::pi!("[ffsubsync] {line}");
            }
        }
        for line in String::from_utf8_lossy(&o.stderr).lines() {
            if !line.trim().is_empty() {
                crate::pe!("[ffsubsync] {line}");
            }
        }
        if !o.status.success() {
            crate::pe!("[ffsubsync] exit {}", o.status);
        }
    } else if let Ok(Err(e)) = &result {
        crate::pe!("[ffsubsync] spawn: {e}");
    }

    let ok = matches!(&result, Ok(Ok(o)) if o.status.success())
        && crate::ffmpeg::convert_sub(&tmp_out, target, "und");
    let _ = std::fs::remove_file(&tmp_out);
    ok
}

type WyzieCacheKey = (i64, Option<i32>, Option<i32>, String);
struct WyzieCacheEntry {
    entries: Vec<WyzieEntry>,
    at: std::time::Instant,
}
static WYZIE_CACHE: std::sync::OnceLock<
    tokio::sync::Mutex<std::collections::HashMap<WyzieCacheKey, WyzieCacheEntry>>,
> = std::sync::OnceLock::new();
const WYZIE_CACHE_TTL_SECS: u64 = 3600;

fn wyzie_cache(
) -> &'static tokio::sync::Mutex<std::collections::HashMap<WyzieCacheKey, WyzieCacheEntry>> {
    WYZIE_CACHE.get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()))
}

async fn wyzie_search(
    keys: &[String],
    tmdb_id: i64,
    season: Option<i32>,
    episode: Option<i32>,
    language: &str,
) -> Option<Vec<WyzieEntry>> {
    if keys.is_empty() {
        return None;
    }

    let cache_key: WyzieCacheKey = (tmdb_id, season, episode, language.to_string());
    {
        let mut cache = wyzie_cache().lock().await;
        if let Some(entry) = cache.get(&cache_key) {
            if entry.at.elapsed().as_secs() < WYZIE_CACHE_TTL_SECS && !entry.entries.is_empty() {
                crate::pi!("[wyzie] cache hit tmdb={tmdb_id} s={season:?} e={episode:?} lang={language} ({} entries, age={}s)", entry.entries.len(), entry.at.elapsed().as_secs());
                return Some(entry.entries.clone());
            }
            cache.remove(&cache_key);
        }
    }

    let mut base: Vec<(String, String)> = vec![
        ("id".into(), tmdb_id.to_string()),
        ("source".into(), "all".into()),
    ];
    if !language.is_empty() {
        base.push(("language".into(), language.into()));
    }
    if let (Some(s), Some(e)) = (season, episode) {
        base.push(("season".into(), s.to_string()));
        base.push(("episode".into(), e.to_string()));
    }
    let client = reqwest::Client::builder()
        .user_agent("curl/8.5.0")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    for (idx, k) in keys.iter().enumerate() {
        let mut params = Vec::with_capacity(base.len() + 1);
        params.push(("key".into(), k.clone()));
        params.extend(base.clone());
        let req = client
            .get("https://sub.wyzie.io/search")
            .query(&params)
            .header("Accept", "*/*");
        let final_url = req
            .try_clone()
            .and_then(|b| b.build().ok())
            .map(|r| r.url().to_string())
            .unwrap_or_default();
        crate::pi!("[wyzie] GET {final_url}");
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                crate::pe!("[wyzie] key #{idx} search req: {e}");
                continue;
            }
        };
        let status = resp.status();
        if status.as_u16() == 401 || status.as_u16() == 403 || status.as_u16() == 429 {
            crate::pe!("[wyzie] key #{idx} {language}: {status}, trying next");
            continue;
        }
        if status.as_u16() == 400 || status.as_u16() == 404 {
            let body = resp.text().await.unwrap_or_default();
            if body.to_lowercase().contains("no subtitles") {
                crate::pi!("[wyzie] {language} tmdb={tmdb_id}: empty result (not cached, will retry on next search)");
                return Some(Vec::new());
            }
            crate::pe!(
                "[wyzie] key #{idx} {language}: {status} body: {}",
                body.chars().take(200).collect::<String>()
            );
            continue;
        }
        if !status.is_success() {
            crate::pe!("[wyzie] key #{idx} {language}: {status}");
            continue;
        }
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                crate::pe!("[wyzie] key #{idx} body read: {e}");
                continue;
            }
        };
        crate::pi!(
            "[wyzie] key #{idx} {language}: 200 OK, body len={}",
            body.len()
        );
        if body.len() < 500 {
            crate::pi!("[wyzie] body: {}", body);
        }
        match serde_json::from_str::<Vec<WyzieEntry>>(&body) {
            Ok(v) => {
                if v.is_empty() {
                    crate::pi!(
                        "[wyzie] {language} tmdb={tmdb_id}: 200 OK but 0 entries (not cached)"
                    );
                } else {
                    let mut cache = wyzie_cache().lock().await;
                    cache.insert(
                        cache_key,
                        WyzieCacheEntry {
                            entries: v.clone(),
                            at: std::time::Instant::now(),
                        },
                    );
                    crate::pi!(
                        "[wyzie] {language} tmdb={tmdb_id}: cached {} entries",
                        v.len()
                    );
                }
                return Some(v);
            }
            Err(e) => {
                crate::pe!("[wyzie] key #{idx} {language} parse: {e}");
                continue;
            }
        }
    }
    crate::pe!("[wyzie] all keys exhausted for {language}");
    None
}

async fn search_wyzie(
    State(state): State<Arc<AppState>>,
    AxPath(owner_id): AxPath<String>,
    Query(q): Query<WyzieQuery>,
) -> Response {
    let keys = state.wyzie_keys_rotated().await;
    if keys.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "wyzie key not configured (set WYZIE_API_KEY, claim at https://sub.wyzie.io/redeem)",
        )
            .into_response();
    }

    let (tmdb_id, season, episode) = match resolve_wyzie_target(&state, &owner_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return (StatusCode::NOT_FOUND, "no tmdb id for owner").into_response(),
        Err(e) => {
            crate::pe!("[subs] wyzie target lookup: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    let lang = q
        .language
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    match wyzie_search(&keys, tmdb_id, season, episode, lang).await {
        Some(entries) => {
            crate::pi!(
                "[subs] wyzie returned {} entries for tmdb_id={tmdb_id}",
                entries.len()
            );
            Json(entries).into_response()
        }
        None => (StatusCode::BAD_GATEWAY, "wyzie keys exhausted").into_response(),
    }
}

#[derive(Deserialize)]
struct AiTranslateReq {
    target: String,
}

fn lang_full_name(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "en" | "eng" | "english" => "English",
        "pl" | "pol" | "polish" => "Polish",
        "de" | "ger" | "deu" | "german" => "German",
        "es" | "spa" | "spanish" => "Spanish",
        "fr" | "fre" | "fra" | "french" => "French",
        "it" | "ita" | "italian" => "Italian",
        "pt" | "por" | "portuguese" => "Portuguese",
        "ru" | "rus" | "russian" => "Russian",
        "ja" | "jpn" | "japanese" => "Japanese",
        "nl" | "dut" | "nld" | "dutch" => "Dutch",
        "sv" | "swe" | "swedish" => "Swedish",
        _ => "English",
    }
}

fn lang_code_canon(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "en" | "eng" | "english" => "eng",
        "pl" | "pol" | "polish" => "pol",
        "de" | "ger" | "deu" | "german" => "ger",
        "es" | "spa" | "spanish" => "spa",
        "fr" | "fre" | "fra" | "french" => "fre",
        "it" | "ita" | "italian" => "ita",
        "pt" | "por" | "portuguese" => "por",
        "ru" | "rus" | "russian" => "rus",
        "ja" | "jpn" | "japanese" => "jpn",
        "nl" | "dut" | "nld" | "dutch" => "dut",
        "sv" | "swe" | "swedish" => "swe",
        _ => "und",
    }
}

async fn ai_translate(
    State(state): State<Arc<AppState>>,
    AxPath(owner_id): AxPath<String>,
    Json(req): Json<AiTranslateReq>,
) -> Response {
    let keys = state.wyzie_keys_rotated().await;
    if keys.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE, "wyzie key not configured").into_response();
    }

    let (tmdb_id, season, episode) = match resolve_wyzie_target(&state, &owner_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return (StatusCode::NOT_FOUND, "no tmdb id for owner").into_response(),
        Err(e) => {
            crate::pe!("[subs] translate target lookup: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    let target_full = lang_full_name(&req.target);
    let target_code = lang_code_canon(&req.target);
    crate::pi!("[subs] ai-translate {owner_id} -> {target_full} (tmdb_id={tmdb_id} s={season:?} e={episode:?})");

    let mut base: Vec<(String, String)> = vec![
        ("id".into(), tmdb_id.to_string()),
        ("target".into(), target_full.into()),
        ("source".into(), "all".into()),
    ];
    if let (Some(s), Some(e)) = (season, episode) {
        base.push(("season".into(), s.to_string()));
        base.push(("episode".into(), e.to_string()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut bytes: Vec<u8> = Vec::new();
    let mut raw_fmt = String::from("srt");
    let mut last_status: Option<u16> = None;
    const TRANSLATE_RETRY_MS: u64 = 2_000;
    const TRANSLATE_MAX_ATTEMPTS: usize = 90;

    'outer: for (idx, k) in keys.iter().enumerate() {
        let mut p = vec![("key".into(), k.clone())];
        p.extend(base.clone());

        for attempt in 0..TRANSLATE_MAX_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(TRANSLATE_RETRY_MS)).await;
            }
            let resp = match client
                .get("https://sub.wyzie.io/translate")
                .query(&p)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    crate::pe!("[subs] translate key #{idx} attempt {attempt} req: {e}");
                    continue;
                }
            };
            let status = resp.status();
            last_status = Some(status.as_u16());
            if status.as_u16() == 401 || status.as_u16() == 403 || status.as_u16() == 429 {
                crate::pe!("[subs] translate key #{idx}: {status}, trying next key");
                continue 'outer;
            }
            if status.as_u16() == 404 {
                crate::pe!("[subs] translate {target_code}: wyzie has no source subs");
                return (
                    StatusCode::NOT_FOUND,
                    "wyzie has no source subs to translate",
                )
                    .into_response();
            }
            if matches!(status.as_u16(), 502 | 503 | 504) {
                if attempt == 0 || attempt % 10 == 9 {
                    crate::pe!(
                        "[subs] translate key #{idx} attempt {}/{}: {} (still waiting)",
                        attempt + 1,
                        TRANSLATE_MAX_ATTEMPTS,
                        status
                    );
                }
                continue;
            }
            if !status.is_success() {
                crate::pe!("[subs] translate key #{idx}: {status}, trying next key");
                continue 'outer;
            }
            let ctype = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let body = match resp.bytes().await {
                Ok(b) => b.to_vec(),
                Err(e) => {
                    crate::pe!("[subs] translate body: {e}");
                    continue;
                }
            };
            if body.is_empty() {
                crate::pe!("[subs] translate key #{idx}: empty body");
                continue;
            }
            if ctype.contains("application/json") || body.starts_with(b"{") {
                let txt = String::from_utf8_lossy(&body[..body.len().min(200)]).to_string();
                crate::pe!("[subs] translate key #{idx}: got json error: {txt}");
                continue 'outer;
            }
            if ctype.contains("vtt") || body.starts_with(b"WEBVTT") {
                raw_fmt = "vtt".into();
            } else if ctype.contains("ass") || body.windows(11).any(|w| w == b"[Script Info") {
                raw_fmt = "ass".into();
            } else {
                raw_fmt = "srt".into();
            }
            bytes = body;
            break 'outer;
        }
    }

    if bytes.is_empty() {
        let msg = format!("wyzie translate failed (last status: {:?})", last_status);
        return (StatusCode::BAD_GATEWAY, msg).into_response();
    }

    let upload_dir = std::path::Path::new(&state.media_root).join("uploads");
    let _ = std::fs::create_dir_all(&upload_dir);

    let sub_id = uuid::Uuid::new_v4().to_string();
    let staging = upload_dir.join(format!("{sub_id}.in.{raw_fmt}"));
    if std::fs::write(&staging, &bytes).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "staging write failed").into_response();
    }
    let dest = upload_dir.join(format!("{sub_id}.vtt"));
    let ok = crate::ffmpeg::convert_sub(&staging, &dest, target_code);
    let _ = std::fs::remove_file(&staging);
    if !ok {
        return (StatusCode::INTERNAL_SERVER_ERROR, "convert failed").into_response();
    }

    let label = format!("{} (AI)", lang_full_name(target_code));
    let sub = crate::models::Subtitle {
        id: sub_id,
        owner_id: owner_id.clone(),
        language: target_code.into(),
        label,
        format: "vtt".into(),
        file_path: dest.to_string_lossy().into_owned(),
        is_default: false,
        media_id: None,
    };
    {
        let db = state.db.lock().await;
        if let Err(e) = db.create_subtitle(&sub) {
            crate::pe!("[subs] translate db: {e}");
            let _ = std::fs::remove_file(&dest);
            return (StatusCode::INTERNAL_SERVER_ERROR, "db insert failed").into_response();
        }
    }
    crate::pi!("[subs] ai-translated {target_code} saved for owner={owner_id}, syncing...");

    let synced = sync_fetched(&state, &owner_id, &dest, target_code).await;
    if synced {
        crate::pi!("[subs] ai-translated {target_code} synced ok");
    } else {
        crate::pe!("[subs] ai-translated {target_code} sync skipped/failed, keeping unsynced");
    }

    (StatusCode::OK, Json(sub)).into_response()
}

async fn resolve_wyzie_target(
    state: &Arc<AppState>,
    owner_id: &str,
) -> Result<Option<(i64, Option<i32>, Option<i32>)>, rusqlite::Error> {
    let db = state.db.lock().await;
    if let Some(ep) = db.find_episode_by_id(owner_id)? {
        if let Some(parent) = db.find_media_by_id(&ep.media_id)? {
            return Ok(parent
                .tmdb_id
                .map(|t| (t, Some(ep.season), Some(ep.episode))));
        }
        return Ok(None);
    }
    if let Some(m) = db.find_media_by_id(owner_id)? {
        return Ok(m.tmdb_id.map(|t| (t, None, None)));
    }
    Ok(None)
}

#[derive(Deserialize)]
struct FetchReq {
    owner_id: String,
    url: String,
    language: Option<String>,
    label: Option<String>,
    format: Option<String>,
}

async fn fetch_from_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FetchReq>,
) -> Response {
    if body.owner_id.is_empty() || body.url.is_empty() {
        return (StatusCode::BAD_REQUEST, "owner_id and url required").into_response();
    }
    if !body.url.starts_with("http://") && !body.url.starts_with("https://") {
        return (StatusCode::BAD_REQUEST, "bad url").into_response();
    }

    let language = body
        .language
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "und".into());
    let fetch_url = if body.url.contains("sub.wyzie") {
        rewrite_wyzie_encoding(&body.url, &language)
    } else {
        body.url.clone()
    };
    crate::pi!("[subs] fetching {fetch_url}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let is_ai_translate = fetch_url.contains("/translate") || fetch_url.contains("target=");
    let (retry_delay_ms, max_attempts) = if is_ai_translate {
        (2_000u64, 90usize)
    } else {
        (1_500u64, 5usize)
    };
    let mut bytes: Vec<u8> = Vec::new();
    let mut last_err = String::new();
    for attempt in 0..max_attempts {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(retry_delay_ms)).await;
        }
        let resp = match client.get(&fetch_url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("{e}");
                crate::pe!("[subs] fetch attempt {attempt} req: {e}");
                continue;
            }
        };
        let status = resp.status();
        if matches!(status.as_u16(), 502 | 503 | 504) {
            last_err = format!("upstream {status}");
            if attempt == 0 || attempt % 10 == 9 {
                crate::pe!(
                    "[subs] fetch attempt {}/{}: {} (still waiting)",
                    attempt + 1,
                    max_attempts,
                    status
                );
            }
            continue;
        }
        if !status.is_success() {
            crate::pe!("[subs] fetch: {status}");
            return (StatusCode::BAD_GATEWAY, format!("upstream {status}")).into_response();
        }
        match resp.bytes().await {
            Ok(b) => {
                bytes = b.to_vec();
                break;
            }
            Err(e) => {
                last_err = format!("body: {e}");
                crate::pe!("[subs] fetch attempt {attempt} body: {e}");
                continue;
            }
        }
    }
    if bytes.is_empty() {
        return (StatusCode::BAD_GATEWAY, format!("fetch failed: {last_err}")).into_response();
    }

    let raw_format = body
        .format
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .or_else(|| {
            body.url
                .rsplit('.')
                .next()
                .map(|s| s.split('?').next().unwrap_or(s).to_ascii_lowercase())
        })
        .filter(|s| matches!(s.as_str(), "srt" | "vtt" | "ass" | "ssa"))
        .unwrap_or_else(|| "srt".into());

    let target_ext = match raw_format.as_str() {
        "vtt" | "ass" | "ssa" => raw_format.clone(),
        _ => "vtt".into(),
    };

    let label = pretty_label(&language, body.label.as_deref().unwrap_or(""));

    let sub_id = uuid::Uuid::new_v4().to_string();
    let upload_dir = std::path::Path::new(&state.media_root).join("uploads");
    if let Err(e) = std::fs::create_dir_all(&upload_dir) {
        crate::pe!("[subs] fetch mkdir: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "mkdir failed").into_response();
    }
    let staging = upload_dir.join(format!("{sub_id}.in.{raw_format}"));
    if let Err(e) = std::fs::write(&staging, &bytes) {
        crate::pe!("[subs] fetch staging write: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    let dest = upload_dir.join(format!("{sub_id}.{target_ext}"));
    let ok = crate::ffmpeg::convert_sub(&staging, &dest, &language);
    let _ = std::fs::remove_file(&staging);
    if !ok {
        return (StatusCode::UNPROCESSABLE_ENTITY, "convert failed").into_response();
    }

    if !sync_fetched(&state, &body.owner_id, &dest, &language).await {
        crate::pe!("[subs] sync failed for {sub_id}, keeping unsynced");
    }

    let sub = crate::models::Subtitle {
        id: sub_id,
        owner_id: body.owner_id,
        language,
        label,
        format: target_ext,
        file_path: dest.to_string_lossy().into_owned(),
        is_default: false,
        media_id: None,
    };
    let db = state.db.lock().await;
    if let Err(e) = db.create_subtitle(&sub) {
        crate::pe!("[subs] fetch db insert: {e}");
        let _ = std::fs::remove_file(&dest);
        return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
    }
    crate::pi!("[subs] fetched wyzie sub {} for {}", sub.id, sub.owner_id);
    Json(sub).into_response()
}
