use crate::{middleware::AuthUser, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/128.0 Safari/537.36";
const REFRESH_EVERY: Duration = Duration::from_secs(6 * 3600);
const LIVE_WINDOW: usize = 8;

#[derive(Clone, Serialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub country: String,
    pub group: Option<String>,
    pub geo: bool,
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    referer: Option<String>,
    #[serde(skip)]
    ua: Option<String>,
}

impl Channel {
    fn dash(&self) -> bool {
        self.url.split('?').next().unwrap_or("").ends_with(".mpd")
    }
}

static CHANNELS: LazyLock<RwLock<HashMap<String, Vec<Channel>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static MPDS: LazyLock<RwLock<HashMap<String, (Instant, String, String)>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn countries() -> Vec<String> {
    std::env::var("TV_COUNTRIES")
        .unwrap_or_else(|_| "pl,uk,us".into())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(8))
        .build()
        .expect("build tv client")
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/api/tv/dash/{token}/master.m3u8", get(dash_master))
        .route("/api/tv/dash/{token}/r/{rep}", get(dash_track));
    let authed = Router::new()
        .route("/api/tv/channels", get(list_channels))
        .route("/api/tv/play", post(play))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ));
    public.merge(authed)
}

pub fn spawn_refresher() {
    tokio::spawn(async {
        loop {
            for c in countries() {
                refresh_country(&c).await;
            }
            tokio::time::sleep(REFRESH_EVERY).await;
        }
    });
}

async fn refresh_country(country: &str) {
    let t0 = Instant::now();
    let url = format!("https://iptv-org.github.io/iptv/countries/{country}.m3u");
    let text = match client()
        .get(&url)
        .timeout(Duration::from_secs(30))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        Ok(r) => {
            crate::pe!("[tv] {country} playlist http {}", r.status());
            return;
        }
        Err(e) => {
            crate::pe!("[tv] {country} playlist: {e}");
            return;
        }
    };
    let all = parse_m3u(&text, country);
    let total = all.len();

    let c = client();
    let mut alive = Vec::with_capacity(total);
    for chunk in all.chunks(32) {
        let mut set = tokio::task::JoinSet::new();
        for ch in chunk.iter().cloned() {
            let c = c.clone();
            set.spawn(async move {
                if probe(&c, &ch).await {
                    Some(ch)
                } else {
                    None
                }
            });
        }
        while let Some(r) = set.join_next().await {
            if let Ok(Some(ch)) = r {
                alive.push(ch);
            }
        }
    }
    alive.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    crate::pi!(
        "[tv] {country}: {}/{total} alive in {:.0}s",
        alive.len(),
        t0.elapsed().as_secs_f32()
    );
    CHANNELS.write().await.insert(country.to_string(), alive);
}

async fn probe(c: &reqwest::Client, ch: &Channel) -> bool {
    if ch.dash() {
        let mut req = c.get(&ch.url);
        if let Some(r) = &ch.referer {
            req = req.header("Referer", r);
        }
        let Ok(resp) = req.send().await else {
            return false;
        };
        if !resp.status().is_success() {
            return false;
        }
        let url = resp.url().to_string();
        let Ok(xml) = resp.text().await else {
            return false;
        };
        return !xml.contains("ContentProtection") && !parse_mpd(&xml, &url).is_empty();
    }
    let mut req = c.get(&ch.url).header("Range", "bytes=0-4095");
    if let Some(r) = &ch.referer {
        req = req.header("Referer", r);
    }
    if let Some(ua) = &ch.ua {
        req = req.header("User-Agent", ua);
    }
    let Ok(resp) = req.send().await else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let Ok(body) = resp.bytes().await else {
        return false;
    };
    let head = String::from_utf8_lossy(&body[..body.len().min(4096)]);
    head.trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("#EXTM3U")
        || head.contains("<MPD")
}

