use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Condvar, Mutex, Once};
use std::thread::available_parallelism;

static FFMPEG_FREE: Mutex<u32> = Mutex::new(0);
static FFMPEG_CV: Condvar = Condvar::new();
static FFMPEG_INIT: Once = Once::new();

#[derive(Debug, Clone, Serialize)]
pub struct ProcessingJob {
    pub id: String,
    pub operation: String,
    pub source: String,
    pub started_at: String,
    pub state: String,
}

static PROCESSING_JOBS: Mutex<Vec<ProcessingJob>> = Mutex::new(Vec::new());

pub fn list_processing_jobs() -> Vec<ProcessingJob> {
    PROCESSING_JOBS
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

pub fn register_job(job: ProcessingJob) {
    if let Ok(mut g) = PROCESSING_JOBS.lock() {
        g.push(job);
    }
}

pub fn update_job_state(id: &str, state: &str) {
    if let Ok(mut g) = PROCESSING_JOBS.lock() {
        if let Some(j) = g.iter_mut().find(|j| j.id == id) {
            j.state = state.to_string();
        }
    }
}

pub fn unregister_job(id: &str) {
    if let Ok(mut g) = PROCESSING_JOBS.lock() {
        g.retain(|j| j.id != id);
    }
}

fn ffmpeg_slots() {
    FFMPEG_INIT.call_once(|| {
        let cpus = available_parallelism().map(|n| n.get()).unwrap_or(2);
        let configured = std::env::var("FFMPEG_SLOTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(2);
        let slots = configured.max(1);
        *FFMPEG_FREE.lock().unwrap_or_else(|e| e.into_inner()) = slots;
        crate::pi!("[ffmpeg] throttle: {slots} slots ({cpus} cpus, 2 reserved for streaming)");
    });
}

fn load_avg_1m() -> f64 {
    #[cfg(unix)]
    unsafe {
        let mut avg = [0.0f64; 3];
        if libc::getloadavg(avg.as_mut_ptr(), 3) == 3 {
            return avg[0];
        }
    }
    0.0
}

pub fn smart_threads() -> u32 {
    let total = available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
        .max(2);
    let reserve = 2;
    let usable = (total - reserve).max(1);
    let busy = load_avg_1m().ceil() as i32;
    let idle = (usable - busy).max(0);
    let polite_cap = (usable / 2).max(1);
    let budget = idle.min(polite_cap).max(1);
    budget.min(usable).max(1) as u32
}

pub fn throttled(cmd: Command) -> std::io::Result<Output> {
    let op = cmd.get_program().to_string_lossy().into_owned();
    let src = cmd
        .get_args()
        .filter_map(|a| a.to_str())
        .find(|a| !a.starts_with('-') && std::path::Path::new(a).exists())
        .unwrap_or("?")
        .to_string();
    throttled_tracked(cmd, Some((&op, &src)))
}

pub fn throttled_tracked(mut cmd: Command, info: Option<(&str, &str)>) -> std::io::Result<Output> {
    ffmpeg_slots();
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        let total = available_parallelism().map(|n| n.get()).unwrap_or(2);
        let stream_reserved = 2usize.min(total.saturating_sub(1));
        let ffmpeg_cores = total.saturating_sub(stream_reserved).max(1);
        cmd.pre_exec(move || {
            libc::setpriority(libc::PRIO_PROCESS, 0, 15);
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut set);
            for cpu in 0..ffmpeg_cores {
                libc::CPU_SET(cpu, &mut set);
            }
            libc::sched_setaffinity(0, std::mem::size_of_val(&set), &set);
            Ok(())
        });
    }

    let job_id = info.map(|(operation, source)| {
        let id = uuid::Uuid::new_v4().to_string();
        register_job(ProcessingJob {
            id: id.clone(),
            operation: operation.to_string(),
            source: source.to_string(),
            started_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            state: "queued".to_string(),
        });
        let src_basename = std::path::Path::new(source)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(source);
        crate::pi!("[ffmpeg] job+ {operation} {src_basename}");
        id
    });

    let job_id_ref = job_id.clone();
    let mut run = move || -> std::io::Result<Output> {
        {
            let mut g = FFMPEG_FREE.lock().unwrap_or_else(|e| e.into_inner());
            while *g == 0 {
                g = FFMPEG_CV.wait(g).unwrap_or_else(|e| e.into_inner());
            }
            *g -= 1;
        }
        if let Some(ref id) = job_id_ref {
            update_job_state(id, "running");
        }
        let out = cmd.output();
        {
            let mut g = FFMPEG_FREE.lock().unwrap_or_else(|e| e.into_inner());
            *g += 1;
            FFMPEG_CV.notify_one();
        }
        out
    };

    let out = if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(run)
    } else {
        run()
    };

    if let Some(id) = job_id {
        unregister_job(&id);
        crate::pi!(
            "[ffmpeg] job- {} active={}",
            id.chars().take(8).collect::<String>(),
            list_processing_jobs().len()
        );
    }
    out
}

