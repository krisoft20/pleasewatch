use crate::models::MangaChapter;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MK_BASE: &str = "https://mangakatana.com";
pub const MK_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36";

const RESOLVE_TTL: Duration = Duration::from_secs(86_400);
const CHAPTERS_TTL: Duration = Duration::from_secs(600);
const PAGES_TTL: Duration = Duration::from_secs(540);
const CHURL_TTL: Duration = Duration::from_secs(86_400);

type Cache<K, V> = OnceLock<Mutex<HashMap<K, (V, Instant)>>>;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static RESOLVE_CACHE: Cache<String, Option<MangaRef>> = OnceLock::new();
static CHAPTERS_CACHE: Cache<String, Vec<MangaChapter>> = OnceLock::new();
static PAGES_CACHE: Cache<String, Vec<String>> = OnceLock::new();
static CHURL_CACHE: Cache<String, String> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(MK_UA)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client")
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

#[derive(Clone)]
pub struct MangaRef {
    pub id: i64,
    pub slug: String,
}

pub async fn resolve_manga(title: &str, year: Option<i32>) -> reqwest::Result<Option<MangaRef>> {
    let key = match year {
        Some(y) => format!("{title}|{y}"),
        None => format!("{title}|"),
    };
    if let Some(hit) = cached(&RESOLVE_CACHE, &key, RESOLVE_TTL) {
        return Ok(hit);
    }

    let url = format!("{MK_BASE}/?search={}", urlencode(title));
    let html = client()
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let pick = best_match(&html, title, year);
    cache_put(&RESOLVE_CACHE, key, pick.clone());
    Ok(pick)
}

