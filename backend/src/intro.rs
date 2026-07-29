use crate::AppState;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Semaphore;

static INTRO_SEM: tokio::sync::OnceCell<Semaphore> = tokio::sync::OnceCell::const_new();

async fn intro_sem() -> &'static Semaphore {
    INTRO_SEM.get_or_init(|| async { Semaphore::new(1) }).await
}

const CHUNK_SECS: f64 = 0.128;
const MIN_MATCH_CHUNKS: usize = 30;
const MAX_OFFSET_CHUNKS: i64 = 2340;
const MATCH_BITS: u32 = 20;
const MIN_INTRO_SECS: i64 = 18;
const MAX_INTRO_SECS: i64 = 240;
const MIN_INTRO_START_SECS: i64 = 30;
const MAX_INTRO_START_SECS: i64 = 480;
const CLUSTER_TOLERANCE: i64 = 16;
const FINGERPRINT_WINDOW_SECS: u32 = 600;

pub async fn detect_from_chapters(path: &Path) -> Option<(i64, i64)> {
    let out = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_chapters"])
        .arg(path)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let chapters = json.get("chapters")?.as_array()?;

    for ch in chapters {
        let title = ch
            .get("tags")
            .and_then(|t| t.get("title"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_lowercase();
        if is_intro_chapter(&title) {
            let s = ch
                .get("start_time")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())?;
            let e = ch
                .get("end_time")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())?;
            let start = s.round() as i64;
            let end = e.round() as i64;
            let len = end - start;
            if end > start
                && len >= MIN_INTRO_SECS
                && len <= MAX_INTRO_SECS
                && start >= MIN_INTRO_START_SECS
                && start <= MAX_INTRO_START_SECS
            {
                return Some((start, end));
            }
        }
    }
    None
}

fn is_intro_chapter(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() || t == "chapter" {
        return false;
    }
    t.contains("intro")
        || t.contains("opening")
        || t == "op"
        || t.starts_with("op ")
        || t.contains("avant")
        || t.contains("prologue")
        || t.contains("titre")
        || t.contains("title sequence")
}

