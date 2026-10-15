use crate::party::WsMsg;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WINDOW: i64 = 60;
const STEP: i64 = 10;
const CACHE: &str = "/tmp/pw-claude";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/party/{code}/claude/next", get(next_batch))
        .route("/api/party/{code}/claude/frame/{name}", get(frame))
        .route("/api/party/{code}/claude/chat", post(chat))
}

struct Source {
    file: PathBuf,
    duration: Option<f64>,
    title: String,
    subs: Option<(String, PathBuf)>,
}

#[derive(Serialize)]
struct Cue {
    t: f64,
    text: String,
}

#[derive(Serialize)]
struct Frame {
    t: i64,
    url: Option<String>,
    similar: bool,
    subs: Vec<Cue>,
}

#[derive(Serialize)]
struct Batch {
    id: i64,
    from: i64,
    to: i64,
    frames: Vec<Frame>,
}

#[derive(Serialize)]
struct NextResp {
    state: &'static str,
    position: f64,
    duration: Option<f64>,
    title: String,
    subtitles: Option<String>,
    batch: Option<Batch>,
}

#[derive(Deserialize)]
struct NextQuery {
    after: Option<i64>,
    wait: Option<u64>,
    lang: Option<String>,
}

fn valid_code(code: &str) -> bool {
    (4..=12).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphanumeric())
}

async fn source(state: &AppState, code: &str, lang: &str) -> Option<Source> {
    let db = state.db.lock().await;
    let (_, media_id, ep_id, _) = db.find_party_by_code(code).ok()??;
    let owner = ep_id.unwrap_or(media_id);
    let (rel, duration, title) = match db.find_media_by_id(&owner).ok().flatten() {
        Some(m) => (m.file_path?, m.duration, m.title),
        None => {
            let ep = db.find_episode_by_id(&owner).ok()??;
            let show = db
                .find_media_by_id(&ep.media_id)
                .ok()
                .flatten()
                .map(|m| m.title)
                .unwrap_or_default();
            (
                ep.file_path?,
                ep.duration,
                format!("{show} S{:02}E{:02}", ep.season, ep.episode),
            )
        }
    };

    let norm = |l: &str| {
        crate::align::lang_key(l)
            .map(String::from)
            .unwrap_or_else(|| l.to_lowercase())
    };
    let want = norm(lang);
    let mut subs: Vec<_> = db
        .list_subtitles_for_owner(&owner)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.format == "vtt" && norm(&s.language) == want)
        .collect();
    subs.sort_by_key(|s| s.label.to_lowercase().contains("sign"));
    let subs = subs.first().map(|s| {
        (
            s.label.clone(),
            crate::party::abs_path(&state.media_root, &s.file_path),
        )
    });

    Some(Source {
        file: crate::party::abs_path(&state.media_root, &rel),
        duration: duration.map(|d| d as f64),
        title,
        subs,
    })
}

fn timestamp(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    let mut total = parts.last()?.replace(',', ".").parse::<f64>().ok()?;
    let mut mul = 60.0;
    for p in parts.iter().rev().skip(1) {
        total += p.parse::<f64>().ok()? * mul;
        mul *= 60.0;
    }
    Some(total)
}

fn cues(vtt: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    let mut lines = vtt.lines();
    while let Some(l) = lines.next() {
        let Some((a, b)) = l.split_once("-->") else {
            continue;
        };
        let end = b.split_whitespace().next().unwrap_or("");
        let (Some(s), Some(e)) = (timestamp(a), timestamp(end)) else {
            continue;
        };
        let mut text = Vec::new();
        for t in lines.by_ref() {
            if t.trim().is_empty() {
                break;
            }
            let mut clean = String::new();
            let mut tag = false;
            for c in t.chars() {
                match c {
                    '<' => tag = true,
                    '>' => tag = false,
                    _ if !tag => clean.push(c),
                    _ => {}
                }
            }
            text.push(clean.trim().to_string());
        }
        out.push((s, e, text.join("\n")));
    }
    out
}

async fn grab(file: &std::path::Path, t: i64, jpg: &std::path::Path) -> Option<Vec<u8>> {
    let thumb = jpg.with_extension("raw");
    if !jpg.exists() || !thumb.exists() {
        let out = tokio::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-ss", &t.to_string(), "-i"])
            .arg(file)
            .args([
                "-filter_complex",
                "[0:v]split=2[a][b];[a]scale=640:-2[big];[b]scale=32:18,format=gray[small]",
                "-map",
                "[big]",
                "-frames:v",
                "1",
                "-q:v",
                "5",
            ])
            .arg(jpg)
            .args(["-map", "[small]", "-frames:v", "1", "-f", "rawvideo"])
            .arg(&thumb)
            .output()
            .await
            .ok()?;
        if !out.status.success() {
            eprintln!(
                "[watchalong] frame {t} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return None;
        }
    }
    tokio::fs::read(&thumb).await.ok()
}

fn prune_cache() {
    let Ok(dirs) = std::fs::read_dir(CACHE) else {
        return;
    };
    for d in dirs.flatten() {
        let old = d
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|e| e > Duration::from_secs(12 * 3600));
        if old {
            let _ = std::fs::remove_dir_all(d.path());
        }
    }
}