#[derive(Debug)]
pub struct MediaProbe {
    pub duration_secs: i64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_tracks: Vec<AudioProbeTrack>,
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

#[derive(Debug, Clone)]
pub struct AudioProbeTrack {
    pub language: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub index: u32,
    pub language: String,
    pub label: String,
    pub codec: String,
    pub forced: bool,
    pub hearing_impaired: bool,
}

pub fn probe_media(path: &Path) -> Result<MediaProbe, String> {
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(path);
    let out = throttled(cmd).map_err(|e| format!("ffprobe spawn: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }

    let parsed: FfprobeOut =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("parse ffprobe: {e}"))?;

    let duration_secs = parsed
        .format
        .duration
        .and_then(|d| d.parse::<f64>().ok())
        .map(|d| d as i64)
        .unwrap_or(0);

    let video_codec = parsed
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .and_then(|s| s.codec_name.clone());

    let audio_codec = parsed
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .and_then(|s| s.codec_name.clone());

    let mkv = mkv_track_meta(path);
    let audio_tracks: Vec<AudioProbeTrack> = parsed
        .streams
        .iter()
        .filter(|s| s.codec_type == "audio")
        .map(|s| {
            let meta = mkv.get(&s.index);
            let title = stream_title(s, meta);
            let language = stream_lang(s, meta, title.as_deref()).unwrap_or_else(|| "und".into());
            if stream_tag_lang(s).is_none() && language != "und" {
                crate::pi!(
                    "[ffmpeg] audio track {} lang {language} from mkv/title",
                    s.index
                );
            }
            AudioProbeTrack {
                language,
                title: title.unwrap_or_default(),
            }
        })
        .collect();

    let subtitle_tracks: Vec<SubtitleTrack> = parsed
        .streams
        .iter()
        .filter(|s| s.codec_type == "subtitle")
        .filter(|s| {
            let codec = s.codec_name.as_deref().unwrap_or("");
            is_text_sub(codec) || is_pgs_sub(codec)
        })
        .map(|s| {
            let meta = mkv.get(&s.index);
            let title = stream_title(s, meta);
            let lang = stream_lang(s, meta, title.as_deref()).unwrap_or_else(|| "und".into());
            if stream_tag_lang(s).is_none() && lang != "und" {
                crate::pi!(
                    "[ffmpeg] subtitle track {} lang {lang} from mkv/title",
                    s.index
                );
            }
            let label = title.unwrap_or_else(|| lang_name(&lang).to_string());
            SubtitleTrack {
                index: s.index,
                language: lang,
                label,
                codec: s.codec_name.clone().unwrap_or_default(),
                forced: s.disposition.forced > 0 || meta.and_then(|m| m.forced).unwrap_or(false),
                hearing_impaired: s.disposition.hearing_impaired > 0
                    || meta.and_then(|m| m.hearing_impaired).unwrap_or(false),
            }
        })
        .collect();

    Ok(MediaProbe {
        duration_secs,
        video_codec,
        audio_codec,
        audio_tracks,
        subtitle_tracks,
    })
}

pub fn fmt_sub_label(track: &SubtitleTrack) -> String {
    let base = if track.label.is_empty() || crate::lang::looks_like_raw_code(&track.label) {
        lang_name(&track.language).to_string()
    } else {
        track.label.clone()
    };

    if track.forced {
        return format!("{base} (Forced)");
    }
    if track.hearing_impaired {
        return format!("{base} (SDH)");
    }
    base
}

fn is_browser_safe_audio(codec: &str) -> bool {
    matches!(codec, "aac" | "mp3" | "opus" | "flac" | "vorbis")
}

fn is_browser_safe_video(codec: &str) -> bool {
    matches!(codec, "h264" | "hevc" | "h265")
}

pub fn extract_all_subtitles(
    input: &Path,
    tracks: &[SubtitleTrack],
    subs_dir: &Path,
) -> Vec<(usize, std::path::PathBuf)> {
    if tracks.is_empty() {
        return vec![];
    }
    let _ = std::fs::create_dir_all(subs_dir);

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-analyzeduration",
        "1000000",
        "-probesize",
        "1000000",
        "-i",
    ])
    .arg(input);

