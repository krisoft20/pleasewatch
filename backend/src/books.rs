use crate::models::{ApiError, Book, BookHit, BookSource};
use crate::{middleware::AuthUser, AppState};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect},
    routing::{get, patch, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio_util::io::ReaderStream;

const OL_API: &str = "https://openlibrary.org";
const OL_COVERS: &str = "https://covers.openlibrary.org";
const LIBGEN_MIRRORS: &[&str] = &[
    "https://libgen.li",
    "https://libgen.vg",
    "https://libgen.la",
    "https://libgen.bz",
    "https://libgen.gl",
];
const ANNAS_MIRRORS: &[&str] = &[
    "https://annas-archive.li",
    "https://annas-archive.se",
    "https://annas-archive.pk",
    "https://annas-archive.gd",
    "https://annas-archive.gl",
];
const UA: &str = "pleasewatch/0.1";
const LIBGEN_TTL: Duration = Duration::from_secs(300);
const OL_SEARCH_TTL: Duration = Duration::from_secs(300);
const OL_WORK_TTL: Duration = Duration::from_secs(3600);
const OL_ENRICH_TTL: Duration = Duration::from_secs(1800);
const AUTHOR_KEYS_TIMEOUT: Duration = Duration::from_secs(6);
const MAX_BOOK_BYTES: usize = 80 * 1024 * 1024;
const BOOK_UPLOAD_BODY_LIMIT: usize = MAX_BOOK_BYTES + 1024 * 1024;
const CONVERT_TIMEOUT: Duration = Duration::from_secs(120);

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static LIBGEN_CACHE: OnceLock<Mutex<HashMap<String, (Vec<BookSource>, Instant)>>> = OnceLock::new();
static OL_SEARCH_CACHE: OnceLock<Mutex<HashMap<String, (Vec<(BookHit, Vec<String>)>, Instant)>>> =
    OnceLock::new();
static OL_WORK_CACHE: OnceLock<Mutex<HashMap<String, (Option<BookHit>, Instant)>>> =
    OnceLock::new();
static OL_META_CACHE: OnceLock<Mutex<HashMap<String, (WorkMeta, Instant)>>> = OnceLock::new();
static OL_RATINGS_CACHE: OnceLock<Mutex<HashMap<String, (Option<(f64, i64)>, Instant)>>> =
    OnceLock::new();
static OL_EDITIONS_CACHE: OnceLock<Mutex<HashMap<String, (EditionBits, Instant)>>> =
    OnceLock::new();
static LIBGEN_GOOD: AtomicUsize = AtomicUsize::new(0);
static ANNAS_GOOD: AtomicUsize = AtomicUsize::new(0);
static CONVERT_GATE: Mutex<()> = Mutex::new(());
static ENRICHING: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
static COVER_RECOVERING: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

const ENRICH_RETRY: Duration = Duration::from_secs(900);

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()
            .expect("reqwest client")
    })
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/books/search", get(handle_search))
        .route("/api/books/popular", get(handle_popular))
        .route("/api/books/series", get(handle_series))
        .route("/api/books/series/detail", get(handle_series_detail))
        .route("/api/books/by-author", get(handle_by_author))
        .route("/api/books/anna", get(handle_anna_redirect))
        .route("/api/books/author-cover/{olid}", get(handle_author_cover))
        .route("/api/books/author/{olid}", get(handle_author))
        .route("/api/books/{ol_key}/torrents", get(handle_torrents))
        .route("/api/books/{ol_key}/torrent-add", post(handle_torrent_add))
        .route("/api/books", get(handle_list).post(handle_add))
        .route("/api/books/continue", get(handle_continue))
        .route("/api/books/progress", post(handle_progress))
        .route(
            "/api/books/marks/{id}",
            patch(handle_mark_update).delete(handle_mark_delete),
        )
        .route("/api/books/shelf", get(handle_shelf))
        .route("/api/books/daily", get(handle_daily_quote))
        .route("/api/books/goal", post(handle_goal))
        .route(
            "/api/books/{ol_key}/shelf",
            post(handle_shelf_set)
                .patch(handle_shelf_showcase)
                .delete(handle_shelf_remove),
        )
        .route(
            "/api/books/{ol_key}/marks",
            get(handle_marks_list).post(handle_mark_create),
        )
        .route("/api/books/cover/{cover_id}", get(handle_cover))
        .route(
            "/api/books/{ol_key}",
            get(handle_detail).delete(handle_delete),
        )
        .route("/api/books/{ol_key}/sources", get(handle_sources))
        .route("/api/books/{ol_key}/fetch", post(handle_fetch))
        .route(
            "/api/books/{ol_key}/upload",
            post(handle_upload).layer(DefaultBodyLimit::max(BOOK_UPLOAD_BODY_LIMIT)),
        )
        .route("/api/books/{ol_key}/file", get(handle_file))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn handle_search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let query = q.q.trim();
    if query.len() < 2 {
        return Json(Vec::<BookHit>::new()).into_response();
    }
    let raw = match ol_search_cached(query).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[books] search failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "openlibrary search failed");
        }
    };
    let mut hits: Vec<BookHit> = raw.iter().map(|(h, _)| h.clone()).collect();
    let subjects: Vec<Vec<String>> = raw.iter().map(|(_, s)| s.clone()).collect();
    mark_in_library(&state, &mut hits).await;
    let series_hits = detect_series_hits(&hits, &subjects).await;
    if series_hits.is_empty() {
        return Json(hits).into_response();
    }
    let series_names: std::collections::HashSet<String> =
        series_hits.iter().map(|s| s.title.to_lowercase()).collect();
    hits.retain(|h| {
        let t = h.title.to_lowercase();
        !is_series_collection_title(&t) || !series_names.iter().any(|n| t.contains(n))
    });
    let mut all = series_hits;
    all.extend(hits);
    Json(all).into_response()
}

#[derive(Deserialize)]
struct SeriesNameQuery {
    name: String,
}

async fn handle_series_detail(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SeriesNameQuery>,
) -> impl IntoResponse {
    let name = q.name.trim();
    if name.is_empty() {
        return err(StatusCode::BAD_REQUEST, "missing name");
    }
    let mut books = match ol_series(name).await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[books] series detail failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "openlibrary unavailable");
        }
    };
    if books.is_empty() {
        return err(StatusCode::NOT_FOUND, "no books in this series");
    }
    let years: Vec<i32> = books.iter().filter_map(|b| b.year).collect();
    let year_min = years.iter().min().copied();
    let year_max = years.iter().max().copied();
    let author = primary_author(&books);
    let cover_url = books.iter().find_map(|b| b.cover_url.clone());
    mark_in_library(&state, &mut books).await;
    Json(serde_json::json!({
        "name": name,
        "author": author,
        "cover_url": cover_url,
        "year_min": year_min,
        "year_max": year_max,
        "books": books,
    }))
    .into_response()
}

fn primary_author(books: &[BookHit]) -> Option<String> {
    let mut counts: HashMap<String, i32> = HashMap::new();
    for b in books {
        if let Some(a) = b.authors.as_deref() {
            for name in a.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
                *counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(name, _)| name)
}

fn is_series_collection_title(lower: &str) -> bool {
    lower.contains("complete collection")
        || lower.contains("complete series")
        || lower.contains("box set")
        || lower.contains("boxed set")
        || lower.contains("series collection")
        || (lower.contains(" series") && lower.contains("books collection"))
}

fn extract_series_name(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    let cuts = [
        " complete collection",
        " complete series",
        " series complete collection",
        " series collection",
        " box set",
        " boxed set",
    ];
    for c in cuts.iter() {
        if let Some(idx) = lower.find(c) {
            let name = title[..idx].trim();
            if name.len() >= 2 {
                return Some(name.to_string());
            }
        }
    }
    if let Some(idx) = lower.find(" series ") {
        let rest = &lower[idx + 8..];
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) || rest.starts_with("books") {
            let name = title[..idx].trim();
            if name.len() >= 2 {
                return Some(name.to_string());
            }
        }
    }
    None
}

async fn detect_series_hits(hits: &[BookHit], subjects: &[Vec<String>]) -> Vec<BookHit> {
    let mut candidates: HashMap<String, (String, Vec<String>, usize)> = HashMap::new();
    for (i, h) in hits.iter().enumerate() {
        if let Some(name) = extract_series_name(&h.title) {
            let key = name.to_lowercase();
            let entry = candidates
                .entry(key)
                .or_insert_with(|| (name.clone(), Vec::new(), 0));
            entry.2 += 1;
            if let Some(a) = h.authors.as_deref() {
                if let Some(first) = a.split([',', ';']).next() {
                    let first = first.trim();
                    if !first.is_empty() {
                        entry.1.push(first.to_string());
                    }
                }
            }
        }
        if let Some(subj_list) = subjects.get(i) {
            for s in subj_list {
                let name = match s.strip_prefix("series:") {
                    Some(n) => n.trim().to_string(),
                    None => continue,
                };
                if name.is_empty() || name.len() > 80 {
                    continue;
                }
                let key = name.to_lowercase();
                let entry = candidates
                    .entry(key)
                    .or_insert_with(|| (name.clone(), Vec::new(), 0));
                entry.2 += 1;
                if let Some(a) = h.authors.as_deref() {
                    if let Some(first) = a.split([',', ';']).next() {
                        let first = first.trim();
                        if !first.is_empty() {
                            entry.1.push(first.to_string());
                        }
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(String, Vec<String>, usize)> = candidates
        .into_iter()
        .filter(|(_, (_, _, count))| *count >= 2)
        .map(|(_, t)| t)
        .collect();
    ranked.sort_by(|a, b| b.2.cmp(&a.2));
    ranked.truncate(3);
    let mut top: Vec<(String, Option<String>)> = ranked
        .into_iter()
        .map(|(name, authors, _)| (name, mode_string(&authors)))
        .collect();
    top.sort_by_key(|(n, _)| n.to_lowercase());
    let lookups = top.iter().map(|(name, _)| {
        let n = name.clone();
        tokio::spawn(async move { (n.clone(), ol_series(&n).await.unwrap_or_default()) })
    });
    let mut out = Vec::new();
    for h in lookups {
        let (name, books) = match h.await {
            Ok(t) => t,
            Err(_) => continue,
        };
        if books.len() < 2 {
            continue;
        }
        let author = top
            .iter()
            .find(|(n, _)| n == &name)
            .and_then(|(_, a)| a.clone());
        let covers: Vec<String> = books
            .iter()
            .filter_map(|b| b.cover_url.clone())
            .take(4)
            .collect();
        let cover_url = covers.first().cloned();
        out.push(BookHit {
            ol_key: name.clone(),
            title: name.clone(),
            authors: author,
            description: None,
            cover_url,
            year: books.iter().filter_map(|b| b.year).min(),
            language: None,
            author_keys: Vec::new(),
            in_library: false,
            ready: false,
            kind: "series".into(),
            series_count: Some(books.len() as i32),
            series_covers: covers,
        });
    }
    out
}

fn mode_string(xs: &[String]) -> Option<String> {
    let mut counts: HashMap<&String, i32> = HashMap::new();
    for x in xs {
        *counts.entry(x).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(s, _)| s.clone())
}

async fn mark_in_library(state: &Arc<AppState>, hits: &mut [BookHit]) {
    let keys: Vec<String> = hits.iter().map(|h| h.ol_key.clone()).collect();
    let owned = {
        let db = state.db.lock().await;
        db.list_book_states_owned(&keys).unwrap_or_default()
    };
    for h in hits.iter_mut() {
        if let Some((has_file, cover_url)) = owned.get(&h.ol_key) {
            h.in_library = true;
            h.ready = *has_file;
            if cover_url
                .as_deref()
                .is_some_and(|url| !url.trim().is_empty())
            {
                h.cover_url.clone_from(cover_url);
            }
        }
    }
}

#[derive(Deserialize)]
struct SeriesQuery {
    name: String,
}

async fn handle_series(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SeriesQuery>,
) -> impl IntoResponse {
    let name = q.name.trim();
    if name.is_empty() {
        return Json(Vec::<BookHit>::new()).into_response();
    }
    let mut hits = match ol_series(name).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[books] series fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "openlibrary series lookup failed");
        }
    };
    mark_in_library(&state, &mut hits).await;
    Json(hits).into_response()
}

async fn handle_popular() -> impl IntoResponse {
    match ol_trending().await {
        Ok(hits) => Json(hits).into_response(),
        Err(e) => {
            eprintln!("[books] trending failed: {e}");
            err(StatusCode::BAD_GATEWAY, "openlibrary unavailable")
        }
    }
}

async fn handle_anna_redirect(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.trim();
    if query.is_empty() {
        return err(StatusCode::BAD_REQUEST, "missing query");
    }
    let start = ANNAS_GOOD.load(Ordering::Relaxed);
    for i in 0..ANNAS_MIRRORS.len() {
        let idx = (start + i) % ANNAS_MIRRORS.len();
        let base = ANNAS_MIRRORS[idx];
        let alive = client()
            .get(base)
            .timeout(Duration::from_secs(4))
            .send()
            .await
            .map(|r| r.status().is_success() || r.status().is_redirection())
            .unwrap_or(false);
        if !alive {
            continue;
        }
        if idx != start {
            println!("[books] anna mirror -> {base}");
        }
        ANNAS_GOOD.store(idx, Ordering::Relaxed);
        let url = format!("{base}/search?q={}", urlencode(query));
        return Redirect::to(&url).into_response();
    }
    eprintln!("[books] no anna mirror reachable for '{query}'");
    err(StatusCode::BAD_GATEWAY, "all anna mirrors are down")
}

#[derive(Deserialize)]
struct AuthorQuery {
    name: String,
    #[serde(default)]
    exclude: Option<String>,
}

async fn handle_by_author(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthorQuery>,
) -> impl IntoResponse {
    let name = q.name.trim();
    if name.is_empty() {
        return Json(Vec::<BookHit>::new()).into_response();
    }
    let mut hits = match ol_by_author(name).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[books] author lookup failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "openlibrary unavailable");
        }
    };
    if let Some(excl) = q.exclude.as_deref().map(normalize_key) {
        hits.retain(|h| h.ol_key != excl);
    }
    mark_in_library(&state, &mut hits).await;
    Json(hits).into_response()
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    mine: bool,
}