async fn build(code: &str, src: &Source, k: i64) -> Batch {
    prune_cache();
    let dir = PathBuf::from(CACHE).join(code);
    let _ = tokio::fs::create_dir_all(&dir).await;
    let all = match &src.subs {
        Some((_, p)) => cues(&tokio::fs::read_to_string(p).await.unwrap_or_default()),
        None => Vec::new(),
    };
    let end = src.duration.unwrap_or(f64::MAX);

    let mut frames = Vec::new();
    let mut prev: Option<Vec<u8>> = None;
    for i in 0..WINDOW / STEP {
        let t = k * WINDOW + i * STEP;
        if t as f64 >= end {
            break;
        }
        let thumb = grab(&src.file, t, &dir.join(format!("{t}.jpg"))).await;
        let similar = match (&prev, &thumb) {
            (Some(a), Some(b)) if a.len() == b.len() && !a.is_empty() => {
                let diff: u64 = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as u64).sum();
                (diff as f64 / a.len() as f64) < 6.0
            }
            _ => false,
        };
        if thumb.is_some() && !similar {
            prev = thumb.clone();
        }
        let subs = all
            .iter()
            .filter(|c| c.0 < (t + STEP) as f64 && c.1 > t as f64)
            .map(|c| Cue {
                t: (c.0 * 10.0).round() / 10.0,
                text: c.2.clone(),
            })
            .collect();
        frames.push(Frame {
            t,
            url: (thumb.is_some() && !similar)
                .then(|| format!("/api/party/{code}/claude/frame/{t}.jpg")),
            similar,
            subs,
        });
    }
    Batch {
        id: k,
        from: k * WINDOW,
        to: (k + 1) * WINDOW,
        frames,
    }
}

async fn next_batch(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Query(q): Query<NextQuery>,
) -> Response {
    if !valid_code(&code) {
        return (StatusCode::BAD_REQUEST, "bad room code").into_response();
    }
    let Some(src) = source(&state, &code, q.lang.as_deref().unwrap_or("en")).await else {
        return (StatusCode::NOT_FOUND, "room not found").into_response();
    };
    let wait = Duration::from_secs(q.wait.unwrap_or(50).min(120));
    let started = Instant::now();

    loop {
        let (playing, pos) = state.party.clock(&code).await.unwrap_or((false, 0.0));
        let ended = src.duration.is_some_and(|d| pos >= d - 2.0);
        let ready = match src.duration {
            Some(d) if ended => (d / WINDOW as f64).ceil() as i64 - 1,
            _ => (pos / WINDOW as f64).floor() as i64 - 1,
        };
        let target = match q.after {
            Some(a) if a > ready || ready - a > 3 => ready,
            Some(a) => a + 1,
            None => ready,
        };
        let st = if ended {
            "ended"
        } else if playing {
            "playing"
        } else {
            "paused"
        };

        let batch = if target >= 0 && target <= ready && Some(target) != q.after {
            Some(build(&code, &src, target).await)
        } else {
            None
        };
        if batch.is_some() || !playing || ended || started.elapsed() >= wait {
            if let Some(b) = &batch {
                println!("[watchalong] {code} batch {} at {pos:.0}s", b.id);
            }
            return Json(NextResp {
                state: st,
                position: (pos * 10.0).round() / 10.0,
                duration: src.duration,
                title: src.title.clone(),
                subtitles: src.subs.as_ref().map(|(l, _)| l.clone()),
                batch,
            })
            .into_response();
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn frame(Path((code, name)): Path<(String, String)>) -> Response {
    let ok = valid_code(&code)
        && name
            .strip_suffix(".jpg")
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    if !ok {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match tokio::fs::read(PathBuf::from(CACHE).join(&code).join(&name)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct ChatBody {
    text: String,
}

async fn chat(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Json(body): Json<ChatBody>,
) -> Response {
    let text: String = body.text.trim().chars().take(300).collect();
    if !valid_code(&code) || text.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if state.party.clock(&code).await.is_none() {
        return (StatusCode::NOT_FOUND, "nobody in the room").into_response();
    }
    let msg = WsMsg::Chat {
        name: "Claude".into(),
        message: text,
    };
    state.party.dispatch(&code, &msg).await;
    Json(serde_json::json!({ "ok": true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtt_cues_and_timestamps() {
        let vtt = "WEBVTT\n\n00:01:02.500 --> 00:01:04.000\n<i>Hey.</i>\nYou there?\n\n01:05.000 --> 01:06,250 line:90%\nYeah.\n";
        let c = cues(vtt);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], (62.5, 64.0, "Hey.\nYou there?".to_string()));
        assert_eq!(c[1], (65.0, 66.25, "Yeah.".to_string()));
    }

    #[test]
    fn room_codes() {
        assert!(valid_code("A1B2C3"));
        assert!(!valid_code("../etc"));
        assert!(!valid_code("ab"));
    }
}
