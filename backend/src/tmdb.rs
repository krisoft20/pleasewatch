use crate::models::{
    TmdbCastMember, TmdbCollection, TmdbCollectionPart, TmdbCollectionRef, TmdbDetail, TmdbEpisode,
    TmdbGenre, TmdbPersonCredit, TmdbPersonDetail, TmdbSearchItem, TmdbSeason, TmdbVideo,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const TMDB_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_POSTER_SIZE: &str = "https://image.tmdb.org/t/p/w500";
const TMDB_BACKDROP_SIZE: &str = "https://image.tmdb.org/t/p/w1280";
const TMDB_STILL_SIZE: &str = "https://image.tmdb.org/t/p/w780";
const TMDB_PROFILE_SIZE: &str = "https://image.tmdb.org/t/p/w185";
const CACHE_TTL: Duration = Duration::from_secs(3600);

type Cache<K, V> = OnceLock<Mutex<HashMap<K, (V, Instant)>>>;

static MOVIE_CACHE: Cache<(i64, String), Option<TmdbDetail>> = OnceLock::new();
static TV_CACHE: Cache<(i64, String), Option<TmdbDetail>> = OnceLock::new();
static VIDEOS_CACHE: Cache<(String, i64, String), Vec<TmdbVideo>> = OnceLock::new();
static SEASON_CACHE: Cache<(i64, i32, String), Option<Vec<TmdbEpisode>>> = OnceLock::new();
static PERSON_CACHE: Cache<(i64, String), Option<TmdbPersonDetail>> = OnceLock::new();
static DISCOVER_CACHE: Cache<(String, String), Vec<TmdbSearchItem>> = OnceLock::new();
static GENRE_CACHE: Cache<(String, String), Vec<TmdbGenre>> = OnceLock::new();
static BROWSE_CACHE: Cache<(String, String, i32, String), Vec<TmdbSearchItem>> = OnceLock::new();
static SIMILAR_CACHE: Cache<(String, i64, String), Vec<TmdbSearchItem>> = OnceLock::new();
static COLLECTION_CACHE: Cache<(i64, String), Option<TmdbCollection>> = OnceLock::new();

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

pub async fn search(api_key: &str, query: &str) -> reqwest::Result<Vec<TmdbSearchItem>> {
    let resp = client()
        .get(format!("{TMDB_BASE}/search/multi"))
        .query(&[
            ("api_key", api_key),
            ("query", query),
            ("include_adult", "false"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<SearchResponse>()
        .await?;

    Ok(resp
        .results
        .into_iter()
        .filter_map(map_search_item)
        .collect())
}

pub async fn movie(api_key: &str, tmdb_id: i64, lang: &str) -> reqwest::Result<Option<TmdbDetail>> {
    let key = (tmdb_id, lang.to_string());
    if let Some(hit) = cache_get(&MOVIE_CACHE, &key) {
        return Ok(hit);
    }
    let r = client()
        .get(format!("{TMDB_BASE}/movie/{tmdb_id}"))
        .query(&[
            ("api_key", api_key),
            ("append_to_response", "credits,external_ids"),
            ("language", lang),
        ])
        .send()
        .await?;

    if r.status() == reqwest::StatusCode::NOT_FOUND {
        cache_put(&MOVIE_CACHE, key, None);
        return Ok(None);
    }

    let m: MovieDetail = r.error_for_status()?.json().await?;
    let out = Some(map_movie(m));
    cache_put(&MOVIE_CACHE, key, out.clone());
    Ok(out)
}

pub async fn tv(api_key: &str, tmdb_id: i64, lang: &str) -> reqwest::Result<Option<TmdbDetail>> {
    let key = (tmdb_id, lang.to_string());
    if let Some(hit) = cache_get(&TV_CACHE, &key) {
        return Ok(hit);
    }
    let r = client()
        .get(format!("{TMDB_BASE}/tv/{tmdb_id}"))
        .query(&[
            ("api_key", api_key),
            ("append_to_response", "credits,external_ids"),
            ("language", lang),
        ])
        .send()
        .await?;

    if r.status() == reqwest::StatusCode::NOT_FOUND {
        cache_put(&TV_CACHE, key, None);
        return Ok(None);
    }

    let t: TvDetail = r.error_for_status()?.json().await?;
    let out = Some(map_tv(t));
    cache_put(&TV_CACHE, key, out.clone());
    Ok(out)
}

pub async fn videos(
    api_key: &str,
    kind: &str,
    tmdb_id: i64,
    lang: &str,
) -> reqwest::Result<Vec<TmdbVideo>> {
    let key = (kind.to_string(), tmdb_id, lang.to_string());
    if let Some(hit) = cache_get(&VIDEOS_CACHE, &key) {
        return Ok(hit);
    }
    let r = client()
        .get(format!("{TMDB_BASE}/{kind}/{tmdb_id}/videos"))
        .query(&[("api_key", api_key), ("language", lang)])
        .send()
        .await?
        .error_for_status()?
        .json::<VideosResponse>()
        .await?;
    let mut out: Vec<TmdbVideo> = r.results.into_iter().filter_map(map_video).collect();
    out.sort_by_key(|v| if v.kind == "Trailer" { 0 } else { 1 });
    cache_put(&VIDEOS_CACHE, key, out.clone());
    Ok(out)
}

pub async fn season(
    api_key: &str,
    tmdb_id: i64,
    season: i32,
    lang: &str,
) -> reqwest::Result<Option<Vec<TmdbEpisode>>> {
    let key = (tmdb_id, season, lang.to_string());
    if let Some(hit) = cache_get(&SEASON_CACHE, &key) {
        return Ok(hit);
    }
    let r = client()
        .get(format!("{TMDB_BASE}/tv/{tmdb_id}/season/{season}"))
        .query(&[("api_key", api_key), ("language", lang)])
        .send()
        .await?;

    if r.status() == reqwest::StatusCode::NOT_FOUND {
        cache_put(&SEASON_CACHE, key, None);
        return Ok(None);
    }

    let s: SeasonDetail = r.error_for_status()?.json().await?;
    let out = Some(s.episodes.into_iter().map(map_episode).collect::<Vec<_>>());
    cache_put(&SEASON_CACHE, key, out.clone());
    Ok(out)
}

pub async fn discover_list(
    api_key: &str,
    kind: &str,
    lang: &str,
) -> reqwest::Result<Vec<TmdbSearchItem>> {
    let key = (kind.to_string(), lang.to_string());
    if let Some(hit) = cache_get(&DISCOVER_CACHE, &key) {
        return Ok(hit);
    }
    let path = match kind {
        "trending" => "/trending/all/week".to_string(),
        "popular_movies" => "/movie/popular".to_string(),
        "popular_tv" => "/tv/popular".to_string(),
        "top_rated_movies" => "/movie/top_rated".to_string(),
        "top_rated_tv" => "/tv/top_rated".to_string(),
        _ => "/trending/all/week".to_string(),
    };
    let r = client()
        .get(format!("{TMDB_BASE}{path}"))
        .query(&[("api_key", api_key), ("language", lang)])
        .send()
        .await?
        .error_for_status()?
        .json::<SearchResponse>()
        .await?;
    let infer = match kind {
        "popular_movies" | "top_rated_movies" => Some("movie"),
        "popular_tv" | "top_rated_tv" => Some("tv"),
        _ => None,
    };
    let items: Vec<TmdbSearchItem> = r
        .results
        .into_iter()
        .filter_map(|s| {
            let mut item = s;
            if item.media_type.is_none() {
                item.media_type = infer.map(|s| s.to_string());
            }
            map_search_item(item)
        })
        .collect();
    cache_put(&DISCOVER_CACHE, key, items.clone());
    Ok(items)
}

pub async fn genres(api_key: &str, kind: &str, lang: &str) -> reqwest::Result<Vec<TmdbGenre>> {
    if kind != "movie" && kind != "tv" {
        return Ok(vec![]);
    }
    let key = (kind.to_string(), lang.to_string());
    if let Some(hit) = cache_get(&GENRE_CACHE, &key) {
        return Ok(hit);
    }
    let resp = client()
        .get(format!("{TMDB_BASE}/genre/{kind}/list"))
        .query(&[("api_key", api_key), ("language", lang)])
        .send()
        .await?
        .error_for_status()?
        .json::<GenreResponse>()
        .await?;
    cache_put(&GENRE_CACHE, key, resp.genres.clone());
    Ok(resp.genres)
}

pub async fn browse(
    api_key: &str,
    kind: &str,
    genre_ids: &[i64],
    page: i32,
    lang: &str,
) -> reqwest::Result<Vec<TmdbSearchItem>> {
    if kind != "movie" && kind != "tv" {
        return Ok(vec![]);
    }
    let page = page.clamp(1, 500);
    let genres = genre_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let key = (kind.to_string(), genres.clone(), page, lang.to_string());
    if let Some(hit) = cache_get(&BROWSE_CACHE, &key) {
        return Ok(hit);
    }

    let mut query = vec![
        ("api_key".to_string(), api_key.to_string()),
        ("language".to_string(), lang.to_string()),
        ("include_adult".to_string(), "false".to_string()),
        ("sort_by".to_string(), "popularity.desc".to_string()),
        ("page".to_string(), page.to_string()),
    ];
    if !genres.is_empty() {
        query.push(("with_genres".to_string(), genres));
    }

    let resp = client()
        .get(format!("{TMDB_BASE}/discover/{kind}"))
        .query(&query)
        .send()
        .await?
        .error_for_status()?
        .json::<SearchResponse>()
        .await?;
    let items = resp
        .results
        .into_iter()
        .filter_map(|mut s| {
            s.media_type = Some(kind.to_string());
            map_search_item(s)
        })
        .collect::<Vec<_>>();
    cache_put(&BROWSE_CACHE, key, items.clone());
    Ok(items)
}

pub async fn similar(
    api_key: &str,
    kind: &str,
    tmdb_id: i64,
    lang: &str,
) -> reqwest::Result<Vec<TmdbSearchItem>> {
    if kind != "movie" && kind != "tv" {
        return Ok(vec![]);
    }
    let key = (kind.to_string(), tmdb_id, lang.to_string());
    if let Some(hit) = cache_get(&SIMILAR_CACHE, &key) {
        return Ok(hit);
    }
    let r = client()
        .get(format!("{TMDB_BASE}/{kind}/{tmdb_id}/recommendations"))
        .query(&[("api_key", api_key), ("language", lang)])
        .send()
        .await?
        .error_for_status()?
        .json::<SearchResponse>()
        .await?;
    let items: Vec<TmdbSearchItem> = r
        .results
        .into_iter()
        .filter_map(|mut s| {
            if s.media_type.is_none() {
                s.media_type = Some(kind.to_string());
            }
            map_search_item(s)
        })
        .collect();
    cache_put(&SIMILAR_CACHE, key, items.clone());
    Ok(items)
}

pub async fn collection(
    api_key: &str,
    id: i64,
    lang: &str,
) -> reqwest::Result<Option<TmdbCollection>> {
    let key = (id, lang.to_string());
    if let Some(hit) = cache_get(&COLLECTION_CACHE, &key) {
        return Ok(hit);
    }
    let r = client()
        .get(format!("{TMDB_BASE}/collection/{id}"))
        .query(&[("api_key", api_key), ("language", lang)])
        .send()
        .await?;

    if r.status() == reqwest::StatusCode::NOT_FOUND {
        cache_put(&COLLECTION_CACHE, key, None);
        return Ok(None);
    }

    let c: CollectionDetail = r.error_for_status()?.json().await?;

    let mut parts: Vec<TmdbCollectionPart> = c
        .parts
        .into_iter()
        .map(|p| {
            let year = p
                .release_date
                .as_ref()
                .and_then(|d| d.get(..4))
                .map(str::to_string);
            TmdbCollectionPart {
                tmdb_id: p.id,
                title: p.title.or(p.name).unwrap_or_default(),
                year,
                release_date: p.release_date,
                overview: p.overview,
                poster_url: p.poster_path.map(poster_url),
                backdrop_url: p.backdrop_path.map(backdrop_url),
                vote_average: p.vote_average,
            }
        })
        .filter(|p| !p.title.is_empty())
        .collect();

    parts.sort_by(|a, b| match (&a.release_date, &b.release_date) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    let out = Some(TmdbCollection {
        id: c.id,
        name: c.name,
        overview: c.overview,
        poster_url: c.poster_path.map(poster_url),
        backdrop_url: c.backdrop_path.map(backdrop_url),
        parts,
    });
    cache_put(&COLLECTION_CACHE, key, out.clone());
    Ok(out)
}

pub async fn person(
    api_key: &str,
    person_id: i64,
    lang: &str,
) -> reqwest::Result<Option<TmdbPersonDetail>> {
    let key = (person_id, lang.to_string());
    if let Some(hit) = cache_get(&PERSON_CACHE, &key) {
        return Ok(hit);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("build reqwest client");
    let r = client
        .get(format!("{TMDB_BASE}/person/{person_id}"))
        .query(&[
            ("api_key", api_key),
            ("append_to_response", "combined_credits"),
            ("language", lang),
        ])
        .send()
        .await?;

    if r.status() == reqwest::StatusCode::NOT_FOUND {
        cache_put(&PERSON_CACHE, key, None);
        return Ok(None);
    }

    let p: PersonRaw = r.error_for_status()?.json().await?;
    let out = Some(map_person(p));
    cache_put(&PERSON_CACHE, key, out.clone());
    Ok(out)
}

#[derive(Deserialize)]
struct PersonRaw {
    id: i64,
    name: String,
    biography: Option<String>,
    birthday: Option<String>,
    deathday: Option<String>,
    place_of_birth: Option<String>,
    profile_path: Option<String>,
    known_for_department: Option<String>,
    #[serde(default)]
    also_known_as: Vec<String>,
    #[serde(default)]
    combined_credits: PersonCredits,
}

#[derive(Deserialize, Default)]
struct PersonCredits {
    #[serde(default)]
    cast: Vec<PersonCreditItem>,
}

#[derive(Deserialize)]
struct PersonCreditItem {
    id: i64,
    media_type: String,
    title: Option<String>,
    name: Option<String>,
    character: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    poster_path: Option<String>,
    vote_average: Option<f64>,
}

fn map_person(p: PersonRaw) -> TmdbPersonDetail {
    let mut seen = std::collections::HashSet::<(i64, String)>::new();
    let mut all_credits: Vec<TmdbPersonCredit> = p
        .combined_credits
        .cast
        .into_iter()
        .filter_map(|c| {
            let key = (c.id, c.media_type.clone());
            if !seen.insert(key) {
                return None;
            }
            let title = c.title.clone().or(c.name.clone())?;
            let year = c
                .release_date
                .as_deref()
                .or(c.first_air_date.as_deref())
                .and_then(|d| d.get(..4))
                .map(|s| s.to_string());
            Some(TmdbPersonCredit {
                tmdb_id: c.id,
                media_type: c.media_type,
                title,
                year,
                poster_url: c.poster_path.map(|p| format!("{TMDB_POSTER_SIZE}{p}")),
                character: c.character.filter(|s| !s.is_empty()),
                vote_average: c.vote_average,
            })
        })
        .collect();

    let total = all_credits.len() as i32;
    let years: Vec<i32> = all_credits
        .iter()
        .filter_map(|c| c.year.as_deref().and_then(|y| y.parse::<i32>().ok()))
        .filter(|y| *y >= 1900 && *y <= 2100)
        .collect();
    let career_start = years.iter().min().copied();
    let career_end = years.iter().max().copied();

    all_credits.sort_by(|a, b| {
        b.vote_average
            .unwrap_or(0.0)
            .partial_cmp(&a.vote_average.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    all_credits.truncate(20);

    let also = p
        .also_known_as
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .take(4)
        .collect();

    TmdbPersonDetail {
        id: p.id,
        name: p.name,
        biography: p.biography.filter(|s| !s.is_empty()),
        birthday: p.birthday,
        deathday: p.deathday,
        place_of_birth: p.place_of_birth,
        photo_url: p.profile_path.map(profile_url),
        known_for_department: p.known_for_department,
        also_known_as: also,
        total_credits: total,
        career_start,
        career_end,
        credits: all_credits,
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct GenreResponse {
    genres: Vec<TmdbGenre>,
}

#[derive(Deserialize)]
struct SearchItem {
    id: i64,
    media_type: Option<String>,
    title: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    vote_average: Option<f64>,
    genre_ids: Option<Vec<i64>>,
}

#[derive(Deserialize)]
struct MovieDetail {
    id: i64,
    imdb_id: Option<String>,
    title: String,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    runtime: Option<i32>,
    vote_average: Option<f64>,
    #[serde(default)]
    genres: Vec<Genre>,
    credits: Option<Credits>,
    belongs_to_collection: Option<CollectionRef>,
}

#[derive(Deserialize)]
struct CollectionRef {
    id: i64,
    name: String,
    poster_path: Option<String>,
}

#[derive(Deserialize)]
struct CollectionDetail {
    id: i64,
    name: String,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    parts: Vec<CollectionPart>,
}

#[derive(Deserialize)]
struct CollectionPart {
    id: i64,
    title: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    vote_average: Option<f64>,
}

#[derive(Deserialize)]
struct TvDetail {
    id: i64,
    name: String,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    first_air_date: Option<String>,
    vote_average: Option<f64>,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default)]
    origin_country: Vec<String>,
    #[serde(default)]
    seasons: Vec<SeasonItem>,
    credits: Option<Credits>,
    external_ids: Option<ExternalIds>,
}

#[derive(Deserialize)]
struct ExternalIds {
    imdb_id: Option<String>,
}

#[derive(Deserialize)]
struct Credits {
    #[serde(default)]
    cast: Vec<CastItem>,
}

#[derive(Deserialize)]
struct CastItem {
    id: i64,
    name: String,
    #[serde(default)]
    character: String,
    profile_path: Option<String>,
}

#[derive(Deserialize)]
struct VideosResponse {
    #[serde(default)]
    results: Vec<VideoItem>,
}

#[derive(Deserialize)]
struct VideoItem {
    key: String,
    name: String,
    site: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct SeasonItem {
    season_number: i32,
    name: String,
    episode_count: i32,
    overview: Option<String>,
    poster_path: Option<String>,
}

#[derive(Deserialize)]
struct Genre {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
struct SeasonDetail {
    #[serde(default)]
    episodes: Vec<EpisodeItem>,
}

#[derive(Deserialize)]
struct EpisodeItem {
    episode_number: i32,
    season_number: i32,
    name: String,
    overview: Option<String>,
    air_date: Option<String>,
    still_path: Option<String>,
    runtime: Option<i32>,
    vote_average: Option<f64>,
}

fn map_episode(e: EpisodeItem) -> TmdbEpisode {
    TmdbEpisode {
        episode_number: e.episode_number,
        season_number: e.season_number,
        name: e.name,
        overview: e.overview,
        air_date: e.air_date,
        still_url: e.still_path.map(still_url),
        runtime: e.runtime,
        vote_average: e.vote_average,
    }
}

fn map_search_item(it: SearchItem) -> Option<TmdbSearchItem> {
    let mt = it.media_type.as_deref()?;
    if mt != "movie" && mt != "tv" {
        return None;
    }
    let title = it.title.or(it.name)?;
    let date = it.release_date.or(it.first_air_date);
    let year = date.as_ref().and_then(|d| d.get(..4)).map(str::to_string);

    Some(TmdbSearchItem {
        tmdb_id: it.id,
        media_type: mt.to_string(),
        title,
        year,
        overview: it.overview,
        poster_url: it.poster_path.map(poster_url),
        backdrop_url: it.backdrop_path.map(backdrop_url),
        vote_average: it.vote_average,
        genre_ids: it.genre_ids,
    })
}

fn map_movie(m: MovieDetail) -> TmdbDetail {
    let year = m
        .release_date
        .as_ref()
        .and_then(|d| d.get(..4))
        .map(str::to_string);

    let cast = m
        .credits
        .map(|c| c.cast.into_iter().take(12).map(map_cast).collect())
        .unwrap_or_default();

    let belongs_to_collection = m.belongs_to_collection.map(|c| TmdbCollectionRef {
        id: c.id,
        name: c.name,
        poster_url: c.poster_path.map(poster_url),
    });

    TmdbDetail {
        tmdb_id: m.id,
        imdb_id: m.imdb_id.filter(|s| !s.is_empty()),
        media_type: "movie".into(),
        title: m.title,
        year,
        overview: m.overview,
        poster_url: m.poster_path.map(poster_url),
        backdrop_url: m.backdrop_path.map(backdrop_url),
        vote_average: m.vote_average,
        runtime: m.runtime,
        genres: m.genres.into_iter().map(|g| g.name).collect(),
        is_anime: false,
        seasons: None,
        cast,
        omdb_seasons: None,
        belongs_to_collection,
    }
}

fn map_tv(t: TvDetail) -> TmdbDetail {
    let year = t
        .first_air_date
        .as_ref()
        .and_then(|d| d.get(..4))
        .map(str::to_string);

    let seasons = t
        .seasons
        .into_iter()
        .filter(|s| s.season_number > 0)
        .map(|s| TmdbSeason {
            season_number: s.season_number,
            name: s.name,
            episode_count: s.episode_count,
            overview: s.overview,
            poster_url: s.poster_path.map(poster_url),
        })
        .collect();

    let cast = t
        .credits
        .map(|c| c.cast.into_iter().take(12).map(map_cast).collect())
        .unwrap_or_default();

    let imdb_id = t
        .external_ids
        .and_then(|e| e.imdb_id)
        .filter(|s| !s.is_empty());
    let is_anime = t.origin_country.iter().any(|country| country == "JP")
        && t.genres.iter().any(|genre| genre.id == 16);

    TmdbDetail {
        tmdb_id: t.id,
        imdb_id,
        media_type: "tv".into(),
        title: t.name,
        year,
        overview: t.overview,
        poster_url: t.poster_path.map(poster_url),
        backdrop_url: t.backdrop_path.map(backdrop_url),
        vote_average: t.vote_average,
        runtime: None,
        genres: t.genres.into_iter().map(|g| g.name).collect(),
        is_anime,
        seasons: Some(seasons),
        cast,
        omdb_seasons: None,
        belongs_to_collection: None,
    }
}

fn poster_url(path: String) -> String {
    format!("{TMDB_POSTER_SIZE}{path}")
}

fn backdrop_url(path: String) -> String {
    format!("{TMDB_BACKDROP_SIZE}{path}")
}

fn still_url(path: String) -> String {
    format!("{TMDB_STILL_SIZE}{path}")
}

fn profile_url(path: String) -> String {
    format!("{TMDB_PROFILE_SIZE}{path}")
}

fn map_cast(c: CastItem) -> TmdbCastMember {
    TmdbCastMember {
        id: c.id,
        name: c.name,
        character: c.character,
        photo_url: c.profile_path.map(profile_url),
    }
}

fn map_video(v: VideoItem) -> Option<TmdbVideo> {
    if v.site != "YouTube" {
        return None;
    }
    if v.kind != "Trailer" && v.kind != "Teaser" {
        return None;
    }
    Some(TmdbVideo {
        key: v.key,
        name: v.name,
        kind: v.kind,
    })
}