async fn handle_list(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let result = if q.mine {
        db.list_books_by_user(&auth.id)
    } else {
        db.list_books()
    };
    match result {
        Ok(list) => Json(list).into_response(),
        Err(e) => {
            eprintln!("[books] list failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

#[derive(Deserialize)]
struct AddBody {
    ol_key: String,
}

async fn handle_add(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<AddBody>,
) -> impl IntoResponse {
    let ol_key = normalize_key(&b.ol_key);
    match find_or_create_book(&state, &ol_key, &auth).await {
        Ok(book) => Json(book).into_response(),
        Err(resp) => resp,
    }
}

async fn find_or_create_book(
    state: &Arc<AppState>,
    key: &str,
    auth: &AuthUser,
) -> Result<Book, axum::response::Response> {
    {
        let db = state.db.lock().await;
        match db.find_book_by_key(key) {
            Ok(Some(existing)) => return Ok(existing),
            Ok(None) => {}
            Err(e) => {
                eprintln!("[books] lookup failed: {e}");
                return Err(err(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed"));
            }
        }
    }

    let mut hit = match ol_work_cached(key).await {
        Ok(Some(h)) => h,
        Ok(None) => return Err(err(StatusCode::NOT_FOUND, "book not on openlibrary")),
        Err(e) => {
            eprintln!("[books] metadata fetch failed: {e}");
            return Err(err(StatusCode::BAD_GATEWAY, "openlibrary unavailable"));
        }
    };
    if let Ok(editions) = ol_editions_cached(key).await {
        if let Some(cover_id) = editions.cover_id {
            hit.cover_url = Some(format!("/api/books/cover/{cover_id}.jpg"));
        }
    }
    let author_keys = hit.author_keys.clone();

    let book = Book {
        id: uuid::Uuid::new_v4().to_string(),
        ol_key: hit.ol_key.clone(),
        title: hit.title,
        authors: hit.authors,
        description: hit.description,
        cover_url: hit.cover_url,
        year: hit.year,
        language: hit.language,
        file_path: None,
        ext: None,
        status: "pending".into(),
        added_by: Some(auth.id.clone()),
        added_at: chrono::Utc::now().to_rfc3339(),
        pages: None,
        subjects: None,
        isbn: None,
        publisher: None,
        rating: None,
        rating_count: None,
        enriched_at: None,
        series: None,
    };

    let db = state.db.lock().await;
    if let Err(e) = db.create_book(&book) {
        eprintln!("[books] insert failed: {e}");
        return Err(err(StatusCode::INTERNAL_SERVER_ERROR, "insert failed"));
    }
    if let Err(e) = db.update_book_author_keys(key, &author_keys) {
        eprintln!("[books] author keys save failed: {e}");
    }
    println!("[books] '{}' added by {}", book.title, auth.username);
    spawn_enrich(state, key);
    Ok(book)
}

async fn ensure_book_author_keys(state: &Arc<AppState>, key: &str) -> Vec<String> {
    {
        let db = state.db.lock().await;
        match db.get_book_author_keys(key) {
            Ok(author_keys) if !author_keys.is_empty() => return author_keys,
            Ok(_) => {}
            Err(e) => eprintln!("[books] author keys lookup failed: {e}"),
        }
    }

    let author_keys = match tokio::time::timeout(AUTHOR_KEYS_TIMEOUT, ol_work_cached(key)).await {
        Ok(Ok(Some(hit))) => hit.author_keys,
        Ok(Ok(None)) => Vec::new(),
        Ok(Err(e)) => {
            eprintln!("[books] author keys fetch failed: {e}");
            Vec::new()
        }
        Err(_) => {
            eprintln!("[books] author keys timed out for {key}");
            Vec::new()
        }
    };
    if author_keys.is_empty() {
        return author_keys;
    }

    let db = state.db.lock().await;
    if let Err(e) = db.update_book_author_keys(key, &author_keys) {
        eprintln!("[books] author keys save failed: {e}");
    }
    author_keys
}

async fn handle_detail(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let (lib, progress, shelf) = {
        let db = state.db.lock().await;
        let b = db.find_book_by_key(&key).unwrap_or(None);
        let p = db.get_book_progress(&auth.id, &key).unwrap_or(None);
        let s = db.get_book_shelf_status(&auth.id, &key).unwrap_or(None);
        (b, p, s)
    };

    if let Some(book) = lib {
        if book.enriched_at.is_none() || book.description.as_deref().is_none_or(str::is_empty) {
            spawn_enrich(&state, &key);
        }
        if book_cover_needs_refresh(book.cover_url.as_deref()) {
            spawn_cover_refresh(&state, &key);
        }
        let file_size = match book.file_path.as_deref() {
            Some(rel) => tokio::fs::metadata(abs_path(&state.media_root, rel))
                .await
                .map(|m| m.len())
                .ok(),
            None => None,
        };
        let author_keys = ensure_book_author_keys(&state, &key).await;
        return Json(serde_json::json!({
            "book": book,
            "in_library": true,
            "progress": progress,
            "file_size": file_size,
            "shelf": shelf,
            "author_keys": author_keys,
        }))
        .into_response();
    }

    let bundle = tokio::time::timeout(std::time::Duration::from_secs(6), async {
        tokio::join!(
            ol_work_cached(&key),
            ol_work_meta_cached(&key),
            ol_ratings_cached(&key),
            ol_editions_cached(&key),
        )
    })
    .await;
    let (work_res, meta_res, rating_res, eds_res) = match bundle {
        Ok(t) => t,
        Err(_) => {
            eprintln!("[books] detail bundle timed out for {key}");
            return err(StatusCode::GATEWAY_TIMEOUT, "openlibrary slow, try again");
        }
    };

    let hit = match work_res {
        Ok(Some(h)) => h,
        Ok(None) => return err(StatusCode::NOT_FOUND, "book not found"),
        Err(e) => {
            eprintln!("[books] detail fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "openlibrary unavailable");
        }
    };
    let meta = meta_res.unwrap_or(WorkMeta {
        subjects: Vec::new(),
        series: None,
    });
    let (rating, rating_count) = match rating_res {
        Ok(Some((a, n))) => (Some(a), Some(n)),
        _ => (None, None),
    };
    let eds = eds_res.unwrap_or(EditionBits {
        cover_id: None,
        pages: None,
        isbn: None,
        publisher: None,
        language: None,
        year: None,
    });

    let subjects = if meta.subjects.is_empty() {
        None
    } else {
        Some(meta.subjects.join(", "))
    };

    let author_keys = hit.author_keys.clone();
    let stub = Book {
        id: String::new(),
        ol_key: hit.ol_key,
        title: hit.title,
        authors: hit.authors,
        description: hit.description,
        cover_url: eds
            .cover_id
            .map(|id| format!("/api/books/cover/{id}.jpg"))
            .or(hit.cover_url),
        year: hit.year.or(eds.year),
        language: hit.language.or(eds.language),
        file_path: None,
        ext: None,
        status: "preview".into(),
        added_by: None,
        added_at: String::new(),
        pages: eds.pages,
        subjects,
        isbn: eds.isbn,
        publisher: eds.publisher,
        rating,
        rating_count,
        enriched_at: None,
        series: meta.series,
    };

    Json(serde_json::json!({
        "book": stub,
        "in_library": false,
        "progress": progress,
        "file_size": null,
        "shelf": shelf,
        "author_keys": author_keys,
    }))
    .into_response()
}

async fn handle_delete(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let db = state.db.lock().await;
    let b = match db.find_book_by_key(&key) {
        Ok(Some(b)) => b,
        Ok(None) => return err(StatusCode::NOT_FOUND, "not in library"),
        Err(e) => {
            eprintln!("[books] delete lookup failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed");
        }
    };
    if !auth.is_admin() && b.added_by.as_deref() != Some(auth.id.as_str()) {
        return err(StatusCode::FORBIDDEN, "not yours");
    }
    drop(db);

    if let Some(path) = b.file_path.as_deref() {
        let abs = abs_path(&state.media_root, path);
        if let Err(e) = tokio::fs::remove_file(&abs).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[books] file remove failed for {}: {}", abs.display(), e);
            }
        }
    }

    let db = state.db.lock().await;
    if let Err(e) = db.delete_book_by_key(&key) {
        eprintln!("[books] delete failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "delete failed");
    }
    println!("[books] '{}' removed by {}", b.title, auth.username);
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_sources(
    Path(ol_key): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let lookup = {
        let db = state.db.lock().await;
        db.find_book_by_key(&key).ok().flatten()
    };

    let query = match lookup {
        Some(b) => {
            let mut q = b.title;
            if let Some(a) = b.authors {
                let first = a.split([',', ';']).next().unwrap_or("").trim();
                if !first.is_empty() {
                    q.push(' ');
                    q.push_str(first);
                }
            }
            q
        }
        None => match ol_work(&key).await {
            Ok(Some(h)) => {
                let mut q = h.title;
                if let Some(a) = h.authors {
                    let first = a.split([',', ';']).next().unwrap_or("").trim();
                    if !first.is_empty() {
                        q.push(' ');
                        q.push_str(first);
                    }
                }
                q
            }
            Ok(None) => return err(StatusCode::NOT_FOUND, "no metadata"),
            Err(e) => {
                eprintln!("[books] sources metadata failed: {e}");
                return err(StatusCode::BAD_GATEWAY, "openlibrary unavailable");
            }
        },
    };

    if let Some(hit) = cache_get(&query) {
        return Json(hit).into_response();
    }
    match libgen_search(&query).await {
        Ok(mut rows) => {
            rows.sort_by_key(|s| {
                let foreign = s
                    .language
                    .as_deref()
                    .map(|l| !l.to_ascii_lowercase().starts_with("en"))
                    .unwrap_or(false);
                let size = s.size.unwrap_or(0);
                let tiny = s.size.map(|sz| sz < min_book_size(&s.ext)).unwrap_or(false);
                (tiny, ext_rank(&s.ext), foreign, std::cmp::Reverse(size))
            });
            cache_put(query, rows.clone());
            Json(rows).into_response()
        }
        Err(e) => {
            eprintln!("[books] libgen search failed: {e}");
            err(StatusCode::BAD_GATEWAY, "libgen unavailable")
        }
    }
}

async fn handle_torrents(
    Path(ol_key): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let book = {
        let db = state.db.lock().await;
        db.find_book_by_key(&key).ok().flatten()
    };
    let (title, authors) = match book {
        Some(b) => (b.title, b.authors),
        None => match ol_work(&key).await {
            Ok(Some(h)) => (h.title, h.authors),
            Ok(None) => return err(StatusCode::NOT_FOUND, "book not found"),
            Err(e) => {
                eprintln!("[books] torrents detail fetch failed: {e}");
                return err(StatusCode::BAD_GATEWAY, "openlibrary unavailable");
            }
        },
    };

    let mut query = title;
    if let Some(a) = authors {
        let first = a.split([',', ';']).next().unwrap_or("").trim();
        if !first.is_empty() {
            query.push(' ');
            query.push_str(first);
        }
    }

    let jackett = match state.jackett.lock().await.clone() {
        Some(j) => j,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "jackett not configured"),
    };
    let indexers = crate::downloads::resolve_indexers(&state, Some("book")).await;
    let results = match tokio::time::timeout(
        std::time::Duration::from_secs(35),
        jackett.search(&query, &indexers, &[7000, 7020], None),
    )
    .await
    {
        Ok(r) => r,
        Err(_) => {
            eprintln!("[books] jackett search timed out for {query}");
            Vec::new()
        }
    };

    let mut sorted = results;
    sorted.sort_by(|a, b| b.seeds.cmp(&a.seeds));
    Json(sorted).into_response()
}

#[derive(Deserialize)]
struct TorrentAddBody {
    magnet: String,
}

async fn handle_torrent_add(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    Json(body): Json<TorrentAddBody>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let is_magnet = body.magnet.starts_with("magnet:");
    let is_http = body.magnet.starts_with("http://") || body.magnet.starts_with("https://");
    if !is_magnet && !is_http {
        return err(
            StatusCode::BAD_REQUEST,
            "expected magnet or http(s) torrent url",
        );
    }

    let book = match find_or_create_book(&state, &key, &auth).await {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    if book.file_path.is_some() {
        return err(StatusCode::CONFLICT, "book already has a file");
    }

    let qbit = match state.qbit.lock().await.clone() {
        Some(q) => q,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "qbit not configured"),
    };

    let save_path = format!("{}/_dl/books/{}", state.media_root, key);
    if let Err(e) = tokio::fs::create_dir_all(&save_path).await {
        eprintln!("[books] mkdir {save_path} failed: {e}");
    }

    if let Err(e) = qbit.add_magnet(&body.magnet, "book", &save_path).await {
        eprintln!("[books] qbit add failed: {e}");
        return err(StatusCode::BAD_GATEWAY, &format!("qbit: {e}"));
    }

    let hash = if is_magnet {
        crate::downloads::extract_hash(&body.magnet)
    } else {
        let mut found: Option<String> = None;
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if let Ok(list) = qbit.list(Some("book")).await {
                if let Some(t) = list
                    .into_iter()
                    .find(|t| t.save_path.ends_with(&format!("/{}", key)))
                {
                    found = Some(t.hash);
                    break;
                }
            }
        }
        found
    };

    let Some(hash) = hash else {
        eprintln!("[books] could not resolve hash for {key}");
        return err(
            StatusCode::BAD_GATEWAY,
            "qbit accepted url but no hash appeared",
        );
    };

    {
        let db = state.db.lock().await;
        if let Err(e) = db.set_book_qbit(&key, &hash) {
            eprintln!("[books] set_book_qbit failed: {e}");
        }
    }
    println!(
        "[books] '{}' torrent queued ({}) by {}",
        book.title,
        &hash[..8],
        auth.username
    );
    tokio::spawn(watch_book_download(
        state.clone(),
        key.clone(),
        hash.clone(),
        save_path,
    ));

    Json(serde_json::json!({ "ok": true, "hash": hash })).into_response()
}

async fn watch_book_download(
    state: Arc<AppState>,
    ol_key: String,
    hash: String,
    save_path: String,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4 * 3600);
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(10));
    tick.tick().await;

    loop {
        if std::time::Instant::now() >= deadline {
            eprintln!("[books] download watcher timed out for {ol_key}");
            let db = state.db.lock().await;
            let _ = db.update_book_status(&ol_key, "error");
            let _ = db.clear_book_qbit(&ol_key);
            return;
        }
        tick.tick().await;

        let qbit = match state.qbit.lock().await.clone() {
            Some(q) => q,
            None => continue,
        };
        let info = match qbit.get(&hash).await {
            Ok(Some(t)) => t,
            Ok(None) => {
                eprintln!("[books] torrent vanished for {ol_key} ({hash})");
                let db = state.db.lock().await;
                let _ = db.update_book_status(&ol_key, "error");
                let _ = db.clear_book_qbit(&ol_key);
                return;
            }
            Err(e) => {
                eprintln!("[books] qbit poll failed for {hash}: {e}");
                continue;
            }
        };
        {
            let db = state.db.lock().await;
            let _ = db.update_book_download_progress(&ol_key, info.progress);
        }
        if info.progress < 1.0 {
            continue;
        }

        let book_title = {
            let db = state.db.lock().await;
            db.find_book_by_key(&ol_key)
                .ok()
                .flatten()
                .map(|b| b.title)
                .unwrap_or_default()
        };
        let picked = pick_ebook_in(&save_path, &book_title).await;
        let Some((src, ext)) = picked else {
            eprintln!("[books] no epub/pdf/mobi found in {save_path} for {ol_key}");
            let db = state.db.lock().await;
            let _ = db.update_book_status(&ol_key, "error");
            let _ = db.clear_book_qbit(&ol_key);
            return;
        };

        let rel = format!("books/{}.{}", ol_key, ext);
        let dst = abs_path(&state.media_root, &rel);
        if let Some(parent) = dst.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(e) = tokio::fs::rename(&src, &dst).await {
            if let Err(e2) = tokio::fs::copy(&src, &dst).await {
                eprintln!(
                    "[books] move {} -> {} failed: rename {e}, copy {e2}",
                    src.display(),
                    dst.display()
                );
                let db = state.db.lock().await;
                let _ = db.update_book_status(&ol_key, "error");
                let _ = db.clear_book_qbit(&ol_key);
                return;
            }
            let _ = tokio::fs::remove_file(&src).await;
        }

        {
            let db = state.db.lock().await;
            let _ = db.update_book_file(&ol_key, &rel, &ext);
            let _ = db.clear_book_qbit(&ol_key);
        }
        let _ = qbit.delete(&hash, true).await;
        let _ = tokio::fs::remove_dir_all(&save_path).await;
        println!("[books] '{}' torrent completed, saved as {}", ol_key, rel);
        return;
    }
}

async fn pick_ebook_in(dir: &str, title: &str) -> Option<(std::path::PathBuf, String)> {
    let title_tokens: Vec<String> = title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_string())
        .collect();

    let mut stack = vec![std::path::PathBuf::from(dir)];
    let mut candidates: Vec<(std::path::PathBuf, String, u64, usize)> = Vec::new();
    while let Some(d) = stack.pop() {
        let mut rd = match tokio::fs::read_dir(&d).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        while let Ok(Some(e)) = rd.next_entry().await {
            let p = e.path();
            let Ok(meta) = e.metadata().await else {
                continue;
            };
            if meta.is_dir() {
                stack.push(p);
                continue;
            }
            let Some(ext) = p
                .extension()
                .and_then(|x| x.to_str())
                .map(|s| s.to_lowercase())
            else {
                continue;
            };
            if !matches!(ext.as_str(), "epub" | "pdf" | "mobi" | "azw3") {
                continue;
            }
            let sz = meta.len();
            let fname = p
                .file_stem()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_lowercase();
            let hits = title_tokens
                .iter()
                .filter(|tok| fname.contains(tok.as_str()))
                .count();
            candidates.push((p, ext, sz, hits));
        }
    }

    candidates.sort_by(|a, b| b.3.cmp(&a.3).then(b.2.cmp(&a.2)));
    if let Some((p, _, _, hits)) = candidates.first() {
        if *hits > 0 {
            println!(
                "[books] picked '{}' (title-match {}/{})",
                p.display(),
                hits,
                title_tokens.len()
            );
        }
    }
    candidates.into_iter().next().map(|(p, ext, _, _)| (p, ext))
}

#[derive(Deserialize)]
struct FetchBody {
    md5: String,
    ext: String,
}

async fn handle_fetch(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    Json(b): Json<FetchBody>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let md5 = b.md5.trim();
    let ext = sanitize_ext(&b.ext);

    if md5.len() != 32 || !md5.chars().all(|c| c.is_ascii_hexdigit()) {
        return err(StatusCode::BAD_REQUEST, "bad md5");
    }
    if ext.is_empty() {
        return err(StatusCode::BAD_REQUEST, "bad extension");
    }

    let book = match find_or_create_book(&state, &key, &auth).await {
        Ok(b) => b,
        Err(resp) => return resp,
    };

    let dl_url = match libgen_resolve(md5).await {
        Ok(Some(u)) => u,
        Ok(None) => return err(StatusCode::BAD_GATEWAY, "no working mirror"),
        Err(e) => {
            eprintln!("[books] mirror lookup failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "mirror lookup failed");
        }
    };

    println!("[books] fetching '{}' from {}", book.title, dl_url);
    let bytes = match client().get(&dl_url).send().await {
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[books] body read failed: {e}");
                return err(StatusCode::BAD_GATEWAY, "download failed");
            }
        },
        Ok(r) => {
            eprintln!("[books] mirror returned {}", r.status());
            return err(StatusCode::BAD_GATEWAY, "mirror rejected request");
        }
        Err(e) => {
            eprintln!("[books] fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "download failed");
        }
    };

    if bytes.len() > MAX_BOOK_BYTES {
        return err(StatusCode::PAYLOAD_TOO_LARGE, "file too big");
    }
    if bytes.len() < 1024 {
        return err(StatusCode::BAD_GATEWAY, "file looks empty");
    }
    if !looks_like_book(&ext, &bytes) {
        eprintln!("[books] mirror served junk for {md5} (wanted {ext})");
        return err(
            StatusCode::BAD_GATEWAY,
            "mirror served junk, try another file",
        );
    }
    let is_sample = ext == "epub" && looks_like_sample_epub(&bytes);
    if is_sample {
        eprintln!(
            "[books] sample epub detected for {md5} ({} bytes) - saving anyway",
            bytes.len()
        );
    }

    let rel = format!("books/{}.{}", key, ext);
    let abs = abs_path(&state.media_root, &rel);
    if let Some(parent) = abs.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Err(e) = tokio::fs::write(&abs, &bytes).await {
        eprintln!("[books] write failed for {}: {e}", abs.display());
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }

    let convert = needs_conversion(&ext);
    {
        let db = state.db.lock().await;
        if let Err(e) = db.update_book_file(&key, &rel, &ext) {
            eprintln!("[books] db update failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
        }
        if convert {
            if let Err(e) = db.update_book_status(&key, "processing") {
                eprintln!("[books] status update failed: {e}");
            }
        }
    }
    println!(
        "[books] '{}' saved ({} bytes) by {}",
        book.title,
        bytes.len(),
        auth.username
    );
    if convert {
        tokio::spawn(convert_in_background(state, key, rel, ext.clone()));
    }
    Json(serde_json::json!({ "ok": true, "size": bytes.len(), "ext": ext })).into_response()
}

async fn handle_upload(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    mut form: Multipart,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);

    let book = match find_or_create_book(&state, &key, &auth).await {
        Ok(b) => b,
        Err(resp) => return resp,
    };

    let mut bytes: Option<bytes::Bytes> = None;
    let mut ext: Option<String> = None;

    loop {
        match form.next_field().await {
            Ok(Some(field)) => {
                if field.name() == Some("file") {
                    let filename = field.file_name().unwrap_or("book").to_string();
                    ext = filename
                        .rsplit('.')
                        .next()
                        .map(|s| sanitize_ext(s))
                        .filter(|s| !s.is_empty());
                    match field.bytes().await {
                        Ok(b) => bytes = Some(b),
                        Err(e) => {
                            eprintln!("[books] upload read failed: {e}");
                            return err(StatusCode::BAD_REQUEST, "read failed");
                        }
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("[books] multipart parse failed: {e}");
                return err(StatusCode::BAD_REQUEST, "bad upload");
            }
        }
    }

    let Some(buf) = bytes else {
        return err(StatusCode::BAD_REQUEST, "no file field");
    };
    let Some(ext) = ext else {
        return err(StatusCode::BAD_REQUEST, "unknown extension");
    };
    if buf.len() > MAX_BOOK_BYTES {
        return err(StatusCode::PAYLOAD_TOO_LARGE, "file too big");
    }
    if !looks_like_book(&ext, &buf) {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "file content doesn't match the extension",
        );
    }

    let rel = format!("books/{}.{}", key, ext);
    let abs = abs_path(&state.media_root, &rel);
    if let Some(parent) = abs.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Err(e) = tokio::fs::write(&abs, &buf).await {
        eprintln!("[books] upload write failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }

    let convert = needs_conversion(&ext);
    {
        let db = state.db.lock().await;
        if let Err(e) = db.update_book_file(&key, &rel, &ext) {
            eprintln!("[books] db update failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
        }
        if convert {
            if let Err(e) = db.update_book_status(&key, "processing") {
                eprintln!("[books] status update failed: {e}");
            }
        }
    }
    println!(
        "[books] '{}' uploaded ({} bytes) by {}",
        book.title,
        buf.len(),
        auth.username
    );
    if convert {
        tokio::spawn(convert_in_background(state, key, rel, ext.clone()));
    }
    Json(serde_json::json!({ "ok": true, "size": buf.len(), "ext": ext })).into_response()
}

async fn convert_in_background(
    state: Arc<AppState>,
    key: String,
    src_rel: String,
    src_ext: String,
) {
    let src = abs_path(&state.media_root, &src_rel);
    let dst_rel = format!("books/{key}.epub");
    let dst = abs_path(&state.media_root, &dst_rel);

    println!("[books] converting {key} {src_ext} -> epub");
    let t0 = Instant::now();
    match convert_to_epub(src.clone(), dst.clone()).await {
        Ok(()) => {
            let _ = tokio::fs::remove_file(&src).await;
            let db = state.db.lock().await;
            if let Err(e) = db.update_book_file(&key, &dst_rel, "epub") {
                eprintln!("[books] db update after convert failed: {e}");
                return;
            }
            println!("[books] convert done in {} ms", t0.elapsed().as_millis());
        }
        Err(e) => {
            eprintln!("[books] convert failed: {e}");
            let _ = tokio::fs::remove_file(&dst).await;
            let db = state.db.lock().await;
            let _ = db.update_book_status(&key, "error");
        }
    }
}

async fn convert_to_epub(
    input: std::path::PathBuf,
    output: std::path::PathBuf,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let job_id = uuid::Uuid::new_v4().to_string();
        crate::ffmpeg::register_job(crate::ffmpeg::ProcessingJob {
            id: job_id.clone(),
            operation: "ebook-convert".to_string(),
            source: input
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string(),
            started_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            state: "queued".to_string(),
        });

        let _slot = CONVERT_GATE.lock().unwrap_or_else(|e| e.into_inner());
        crate::ffmpeg::update_job_state(&job_id, "running");
        let res = run_ebook_convert(&input, &output);
        crate::ffmpeg::unregister_job(&job_id);
        res
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

fn run_ebook_convert(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let mut child = std::process::Command::new("ebook-convert")
        .arg(input)
        .arg(output)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;

    let t0 = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if t0.elapsed() > CONVERT_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("killed after {} s", CONVERT_TIMEOUT.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(400)),
            Err(e) => return Err(format!("wait: {e}")),
        }
    };

    if status.success() {
        return Ok(());
    }
    let mut tail = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        use std::io::Read;
        let _ = pipe.read_to_string(&mut tail);
    }
    let last = tail
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("no stderr");
    Err(format!("exit {}: {last}", status.code().unwrap_or(-1)))
}

#[derive(Deserialize)]
struct FileQuery {
    dl: Option<String>,
}

async fn handle_file(
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    Query(q): Query<FileQuery>,
    req_headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let key = normalize_key(&ol_key);
    let book = {
        let db = state.db.lock().await;
        match db.find_book_by_key(&key) {
            Ok(Some(b)) => b,
            Ok(None) => return err(StatusCode::NOT_FOUND, "not in library"),
            Err(e) => {
                eprintln!("[books] file lookup failed: {e}");
                return err(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed");
            }
        }
    };

    let Some(rel) = book.file_path else {
        return err(StatusCode::NOT_FOUND, "no file yet, fetch one first");
    };
    let abs = abs_path(&state.media_root, &rel);
    let mut file = match tokio::fs::File::open(&abs).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[books] file open failed {}: {e}", abs.display());
            return err(StatusCode::NOT_FOUND, "file gone");
        }
    };

    let meta = match file.metadata().await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[books] stat failed {}: {e}", abs.display());
            return err(StatusCode::INTERNAL_SERVER_ERROR, "stat failed");
        }
    };
    let len = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let etag = format!("\"{len:x}-{mtime:x}\"");

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(header::ETAG, etag.parse().unwrap());
    headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "private, no-cache".parse().unwrap());

    if req_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == etag)
    {
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }

    let mime = book_mime(book.ext.as_deref().unwrap_or(""));
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    if matches!(q.dl.as_deref(), Some("1") | Some("true")) {
        let name = ascii_filename(&book.title, &key);
        let ext = book.ext.as_deref().unwrap_or("bin");
        headers.insert(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}.{ext}\"")
                .parse()
                .unwrap(),
        );
    }

    if let Some(rh) = req_headers.get(header::RANGE).and_then(|h| h.to_str().ok()) {
        if let Some((start, end)) = parse_range(rh, len) {
            let chunk = end - start + 1;
            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return err(StatusCode::INTERNAL_SERVER_ERROR, "seek failed");
            }
            headers.insert(header::CONTENT_LENGTH, chunk.into());
            headers.insert(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{len}").parse().unwrap(),
            );
            let body = Body::from_stream(ReaderStream::new(file.take(chunk)));
            return (StatusCode::PARTIAL_CONTENT, headers, body).into_response();
        }
    }

    headers.insert(header::CONTENT_LENGTH, len.into());
    let body = Body::from_stream(ReaderStream::new(file));
    (StatusCode::OK, headers, body).into_response()
}