fn parse_m3u(text: &str, country: &str) -> Vec<Channel> {
    let mut out = Vec::new();
    let mut info: Option<&str> = None;
    let mut referer = None;
    let mut ua = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with("#EXTINF") {
            info = Some(line);
            referer = None;
            ua = None;
        } else if let Some(v) = line.strip_prefix("#EXTVLCOPT:http-referrer=") {
            referer = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("#EXTVLCOPT:http-user-agent=") {
            ua = Some(v.to_string());
        } else if line.starts_with("http://") || line.starts_with("https://") {
            let Some(inf) = info.take() else { continue };
            let raw_name = inf.rsplit_once(',').map(|(_, n)| n.trim()).unwrap_or("");
            let geo = raw_name.contains("[Geo-blocked]");
            let name = raw_name
                .split(" (")
                .next()
                .unwrap_or(raw_name)
                .split(" [")
                .next()
                .unwrap_or(raw_name)
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let hash = format!("{:x}", sha2::Sha256::digest(line.as_bytes()));
            out.push(Channel {
                id: format!("{country}-{}", &hash[..12]),
                name,
                logo: attr(inf, "tvg-logo"),
                country: country.to_string(),
                group: attr(inf, "group-title").filter(|g| g != "Undefined"),
                geo,
                url: line.to_string(),
                referer: referer.take(),
                ua: ua.take(),
            });
        }
    }
    out
}

fn attr(line: &str, key: &str) -> Option<String> {
    let needle = format!(" {key}=\"");
    let i = line.find(&needle)? + needle.len();
    let j = line[i..].find('"')? + i;
    Some(line[i..j].to_string()).filter(|s| !s.is_empty())
}

async fn find(id: &str) -> Option<Channel> {
    CHANNELS
        .read()
        .await
        .values()
        .flatten()
        .find(|c| c.id == id)
        .cloned()
}

#[derive(Serialize)]
struct ChannelsResp {
    channels: Vec<Channel>,
    pending: Vec<String>,
}

async fn list_channels(Extension(_auth): Extension<AuthUser>) -> Json<ChannelsResp> {
    let map = CHANNELS.read().await;
    let mut channels = Vec::new();
    let mut pending = Vec::new();
    for c in countries() {
        match map.get(&c) {
            Some(list) => channels.extend(list.iter().cloned()),
            None => pending.push(c),
        }
    }
    Json(ChannelsResp { channels, pending })
}

#[derive(Deserialize)]
struct PlayReq {
    id: String,
}

#[derive(Serialize)]
struct PlayResp {
    master_url: String,
}

async fn play(
    Extension(_auth): Extension<AuthUser>,
    State(_s): State<Arc<AppState>>,
    Json(body): Json<PlayReq>,
) -> Response {
    let Some(ch) = find(&body.id).await else {
        return (StatusCode::NOT_FOUND, "unknown channel").into_response();
    };
    let referer = ch
        .referer
        .clone()
        .unwrap_or_else(|| crate::live::origin_of(&ch.url));
    let token = crate::live::issue_token(ch.url.clone(), referer).await;
    if ch.dash() {
        crate::pi!("[tv] play {} via mpd -> hls", ch.name);
        return Json(PlayResp {
            master_url: format!("/api/tv/dash/{token}/master.m3u8"),
        })
        .into_response();
    }
    crate::pi!("[tv] play {} via proxy", ch.name);
    Json(PlayResp {
        master_url: format!("/api/live/hls/{token}/master.m3u8"),
    })
    .into_response()
}

struct Track {
    id: String,
    video: bool,
    bandwidth: u64,
    width: u32,
    height: u32,
    codecs: String,
    lang: Option<String>,
    label: Option<String>,
    alt: bool,
    init: String,
    media: String,
    timescale: u64,
    start_number: u64,
    segs: Vec<(u64, u64)>,
}