fn best_match(html: &str, title: &str, year: Option<i32>) -> Option<MangaRef> {
    let want = norm(title);
    let mut scored: Vec<(i32, MangaRef)> = Vec::new();
    let mut seen = std::collections::HashSet::<i64>::new();
    let prefix = format!("{MK_BASE}/manga/");

    for block in html.split("h3 class=\"title\"").skip(1) {
        let Some((href, after)) = find_attr(block, "href=\"", "\"") else {
            continue;
        };
        let Some(rest) = href.strip_prefix(prefix.as_str()) else {
            continue;
        };
        let trimmed = rest.trim_end_matches('/');
        let Some(dot) = trimmed.rfind('.') else {
            continue;
        };
        let slug = trimmed[..dot].to_string();
        let Some(id_str) = trimmed[dot + 1..].split('/').next() else {
            continue;
        };
        let Ok(id) = id_str.parse::<i64>() else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }

        let after_href = &block[after..];
        let title_text = match (after_href.find('>'), after_href.find("</a>")) {
            (Some(open), Some(close)) if open < close => strip_tags(&after_href[open + 1..close]),
            _ => String::new(),
        };
        let yr = extract_year(block);

        let s = score(&title_text, &want, year, yr);
        if s > 0 {
            scored.push((s, MangaRef { id, slug }));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().next().map(|(_, r)| r)
}

fn score(title_raw: &str, want: &str, want_year: Option<i32>, have_year: Option<i32>) -> i32 {
    let title_n = norm(title_raw);
    let exact = title_n == *want;
    let partial = !exact
        && (title_n.contains(want) || want.contains(&title_n))
        && (title_n.len() * 4 >= want.len() * 3 || want.len() * 4 >= title_n.len() * 3);
    if !exact && !partial {
        return 0;
    }

    let year_diff = match (have_year, want_year) {
        (Some(h), Some(w)) => Some((h - w).abs()),
        _ => None,
    };
    if partial && year_diff.is_some_and(|d| d > 2) {
        return 0;
    }

    let mut s = if exact { 100 } else { 40 };
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
    s
}

pub async fn chapters(r: &MangaRef) -> reqwest::Result<Vec<MangaChapter>> {
    let key = format!("{}:{}", r.id, r.slug);
    if let Some(hit) = cached(&CHAPTERS_CACHE, &key, CHAPTERS_TTL) {
        return Ok(hit);
    }

    let url = format!("{MK_BASE}/manga/{}.{}", r.slug, r.id);
    let html = client()
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let mut out: Vec<MangaChapter> = Vec::new();

    let prefix = format!("{MK_BASE}/manga/{}.{}/c", r.slug, r.id);
    let mut i = 0usize;
    let bytes = html.as_bytes();
    let needle = prefix.as_bytes();
    while let Some(pos) = find_subslice(&bytes[i..], needle) {
        let start = i + pos + needle.len();
        let end = bytes[start..]
            .iter()
            .position(|&b| b == b'"' || b == b'\'' || b == b' ' || b == b'>')
            .map(|p| start + p)
            .unwrap_or(bytes.len());
        let ch_part = &html[start..end];
        let ch_num = ch_part.split('/').next().unwrap_or("").to_string();
        i = end;
        if ch_num.is_empty() {
            continue;
        }

        let chapter_url = format!("{prefix}{ch_num}");
        let id = format!("mk:{}:{ch_num}", r.id);
        if out.iter().any(|c| c.id == id) {
            continue;
        }
        cache_put(&CHURL_CACHE, id.clone(), chapter_url);
        out.push(MangaChapter {
            id,
            chapter: Some(ch_num),
            title: None,
            volume: None,
            lang: "en".into(),
            pages: 0,
            published_at: None,
        });
    }

    out.reverse();
    cache_put(&CHAPTERS_CACHE, key, out.clone());
    Ok(out)
}

pub async fn pages(chapter_id: &str) -> reqwest::Result<Vec<String>> {
    let key = chapter_id.to_string();
    if let Some(hit) = cached(&PAGES_CACHE, &key, PAGES_TTL) {
        return Ok(hit);
    }
    let Some(chapter_url) = cached(&CHURL_CACHE, &key, CHURL_TTL) else {
        return Ok(Vec::new());
    };
    let html = client()
        .get(&chapter_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let urls = extract_image_urls(&html);
    cache_put(&PAGES_CACHE, key, urls.clone());
    Ok(urls)
}

fn extract_image_urls(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let needle = "https://i1.mangakatana.com/token/";
    let bytes = html.as_bytes();
    let n = needle.as_bytes();
    let mut i = 0;
    while let Some(pos) = find_subslice(&bytes[i..], n) {
        let start = i + pos;
        let end = bytes[start..]
            .iter()
            .position(|&b| b == b'"' || b == b'\'' || b == b' ' || b == b'<' || b == b'\n')
            .map(|p| start + p)
            .unwrap_or(bytes.len());
        let url = html[start..end].to_string();
        i = end;
        if url.ends_with(".jpg")
            || url.ends_with(".jpeg")
            || url.ends_with(".png")
            || url.ends_with(".webp")
        {
            if seen.insert(url.clone()) {
                out.push(url);
            }
        }
    }
    out
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

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn find_attr<'a>(s: &'a str, open: &str, close: &str) -> Option<(&'a str, usize)> {
    let start = s.find(open)? + open.len();
    let rest = &s[start..];
    let end = rest.find(close)?;
    Some((&rest[..end], start + end))
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn extract_year(block: &str) -> Option<i32> {
    let mut i = 0;
    let bytes = block.as_bytes();
    while i + 4 <= bytes.len() {
        if bytes[i] == b'1' && (bytes[i + 1] == b'9' || bytes[i + 1] == b'0') {
            if bytes[i + 2].is_ascii_digit() && bytes[i + 3].is_ascii_digit() {
                let y: i32 = std::str::from_utf8(&bytes[i..i + 4]).ok()?.parse().ok()?;
                if (1970..=2100).contains(&y) {
                    return Some(y);
                }
            }
        }
        if bytes[i] == b'2' && bytes[i + 1] == b'0' {
            if bytes[i + 2].is_ascii_digit() && bytes[i + 3].is_ascii_digit() {
                let y: i32 = std::str::from_utf8(&bytes[i..i + 4]).ok()?.parse().ok()?;
                if (1970..=2100).contains(&y) {
                    return Some(y);
                }
            }
        }
        i += 1;
    }
    None
}