fn parse_range(s: &str, total: u64) -> Option<(u64, u64)> {
    let rest = s.strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    let start: u64 = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        total.saturating_sub(n)
    } else {
        a.parse().ok()?
    };
    let end: u64 = if b.is_empty() {
        total.checked_sub(1)?
    } else {
        let parsed: u64 = b.parse().ok()?;
        parsed.min(total.checked_sub(1)?)
    };
    if start > end {
        return None;
    }
    Some((start, end))
}

async fn handle_cover(Path(cover_id): Path<String>) -> impl IntoResponse {
    let id = cover_id.trim_end_matches(".jpg");
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return err(StatusCode::BAD_REQUEST, "bad cover id");
    }
    let url = format!("{OL_COVERS}/b/id/{id}-L.jpg?default=false");
    proxy_image(&url, "image/jpeg").await
}

async fn handle_author_cover(
    State(state): State<Arc<AppState>>,
    Path(olid): Path<String>,
) -> impl IntoResponse {
    let id = olid.trim_end_matches(".jpg");
    if !id.starts_with("OL") || !id.chars().all(|c| c.is_ascii_alphanumeric()) || id.len() > 24 {
        return err(StatusCode::BAD_REQUEST, "bad author olid");
    }
    let url = format!("{OL_COVERS}/a/olid/{id}-M.jpg?default=false");
    let cache_path = abs_path(
        &state.media_root,
        &format!("books/_author_portraits/{id}.jpg"),
    );
    proxy_cached_image(&url, "image/jpeg", &cache_path).await
}