fn parse_mpd(xml: &str, mpd_url: &str) -> Vec<Track> {
    let dir = mpd_url.split('?').next().unwrap_or(mpd_url);
    let dir = dir.rsplit_once('/').map(|(d, _)| d).unwrap_or(dir);
    let abs = |u: &str| {
        if u.starts_with("http") {
            u.to_string()
        } else {
            format!("{dir}/{u}")
        }
    };
    let head = |s: &str| format!(" {}", &s[..s.find('>').unwrap_or(s.len())]);

    let mut out = Vec::new();
    for set in xml.split("<AdaptationSet").skip(1) {
        let set = set.split("</AdaptationSet>").next().unwrap_or(set);
        let sh = head(set);
        let ct = attr(&sh, "contentType")
            .or_else(|| attr(&sh, "mimeType"))
            .unwrap_or_default();
        let video = ct.starts_with("video");
        if !video && !ct.starts_with("audio") {
            continue;
        }
        let tpl = |s: &str| -> Option<(String, String, u64, u64, Vec<(u64, u64)>)> {
            let ti = s.find("<SegmentTemplate")?;
            let body = &s[ti..s[ti..]
                .find("</SegmentTemplate>")
                .map(|e| ti + e)
                .unwrap_or(s.len())];
            let th = head(&body[16..]);
            let timescale = attr(&th, "timescale")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1);
            let start = attr(&th, "startNumber")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1);
            let mut segs = Vec::new();
            let mut next = 0u64;
            for s in body.split("<S ").skip(1) {
                let s = head(s);
                let Some(d) = attr(&s, "d").and_then(|v| v.parse::<u64>().ok()) else {
                    continue;
                };
                let mut t = attr(&s, "t").and_then(|v| v.parse().ok()).unwrap_or(next);
                let r = attr(&s, "r")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0)
                    .max(0);
                for _ in 0..=r {
                    segs.push((t, d));
                    t += d;
                }
                next = t;
            }
            Some((
                attr(&th, "media")?,
                attr(&th, "initialization")?,
                timescale,
                start,
                segs,
            ))
        };
        let set_tpl = tpl(&set[..set.find("<Representation").unwrap_or(set.len())]);

        let label = set
            .split("<Label>")
            .nth(1)
            .and_then(|l| l.split("</Label>").next())
            .map(str::to_string);
        let alt = set.contains("value=\"alternate\"");
        for rep in set.split("<Representation").skip(1) {
            let rh = head(rep);
            let Some(id) = attr(&rh, "id") else { continue };
            let Some((media, init, timescale, start_number, segs)) =
                tpl(rep).or_else(|| set_tpl.clone())
            else {
                continue;
            };
            if segs.is_empty() {
                continue;
            }
            let num = |k: &str| {
                attr(&rh, k)
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0)
            };
            out.push(Track {
                video,
                bandwidth: num("bandwidth"),
                width: num("width") as u32,
                height: num("height") as u32,
                codecs: attr(&rh, "codecs").unwrap_or_default(),
                lang: attr(&sh, "lang"),
                label: label.clone(),
                alt,
                init: abs(&init.replace("$RepresentationID$", &id)),
                media: abs(&media.replace("$RepresentationID$", &id)),
                timescale,
                start_number,
                segs,
                id,
            });
        }
    }
    out
}

async fn fetch_mpd(token: &str) -> Result<Vec<Track>, Response> {
    let Some(t) = crate::live::load_token(token).await else {
        return Err((StatusCode::NOT_FOUND, "token expired").into_response());
    };
    let cached = MPDS
        .read()
        .await
        .get(token)
        .filter(|(at, _, _)| at.elapsed() < Duration::from_secs(2))
        .map(|(_, xml, url)| (xml.clone(), url.clone()));
    let (xml, url) = match cached {
        Some(c) => c,
        None => {
            let resp = client()
                .get(&t.master_url)
                .header("Referer", &t.referer)
                .send()
                .await
                .map_err(|e| {
                    crate::pe!("[tv] mpd fetch: {e}");
                    (StatusCode::BAD_GATEWAY, "upstream error").into_response()
                })?;
            let url = resp.url().to_string();
            let xml = resp.text().await.unwrap_or_default();
            let mut m = MPDS.write().await;
            m.retain(|_, (at, _, _)| at.elapsed() < Duration::from_secs(60));
            m.insert(
                token.to_string(),
                (Instant::now(), xml.clone(), url.clone()),
            );
            (xml, url)
        }
    };
    let tracks = parse_mpd(&xml, &url);
    if tracks.is_empty() {
        crate::pe!("[tv] mpd has no playable tracks ({} bytes)", xml.len());
        return Err((StatusCode::BAD_GATEWAY, "empty mpd").into_response());
    }
    Ok(tracks)
}

const M3U8: [(&str, &str); 2] = [
    ("content-type", "application/vnd.apple.mpegurl"),
    ("cache-control", "no-cache"),
];

