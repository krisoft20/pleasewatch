use crate::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ws/downloads", get(upgrade))
        .route("/ws/admin/logs", get(upgrade_logs))
}

async fn upgrade_logs(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    ws: WebSocketUpgrade,
) -> Response {
    let token = jar.get("token").map(|c| c.value().to_string());
    let role: Option<String> = match token {
        Some(t) => {
            let db = state.db.lock().await;
            db.find_user_by_session(&t).ok().flatten().map(|u| u.role)
        }
        None => None,
    };
    if role.as_deref() != Some("admin") {
        return axum::http::Response::builder()
            .status(axum::http::StatusCode::UNAUTHORIZED)
            .body(axum::body::Body::empty())
            .unwrap();
    }
    ws.on_upgrade(move |socket| pump_logs(socket))
}

async fn pump_logs(mut socket: WebSocket) {
    let snap = crate::log_buf::snapshot();
    let mut last_seq = snap.last().map(|e| e.seq).unwrap_or(0);
    if !snap.is_empty() {
        if let Ok(j) = serde_json::to_string(&snap) {
            if socket.send(Message::Text(j.into())).await.is_err() {
                return;
            }
        }
    }

    let mut rx = crate::log_buf::subscribe();
    loop {
        tokio::select! {
            recv = rx.recv() => {
                match recv {
                    Ok(entry) => {
                        if entry.seq <= last_seq { continue; }
                        last_seq = entry.seq;
                        if let Ok(j) = serde_json::to_string(&vec![entry]) {
                            if socket.send(Message::Text(j.into())).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let snap = crate::log_buf::snapshot();
                        let fresh: Vec<_> = snap.iter().filter(|e| e.seq > last_seq).cloned().collect();
                        if let Some(e) = fresh.last() { last_seq = e.seq; }
                        if !fresh.is_empty() {
                            if let Ok(j) = serde_json::to_string(&fresh) {
                                if socket.send(Message::Text(j.into())).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    return;
                }
            }
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
struct DownloadUpdate {
    download_id: String,
    media_id: String,
    status: String,
    progress: f64,
    state: Option<String>,
    title: Option<String>,
    episodes: Vec<EpisodeFileProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seeds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peers: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dlspeed: Option<i64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
struct EpisodeFileProgress {
    season: i32,
    episode: i32,
    progress: f64,
    name: String,
}

async fn upgrade(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    ws: WebSocketUpgrade,
) -> Response {
    let token = jar.get("token").map(|c| c.value().to_string());
    let authed = match token {
        Some(t) => {
            let db = state.db.lock().await;
            db.find_user_by_session(&t)
                .ok()
                .flatten()
                .map(|u| !u.is_pending())
                .unwrap_or(false)
        }
        None => false,
    };
    if !authed {
        return axum::http::Response::builder()
            .status(axum::http::StatusCode::UNAUTHORIZED)
            .body(axum::body::Body::empty())
            .unwrap();
    }
    ws.on_upgrade(move |socket| pump(socket, state))
}

async fn pump(mut socket: WebSocket, state: Arc<AppState>) {
    let mut last: HashMap<String, DownloadUpdate> = HashMap::new();
    let mut first = true;

    loop {
        if !first {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                return;
            }
        }

        let updates = collect(&state).await;

        let mut diff: Vec<DownloadUpdate> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for u in &updates {
            seen.insert(u.download_id.clone());
            if first {
                diff.push(u.clone());
                continue;
            }
            match last.get(&u.download_id) {
                Some(prev) if prev == u => {}
                _ => diff.push(u.clone()),
            }
        }
        for old_id in last.keys() {
            if !seen.contains(old_id) {
                let mut gone = last.get(old_id).cloned().unwrap();
                gone.status = "gone".into();
                diff.push(gone);
            }
        }

        last = updates
            .into_iter()
            .map(|u| (u.download_id.clone(), u))
            .collect();
        first = false;

        if diff.is_empty() {
            continue;
        }

        let json = match serde_json::to_string(&diff) {
            Ok(j) => j,
            Err(_) => continue,
        };
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }
}

async fn collect(state: &Arc<AppState>) -> Vec<DownloadUpdate> {
    let downloads = {
        let db = state.db.lock().await;
        db.list_downloads().unwrap_or_default()
    };

    let qbit = state.qbit.lock().await.clone();
    let mut out = Vec::new();

    for d in downloads {
        let mut progress = if d.status == "complete" { 1.0 } else { 0.0 };
        let mut state_str: Option<String> = None;
        let mut episodes: Vec<EpisodeFileProgress> = Vec::new();
        let mut seeds: Option<i64> = None;
        let mut peers: Option<i64> = None;
        let mut dlspeed: Option<i64> = None;

        if let (Some(q), Some(hash)) = (qbit.as_ref(), d.qbit_hash.as_deref()) {
            if let Ok(Some(t)) = q.get(hash).await {
                progress = t.progress;
                state_str = Some(t.state);
                seeds = Some(t.num_seeds);
                peers = Some(t.num_leechs);
                dlspeed = Some(t.dlspeed);
                if let Some(eid) = d.episode_id.as_deref() {
                    let ep_row = {
                        let db = state.db.lock().await;
                        db.find_episode_by_id(eid).ok().flatten()
                    };
                    if let Some(ep) = ep_row {
                        episodes.push(EpisodeFileProgress {
                            season: ep.season,
                            episode: ep.episode,
                            progress,
                            name: d.title.clone().unwrap_or_default(),
                        });
                    }
                } else if let Ok(files) = q.files(hash).await {
                    for f in files {
                        let lower = f.name.to_lowercase();
                        if !is_video(&lower) || is_bonus(&lower) {
                            continue;
                        }
                        if let Some((s, e)) =
                            crate::downloads::parse_episode_from_path(std::path::Path::new(&lower))
                        {
                            episodes.push(EpisodeFileProgress {
                                season: s,
                                episode: e,
                                progress: f.progress,
                                name: f.name,
                            });
                        }
                    }
                }
            }
        }

        out.push(DownloadUpdate {
            download_id: d.id,
            media_id: d.media_id,
            status: d.status,
            progress,
            state: state_str,
            title: d.title,
            episodes,
            seeds,
            peers,
            dlspeed,
        });
    }

    out
}

fn is_video(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(""),
        "mkv" | "mp4" | "avi" | "mov" | "webm" | "m4v"
    )
}

fn is_bonus(name: &str) -> bool {
    const SKIP: &[&str] = &[
        "featurettes",
        "featurette",
        "extras",
        "extra",
        "bonus",
        "specials",
        "special",
        "behind the scenes",
        "deleted scenes",
        "interviews",
        "samples",
        "sample",
    ];
    name.split(['/', '\\'])
        .any(|part| SKIP.contains(&part.trim()))
}