    let mut outputs = Vec::new();
    for (i, track) in tracks.iter().enumerate() {
        let out_path = subs_dir.join(format!("{}_{}.vtt", track.language, track.index));
        cmd.args(["-map", &format!("0:{}", track.index)]);
        cmd.args(["-c:s", "webvtt"]);
        cmd.arg(&out_path);
        outputs.push((i, out_path));
    }

    if let Err(e) = throttled(cmd) {
        crate::pe!("[ffmpeg] sub extract spawn failed: {e}");
        return vec![];
    }

    outputs
        .into_iter()
        .filter(|(_, p)| std::fs::metadata(p).map(|m| m.len() > 100).unwrap_or(false))
        .inspect(|(_, p)| {
            clean_pipe_breaks(p);
            strip_bidi_marks(p);
        })
        .collect()
}

pub fn remux_to_mp4(input: &Path, output: &Path) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let probe = probe_media(input).ok();
    let vcodec = probe
        .as_ref()
        .and_then(|p| p.video_codec.clone())
        .unwrap_or_default();
    let acodec = probe
        .as_ref()
        .and_then(|p| p.audio_codec.clone())
        .unwrap_or_default();

    if !is_browser_safe_video(&vcodec) {
        crate::pe!("[ffmpeg] video '{vcodec}' not browser-safe, going full transcode");
        return transcode_to_mp4(input, output);
    }

    let threads = smart_threads().to_string();
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-threads",
        &threads,
        "-analyzeduration",
        "1000000",
        "-probesize",
        "1000000",
        "-i",
    ])
    .arg(input)
    .args(["-map", "0:v:0", "-map", "0:a", "-c:v", "copy"]);
    if matches!(vcodec.as_str(), "hevc" | "h265") {
        cmd.args(["-tag:v", "hvc1"]);
    }
    if is_browser_safe_audio(&acodec) {
        crate::pe!("[ffmpeg] audio '{acodec}' is browser-safe, copying");
        cmd.args(["-c:a", "copy"]);
    } else {
        crate::pe!(
            "[ffmpeg] audio '{acodec}' not browser-safe, transcoding to aac (threads={threads})"
        );
        cmd.args([
            "-c:a", "aac", "-b:a", "192k", "-ac", "2", "-threads", &threads,
        ]);
    }
    add_audio_metadata(&mut cmd, probe.as_ref());
    cmd.args([
        "-max_muxing_queue_size",
        "1024",
        "-sn",
        "-movflags",
        "+faststart",
    ])
    .arg(output);

    let source = input.display().to_string();
    let out = throttled_tracked(cmd, Some(("remux", &source))).map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if stderr.contains("could not find tag")
        || stderr.contains("codec not currently supported")
        || stderr.contains("muxer does not support")
    {
        crate::pe!("[ffmpeg] copy failed, falling back to transcode");
        return transcode_to_mp4(input, output);
    }
    Err(format!("remux failed: {stderr}"))
}

pub fn transcode_to_mp4(input: &Path, output: &Path) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let threads = smart_threads().to_string();
    let probe = probe_media(input).ok();
    crate::pe!(
        "[ffmpeg] transcode with -threads {threads} (load_avg_1m={:.2})",
        load_avg_1m()
    );
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-threads", &threads, "-i"])
        .arg(input)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-threads",
            &threads,
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-ac",
            "2",
            "-sn",
            "-movflags",
            "+faststart",
        ]);
    add_audio_metadata(&mut cmd, probe.as_ref());
    cmd.arg(output);
    let source = input.display().to_string();
    let out = throttled_tracked(cmd, Some(("transcode", &source))).map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