async fn dash_master(Path(token): Path<String>) -> Response {
    let tracks = match fetch_mpd(&token).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let mut audio: Vec<&Track> = tracks.iter().filter(|t| !t.video).collect();
    audio.sort_by_key(|t| t.alt);
    let mut video: Vec<&Track> = tracks.iter().filter(|t| t.video).collect();
    video.sort_by(|a, b| b.bandwidth.cmp(&a.bandwidth));

    let mut m = String::from("#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-INDEPENDENT-SEGMENTS\n");
    for (i, a) in audio.iter().enumerate() {
        let name = a
            .label
            .clone()
            .or_else(|| a.lang.clone())
            .unwrap_or_else(|| a.id.clone())
            .replace('"', "'");
        m.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"{name}\",LANGUAGE=\"{}\",DEFAULT={},AUTOSELECT={},URI=\"/api/tv/dash/{token}/r/{}.m3u8\"\n",
            a.lang.as_deref().unwrap_or("und"),
            if i == 0 { "YES" } else { "NO" },
            if a.alt { "NO" } else { "YES" },
            a.id
        ));
    }
    let main_audio = audio.first();
    if video.is_empty() {
        if let Some(a) = main_audio {
            m.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH={},CODECS=\"{}\"\n/api/tv/dash/{token}/r/{}.m3u8\n",
                a.bandwidth, a.codecs, a.id
            ));
        }
    }
    for v in video {
        let (bw, codecs, group) = match main_audio {
            Some(a) => (
                v.bandwidth + a.bandwidth,
                format!("{},{}", v.codecs, a.codecs),
                ",AUDIO=\"aud\"",
            ),
            None => (v.bandwidth, v.codecs.clone(), ""),
        };
        m.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={bw},RESOLUTION={}x{},CODECS=\"{codecs}\"{group}\n/api/tv/dash/{token}/r/{}.m3u8\n",
            v.width, v.height, v.id
        ));
    }
    (M3U8, m).into_response()
}

