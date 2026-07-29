use crate::{
    models::{ApiError, DiscoverGenresResponse, DiscoverResponse, TmdbSearchItem},
    tmdb, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/search", get(handle_search))
        .route("/api/search/{kind}/{tmdb_id}", get(handle_detail))
        .route("/api/search/{kind}/{tmdb_id}/videos", get(handle_videos))
        .route("/api/search/{kind}/{tmdb_id}/similar", get(handle_similar))
        .route("/api/search/tv/{tmdb_id}/season/{n}", get(handle_season))
        .route("/api/tmdb/person/{person_id}", get(handle_person))
        .route("/api/tmdb/collection/{id}", get(handle_collection))
        .route("/api/discover", get(handle_discover))
        .route("/api/discover/genres", get(handle_discover_genres))
        .route("/api/discover/browse", get(handle_discover_browse))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Deserialize)]
struct LangQuery {
    lang: Option<String>,
}

#[derive(Deserialize)]
struct BrowseQuery {
    kind: Option<String>,
    genres: Option<String>,
    page: Option<i32>,
    lang: Option<String>,
}

fn lang_or_default(l: Option<String>) -> String {
    l.filter(|s| !s.is_empty())
        .unwrap_or_else(|| "en-US".into())
}

fn parse_genre_ids(raw: Option<String>) -> Vec<i64> {
    raw.unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .take(12)
        .collect()
}