pub fn is_text_sub(codec: &str) -> bool {
    matches!(
        codec,
        "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "vtt" | "mov_text" | "text" | "microdvd"
    )
}

pub fn is_pgs_sub(codec: &str) -> bool {
    matches!(codec, "hdmv_pgs_subtitle" | "pgs" | "pgssub")
}

pub fn extract_pgs_track(video: &Path, track_index: u32, dest_sup: &Path) -> Result<(), String> {
    if let Some(parent) = dest_sup.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i"])
        .arg(video)
        .args([
            "-map",
            &format!("0:{track_index}"),
            "-c:s",
            "copy",
            "-f",
            "sup",
        ])
        .arg(dest_sup);
    let out = throttled(cmd).map_err(|e| format!("ffmpeg spawn: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(())
}

pub fn ocr_pgs_to_srt(sup: &Path, lang: &str) -> Result<std::path::PathBuf, String> {
    let mut cmd = Command::new("pgsrip");
    cmd.args(["--language", lang, "--force"]).arg(sup);
    let out = throttled(cmd).map_err(|e| format!("pgsrip spawn: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !stdout.trim().is_empty() {
        crate::pe!("[pgsrip] stdout: {}", stdout.trim());
    }
    if !stderr.trim().is_empty() {
        crate::pe!("[pgsrip] stderr: {}", stderr.trim());
    }
    if !out.status.success() {
        return Err(format!("exit {}: {}", out.status, stderr.trim()));
    }
    let stem = sup.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let parent = sup.parent().unwrap_or(Path::new("."));
    for name in [format!("{stem}.{lang}.srt"), format!("{stem}.srt")] {
        let p = parent.join(&name);
        if p.exists() {
            return Ok(p);
        }
    }
    if let Ok(rd) = std::fs::read_dir(parent) {
        let listing: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        return Err(format!("no srt; dir contents: {}", listing.join(", ")));
    }
    Err("no srt and dir unreadable".into())
}

pub fn convert_sub(src: &Path, dest: &Path, lang: &str) -> bool {
    let bytes = match std::fs::read(src) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let utf8 = std::str::from_utf8(&bytes).is_ok();
    let microdvd = looks_like_microdvd(&bytes);
    let mpl2 = !microdvd && looks_like_mpl2(&bytes);
    let same_ext =
        src.extension().and_then(|e| e.to_str()) == dest.extension().and_then(|e| e.to_str());

    if utf8 && same_ext && !microdvd && !mpl2 {
        if std::fs::write(dest, &bytes).is_err() {
            return false;
        }
        clean_pipe_breaks(dest);
        strip_bidi_marks(dest);
        return true;
    }

    let charenc = if !utf8 {
        Some(charenc_for_lang(lang))
    } else {
        None
    };
    crate::pi!(
        "[ffmpeg] convert_sub lang={lang} utf8={utf8} microdvd={microdvd} mpl2={mpl2} charenc={charenc:?}"
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y"]);
    if let Some(enc) = charenc {
        cmd.args(["-sub_charenc", enc]);
    }
    if microdvd {
        cmd.args(["-f", "microdvd"]);
    } else if mpl2 {
        cmd.args(["-f", "mpl2"]);
    }
    cmd.args(["-i"]).arg(src).arg(dest);
    let ok = throttled(cmd).map(|o| o.status.success()).unwrap_or(false);
    if !ok {
        crate::pe!(
            "[ffmpeg] convert_sub failed: {} -> {}",
            src.display(),
            dest.display()
        );
        return false;
    }
    clean_pipe_breaks(dest);
    strip_bidi_marks(dest);
    true
}

fn strip_bidi_marks(dest: &Path) {
    let Ok(content) = std::fs::read_to_string(dest) else {
        return;
    };
    if !content.contains("&lrm;") && !content.contains("&rlm;") {
        return;
    }
    let cleaned = content.replace("&lrm;", "").replace("&rlm;", "");
    let _ = std::fs::write(dest, cleaned);
}

fn clean_pipe_breaks(dest: &Path) {
    let Ok(content) = std::fs::read_to_string(dest) else {
        return;
    };
    if !content.contains('|') {
        return;
    }
    let chars: Vec<char> = content.chars().collect();
    let mut out = String::with_capacity(content.len());
    let n = chars.len();
    for i in 0..n {
        let c = chars[i];
        if c == '|' {
            let prev_nl = i > 0 && chars[i - 1] == '\n';
            let next_nl = i + 1 < n && chars[i + 1] == '\n';
            if !prev_nl && !next_nl {
                out.push('\n');
            }
        } else {
            out.push(c);
        }
    }
    let _ = std::fs::write(dest, out);
}

pub fn looks_like_microdvd(bytes: &[u8]) -> bool {
    let mut checked = 0;
    let mut start = 0usize;
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' || i + 1 == bytes.len() {
            let end = if bytes[i] == b'\n' { i } else { i + 1 };
            let line = trim_ascii(&bytes[start..end]);
            if !line.is_empty() {
                if line_is_microdvd(line) {
                    return true;
                }
                checked += 1;
                if checked >= 6 {
                    return false;
                }
            }
            start = i + 1;
        }
    }
    false
}

pub fn looks_like_mpl2(bytes: &[u8]) -> bool {
    let mut checked = 0;
    let mut start = 0usize;
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' || i + 1 == bytes.len() {
            let end = if bytes[i] == b'\n' { i } else { i + 1 };
            let line = trim_ascii(&bytes[start..end]);
            if !line.is_empty() {
                if line_is_mpl2(line) {
                    return true;
                }
                checked += 1;
                if checked >= 6 {
                    return false;
                }
            }
            start = i + 1;
        }
    }
    false
}