async fn dash_track(Path((token, rep)): Path<(String, String)>) -> Response {
    let tracks = match fetch_mpd(&token).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let id = rep.strip_suffix(".m3u8").unwrap_or(&rep);
    let Some(t) = tracks.iter().find(|t| t.id == id) else {
        return (StatusCode::NOT_FOUND, "no such track").into_response();
    };
    let proxied = |u: &str| {
        format!(
            "/api/live/hls/{token}/p/{}",
            crate::live::hex_encode(u.as_bytes())
        )
    };
    let first = t.segs.len().saturating_sub(LIVE_WINDOW);
    let nominal = t.segs[0].1.max(1);
    let by_time = t.media.contains("$Time$");
    let seq = if by_time {
        t.segs[first].0 / nominal
    } else {
        t.start_number + first as u64
    };
    let target = t.segs[first..]
        .iter()
        .map(|s| s.1)
        .max()
        .unwrap_or(nominal)
        .div_ceil(t.timescale);

    let mut p = format!(
        "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:{seq}\n#EXT-X-MAP:URI=\"{}\"\n",
        proxied(&t.init)
    );
    for (i, (time, d)) in t.segs[first..].iter().enumerate() {
        let url = t
            .media
            .replace("$Time$", &time.to_string())
            .replace(
                "$Number$",
                &(t.start_number + (first + i) as u64).to_string(),
            )
            .replace("$Bandwidth$", &t.bandwidth.to_string());
        p.push_str(&format!(
            "#EXTINF:{:.3},\n{}\n",
            *d as f64 / t.timescale as f64,
            proxied(&url)
        ));
    }
    (M3U8, p).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u_parses_names_flags_and_opts() {
        let m = "#EXTM3U\n#EXTINF:-1 tvg-id=\"Polsat.pl@SD\" tvg-logo=\"https://l/p.png\" group-title=\"General\",Polsat (1080p) [Geo-blocked]\n#EXTVLCOPT:http-referrer=https://r.pl/\nhttps://cdn/x/live.mpd\n#EXTINF:-1 tvg-id=\"\" group-title=\"Undefined\",TVP Info\nhttp://a/b.m3u8\n";
        let chs = parse_m3u(m, "pl");
        assert_eq!(chs.len(), 2);
        assert_eq!(chs[0].name, "Polsat");
        assert!(chs[0].geo && chs[0].dash());
        assert_eq!(chs[0].referer.as_deref(), Some("https://r.pl/"));
        assert_eq!(chs[0].logo.as_deref(), Some("https://l/p.png"));
        assert_eq!(chs[1].name, "TVP Info");
        assert!(chs[1].group.is_none() && chs[1].referer.is_none() && !chs[1].dash());
        assert!(chs[0].id.starts_with("pl-") && chs[0].id.len() == 15);
    }

    #[test]
    fn mpd_expands_timeline_and_templates() {
        let mpd = r#"<MPD><Period id="p">
<AdaptationSet id="0" lang="pl" contentType="audio"><SegmentTemplate media="$RepresentationID$/p/t$Time$.mp4" initialization="$RepresentationID$/p/init.mp4" timescale="90000"><SegmentTimeline><S t="1000" d="230400" r="2"/><S d="230400"/></SegmentTimeline></SegmentTemplate><Representation id="96kbps" bandwidth="96000" codecs="mp4a.40.2"/></AdaptationSet>
<AdaptationSet id="1" contentType="text"><SegmentTemplate media="x" initialization="y"><SegmentTimeline><S t="1" d="1"/></SegmentTimeline></SegmentTemplate><Representation id="sub" codecs="wvtt"/></AdaptationSet>
<AdaptationSet id="3" contentType="video" maxWidth="1920"><SegmentTemplate media="$RepresentationID$/p/t$Time$.mp4" initialization="$RepresentationID$/p/init.mp4" timescale="90000"><SegmentTimeline><S t="1000" d="230400" r="3"/></SegmentTimeline></SegmentTemplate><Representation id="1080p" bandwidth="5000000" width="1920" height="1080" codecs="avc1.640028"/><Representation id="720p" bandwidth="2800000" width="1280" height="720" codecs="avc1.64001f"/></AdaptationSet>
</Period></MPD>"#;
        let t = parse_mpd(mpd, "https://edge.cdn/ch/1/dash/live.mpd?x=1");
        assert_eq!(t.len(), 3);
        let a = &t[0];
        assert!(!a.video && a.lang.as_deref() == Some("pl"));
        assert_eq!(
            a.segs,
            vec![
                (1000, 230400),
                (231400, 230400),
                (461800, 230400),
                (692200, 230400)
            ]
        );
        assert_eq!(a.init, "https://edge.cdn/ch/1/dash/96kbps/p/init.mp4");
        let v = &t[1];
        assert!(v.video && v.width == 1920 && v.height == 1080 && v.codecs == "avc1.640028");
        assert_eq!(v.media, "https://edge.cdn/ch/1/dash/1080p/p/t$Time$.mp4");
        assert_eq!(t[2].id, "720p");
    }

    #[test]
    fn mpd_per_representation_templates_stay_separate() {
        let mpd = r#"<MPD><Period><AdaptationSet mimeType="video/mp4">
<Representation id="1" width="1920" height="1080" bandwidth="5600000" codecs="avc1.640028"><SegmentTemplate timescale="25000" media="v_1_$Number$.mp4?m=1" initialization="v_1_init.mp4?m=1" startNumber="100"><SegmentTimeline><S t="500" d="50000" r="9"/></SegmentTimeline></SegmentTemplate></Representation>
<Representation id="2" width="1280" height="720" bandwidth="2300000" codecs="avc1.64001F"><SegmentTemplate timescale="25000" media="v_3_$Number$.mp4?m=1" initialization="v_3_init.mp4?m=1" startNumber="100"><SegmentTimeline><S t="500" d="50000" r="9"/></SegmentTimeline></SegmentTemplate></Representation>
</AdaptationSet></Period></MPD>"#;
        let t = parse_mpd(mpd, "https://h/out/live.mpd");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].segs.len(), 10);
        assert_eq!(t[1].segs.len(), 10);
        assert_eq!(t[1].media, "https://h/out/v_3_$Number$.mp4?m=1");
        assert_eq!(t[1].start_number, 100);
    }
}