async fn handle_author(
    State(state): State<Arc<AppState>>,
    Path(olid): Path<String>,
) -> impl IntoResponse {
    if !olid.starts_with("OL")
        || !olid.chars().all(|c| c.is_ascii_alphanumeric())
        || olid.len() > 24
    {
        return err(StatusCode::BAD_REQUEST, "bad author olid");
    }
    let (info, works) = tokio::join!(ol_author_full(&olid), ol_author_works(&olid));
    let info = match info {
        Ok(Some(a)) => a,
        Ok(None) => return err(StatusCode::NOT_FOUND, "no such author"),
        Err(e) => {
            eprintln!("[books] author info failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "openlibrary unavailable");
        }
    };
    let mut top_works = works.unwrap_or_default();
    mark_in_library(&state, &mut top_works).await;
    Json(serde_json::json!({
        "olid": olid,
        "name": info.name,
        "bio": info.bio,
        "birth_date": info.birth_date,
        "death_date": info.death_date,
        "photo_url": info.photo_id.map(|_| format!("/api/books/author-cover/{olid}.jpg")),
        "top_works": top_works,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct ProgressBody {
    ol_key: String,
    cfi: Option<String>,
    percent: f64,
}

async fn handle_progress(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<ProgressBody>,
) -> impl IntoResponse {
    let key = normalize_key(&b.ol_key);
    let pct = b.percent.clamp(0.0, 1.0);
    let db = state.db.lock().await;
    if let Err(e) = db.upsert_book_progress(&auth.id, &key, b.cfi.as_deref(), pct) {
        eprintln!("[books] progress save failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }
    let shelf = if pct >= 0.97 {
        db.set_book_shelf(&auth.id, &key, "read")
    } else if pct > 0.0 {
        db.touch_book_shelf_reading(&auth.id, &key)
    } else {
        db.touch_book_shelf_want(&auth.id, &key)
    };
    if let Err(e) = shelf {
        eprintln!("[books] shelf update failed: {e}");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_continue(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.list_book_continue(&auth.id, 12) {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            eprintln!("[books] continue list failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

const SHELF_STATUSES: &[&str] = &["want", "reading", "read"];

#[derive(Deserialize)]
struct ShelfBody {
    status: String,
}

#[derive(Deserialize)]
struct ShowcaseBody {
    showcased: bool,
}

async fn handle_shelf(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let items = match db.list_book_shelf(&auth.id) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[books] shelf list failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "list failed");
        }
    };
    let year = chrono::Utc::now().format("%Y").to_string();
    let read_total = items.iter().filter(|item| item.status == "read").count();
    let read_year = items
        .iter()
        .filter(|item| {
            item.status == "read"
                && item
                    .finished_at
                    .as_deref()
                    .is_some_and(|finished| finished.starts_with(&year))
        })
        .count();
    let goal = db.get_book_goal(&auth.id).unwrap_or(None);
    drop(db);

    for item in items
        .iter()
        .filter(|item| book_cover_needs_refresh(item.cover_url.as_deref()))
    {
        spawn_cover_refresh(&state, &item.ol_key);
    }
    Json(serde_json::json!({
        "items": items,
        "read_total": read_total,
        "read_year": read_year,
        "goal": goal,
    }))
    .into_response()
}

async fn handle_daily_quote(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let candidates = {
        let db = state.db.lock().await;
        db.list_daily_quote_candidates(&auth.id).unwrap_or_default()
    };
    if candidates.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }

    let mut by_book: HashMap<&str, usize> = HashMap::new();
    for (m, _, _, _) in &candidates {
        *by_book.entry(m.ol_key.as_str()).or_insert(0) += 1;
    }

    use sha2::Digest;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut hasher = sha2::Sha256::new();
    hasher.update(auth.id.as_bytes());
    hasher.update(b"|");
    hasher.update(today.as_bytes());
    let digest = hasher.finalize();
    let seed = u64::from_le_bytes(digest[0..8].try_into().unwrap());
    let r = (seed as f64) / (u64::MAX as f64);

    let total: f64 = candidates
        .iter()
        .map(|(m, _, _, _)| 1.0 / *by_book.get(m.ol_key.as_str()).unwrap_or(&1) as f64)
        .sum();
    let target = r * total;
    let mut cum = 0.0;
    let mut picked = &candidates[0];
    for c in &candidates {
        let w = 1.0 / *by_book.get(c.0.ol_key.as_str()).unwrap_or(&1) as f64;
        cum += w;
        if cum >= target {
            picked = c;
            break;
        }
    }

    let (mark, title, authors, cover_url) = picked;
    Json(serde_json::json!({
        "id": mark.id,
        "ol_key": mark.ol_key,
        "cfi": mark.cfi,
        "snippet": mark.snippet,
        "chapter": mark.chapter,
        "title": title,
        "authors": authors,
        "cover_url": cover_url,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct GoalBody {
    goal: i64,
}

async fn handle_goal(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(b): Json<GoalBody>,
) -> impl IntoResponse {
    if !(0..=999).contains(&b.goal) {
        return err(StatusCode::BAD_REQUEST, "bad goal");
    }
    let goal = if b.goal == 0 { None } else { Some(b.goal) };
    let db = state.db.lock().await;
    if let Err(e) = db.set_book_goal(&auth.id, goal) {
        eprintln!("[books] goal save failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_shelf_set(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    Json(b): Json<ShelfBody>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    if !SHELF_STATUSES.contains(&b.status.as_str()) {
        return err(StatusCode::BAD_REQUEST, "bad status");
    }
    if let Err(resp) = find_or_create_book(&state, &key, &auth).await {
        return resp;
    }
    let db = state.db.lock().await;
    if let Err(e) = db.set_book_shelf(&auth.id, &key, &b.status) {
        eprintln!("[books] shelf set failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_shelf_showcase(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    Json(body): Json<ShowcaseBody>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let db = state.db.lock().await;
    match db.set_book_showcased(&auth.id, &key, body.showcased) {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(crate::db::CollectionError::NotFound) => err(StatusCode::NOT_FOUND, "not on shelf"),
        Err(crate::db::CollectionError::ShowcaseLimit) => {
            err(StatusCode::CONFLICT, "showcase is limited to five items")
        }
        Err(crate::db::CollectionError::ShowcaseRequiresCompletion) => err(
            StatusCode::CONFLICT,
            "only completed items can be showcased",
        ),
        Err(crate::db::CollectionError::Database(error)) => {
            eprintln!("[books] showcase save failed: {error}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "save failed")
        }
    }
}

async fn handle_shelf_remove(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let db = state.db.lock().await;
    match db.delete_book_shelf(&auth.id, &key) {
        Ok(true) => Json(serde_json::json!({ "ok": true })).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "not on shelf"),
        Err(e) => {
            eprintln!("[books] shelf remove failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "remove failed")
        }
    }
}

const MARK_COLORS: &[&str] = &["yellow", "green", "blue", "pink"];

#[derive(Deserialize)]
struct MarkBody {
    kind: String,
    cfi: String,
    color: Option<String>,
    note: Option<String>,
    snippet: Option<String>,
    chapter: Option<String>,
}

#[derive(Deserialize)]
struct MarkPatch {
    color: Option<String>,
    note: Option<String>,
}

async fn handle_marks_list(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    let db = state.db.lock().await;
    match db.list_book_marks(&auth.id, &key) {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => {
            eprintln!("[books] marks list failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

async fn handle_mark_create(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ol_key): Path<String>,
    Json(b): Json<MarkBody>,
) -> impl IntoResponse {
    let key = normalize_key(&ol_key);
    if b.kind != "highlight" && b.kind != "bookmark" {
        return err(StatusCode::BAD_REQUEST, "bad kind");
    }
    if b.cfi.is_empty() || b.cfi.len() > 2000 {
        return err(StatusCode::BAD_REQUEST, "bad cfi");
    }
    if b.kind == "highlight" && !b.color.as_deref().is_some_and(|c| MARK_COLORS.contains(&c)) {
        return err(StatusCode::BAD_REQUEST, "bad color");
    }
    if b.note.as_deref().is_some_and(|s| s.len() > 4000)
        || b.snippet.as_deref().is_some_and(|s| s.len() > 2000)
        || b.chapter.as_deref().is_some_and(|s| s.len() > 500)
    {
        return err(StatusCode::BAD_REQUEST, "too long");
    }
    let color = if b.kind == "highlight" {
        b.color.as_deref()
    } else {
        None
    };
    let db = state.db.lock().await;
    match db.create_book_mark(
        &auth.id,
        &key,
        &b.kind,
        &b.cfi,
        color,
        b.note.as_deref(),
        b.snippet.as_deref(),
        b.chapter.as_deref(),
    ) {
        Ok(mark) => Json(mark).into_response(),
        Err(e) => {
            eprintln!("[books] mark create failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "create failed")
        }
    }
}

async fn handle_mark_update(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(b): Json<MarkPatch>,
) -> impl IntoResponse {
    if b.color
        .as_deref()
        .is_some_and(|c| !MARK_COLORS.contains(&c))
    {
        return err(StatusCode::BAD_REQUEST, "bad color");
    }
    if b.note.as_deref().is_some_and(|s| s.len() > 4000) {
        return err(StatusCode::BAD_REQUEST, "too long");
    }
    let db = state.db.lock().await;
    match db.update_book_mark(&id, &auth.id, b.color.as_deref(), b.note.as_deref()) {
        Ok(true) => Json(serde_json::json!({ "ok": true })).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "no such mark"),
        Err(e) => {
            eprintln!("[books] mark update failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "update failed")
        }
    }
}

async fn handle_mark_delete(
    Extension(auth): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_book_mark(&id, &auth.id) {
        Ok(true) => Json(serde_json::json!({ "ok": true })).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "no such mark"),
        Err(e) => {
            eprintln!("[books] mark delete failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "delete failed")
        }
    }
}

fn enrich_claims() -> &'static Mutex<HashMap<String, Instant>> {
    ENRICHING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn spawn_enrich(state: &Arc<AppState>, ol_key: &str) {
    {
        let mut m = enrich_claims().lock().unwrap();
        if m.get(ol_key).is_some_and(|t| t.elapsed() < ENRICH_RETRY) {
            return;
        }
        m.insert(ol_key.to_string(), Instant::now());
    }
    tokio::spawn(enrich_book(state.clone(), ol_key.to_string()));
}

fn cover_recovery_claims() -> &'static Mutex<HashMap<String, Instant>> {
    COVER_RECOVERING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn spawn_cover_refresh(state: &Arc<AppState>, ol_key: &str) {
    {
        let mut claims = cover_recovery_claims().lock().unwrap();
        if claims
            .get(ol_key)
            .is_some_and(|at| at.elapsed() < ENRICH_RETRY)
        {
            return;
        }
        claims.insert(ol_key.to_string(), Instant::now());
    }
    tokio::spawn(refresh_book_cover(state.clone(), ol_key.to_string()));
}

async fn refresh_book_cover(state: Arc<AppState>, ol_key: String) {
    let edition_cover = ol_editions_cached(&ol_key)
        .await
        .ok()
        .and_then(|edition| edition.cover_id)
        .map(|id| format!("/api/books/cover/{id}.jpg"));
    let cover_url = match edition_cover {
        Some(url) => Some(url),
        None => match ol_work_cached(&ol_key).await {
            Ok(Some(hit)) => hit.cover_url,
            Ok(None) => None,
            Err(e) => {
                eprintln!("[books] cover refresh failed for {ol_key}: {e}");
                None
            }
        },
    };

    if let Some(cover_url) = cover_url {
        let db = state.db.lock().await;
        match db.update_book_cover(&ol_key, &cover_url) {
            Ok(true) => println!("[books] refreshed cover for {ol_key}"),
            Ok(false) => {}
            Err(e) => eprintln!("[books] cover save failed for {ol_key}: {e}"),
        }
    }
    cover_recovery_claims().lock().unwrap().remove(&ol_key);
}

fn book_cover_needs_refresh(cover_url: Option<&str>) -> bool {
    cover_url.is_none_or(|url| {
        url.trim().is_empty() || url.contains("/api/books/cover/") && !url.contains(".jpg")
    })
}

async fn enrich_book(state: Arc<AppState>, ol_key: String) {
    let book = {
        let db = state.db.lock().await;
        db.find_book_by_key(&ol_key).ok().flatten()
    };
    let Some(book) = book else { return };
    let has_desc = book.description.as_deref().is_some_and(|s| !s.is_empty());
    if book.enriched_at.is_some() && has_desc {
        enrich_claims().lock().unwrap().remove(&ol_key);
        return;
    }
    println!("[books] enriching '{}'", book.title);

    let mut pages = None;
    let mut subjects = None;
    let mut isbn = None;
    let mut publisher = None;
    let mut rating = None;
    let mut rating_count = None;
    let mut series: Option<String> = None;
    let mut description: Option<String> = None;

    match ol_work_meta_cached(&ol_key).await {
        Ok(m) => {
            if !m.subjects.is_empty() {
                subjects = Some(m.subjects.join(", "));
            }
            series = m.series;
        }
        Err(e) => eprintln!("[books] subjects fetch failed: {e}"),
    }
    match ol_ratings_cached(&ol_key).await {
        Ok(Some((avg, count))) => {
            rating = Some(avg);
            rating_count = Some(count);
        }
        Ok(None) => {}
        Err(e) => eprintln!("[books] ratings fetch failed: {e}"),
    }
    match ol_editions_cached(&ol_key).await {
        Ok(ed) => {
            pages = ed.pages;
            isbn = ed.isbn;
            publisher = ed.publisher;
        }
        Err(e) => eprintln!("[books] editions fetch failed: {e}"),
    }

    let need_desc = book.description.as_deref().is_none_or(str::is_empty);
    if pages.is_none() || subjects.is_none() || rating.is_none() || need_desc {
        match gbooks_lookup(isbn.as_deref(), &book.title, book.authors.as_deref()).await {
            Ok(Some(g)) => {
                if pages.is_none() {
                    pages = g.pages;
                }
                if subjects.is_none() {
                    subjects = g.categories;
                }
                if rating.is_none() {
                    rating = g.rating;
                    rating_count = g.rating_count;
                }
                if publisher.is_none() {
                    publisher = g.publisher;
                }
                if need_desc {
                    description = g.description;
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("[books] gbooks fetch failed: {e}"),
        }
    }

    let got = [
        pages.is_some(),
        subjects.is_some(),
        isbn.is_some(),
        publisher.is_some(),
        rating.is_some(),
        series.is_some(),
        description.is_some(),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    if got == 0 {
        eprintln!("[books] enrich got nothing for '{}'", book.title);
        return;
    }

    {
        let db = state.db.lock().await;
        if let Err(e) = db.update_book_enrichment(
            &ol_key,
            pages,
            subjects.as_deref(),
            isbn.as_deref(),
            publisher.as_deref(),
            rating,
            rating_count,
            series.as_deref(),
            description.as_deref(),
        ) {
            eprintln!("[books] enrich save failed: {e}");
            return;
        }
    }
    enrich_claims().lock().unwrap().remove(&ol_key);
    println!("[books] enrich done: {got} fields for '{}'", book.title);
}

fn clean_subject(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 60 {
        return None;
    }
    let low = raw.to_ascii_lowercase();
    let junk = [
        "accessible book",
        "protected daisy",
        "in library",
        "overdrive",
        "large type",
        "reading level",
        "staff picks",
        "internet archive",
    ];
    if junk.iter().any(|j| low.contains(j)) {
        return None;
    }

    let cleaned = match raw.split_once(':') {
        Some((scheme, value)) => match scheme.to_ascii_lowercase().as_str() {
            "nyt" | "lcc" | "lc_classifications" | "ddc" => return None,
            "award" => {
                let name = value.split('=').next()?.replace('_', " ");
                let t = title_case(name.trim());
                if t.is_empty() {
                    return None;
                } else {
                    format!("{t} Winner")
                }
            }
            "franchise" => return None,
            _ => title_case(value.trim()),
        },
        None => raw.to_string(),
    };
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let lower = w.to_ascii_lowercase();
            let mut c = lower.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone)]
struct WorkMeta {
    subjects: Vec<String>,
    series: Option<String>,
}

async fn ol_work_meta(key: &str) -> reqwest::Result<WorkMeta> {
    #[derive(Deserialize)]
    struct W {
        subjects: Option<Vec<String>>,
    }
    let w: W = client()
        .get(format!("{OL_API}/works/{key}.json"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let raw = w.subjects.unwrap_or_default();
    let series = raw
        .iter()
        .find_map(|s| s.strip_prefix("series:").map(|n| n.trim().to_string()))
        .filter(|s| !s.is_empty());
    let mut seen = std::collections::HashSet::new();
    let subjects: Vec<String> = raw
        .into_iter()
        .filter_map(|s| clean_subject(&s))
        .filter(|s| seen.insert(s.to_lowercase()))
        .take(10)
        .collect();
    Ok(WorkMeta { subjects, series })
}

async fn ol_ratings(key: &str) -> reqwest::Result<Option<(f64, i64)>> {
    #[derive(Deserialize)]
    struct Summary {
        average: Option<f64>,
        count: Option<i64>,
    }
    #[derive(Deserialize)]
    struct R {
        summary: Option<Summary>,
    }
    let r: R = client()
        .get(format!("{OL_API}/works/{key}/ratings.json"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.summary.and_then(|s| match (s.average, s.count) {
        (Some(a), Some(c)) if c > 0 => Some((a, c)),
        _ => None,
    }))
}

#[derive(Clone)]
struct EditionBits {
    cover_id: Option<i64>,
    pages: Option<i64>,
    isbn: Option<String>,
    publisher: Option<String>,
    language: Option<String>,
    year: Option<i32>,
}

async fn ol_editions(key: &str) -> reqwest::Result<EditionBits> {
    let v: serde_json::Value = client()
        .get(format!("{OL_API}/works/{key}/editions.json"))
        .query(&[("limit", "20")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut out = EditionBits {
        cover_id: None,
        pages: None,
        isbn: None,
        publisher: None,
        language: None,
        year: None,
    };
    let entries = v["entries"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    out.cover_id = best_edition_cover(entries);
    for e in entries {
        if is_audio_edition(e) {
            continue;
        }
        if out.pages.is_none() {
            let p = e["number_of_pages"].as_i64().or_else(|| {
                e["number_of_pages"]
                    .as_str()
                    .and_then(|s| s.trim().parse().ok())
            });
            if let Some(p) = p {
                if p >= 5 && p < 20000 {
                    out.pages = Some(p);
                }
            }
        }
        if out.language.is_none() {
            if let Some(arr) = e["languages"].as_array() {
                for l in arr {
                    if let Some(k) = l["key"].as_str() {
                        let code = k.trim_start_matches("/languages/").to_string();
                        if !code.is_empty() {
                            out.language = Some(code);
                            break;
                        }
                    }
                }
            }
        }
        if out.year.is_none() {
            if let Some(d) = e["publish_date"].as_str() {
                out.year = parse_year(d);
            }
        }
        if out.isbn.is_none() {
            out.isbn = e["isbn_13"][0]
                .as_str()
                .or_else(|| e["isbn_10"][0].as_str())
                .map(str::to_string);
        }
        if out.publisher.is_none() {
            out.publisher = e["publishers"][0].as_str().map(str::to_string);
        }
    }
    Ok(out)
}

fn best_edition_cover(entries: &[serde_json::Value]) -> Option<i64> {
    entries
        .iter()
        .filter_map(|edition| {
            newest_edition_cover(edition).map(|id| (edition_cover_rank(edition), id))
        })
        .max()
        .map(|(_, id)| id)
}

fn newest_edition_cover(e: &serde_json::Value) -> Option<i64> {
    e["covers"]
        .as_array()?
        .iter()
        .filter_map(serde_json::Value::as_i64)
        .filter(|id| *id > 0)
        .max()
}

fn edition_cover_rank(e: &serde_json::Value) -> u8 {
    if is_audio_edition(e) {
        return 0;
    }
    let fmt = e["physical_format"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    if [
        "hardcover",
        "hardback",
        "paperback",
        "softcover",
        "mass market",
    ]
    .iter()
    .any(|kind| fmt.contains(kind))
        || e["number_of_pages"].as_i64().is_some()
    {
        return 3;
    }
    if fmt.contains("ebook") || fmt.contains("e-book") || fmt.contains("kindle") {
        return 1;
    }
    2
}

fn is_audio_edition(e: &serde_json::Value) -> bool {
    let fmt = e["physical_format"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    if fmt.contains("audio")
        || fmt.contains("mp3")
        || fmt.contains("cassette")
        || fmt.contains("cd")
    {
        return true;
    }
    let pub_str = e["publishers"][0]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    const AUDIO_PUBS: &[&str] = &[
        "recorded books",
        "audible",
        "brilliance audio",
        "blackstone",
        "tantor",
        "macmillan audio",
        "harperaudio",
        "harper audio",
        "books on tape",
        "listening library",
        "audiogo",
        "highbridge",
    ];
    AUDIO_PUBS.iter().any(|p| pub_str.contains(p))
}

struct GbBits {
    pages: Option<i64>,
    categories: Option<String>,
    rating: Option<f64>,
    rating_count: Option<i64>,
    publisher: Option<String>,
    description: Option<String>,
}

async fn gbooks_lookup(
    isbn: Option<&str>,
    title: &str,
    authors: Option<&str>,
) -> reqwest::Result<Option<GbBits>> {
    let q = match isbn {
        Some(i) => format!("isbn:{i}"),
        None => {
            let t = title.replace('"', "");
            let author = authors
                .unwrap_or("")
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .replace('"', "");
            if author.is_empty() {
                format!("intitle:\"{t}\"")
            } else {
                format!("intitle:\"{t}\" inauthor:\"{author}\"")
            }
        }
    };
    let v: serde_json::Value = client()
        .get("https://www.googleapis.com/books/v1/volumes")
        .query(&[("q", q.as_str()), ("maxResults", "1"), ("country", "US")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(info) = v["items"][0]["volumeInfo"].as_object() else {
        return Ok(None);
    };
    let cats: Vec<String> = info
        .get("categories")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .filter_map(clean_subject)
                .take(6)
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(GbBits {
        pages: info
            .get("pageCount")
            .and_then(|p| p.as_i64())
            .filter(|p| *p > 0),
        categories: if cats.is_empty() {
            None
        } else {
            Some(cats.join(", "))
        },
        rating: info.get("averageRating").and_then(|r| r.as_f64()),
        rating_count: info.get("ratingsCount").and_then(|r| r.as_i64()),
        publisher: info
            .get("publisher")
            .and_then(|p| p.as_str())
            .map(str::to_string),
        description: info
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() > 20),
    }))
}

async fn ol_search(query: &str) -> reqwest::Result<Vec<(BookHit, Vec<String>)>> {
    let r: OlSearch = client()
        .get(format!("{OL_API}/search.json"))
        .query(&[
            ("q", query),
            ("limit", "24"),
            (
                "fields",
                "key,title,author_name,first_publish_year,cover_i,language,edition_count,subject",
            ),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.docs
        .iter()
        .map(|d| {
            let hit = map_ol_doc(d);
            let subj = d.subject.clone().unwrap_or_default();
            (hit, subj)
        })
        .collect())
}

async fn ol_series(name: &str) -> reqwest::Result<Vec<BookHit>> {
    let subject = format!("series:{name}");
    let r: OlSearch = client()
        .get(format!("{OL_API}/search.json"))
        .query(&[
            ("subject", subject.as_str()),
            ("limit", "20"),
            (
                "fields",
                "key,title,author_name,first_publish_year,cover_i,language",
            ),
            ("sort", "old"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.docs.iter().map(map_ol_doc).collect())
}

async fn ol_by_author(name: &str) -> reqwest::Result<Vec<BookHit>> {
    let r: OlSearch = client()
        .get(format!("{OL_API}/search.json"))
        .query(&[
            ("author", name),
            ("limit", "40"),
            (
                "fields",
                "key,title,author_name,first_publish_year,cover_i,language,edition_count",
            ),
            ("sort", "rating"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut hits: Vec<BookHit> = r.docs.iter().map(map_ol_doc).collect();
    hits.retain(|h| {
        let t = h.title.to_ascii_lowercase();
        !t.contains("duplicate of") && !t.contains("(duplicate") && t != "untitled"
    });
    hits.sort_by_key(|h| h.cover_url.is_none());
    hits.truncate(24);
    Ok(hits)
}

async fn ol_trending() -> reqwest::Result<Vec<BookHit>> {
    let r: OlTrending = client()
        .get(format!("{OL_API}/trending/weekly.json"))
        .query(&[("limit", "24")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.works.iter().map(map_ol_doc).collect())
}

async fn ol_work(ol_key: &str) -> reqwest::Result<Option<BookHit>> {
    let r = client()
        .get(format!("{OL_API}/works/{ol_key}.json"))
        .send()
        .await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let w: OlWork = r.error_for_status()?.json().await?;

    let mut hit = BookHit {
        ol_key: ol_key.to_string(),
        title: w.title.unwrap_or_else(|| "untitled".into()),
        authors: None,
        description: w
            .description
            .map(|d| d.into_text())
            .filter(|s| !s.is_empty()),
        cover_url: w
            .covers
            .and_then(|c| c.into_iter().next())
            .map(|id| format!("/api/books/cover/{id}.jpg")),
        year: w.first_publish_date.as_deref().and_then(parse_year),
        language: None,
        author_keys: Vec::new(),
        in_library: false,
        ready: false,
        kind: "book".into(),
        series_count: None,
        series_covers: Vec::new(),
    };

    if let Some(refs) = w.authors {
        let mut keys = Vec::new();
        let mut handles = Vec::new();
        for r in &refs {
            let olid = r
                .author
                .key
                .trim_start_matches('/')
                .trim_start_matches("authors/")
                .to_string();
            if !olid.is_empty() {
                keys.push(olid);
            }
        }
        for r in refs {
            let key = r.author.key.clone();
            handles.push(tokio::spawn(async move { ol_author_name(&key).await }));
        }
        let mut names = Vec::new();
        for h in handles {
            match h.await {
                Ok(Ok(Some(name))) => names.push(name),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => return Err(e),
                Err(_) => {}
            }
        }
        if !names.is_empty() {
            hit.authors = Some(names.join(", "));
        }
        hit.author_keys = keys;
    }
    Ok(Some(hit))
}

async fn ol_author_name(key: &str) -> reqwest::Result<Option<String>> {
    let key = key.trim_start_matches('/');
    let r = client().get(format!("{OL_API}/{key}.json")).send().await?;
    if !r.status().is_success() {
        return Ok(None);
    }
    let a: OlAuthor = r.json().await?;
    Ok(a.name)
}

struct AuthorInfo {
    name: String,
    bio: Option<String>,
    birth_date: Option<String>,
    death_date: Option<String>,
    photo_id: Option<i64>,
}

async fn ol_author_full(olid: &str) -> reqwest::Result<Option<AuthorInfo>> {
    let r = client()
        .get(format!("{OL_API}/authors/{olid}.json"))
        .send()
        .await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let a: OlAuthorFull = r.error_for_status()?.json().await?;
    let bio = a.bio.map(|b| b.into_text()).filter(|s| !s.is_empty());
    let photo_id = a.photos.and_then(|v| v.into_iter().find(|id| *id > 0));
    Ok(Some(AuthorInfo {
        name: a.name.unwrap_or_else(|| olid.to_string()),
        bio,
        birth_date: a.birth_date.filter(|s| !s.is_empty()),
        death_date: a.death_date.filter(|s| !s.is_empty()),
        photo_id,
    }))
}

async fn ol_author_works(olid: &str) -> reqwest::Result<Vec<BookHit>> {
    let v: serde_json::Value = client()
        .get(format!("{OL_API}/authors/{olid}/works.json"))
        .query(&[("limit", "50")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut out = Vec::new();
    for e in v["entries"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let key = match e["key"].as_str() {
            Some(k) => normalize_key(k),
            None => continue,
        };
        let title = e["title"].as_str().unwrap_or("untitled").to_string();
        let cover_url = e["covers"][0]
            .as_i64()
            .filter(|id| *id > 0)
            .map(|id| format!("/api/books/cover/{id}.jpg"));
        let year = e["first_publish_date"].as_str().and_then(parse_year);
        out.push(BookHit {
            ol_key: key,
            title,
            authors: None,
            description: None,
            cover_url,
            year,
            language: None,
            author_keys: Vec::new(),
            in_library: false,
            ready: false,
            kind: "book".into(),
            series_count: None,
            series_covers: Vec::new(),
        });
        if out.len() >= 24 {
            break;
        }
    }
    Ok(out)
}

fn map_ol_doc(d: &OlDoc) -> BookHit {
    let authors = d.author_name.as_ref().map(|v| v.join(", "));
    let cover_url = d.cover_i.map(|id| format!("/api/books/cover/{id}.jpg"));
    let language = d.language.as_ref().and_then(|v| v.first().cloned());
    BookHit {
        ol_key: normalize_key(&d.key),
        title: d.title.clone().unwrap_or_else(|| "untitled".into()),
        authors,
        description: None,
        cover_url,
        year: d.first_publish_year,
        language,
        author_keys: Vec::new(),
        in_library: false,
        ready: false,
        kind: "book".into(),
        series_count: None,
        series_covers: Vec::new(),
    }
}

async fn libgen_html(
    path: &str,
    query: &[(&str, &str)],
) -> reqwest::Result<(&'static str, String)> {
    let start = LIBGEN_GOOD.load(Ordering::Relaxed);
    let mut last_err = None;
    for i in 0..LIBGEN_MIRRORS.len() {
        let idx = (start + i) % LIBGEN_MIRRORS.len();
        let base = LIBGEN_MIRRORS[idx];
        let res = client()
            .get(format!("{base}/{path}"))
            .query(query)
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .and_then(|r| r.error_for_status());
        match res {
            Ok(r) => match r.text().await {
                Ok(html) => {
                    if idx != start {
                        println!("[books] libgen mirror -> {base}");
                    }
                    LIBGEN_GOOD.store(idx, Ordering::Relaxed);
                    return Ok((base, html));
                }
                Err(e) => last_err = Some(e),
            },
            Err(e) => {
                eprintln!("[books] {base}: {e}");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("mirror list is not empty"))
}

async fn libgen_search(query: &str) -> reqwest::Result<Vec<BookSource>> {
    let (_, html) = libgen_html(
        "index.php",
        &[
            ("req", query),
            ("res", "25"),
            ("phrase", "1"),
            ("columns[]", "t"),
            ("columns[]", "a"),
            ("curtab", "f"),
        ],
    )
    .await?;
    Ok(parse_libgen_li(&html))
}

async fn libgen_resolve(md5: &str) -> reqwest::Result<Option<String>> {
    let (base, html) = libgen_html("ads.php", &[("md5", md5)]).await?;
    Ok(extract_get_link(&html).map(|p| {
        if p.starts_with("http") {
            p
        } else {
            format!("{base}/{}", p.trim_start_matches('/'))
        }
    }))
}

fn parse_libgen_li(html: &str) -> Vec<BookSource> {
    let mut out = Vec::new();
    let body = match html.split_once("<tbody>") {
        Some((_, rest)) => rest,
        None => return out,
    };

    for row in body.split("<tr>") {
        if !row.contains("ads.php?md5=") {
            continue;
        }
        let Some(md5) = row
            .split("ads.php?md5=")
            .nth(1)
            .and_then(|s| s.get(..32))
            .filter(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
            .map(|s| s.to_ascii_lowercase())
        else {
            continue;
        };

        let cells: Vec<&str> = row.split("</td>").collect();
        if cells.len() < 8 {
            continue;
        }

        let title = extract_li_title(cells[0]);
        let author = strip_tags(cells[1]);
        let publisher = strip_tags(cells[2]);
        let year = strip_tags(cells[3])
            .split([';', '-', ' '])
            .next()
            .and_then(|s| s.parse::<i32>().ok());
        let language = {
            let s = strip_tags(cells[4]);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        let pages = strip_tags(cells[5])
            .split('/')
            .next_back()
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<i32>().ok());
        let size = parse_size(&strip_tags(cells[6]));
        let ext = strip_tags(cells[7]).to_ascii_lowercase();

        if title.is_empty() || ext.is_empty() {
            continue;
        }

        out.push(BookSource {
            md5,
            title,
            authors: if author.is_empty() {
                None
            } else {
                Some(author)
            },
            publisher: if publisher.is_empty() {
                None
            } else {
                Some(publisher)
            },
            ext,
            language,
            size,
            year,
            pages,
        });
    }
    out
}

fn extract_li_title(cell: &str) -> String {
    let mut cur = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    let mut quote: Option<char> = None;
    let mut in_edition = false;

    for c in cell.chars() {
        if !in_tag {
            if c == '<' {
                in_tag = true;
                tag.clear();
            } else if in_edition {
                cur.push(c);
            }
            continue;
        }
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
                tag.push(c);
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    tag.push(c);
                }
                '>' => {
                    in_tag = false;
                    if tag.starts_with("a ") && tag.contains("edition.php") {
                        in_edition = true;
                        cur.clear();
                    } else if tag.starts_with("/a") && in_edition {
                        in_edition = false;
                        let t = decode_entities(cur.trim());
                        if !t.is_empty() {
                            return t;
                        }
                    }
                }
                _ => tag.push(c),
            },
        }
    }
    strip_tags(cell)
}

fn extract_get_link(html: &str) -> Option<String> {
    let needle = ">GET</";
    let cut = html.find(needle)?;
    let before = &html[..cut];
    let href = before.rfind("href=\"")?;
    let start = href + 6;
    let end = before[start..].find('"')?;
    let raw = &before[start..start + end];
    Some(decode_entities(raw))
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        if in_tag {
            match quote {
                Some(q) => {
                    if c == q {
                        quote = None;
                    }
                }
                None => match c {
                    '"' | '\'' => quote = Some(c),
                    '>' => in_tag = false,
                    _ => {}
                },
            }
        } else if c == '<' {
            in_tag = true;
        } else {
            out.push(c);
        }
    }
    decode_entities(out.trim())
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn parse_size(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 1 {
        return parts[0].parse().ok();
    }
    let num: f64 = parts[0].parse().ok()?;
    let mult = match parts[1].to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((num * mult) as i64)
}

fn parse_year(s: &str) -> Option<i32> {
    s.split(['-', ' ', '/']).next()?.parse().ok()
}

fn normalize_key(k: &str) -> String {
    k.trim_start_matches('/')
        .trim_start_matches("works/")
        .to_string()
}

fn sanitize_ext(s: &str) -> String {
    let s = s.trim().trim_start_matches('.').to_ascii_lowercase();
    if matches!(
        s.as_str(),
        "epub" | "mobi" | "pdf" | "azw3" | "fb2" | "djvu" | "txt"
    ) {
        s
    } else {
        String::new()
    }
}

fn needs_conversion(ext: &str) -> bool {
    matches!(ext, "mobi" | "azw3" | "fb2" | "djvu")
}

fn looks_like_book(ext: &str, b: &[u8]) -> bool {
    match ext {
        "epub" => b.starts_with(b"PK\x03\x04"),
        "pdf" => b.starts_with(b"%PDF"),
        "mobi" | "azw3" => b.len() > 68 && &b[60..68] == b"BOOKMOBI",
        "djvu" => b.starts_with(b"AT&T"),
        "fb2" => b.starts_with(b"<?xml") || b.starts_with(b"\xef\xbb\xbf<?xml"),
        _ => true,
    }
}

fn looks_like_sample_epub(b: &[u8]) -> bool {
    let strong = [
        b"newsletter-signup".as_slice(),
        b"newsletter_signup".as_slice(),
        b"reading-sample".as_slice(),
        b"sample chapter".as_slice(),
    ];
    if strong.iter().any(|n| contains_slice(b, n)) {
        return true;
    }
    let weak = [
        b"Begin Reading".as_slice(),
        b"About the Author".as_slice(),
        b"Newsletter Sign-up".as_slice(),
        b"Buy the Book".as_slice(),
    ];
    let hits = weak.iter().filter(|n| contains_slice(b, n)).count();
    hits >= 3
}

fn contains_slice(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len() && haystack.windows(needle.len()).any(|w| w == needle)
}

fn ext_rank(ext: &str) -> u8 {
    match ext {
        "epub" => 0,
        "pdf" => 1,
        "mobi" => 2,
        "azw3" => 3,
        "fb2" => 4,
        "djvu" => 5,
        _ => 99,
    }
}

fn min_book_size(ext: &str) -> i64 {
    match ext {
        "epub" => 220_000,
        "pdf" => 200_000,
        "mobi" | "azw3" => 150_000,
        "djvu" => 500_000,
        "fb2" => 80_000,
        "txt" => 40_000,
        _ => 50_000,
    }
}

fn ascii_filename(title: &str, fallback: &str) -> String {
    let kept: String = title
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'))
        .collect();
    let kept = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    if kept.is_empty() {
        fallback.to_string()
    } else {
        kept
    }
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

fn book_mime(ext: &str) -> &'static str {
    match ext {
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        "mobi" => "application/x-mobipocket-ebook",
        "azw3" => "application/vnd.amazon.ebook",
        "fb2" => "application/x-fictionbook+xml",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn abs_path(media_root: &str, rel: &str) -> std::path::PathBuf {
    std::path::Path::new(media_root).join(rel)
}

async fn proxy_image(url: &str, mime: &'static str) -> axum::response::Response {
    let resp = match client().get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("[books] cover -> {} for {url}", r.status());
            return err(StatusCode::BAD_GATEWAY, "cover unavailable");
        }
        Err(e) => {
            eprintln!("[books] cover fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "cover unavailable");
        }
    };
    match resp.bytes().await {
        Ok(body) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, "public, max-age=604800, immutable"),
            ],
            body,
        )
            .into_response(),
        Err(e) => {
            eprintln!("[books] cover body failed: {e}");
            err(StatusCode::BAD_GATEWAY, "cover unavailable")
        }
    }
}

async fn proxy_cached_image(
    url: &str,
    mime: &'static str,
    cache_path: &std::path::Path,
) -> axum::response::Response {
    if let Ok(body) = tokio::fs::read(cache_path).await {
        if !body.is_empty() {
            return image_response(mime, body.into());
        }
    }

    let resp = match client().get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("[books] portrait -> {} for {url}", r.status());
            return err(StatusCode::BAD_GATEWAY, "portrait unavailable");
        }
        Err(e) => {
            eprintln!("[books] portrait fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "portrait unavailable");
        }
    };
    let body = match resp.bytes().await {
        Ok(body) => body,
        Err(e) => {
            eprintln!("[books] portrait body failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "portrait unavailable");
        }
    };

    if let Some(parent) = cache_path.parent() {
        if tokio::fs::create_dir_all(parent).await.is_ok() {
            let tmp = cache_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            if tokio::fs::write(&tmp, &body).await.is_ok() {
                if let Err(e) = tokio::fs::rename(&tmp, cache_path).await {
                    eprintln!("[books] portrait cache rename failed: {e}");
                    let _ = tokio::fs::remove_file(&tmp).await;
                }
            }
        }
    }

    image_response(mime, body)
}

fn image_response(mime: &'static str, body: bytes::Bytes) -> axum::response::Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "public, max-age=604800, immutable"),
        ],
        body,
    )
        .into_response()
}

fn cache_get(key: &str) -> Option<Vec<BookSource>> {
    let m = LIBGEN_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let g = m.lock().ok()?;
    let (v, t) = g.get(key)?;
    if t.elapsed() < LIBGEN_TTL {
        Some(v.clone())
    } else {
        None
    }
}

fn cache_put(key: String, v: Vec<BookSource>) {
    let m = LIBGEN_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut g) = m.lock() {
        g.insert(key, (v, Instant::now()));
    }
}

async fn ol_search_cached(q: &str) -> reqwest::Result<Vec<(BookHit, Vec<String>)>> {
    let key = q.to_ascii_lowercase();
    let cache = OL_SEARCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some((v, t)) = g.get(&key) {
            if t.elapsed() < OL_SEARCH_TTL {
                return Ok(v.clone());
            }
        }
    }
    let v = ol_search(q).await?;
    if let Ok(mut g) = cache.lock() {
        g.insert(key, (v.clone(), Instant::now()));
    }
    Ok(v)
}

async fn ol_work_cached(key: &str) -> reqwest::Result<Option<BookHit>> {
    let cache = OL_WORK_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some((v, t)) = g.get(key) {
            if t.elapsed() < OL_WORK_TTL {
                return Ok(v.clone());
            }
        }
    }
    let v = ol_work(key).await?;
    if let Ok(mut g) = cache.lock() {
        g.insert(key.to_string(), (v.clone(), Instant::now()));
    }
    Ok(v)
}

async fn ol_work_meta_cached(key: &str) -> reqwest::Result<WorkMeta> {
    let cache = OL_META_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some((v, t)) = g.get(key) {
            if t.elapsed() < OL_ENRICH_TTL {
                return Ok(v.clone());
            }
        }
    }
    let v = ol_work_meta(key).await?;
    if let Ok(mut g) = cache.lock() {
        g.insert(key.to_string(), (v.clone(), Instant::now()));
    }
    Ok(v)
}

async fn ol_ratings_cached(key: &str) -> reqwest::Result<Option<(f64, i64)>> {
    let cache = OL_RATINGS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some((v, t)) = g.get(key) {
            if t.elapsed() < OL_ENRICH_TTL {
                return Ok(*v);
            }
        }
    }
    let v = ol_ratings(key).await?;
    if let Ok(mut g) = cache.lock() {
        g.insert(key.to_string(), (v, Instant::now()));
    }
    Ok(v)
}

async fn ol_editions_cached(key: &str) -> reqwest::Result<EditionBits> {
    let cache = OL_EDITIONS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some((v, t)) = g.get(key) {
            if t.elapsed() < OL_ENRICH_TTL {
                return Ok(v.clone());
            }
        }
    }
    let v = ol_editions(key).await?;
    if let Ok(mut g) = cache.lock() {
        g.insert(key.to_string(), (v.clone(), Instant::now()));
    }
    Ok(v)
}

#[derive(Deserialize)]
struct OlSearch {
    docs: Vec<OlDoc>,
}

#[derive(Deserialize)]
struct OlTrending {
    works: Vec<OlDoc>,
}

#[derive(Deserialize)]
struct OlDoc {
    key: String,
    title: Option<String>,
    author_name: Option<Vec<String>>,
    first_publish_year: Option<i32>,
    cover_i: Option<i64>,
    language: Option<Vec<String>>,
    #[serde(default)]
    subject: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OlWork {
    title: Option<String>,
    description: Option<OlDescription>,
    covers: Option<Vec<i64>>,
    first_publish_date: Option<String>,
    authors: Option<Vec<OlAuthorRef>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OlDescription {
    Plain(String),
    Structured { value: String },
}

impl OlDescription {
    fn into_text(self) -> String {
        match self {
            OlDescription::Plain(s) => s.trim().to_string(),
            OlDescription::Structured { value } => value.trim().to_string(),
        }
    }
}

#[derive(Deserialize)]
struct OlAuthorRef {
    author: OlKeyRef,
}

#[derive(Deserialize)]
struct OlKeyRef {
    key: String,
}

#[derive(Deserialize)]
struct OlAuthor {
    name: Option<String>,
}

#[derive(Deserialize)]
struct OlAuthorFull {
    name: Option<String>,
    bio: Option<OlDescription>,
    birth_date: Option<String>,
    death_date: Option<String>,
    photos: Option<Vec<i64>>,
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_edition_cover_wins_over_audio_and_ebook() {
        let editions = serde_json::json!([
            {
                "covers": [15156250],
                "physical_format": "eAudiobook",
                "publishers": ["Recorded Books"]
            },
            {
                "covers": [14324535, -1],
                "physical_format": "ebook",
                "publishers": ["Tor"]
            },
            {
                "covers": [14538949, 13127133],
                "physical_format": "hardcover",
                "publishers": ["Tordotcom"],
                "number_of_pages": 245
            }
        ]);

        assert_eq!(
            best_edition_cover(editions.as_array().unwrap()),
            Some(14538949)
        );
    }

    #[test]
    fn legacy_book_cover_is_refreshed_once() {
        assert!(book_cover_needs_refresh(Some("")));
        assert!(book_cover_needs_refresh(Some("/api/books/cover/13127133")));
        assert!(!book_cover_needs_refresh(Some(
            "/api/books/cover/14538949.jpg"
        )));
    }
}