fn line_is_mpl2(line: &[u8]) -> bool {
    if line.first() != Some(&b'[') {
        return false;
    }
    let mut i = 1;
    while i < line.len() && line[i].is_ascii_digit() {
        i += 1;
    }
    if i == 1 || line.get(i) != Some(&b']') {
        return false;
    }
    i += 1;
    if line.get(i) != Some(&b'[') {
        return false;
    }
    let s2 = i + 1;
    let mut j = s2;
    while j < line.len() && line[j].is_ascii_digit() {
        j += 1;
    }
    j != s2 && line.get(j) == Some(&b']')
}

fn trim_ascii(s: &[u8]) -> &[u8] {
    let lead = s
        .iter()
        .position(|&b| b != b' ' && b != b'\t' && b != b'\r')
        .unwrap_or(s.len());
    let tail_off = s[lead..]
        .iter()
        .rposition(|&b| b != b' ' && b != b'\t' && b != b'\r')
        .map(|n| n + 1)
        .unwrap_or(0);
    &s[lead..lead + tail_off]
}

fn line_is_microdvd(line: &[u8]) -> bool {
    if line.first() != Some(&b'{') {
        return false;
    }
    let mut i = 1;
    while i < line.len() && line[i].is_ascii_digit() {
        i += 1;
    }
    if i == 1 || line.get(i) != Some(&b'}') {
        return false;
    }
    i += 1;
    if line.get(i) != Some(&b'{') {
        return false;
    }
    let s2 = i + 1;
    let mut j = s2;
    while j < line.len() && line[j].is_ascii_digit() {
        j += 1;
    }
    j != s2 && line.get(j) == Some(&b'}')
}

pub fn charenc_for_lang(lang: &str) -> &'static str {
    match lang {
        "pol" | "pl" | "ces" | "cze" | "cs" | "slk" | "slo" | "sk" | "hun" | "hu" | "hrv"
        | "hr" | "scc" | "ron" | "rum" | "ro" => "cp1250",
        "rus" | "ru" | "ukr" | "uk" | "bul" | "bg" | "srp" | "sr" | "bel" | "be" => "cp1251",
        "ell" | "gre" | "el" => "cp1253",
        "tur" | "tr" => "cp1254",
        "heb" | "he" | "iw" => "cp1255",
        "ara" | "ar" => "cp1256",
        "tha" | "th" => "cp874",
        "vie" | "vi" => "cp1258",
        "jpn" | "ja" => "shift_jis",
        "kor" | "ko" => "euc-kr",
        "chi" | "zho" | "zh" => "gb18030",
        _ => "cp1252",
    }
}

fn lang_name(code: &str) -> String {
    crate::lang::lang_name(code)
}

#[derive(Deserialize)]
struct FfprobeOut {
    streams: Vec<FfprobeStream>,
    format: FfprobeFormat,
}

#[derive(Deserialize)]
struct FfprobeStream {
    index: u32,
    codec_type: String,
    codec_name: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    disposition: StreamDisposition,
}