pub async fn detect_credits_from_chapters(path: &Path) -> Option<i64> {
    let out = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_chapters"])
        .arg(path)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let chapters = json.get("chapters")?.as_array()?;

    for ch in chapters {
        let title = ch
            .get("tags")
            .and_then(|t| t.get("title"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_lowercase();
        if is_credits_chapter(&title) {
            let s = ch
                .get("start_time")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())?;
            let start = s.round() as i64;
            if start > MIN_INTRO_START_SECS {
                return Some(start);
            }
        }
    }
    None
}

fn is_credits_chapter(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() || t == "chapter" {
        return false;
    }
    t == "ed"
        || t.starts_with("ed ")
        || t.contains("ending")
        || t.contains("credits")
        || t.contains("outro")
        || t.contains("end credit")
        || t.contains("endsong")
}

pub async fn fingerprint(path: &Path, secs: u32) -> Result<Vec<u32>, String> {
    let out = Command::new("fpcalc")
        .args(["-raw", "-length", &secs.to_string(), "-json"])
        .arg(path)
        .output()
        .await
        .map_err(|e| format!("fpcalc spawn: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("parse: {e}"))?;
    let arr = json
        .get("fingerprint")
        .and_then(|f| f.as_array())
        .ok_or_else(|| "no fingerprint array".to_string())?;
    Ok(arr
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u32))
        .collect())
}

pub fn find_common(fp_a: &[u32], fp_b: &[u32]) -> Option<(usize, usize, usize)> {
    let min_off = -(MAX_OFFSET_CHUNKS.min(fp_b.len() as i64));
    let max_off = MAX_OFFSET_CHUNKS.min(fp_a.len() as i64);
    let mut best: Option<(usize, usize, usize)> = None;

    for offset in min_off..=max_off {
        let start_a = offset.max(0) as usize;
        let start_b = (-offset).max(0) as usize;
        if start_a >= fp_a.len() || start_b >= fp_b.len() {
            continue;
        }
        let overlap = (fp_a.len() - start_a).min(fp_b.len() - start_b);

        let mut run_start: Option<usize> = None;
        let mut run_len = 0usize;
        let mut best_len = 0usize;
        let mut best_start = 0usize;

        for i in 0..overlap {
            let a = fp_a[start_a + i];
            let b = fp_b[start_b + i];
            let bits = 32 - (a ^ b).count_ones();
            if bits >= MATCH_BITS {
                if run_start.is_none() {
                    run_start = Some(i);
                }
                run_len += 1;
                if run_len > best_len {
                    best_len = run_len;
                    best_start = run_start.unwrap();
                }
            } else if run_len > 3 && bits >= MATCH_BITS - 4 {
                run_len += 1;
            } else {
                run_start = None;
                run_len = 0;
            }
        }

        if best_len >= MIN_MATCH_CHUNKS {
            let a_off = start_a + best_start;
            let b_off = start_b + best_start;
            if best.is_none() || best_len > best.unwrap().2 {
                best = Some((a_off, b_off, best_len));
            }
        }
    }
    best
}

fn cluster_consensus(pairs: &[(usize, usize)]) -> Option<(i64, i64)> {
    if pairs.is_empty() {
        return None;
    }
    let mut best: Vec<&(usize, usize)> = Vec::new();
    for (off_i, _) in pairs.iter() {
        let cluster: Vec<&(usize, usize)> = pairs
            .iter()
            .filter(|(off_j, _)| ((*off_j as i64) - (*off_i as i64)).abs() <= CLUSTER_TOLERANCE)
            .collect();
        if cluster.len() > best.len() {
            best = cluster;
        }
    }
    if best.is_empty() {
        return None;
    }
    let avg_off = best.iter().map(|(o, _)| *o as i64).sum::<i64>() / best.len() as i64;
    let avg_len = best.iter().map(|(_, l)| *l as i64).sum::<i64>() / best.len() as i64;
    Some((avg_off, avg_len))
}

fn resolve_abs(media_root: &str, file_path: &str) -> std::path::PathBuf {
    if Path::new(file_path).is_absolute() {
        std::path::PathBuf::from(file_path)
    } else {
        Path::new(media_root).join(file_path)
    }
}

pub async fn detect_for_season(state: &Arc<AppState>, media_id: &str, season: i32) -> usize {
    let _permit = intro_sem().await.acquire().await.ok();
    let eps = {
        let db = state.db.lock().await;
        db.list_ready_episodes_without_intro(media_id, season, 8)
            .unwrap_or_default()
    };
    if eps.is_empty() {
        return 0;
    }

    let mut saved = 0usize;
    let mut need_fp: Vec<(String, String, i32)> = Vec::new();

    for (ep_id, file_path, ep_num) in eps {
        let abs = resolve_abs(&state.media_root, &file_path);
        if !abs.exists() {
            continue;
        }
        if let Some((s, e)) = detect_from_chapters(&abs).await {
            let db = state.db.lock().await;
            if db.update_episode_intro(&ep_id, s, e).is_ok() {
                saved += 1;
                crate::pi!("[intro] S{season:02}E{ep_num:02} from chapter: {s}s-{e}s");
            }
        } else {
            need_fp.push((ep_id, file_path, ep_num));
        }
    }

    if need_fp.len() < 2 {
        return saved;
    }

    let mut fps: Vec<(String, i32, Vec<u32>)> = Vec::new();
    for (ep_id, file_path, ep_num) in &need_fp {
        let abs = resolve_abs(&state.media_root, file_path);
        match fingerprint(&abs, FINGERPRINT_WINDOW_SECS).await {
            Ok(fp) if fp.len() > MIN_MATCH_CHUNKS => fps.push((ep_id.clone(), *ep_num, fp)),
            Ok(_) => crate::pe!("[intro] S{season:02}E{ep_num:02}: too few chunks"),
            Err(e) => crate::pe!("[intro] S{season:02}E{ep_num:02}: fpcalc: {e}"),
        }
    }

    if fps.len() < 2 {
        return saved;
    }

    let mut pairs: Vec<(usize, usize, usize, usize)> = Vec::new();
    for i in 0..fps.len() {
        for j in (i + 1)..fps.len() {
            if let Some((a, b, len)) = find_common(&fps[i].2, &fps[j].2) {
                pairs.push((i, j, a, len));
                pairs.push((j, i, b, len));
            }
        }
    }

    for (idx, (ep_id, ep_num, _)) in fps.iter().enumerate() {
        let my_pairs: Vec<(usize, usize)> = pairs
            .iter()
            .filter(|(me, _, _, _)| *me == idx)
            .map(|(_, _, off, len)| (*off, *len))
            .collect();
        if my_pairs.len() < 2 {
            continue;
        }
        let Some((off_chunks, len_chunks)) = cluster_consensus(&my_pairs) else {
            continue;
        };
        let intro_start = (off_chunks as f64 * CHUNK_SECS).round() as i64;
        let intro_end = ((off_chunks + len_chunks) as f64 * CHUNK_SECS).round() as i64;

        let len = intro_end - intro_start;
        if intro_end <= intro_start
            || len < MIN_INTRO_SECS
            || len > MAX_INTRO_SECS
            || intro_start < MIN_INTRO_START_SECS
            || intro_start > MAX_INTRO_START_SECS
        {
            crate::pi!("[intro] S{season:02}E{ep_num:02}: rejected unrealistic ({intro_start}s-{intro_end}s, len {len})");
            continue;
        }

        let db = state.db.lock().await;
        if db
            .update_episode_intro(ep_id, intro_start, intro_end)
            .is_ok()
        {
            saved += 1;
            crate::pi!(
                "[intro] S{season:02}E{ep_num:02}: {intro_start}s-{intro_end}s ({})",
                intro_end - intro_start
            );
        }
    }

    saved
}

pub async fn detect_for_media(state: &Arc<AppState>, media_id: &str) -> usize {
    let seasons = {
        let db = state.db.lock().await;
        db.list_seasons_for_media(media_id).unwrap_or_default()
    };
    let mut total = 0;
    for s in seasons {
        total += detect_for_season(state, media_id, s).await;
    }
    total
}
