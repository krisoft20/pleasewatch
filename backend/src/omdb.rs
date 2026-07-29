use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const OMDB_URL: &str = "http://www.omdbapi.com/";
const CACHE_TTL: Duration = Duration::from_secs(86400);

type Cache<K, V> = OnceLock<Mutex<HashMap<K, (V, Instant)>>>;

static MAIN_CACHE: Cache<String, Option<OmdbShow>> = OnceLock::new();
static SEASON_CACHE: Cache<(String, i32), Option<Vec<OmdbEpisode>>> = OnceLock::new();

fn cache_get<K: Eq + std::hash::Hash, V: Clone>(c: &Cache<K, V>, k: &K) -> Option<V> {
    let m = c.get_or_init(|| Mutex::new(HashMap::new()));
    let g = m.lock().ok()?;
    let (v, t) = g.get(k)?;
    if t.elapsed() < CACHE_TTL {
        Some(v.clone())
    } else {
        None
    }
}

fn cache_put<K: Eq + std::hash::Hash, V>(c: &Cache<K, V>, k: K, v: V) {
    let m = c.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut g) = m.lock() {
        g.insert(k, (v, Instant::now()));
    }
}

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(reqwest::Client::new)
}

#[derive(Debug, Clone)]
pub struct OmdbShow {
    pub total_seasons: i32,
}

#[derive(Debug, Clone)]
pub struct OmdbEpisode {
    pub is_recap: bool,
}

#[derive(Deserialize)]
struct ShowResp {
    #[serde(rename = "Response")]
    response: String,
    #[serde(rename = "totalSeasons")]
    total_seasons: Option<String>,
}

#[derive(Deserialize)]
struct SeasonResp {
    #[serde(rename = "Response")]
    response: String,
    #[serde(rename = "Episodes")]
    episodes: Option<Vec<EpisodeResp>>,
}

#[derive(Deserialize)]
struct EpisodeResp {
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Episode")]
    episode: Option<String>,
}

pub async fn show(keys: &[String], imdb_id: &str) -> Option<OmdbShow> {
    if keys.is_empty() || imdb_id.is_empty() {
        return None;
    }
    let cache_key = imdb_id.to_string();
    if let Some(cached) = cache_get(&MAIN_CACHE, &cache_key) {
        return cached;
    }

    let client = client();
    for (idx, key) in keys.iter().enumerate() {
        let url = format!("{OMDB_URL}?apikey={key}&i={imdb_id}");
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                crate::pe!("[omdb] key #{idx} request error: {e}");
                continue;
            }
        };
        let body = match resp.json::<ShowResp>().await {
            Ok(b) => b,
            Err(_) => continue,
        };
        if body.response != "True" {
            crate::pe!("[omdb] key #{idx} rejected for {imdb_id}, trying next");
            continue;
        }
        let total_seasons = body
            .total_seasons
            .as_deref()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        let out = OmdbShow { total_seasons };
        cache_put(&MAIN_CACHE, cache_key, Some(out.clone()));
        crate::pi!(
            "[omdb] cached show {imdb_id} totalSeasons={}",
            out.total_seasons
        );
        return Some(out);
    }

    crate::pe!("[omdb] all keys failed for show {imdb_id}");
    None
}

pub async fn season(keys: &[String], imdb_id: &str, season_n: i32) -> Option<Vec<OmdbEpisode>> {
    if keys.is_empty() || imdb_id.is_empty() || season_n < 1 {
        return None;
    }
    let cache_key = (imdb_id.to_string(), season_n);
    if let Some(cached) = cache_get(&SEASON_CACHE, &cache_key) {
        return cached;
    }

    let client = client();
    for (idx, key) in keys.iter().enumerate() {
        let url = format!("{OMDB_URL}?apikey={key}&i={imdb_id}&season={season_n}");
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let body = match resp.json::<SeasonResp>().await {
            Ok(b) => b,
            Err(_) => continue,
        };
        if body.response != "True" {
            crate::pe!("[omdb] key #{idx} rejected for {imdb_id} S{season_n}, trying next");
            continue;
        }
        let items: Vec<OmdbEpisode> = body
            .episodes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| {
                let _ep = e.episode?.parse::<i32>().ok()?;
                let title = e.title.unwrap_or_default();
                let lower = title.to_lowercase();
                let is_recap = lower.starts_with("recap")
                    || lower.contains("episode #")
                    || lower.contains(".5");
                Some(OmdbEpisode { is_recap })
            })
            .collect();
        let count = items.len();
        cache_put(&SEASON_CACHE, cache_key, Some(items.clone()));
        crate::pi!("[omdb] cached {imdb_id} S{season_n:02} -> {count} eps");
        return Some(items);
    }

    crate::pe!("[omdb] all keys failed for {imdb_id} S{season_n}");
    None
}

pub async fn season_ep_counts(keys: &[String], imdb_id: &str) -> Vec<i32> {
    let Some(show) = show(keys, imdb_id).await else {
        return vec![];
    };
    if show.total_seasons < 1 {
        return vec![];
    }
    let mut set = tokio::task::JoinSet::new();
    for n in 1..=show.total_seasons {
        let keys = keys.to_vec();
        let id = imdb_id.to_string();
        set.spawn(async move {
            let eps = season(&keys, &id, n).await.unwrap_or_default();
            (n, eps.iter().filter(|e| !e.is_recap).count() as i32)
        });
    }
    let mut counts = vec![0; show.total_seasons as usize];
    while let Some(res) = set.join_next().await {
        if let Ok((n, regular)) = res {
            counts[(n - 1) as usize] = regular;
        }
    }
    counts
}