#[derive(Deserialize, Default)]
struct StreamDisposition {
    #[serde(default)]
    forced: i32,
    #[serde(default)]
    hearing_impaired: i32,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

#[derive(Default)]
struct MkvTrackMeta {
    language: Option<String>,
    title: Option<String>,
    forced: Option<bool>,
    hearing_impaired: Option<bool>,
}

fn add_audio_metadata(cmd: &mut Command, probe: Option<&MediaProbe>) {
    let Some(probe) = probe else { return };
    for (i, track) in probe.audio_tracks.iter().enumerate() {
        if track.language != "und" && !track.language.is_empty() {
            cmd.arg(format!("-metadata:s:a:{i}"))
                .arg(format!("language={}", track.language));
        }
        if !track.title.is_empty() && !crate::lang::looks_like_raw_code(&track.title) {
            cmd.arg(format!("-metadata:s:a:{i}"))
                .arg(format!("title={}", track.title));
        }
    }
}

fn stream_title(s: &FfprobeStream, meta: Option<&MkvTrackMeta>) -> Option<String> {
    tag_value(&s.tags, "title")
        .or_else(|| tag_value(&s.tags, "track_name"))
        .or_else(|| meta.and_then(|m| m.title.clone()))
        .filter(|v| !v.eq_ignore_ascii_case("unknown"))
}

fn stream_lang(
    s: &FfprobeStream,
    meta: Option<&MkvTrackMeta>,
    title: Option<&str>,
) -> Option<String> {
    stream_tag_lang(s)
        .or_else(|| meta.and_then(|m| m.language.clone()))
        .or_else(|| {
            title
                .and_then(crate::lang::lang_code_from_label)
                .map(str::to_string)
        })
}

fn stream_tag_lang(s: &FfprobeStream) -> Option<String> {
    tag_value(&s.tags, "language")
        .and_then(|v| clean_lang(&v))
        .or_else(|| tag_value(&s.tags, "language_ietf").and_then(|v| clean_lang(&v)))
        .or_else(|| tag_value(&s.tags, "language-ietf").and_then(|v| clean_lang(&v)))
}

fn tag_value(tags: &HashMap<String, String>, key: &str) -> Option<String> {
    tags.iter()
        .find(|(k, v)| k.eq_ignore_ascii_case(key) && !v.trim().is_empty())
        .map(|(_, v)| v.trim().to_string())
}

fn clean_lang(raw: &str) -> Option<String> {
    crate::lang::canon_lang_code(raw)
        .map(str::to_string)
        .or_else(|| {
            let v = raw.trim();
            if v.is_empty() || v.eq_ignore_ascii_case("und") || v.eq_ignore_ascii_case("unknown") {
                None
            } else {
                Some(v.to_lowercase())
            }
        })
}

fn mkv_track_meta(path: &Path) -> HashMap<u32, MkvTrackMeta> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| !e.eq_ignore_ascii_case("mkv"))
        .unwrap_or(true)
    {
        return HashMap::new();
    }

    let out = match Command::new("mkvmerge").args(["-J"]).arg(path).output() {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(),
    };
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let mut out = HashMap::new();
    let Some(tracks) = v.get("tracks").and_then(|x| x.as_array()) else {
        return out;
    };
    for t in tracks {
        let Some(id) = t.get("id").and_then(|x| x.as_u64()).map(|x| x as u32) else {
            continue;
        };
        let props = t.get("properties").unwrap_or(&serde_json::Value::Null);
        let title = json_str(props, "track_name");
        let language = json_str(props, "language_ietf")
            .and_then(|v| clean_lang(&v))
            .or_else(|| json_str(props, "language").and_then(|v| clean_lang(&v)))
            .or_else(|| {
                title
                    .as_deref()
                    .and_then(crate::lang::lang_code_from_label)
                    .map(str::to_string)
            });
        let forced = json_bool(props, "forced_track");
        let hearing_impaired = json_bool(props, "hearing_impaired");
        out.insert(
            id,
            MkvTrackMeta {
                language,
                title,
                forced,
                hearing_impaired,
            },
        );
    }
    out
}

fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn json_bool(v: &serde_json::Value, key: &str) -> Option<bool> {
    v.get(key).and_then(|x| x.as_bool())
}
