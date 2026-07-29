use crate::models::MangaChapter;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CK_API: &str = "https://api.comick.dev";
pub const CK_CDN: &str = "https://meo.comick.pictures";
pub const CK_REF: &str = "https://comick.dev/";
pub const CK_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36";

const HID_TTL: Duration = Duration::from_secs(86_400);
const CHAPTERS_TTL: Duration = Duration::from_secs(600);
const PAGES_TTL: Duration = Duration::from_secs(540);

type Cache<K, V> = OnceLock<Mutex<HashMap<K, (V, Instant)>>>;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static HID_CACHE: Cache<String, Option<String>> = OnceLock::new();
static CHAPTERS_CACHE: Cache<(String, String), Vec<MangaChapter>> = OnceLock::new();
static PAGES_CACHE: Cache<String, Vec<String>> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        let mut b = reqwest::Client::builder()
            .user_agent(CK_UA)
            .timeout(Duration::from_secs(15));
        if let Ok(p) = std::env::var("MANGA_CK_PROXY") {
            match reqwest::Proxy::all(&p) {
                Ok(px) => {
                    println!("[manga_ck] routing through proxy {p}");
                    b = b.proxy(px);
                }
                Err(e) => eprintln!("[manga_ck] bad MANGA_CK_PROXY {p}: {e}"),
            }
        }
        b.build().expect("reqwest client")
    })
}

fn cached<K, V>(c: &Cache<K, V>, k: &K, ttl: Duration) -> Option<V>
where
    K: Eq + std::hash::Hash,
    V: Clone,
{
    let m = c.get_or_init(|| Mutex::new(HashMap::new()));
    let g = m.lock().ok()?;
    let (v, t) = g.get(k)?;
    if t.elapsed() < ttl {
        Some(v.clone())
    } else {
        None
    }
}

fn cache_put<K, V>(c: &Cache<K, V>, k: K, v: V)
where
    K: Eq + std::hash::Hash,
{
    let m = c.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut g) = m.lock() {
        g.insert(k, (v, Instant::now()));
    }
}

fn fetch(url: String) -> reqwest::RequestBuilder {
    client().get(url).header("Referer", CK_REF)
}

pub async fn search(query: &str) -> reqwest::Result<Vec<SearchHit>> {
    let r: Vec<SearchHit> = fetch(format!("{CK_API}/v1.0/search"))
        .query(&[("q", query), ("limit", "8"), ("type", "comic")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r)
}

pub async fn resolve_hid(title: &str, year: Option<i32>) -> reqwest::Result<Option<String>> {
    let key = match year {
        Some(y) => format!("{title}|{y}"),
        None => format!("{title}|"),
    };
    if let Some(hit) = cached(&HID_CACHE, &key, HID_TTL) {
        return Ok(hit);
    }
    let hits = search(title).await?;
    let pick = best_match(&hits, title, year);
    cache_put(&HID_CACHE, key, pick.clone());
    Ok(pick)
}

fn best_match(hits: &[SearchHit], title: &str, year: Option<i32>) -> Option<String> {
    let want = norm(title);
    let mut scored: Vec<(i32, &SearchHit)> = hits
        .iter()
        .map(|h| (score(h, &want, year), h))
        .filter(|(s, _)| *s > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.first().map(|(_, h)| h.hid.clone())
}

fn score(h: &SearchHit, want: &str, want_year: Option<i32>) -> i32 {
    let title_n = norm(&h.title);
    let alt_exact = h.md_titles.iter().any(|t| norm(&t.title) == *want);

    let exact = title_n == *want;
    let partial = !exact
        && !alt_exact
        && (title_n.contains(want) || want.contains(&title_n))
        && (title_n.len() * 4 >= want.len() * 3 || want.len() * 4 >= title_n.len() * 3);

    if !exact && !alt_exact && !partial {
        return 0;
    }

    let year_diff = match (h.year, want_year) {
        (Some(y), Some(w)) => Some((y - w).abs()),
        _ => None,
    };

    if partial && year_diff.is_some_and(|d| d > 2) {
        return 0;
    }

    let mut s = if exact {
        100
    } else if alt_exact {
        80
    } else {
        40
    };
    if let Some(d) = year_diff {
        if d == 0 {
            s += 20;
        } else if d <= 1 {
            s += 10;
        } else if d <= 3 {
            s += 3;
        } else {
            s -= 5;
        }
    }
    if h.country == "jp" {
        s += 2;
    }
    s += (h.view_count.unwrap_or(0) / 100_000).min(10) as i32;
    s
}

fn norm(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() {
                Some(' ')
            } else {
                None
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn chapters(hid: &str, lang: &str) -> reqwest::Result<Vec<MangaChapter>> {
    let key = (hid.to_string(), lang.to_string());
    if let Some(hit) = cached(&CHAPTERS_CACHE, &key, CHAPTERS_TTL) {
        return Ok(hit);
    }

    let mut out: Vec<MangaChapter> = Vec::new();
    let mut page: i32 = 1;
    loop {
        let r: ChapterPage = fetch(format!("{CK_API}/comic/{hid}/chapters"))
            .query(&[
                ("lang", lang),
                ("page", &page.to_string()),
                ("limit", "100"),
                ("chap-order", "1"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let n = r.chapters.len();
        for c in r.chapters {
            out.push(MangaChapter {
                id: format!("ck:{}", c.hid),
                chapter: c.chap,
                title: c.title,
                volume: c.vol,
                lang: c.lang,
                pages: 0,
                published_at: c.publish_at,
            });
        }
        if n == 0 || (page as i64) * 100 >= r.total || page >= 40 {
            break;
        }
        page += 1;
    }

    cache_put(&CHAPTERS_CACHE, key, out.clone());
    Ok(out)
}

pub async fn pages(chapter_hid: &str) -> reqwest::Result<Vec<String>> {
    let key = chapter_hid.to_string();
    if let Some(hit) = cached(&PAGES_CACHE, &key, PAGES_TTL) {
        return Ok(hit);
    }
    let r: ChapterDetail = fetch(format!("{CK_API}/chapter/{chapter_hid}"))
        .query(&[("tachiyomi", "true")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let urls: Vec<String> = r
        .chapter
        .md_images
        .into_iter()
        .map(|img| format!("{CK_CDN}/{}", img.b))
        .collect();
    cache_put(&PAGES_CACHE, key, urls.clone());
    Ok(urls)
}

#[derive(Deserialize, Clone)]
pub struct SearchHit {
    pub hid: String,
    pub title: String,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub view_count: Option<i64>,
    #[serde(default)]
    pub md_titles: Vec<MdTitle>,
}

#[derive(Deserialize, Clone)]
pub struct MdTitle {
    pub title: String,
}

#[derive(Deserialize)]
struct ChapterPage {
    chapters: Vec<RawChapter>,
    #[serde(default)]
    total: i64,
}

#[derive(Deserialize)]
struct RawChapter {
    hid: String,
    #[serde(default)]
    chap: Option<String>,
    #[serde(default)]
    vol: Option<String>,
    #[serde(default)]
    title: Option<String>,
    lang: String,
    #[serde(default)]
    publish_at: Option<String>,
}

#[derive(Deserialize)]
struct ChapterDetail {
    chapter: ChapterImages,
}

#[derive(Deserialize)]
struct ChapterImages {
    #[serde(default)]
    md_images: Vec<MdImage>,
}

#[derive(Deserialize)]
struct MdImage {
    b: String,
}