async fn handle_search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }

    let raw = q.q.unwrap_or_default();
    if raw.trim().is_empty() {
        return Json::<Vec<TmdbSearchItem>>(Vec::new()).into_response();
    }
    let query = strip_se_tokens(&raw);
    let query = if query.is_empty() {
        raw.trim().to_string()
    } else {
        query
    };

    match tmdb::search(&key, &query).await {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            eprintln!("[search] tmdb error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

fn strip_se_tokens(input: &str) -> String {
    let lower = input.to_lowercase();
    let bytes = lower.as_bytes();
    let mut keep = vec![true; bytes.len()];
    let n = bytes.len();

    let is_sep = |b: u8| matches!(b, b' ' | b'.' | b'_' | b'-');
    let at_word_start = |i: usize| i == 0 || is_sep(bytes[i - 1]);

    let mut i = 0;
    while i < n {
        if !at_word_start(i) {
            i += 1;
            continue;
        }

        if bytes[i] == b's'
            && i + 2 < n
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
        {
            let mut j = i + 3;
            if j < n && bytes[j] == b'e' && j + 1 < n && bytes[j + 1].is_ascii_digit() {
                j += 2;
                if j < n && bytes[j].is_ascii_digit() {
                    j += 1;
                }
            }
            if j == n || is_sep(bytes[j]) {
                for k in i..j {
                    keep[k] = false;
                }
                i = j;
                continue;
            }
        }

        if bytes[i].is_ascii_digit() {
            let mut j = i + 1;
            if j < n && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < n && bytes[j] == b'x' && j + 1 < n && bytes[j + 1].is_ascii_digit() {
                let mut k = j + 1;
                while k < n && bytes[k].is_ascii_digit() && k - j <= 3 {
                    k += 1;
                }
                if (k - j) >= 2 && (k == n || is_sep(bytes[k])) {
                    for m in i..k {
                        keep[m] = false;
                    }
                    i = k;
                    continue;
                }
            }
        }

        if lower[i..].starts_with("season ") {
            let mut j = i + 7;
            while j < n && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 7 && (j == n || is_sep(bytes[j])) {
                for k in i..j {
                    keep[k] = false;
                }
                i = j;
                continue;
            }
        }

        i += 1;
    }

    let cleaned: String = input
        .chars()
        .zip(keep.iter())
        .filter_map(|(c, k)| if *k { Some(c) } else { None })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn handle_detail(
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);

    let result = match kind.as_str() {
        "movie" => tmdb::movie(&key, tmdb_id, &lang).await,
        "tv" => tmdb::tv(&key, tmdb_id, &lang).await,
        _ => return error_resp(StatusCode::BAD_REQUEST, "kind must be 'movie' or 'tv'"),
    };

    match result {
        Ok(Some(mut d)) => {
            if kind == "tv" {
                let is_animation = d.genres.iter().any(|g| g.eq_ignore_ascii_case("animation"));
                if is_animation {
                    if let Some(imdb_id) = d.imdb_id.clone() {
                        let omdb_keys = state.omdb_keys_rotated().await;
                        if !omdb_keys.is_empty() {
                            let counts = crate::omdb::season_ep_counts(&omdb_keys, &imdb_id).await;
                            let tmdb_seasons = d
                                .seasons
                                .as_ref()
                                .map(|s| s.iter().filter(|x| x.season_number >= 1).count() as i32)
                                .unwrap_or(0);
                            if counts.len() as i32 > tmdb_seasons
                                && !counts.is_empty()
                                && tmdb_seasons <= 1
                            {
                                crate::pi!("[omdb] {imdb_id} re-bucket (animation): tmdb={tmdb_seasons}seasons omdb={:?}", counts);
                                d.omdb_seasons = Some(counts);
                            }
                        }
                    }
                }
            }
            Json(d).into_response()
        }
        Ok(None) => error_resp(StatusCode::NOT_FOUND, "not found"),
        Err(e) => {
            eprintln!("[search] tmdb error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

async fn handle_season(
    State(state): State<Arc<AppState>>,
    Path((tmdb_id, n)): Path<(i64, i32)>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "missing tmdb key");
    }
    let lang = lang_or_default(q.lang);

    match tmdb::season(&key, tmdb_id, n, &lang).await {
        Ok(Some(eps)) => Json(eps).into_response(),
        Ok(None) => error_resp(StatusCode::NOT_FOUND, "season not found"),
        Err(e) => {
            eprintln!("[search] tmdb season error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

async fn handle_videos(
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    if kind != "movie" && kind != "tv" {
        return error_resp(StatusCode::BAD_REQUEST, "kind must be 'movie' or 'tv'");
    }
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);

    match tmdb::videos(&key, &kind, tmdb_id, &lang).await {
        Ok(vs) => Json(vs).into_response(),
        Err(e) => {
            eprintln!("[search] tmdb videos error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

async fn handle_person(
    State(state): State<Arc<AppState>>,
    Path(person_id): Path<i64>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);
    println!("[search] person {person_id} lang={lang} start");

    let result = tmdb::person(&key, person_id, &lang).await;
    match result {
        Ok(Some(p)) => {
            println!(
                "[search] person {person_id} ok ({} credits)",
                p.credits.len()
            );
            (
                [(axum::http::header::CACHE_CONTROL, "private, max-age=3600")],
                Json(p),
            )
                .into_response()
        }
        Ok(None) => {
            println!("[search] person {person_id} not found");
            error_resp(StatusCode::NOT_FOUND, "person not found")
        }
        Err(e) => {
            eprintln!("[search] tmdb person error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

async fn handle_collection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    let api_key = state.tmdb_key().await;
    if api_key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);
    match tmdb::collection(&api_key, id, &lang).await {
        Ok(Some(c)) => {
            println!("[search] collection {id} ok ({} parts)", c.parts.len());
            (
                [(axum::http::header::CACHE_CONTROL, "private, max-age=3600")],
                Json(c),
            )
                .into_response()
        }
        Ok(None) => error_resp(StatusCode::NOT_FOUND, "collection not found"),
        Err(e) => {
            eprintln!("[search] tmdb collection error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

async fn handle_similar(
    State(state): State<Arc<AppState>>,
    Path((kind, tmdb_id)): Path<(String, i64)>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    if kind != "movie" && kind != "tv" {
        return error_resp(StatusCode::BAD_REQUEST, "kind must be 'movie' or 'tv'");
    }
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);

    match tmdb::similar(&key, &kind, tmdb_id, &lang).await {
        Ok(items) => (
            [(axum::http::header::CACHE_CONTROL, "private, max-age=3600")],
            Json(items),
        )
            .into_response(),
        Err(e) => {
            eprintln!("[search] tmdb similar error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

async fn handle_discover(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);

    let (trending, popular_movies, popular_tv, top_rated_movies, top_rated_tv) = tokio::join!(
        tmdb::discover_list(&key, "trending", &lang),
        tmdb::discover_list(&key, "popular_movies", &lang),
        tmdb::discover_list(&key, "popular_tv", &lang),
        tmdb::discover_list(&key, "top_rated_movies", &lang),
        tmdb::discover_list(&key, "top_rated_tv", &lang),
    );

    let resp = DiscoverResponse {
        trending: trending.unwrap_or_default(),
        popular_movies: popular_movies.unwrap_or_default(),
        popular_tv: popular_tv.unwrap_or_default(),
        top_rated_movies: top_rated_movies.unwrap_or_default(),
        top_rated_tv: top_rated_tv.unwrap_or_default(),
    };
    (
        [(axum::http::header::CACHE_CONTROL, "private, max-age=900")],
        Json(resp),
    )
        .into_response()
}

async fn handle_discover_genres(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LangQuery>,
) -> impl IntoResponse {
    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);

    let (movie, tv) = tokio::join!(
        tmdb::genres(&key, "movie", &lang),
        tmdb::genres(&key, "tv", &lang),
    );

    (
        [(axum::http::header::CACHE_CONTROL, "private, max-age=3600")],
        Json(DiscoverGenresResponse {
            movie: movie.unwrap_or_default(),
            tv: tv.unwrap_or_default(),
        }),
    )
        .into_response()
}

async fn handle_discover_browse(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BrowseQuery>,
) -> impl IntoResponse {
    let kind = q.kind.unwrap_or("movie".to_string());
    if kind != "movie" && kind != "tv" {
        return error_resp(StatusCode::BAD_REQUEST, "kind must be 'movie' or 'tv'");
    }

    let key = state.tmdb_key().await;
    if key.is_empty() {
        return error_resp(StatusCode::SERVICE_UNAVAILABLE, "tmdb not configured");
    }
    let lang = lang_or_default(q.lang);
    let ids = parse_genre_ids(q.genres);
    let page = q.page.unwrap_or(1);

    match tmdb::browse(&key, &kind, &ids, page, &lang).await {
        Ok(items) => (
            [(axum::http::header::CACHE_CONTROL, "private, max-age=900")],
            Json(items),
        )
            .into_response(),
        Err(e) => {
            eprintln!("[discover] browse error: {e}");
            error_resp(StatusCode::BAD_GATEWAY, "tmdb error")
        }
    }
}

fn error_resp(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
