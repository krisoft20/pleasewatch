use crate::db::MangaEnrichment;
use crate::manga_ck;
use crate::manga_mk;
use crate::models::{ApiError, Manga, MangaChapter, MangaHit};
use crate::{anilist, middleware::AuthUser, AppState};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const ENRICH_RETRY: Duration = Duration::from_secs(900);
static ENRICHING: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

const MD_API: &str = "https://api.mangadex.org";
const MD_COVERS: &str = "https://uploads.mangadex.org/covers";
const UA: &str = "pleasewatch/0.1";
const FEED_TTL: Duration = Duration::from_secs(600);
const ATHOME_TTL: Duration = Duration::from_secs(540);

type Cache<K, V> = OnceLock<Mutex<HashMap<K, (V, Instant)>>>;

static FEED_CACHE: Cache<(String, String), ChaptersReport> = OnceLock::new();
static ATHOME_CACHE: Cache<String, AtHome> = OnceLock::new();

fn cache_get<K: Eq + std::hash::Hash, V: Clone>(
    c: &Cache<K, V>,
    k: &K,
    ttl: Duration,
) -> Option<V> {
    let m = c.get_or_init(|| Mutex::new(HashMap::new()));
    let g = m.lock().ok()?;
    let (v, t) = g.get(k)?;
    if t.elapsed() < ttl {
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
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(UA)
            .build()
            .expect("reqwest client")
    })
}

static MK_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn mk_client() -> &'static reqwest::Client {
    MK_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(manga_mk::MK_UA)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("mk client")
    })
}

static CK_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn ck_client() -> &'static reqwest::Client {
    CK_CLIENT.get_or_init(|| {
        let mut b = reqwest::Client::builder()
            .user_agent(manga_ck::CK_UA)
            .timeout(Duration::from_secs(15));
        if let Ok(p) = std::env::var("MANGA_CK_PROXY") {
            if let Ok(px) = reqwest::Proxy::all(&p) {
                b = b.proxy(px);
            }
        }
        b.build().expect("ck client")
    })
}

pub fn routes(state: std::sync::Arc<AppState>) -> Router<std::sync::Arc<AppState>> {
    Router::new()
        .route("/api/manga/search", get(handle_search))
        .route("/api/manga/popular", get(handle_popular))
        .route("/api/manga/discover", get(handle_discover))
        .route("/api/manga", get(handle_list).post(handle_add))
        .route("/api/manga/continue", get(handle_continue))
        .route("/api/manga/progress", post(handle_progress))
        .route(
            "/api/manga/chapter/{chapter_id}/pages",
            get(handle_chapter_pages),
        )
        .route("/api/manga/page/{chapter_id}/{idx}", get(handle_page))
        .route("/api/manga/cover/{md_id}/{file}", get(handle_cover))
        .route(
            "/api/manga/{md_id}",
            get(handle_detail).delete(handle_delete),
        )
        .route("/api/manga/{md_id}/chapters", get(handle_chapters))
        .route("/api/manga/{md_id}/related", get(handle_related))
        .route(
            "/api/manga/{md_id}/recommendations",
            get(handle_recommendations),
        )
        .route("/api/manga/{md_id}/anime", get(handle_anime))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn handle_search(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.trim();
    if query.len() < 2 {
        return Json(Vec::<MangaHit>::new()).into_response();
    }
    match md_search(query).await {
        Ok(hits) => Json(hits).into_response(),
        Err(e) => {
            eprintln!("[manga] search failed: {e}");
            err(StatusCode::BAD_GATEWAY, "mangadex search failed")
        }
    }
}

async fn handle_popular() -> impl IntoResponse {
    match md_popular().await {
        Ok(hits) => Json(hits).into_response(),
        Err(e) => {
            eprintln!("[manga] popular failed: {e}");
            err(StatusCode::BAD_GATEWAY, "mangadex unavailable")
        }
    }
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    mine: bool,
}

async fn handle_list(
    Extension(auth): Extension<AuthUser>,
    State(state): State<std::sync::Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let result = if q.mine {
        db.list_manga_by_user(&auth.id)
    } else {
        db.list_manga()
    };
    match result {
        Ok(list) => Json(list).into_response(),
        Err(e) => {
            eprintln!("[manga] list failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

#[derive(Deserialize)]
struct AddBody {
    md_id: String,
}

async fn handle_add(
    Extension(auth): Extension<AuthUser>,
    State(state): State<std::sync::Arc<AppState>>,
    Json(b): Json<AddBody>,
) -> impl IntoResponse {
    {
        let db = state.db.lock().await;
        match db.find_manga_by_md(&b.md_id) {
            Ok(Some(existing)) => return Json(existing).into_response(),
            Ok(None) => {}
            Err(e) => {
                eprintln!("[manga] add lookup failed: {e}");
                return err(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed");
            }
        }
    }

    let (hit, ex) = match md_manga(&b.md_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return err(StatusCode::NOT_FOUND, "manga not found on mangadex"),
        Err(e) => {
            eprintln!("[manga] add fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "mangadex unavailable");
        }
    };

    let m = Manga {
        id: uuid::Uuid::new_v4().to_string(),
        md_id: hit.md_id,
        title: hit.title,
        description: hit.description,
        cover_url: hit.cover_url,
        year: hit.year,
        status: hit.status,
        added_by: Some(auth.id),
        added_at: chrono::Utc::now().to_rfc3339(),
        restricted: false,
        restricted_langs: None,
        comick_hid: None,
        anilist_id: ex.anilist_id,
        mal_id: ex.mal_id,
        links_json: ex.links_json,
        tags: None,
        demographic: None,
        content_rating: None,
        original_language: None,
        authors: None,
        artists: None,
        score: None,
        score_count: None,
        follow_count: None,
        last_chapter: None,
        enriched_at: None,
    };

    {
        let db = state.db.lock().await;
        if let Err(e) = db.create_manga(&m) {
            eprintln!("[manga] insert failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "insert failed");
        }
    }
    println!("[manga] '{}' added by {}", m.title, auth.username);
    spawn_enrich(&state, &m.md_id);
    Json(m).into_response()
}

async fn handle_detail(
    Extension(auth): Extension<AuthUser>,
    State(state): State<std::sync::Arc<AppState>>,
    Path(md_id): Path<String>,
) -> impl IntoResponse {
    let (in_library, library_row, progress) = {
        let db = state.db.lock().await;
        let row = db.find_manga_by_md(&md_id).unwrap_or(None);
        let progress = db.get_manga_progress(&auth.id, &md_id).unwrap_or(None);
        (row.is_some(), row, progress)
    };

    let (hit, ex) = match md_manga(&md_id).await {
        Ok(Some(p)) => p,
        Ok(None) => match &library_row {
            Some(m) => (
                MangaHit {
                    md_id: m.md_id.clone(),
                    title: m.title.clone(),
                    description: m.description.clone(),
                    cover_url: m.cover_url.clone(),
                    year: m.year,
                    status: m.status.clone(),
                },
                MangaExtras::default(),
            ),
            None => return err(StatusCode::NOT_FOUND, "manga not found"),
        },
        Err(e) => {
            eprintln!("[manga] detail fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "mangadex unavailable");
        }
    };

    let links: HashMap<String, String> = ex
        .links_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let restricted_langs: Vec<String> = library_row
        .as_ref()
        .and_then(|m| m.restricted_langs.as_deref())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let (score, score_count, follow_count, last_chapter) = match library_row.as_ref() {
        Some(m) => (
            m.score,
            m.score_count,
            m.follow_count,
            m.last_chapter.clone(),
        ),
        None => (None, None, None, ex.last_chapter.clone()),
    };

    if in_library
        && library_row
            .as_ref()
            .and_then(|m| m.enriched_at.as_ref())
            .is_none()
    {
        spawn_enrich(&state, &md_id);
    }

    Json(serde_json::json!({
        "manga": hit,
        "in_library": in_library,
        "progress": progress,
        "restricted":       !restricted_langs.is_empty(),
        "restricted_langs": restricted_langs,
        "available_langs":  ex.available_langs,
        "links":            links,
        "anilist_id":       ex.anilist_id,
        "mal_id":           ex.mal_id,
        "tags":             ex.tags,
        "demographic":      ex.demographic,
        "content_rating":   ex.content_rating,
        "authors":          ex.authors,
        "artists":          ex.artists,
        "score":            score,
        "score_count":      score_count,
        "follow_count":     follow_count,
        "last_chapter":     last_chapter,
    }))
    .into_response()
}

async fn handle_delete(
    Extension(auth): Extension<AuthUser>,
    State(state): State<std::sync::Arc<AppState>>,
    Path(md_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let m = match db.find_manga_by_md(&md_id) {
        Ok(Some(m)) => m,
        Ok(None) => return err(StatusCode::NOT_FOUND, "not in library"),
        Err(e) => {
            eprintln!("[manga] delete lookup failed: {e}");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "lookup failed");
        }
    };
    if !auth.is_admin() && m.added_by.as_deref() != Some(auth.id.as_str()) {
        return err(StatusCode::FORBIDDEN, "not yours");
    }
    if let Err(e) = db.delete_manga_by_md(&md_id) {
        eprintln!("[manga] delete failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "delete failed");
    }
    println!("[manga] '{}' removed by {}", m.title, auth.username);
    Json(serde_json::json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct ChaptersQuery {
    #[serde(default)]
    lang: Option<String>,
}

async fn handle_chapters(
    State(state): State<std::sync::Arc<AppState>>,
    Path(md_id): Path<String>,
    Query(q): Query<ChaptersQuery>,
) -> impl IntoResponse {
    let want_lang = q.lang.as_deref().unwrap_or("en").to_string();
    let cache_key = (md_id.clone(), want_lang.clone());

    if let Some(hit) = cache_get(&FEED_CACHE, &cache_key, FEED_TTL) {
        return chapters_response(&state, &md_id, hit).await;
    }

    let langs: Vec<&str> = if want_lang == "pl" {
        vec!["pl", "en"]
    } else {
        vec![want_lang.as_str(), "pl"]
    };

    let mut report = match md_chapters(&md_id, &langs).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[manga] chapter feed failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "mangadex unavailable");
        }
    };

    let restricted_for_want = report.restricted_langs.iter().any(|l| l == &want_lang);
    let has_want = report.chapters.iter().any(|c| c.lang == want_lang);
    if restricted_for_want && !has_want {
        match comick_fallback_chapters(&state, &md_id, &want_lang).await {
            Ok(extra) => {
                if !extra.is_empty() {
                    println!(
                        "[manga] {md_id}: comick filled {} chapters for {want_lang}",
                        extra.len()
                    );
                    report.chapters.extend(extra);
                }
            }
            Err(e) => eprintln!("[manga] comick fallback failed: {e}"),
        }
    }

    let still_empty = restricted_for_want && !report.chapters.iter().any(|c| c.lang == want_lang);
    if still_empty && want_lang == "en" {
        match mk_fallback_chapters(&state, &md_id).await {
            Ok(extra) => {
                if !extra.is_empty() {
                    println!(
                        "[manga] {md_id}: mangakatana filled {} chapters",
                        extra.len()
                    );
                    report.chapters.extend(extra);
                }
            }
            Err(e) => eprintln!("[manga] mangakatana fallback failed: {e}"),
        }
    }

    persist_restricted(&state, &md_id, &report).await;

    if !report.chapters.is_empty() {
        cache_put(&FEED_CACHE, cache_key, report.clone());
    }
    chapters_response(&state, &md_id, report).await
}

async fn chapters_response(
    _state: &std::sync::Arc<AppState>,
    _md_id: &str,
    report: ChaptersReport,
) -> axum::response::Response {
    Json(report.chapters).into_response()
}

async fn persist_restricted(
    state: &std::sync::Arc<AppState>,
    md_id: &str,
    report: &ChaptersReport,
) {
    let langs_json = serde_json::to_string(&report.restricted_langs).ok();
    let restricted = !report.restricted_langs.is_empty();
    let db = state.db.lock().await;
    let exists = matches!(db.find_manga_by_md(md_id), Ok(Some(_)));
    if !exists {
        return;
    }
    if let Err(e) = db.update_manga_restricted(md_id, restricted, langs_json.as_deref()) {
        eprintln!("[manga] persist restricted failed: {e}");
    }
}

async fn mk_fallback_chapters(
    state: &std::sync::Arc<AppState>,
    md_id: &str,
) -> reqwest::Result<Vec<MangaChapter>> {
    let row_title_year = {
        let db = state.db.lock().await;
        db.find_manga_by_md(md_id)
            .ok()
            .flatten()
            .map(|m| (m.title, m.year))
    };
    let (title, year) = match row_title_year {
        Some(p) => p,
        None => match md_manga(md_id).await? {
            Some((h, _)) => (h.title, h.year),
            None => return Ok(Vec::new()),
        },
    };

    let r = match manga_mk::resolve_manga(&title, year).await? {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    manga_mk::chapters(&r).await
}

async fn comick_fallback_chapters(
    state: &std::sync::Arc<AppState>,
    md_id: &str,
    lang: &str,
) -> reqwest::Result<Vec<MangaChapter>> {
    let row = state
        .db
        .lock()
        .await
        .find_manga_by_md(md_id)
        .unwrap_or(None);

    let (hid_cached, title_year) = match row {
        Some(m) => (m.comick_hid.clone(), Some((m.title.clone(), m.year))),
        None => (None, None),
    };

    let hid = match hid_cached {
        Some(h) => h,
        None => {
            let (title, year) = match title_year {
                Some(p) => p,
                None => match md_manga(md_id).await? {
                    Some((h, _)) => (h.title, h.year),
                    None => return Ok(Vec::new()),
                },
            };
            match manga_ck::resolve_hid(&title, year).await? {
                Some(h) => {
                    let db = state.db.lock().await;
                    if let Err(e) = db.update_manga_comick_hid(md_id, &h) {
                        eprintln!("[manga] cache comick hid failed: {e}");
                    }
                    h
                }
                None => return Ok(Vec::new()),
            }
        }
    };

    manga_ck::chapters(&hid, lang).await
}

async fn handle_chapter_pages(Path(chapter_id): Path<String>) -> impl IntoResponse {
    if let Some(hid) = chapter_id.strip_prefix("ck:") {
        return match manga_ck::pages(hid).await {
            Ok(urls) => {
                let pages: Vec<String> = (0..urls.len())
                    .map(|i| format!("/api/manga/page/{chapter_id}/{i}"))
                    .collect();
                Json(serde_json::json!({ "pages": pages })).into_response()
            }
            Err(e) => {
                eprintln!("[manga] comick pages failed for {hid}: {e}");
                err(StatusCode::BAD_GATEWAY, "chapter unavailable")
            }
        };
    }
    if chapter_id.starts_with("mk:") {
        return match manga_mk::pages(&chapter_id).await {
            Ok(urls) => {
                let pages: Vec<String> = (0..urls.len())
                    .map(|i| format!("/api/manga/page/{chapter_id}/{i}"))
                    .collect();
                Json(serde_json::json!({ "pages": pages })).into_response()
            }
            Err(e) => {
                eprintln!("[manga] mangakatana pages failed for {chapter_id}: {e}");
                err(StatusCode::BAD_GATEWAY, "chapter unavailable")
            }
        };
    }

    let ah = match at_home(&chapter_id).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[manga] at-home failed for {chapter_id}: {e}");
            return err(StatusCode::BAD_GATEWAY, "chapter unavailable");
        }
    };
    let pages: Vec<String> = (0..ah.chapter.data.len())
        .map(|i| format!("/api/manga/page/{chapter_id}/{i}"))
        .collect();
    Json(serde_json::json!({ "pages": pages })).into_response()
}

async fn handle_page(Path((chapter_id, idx)): Path<(String, usize)>) -> impl IntoResponse {
    if let Some(hid) = chapter_id.strip_prefix("ck:") {
        return match manga_ck::pages(hid).await {
            Ok(urls) => match urls.get(idx) {
                Some(url) => proxy_ck_image(url).await,
                None => err(StatusCode::NOT_FOUND, "no such page"),
            },
            Err(e) => {
                eprintln!("[manga] comick pages failed for {hid}: {e}");
                err(StatusCode::BAD_GATEWAY, "chapter unavailable")
            }
        };
    }
    if chapter_id.starts_with("mk:") {
        return match manga_mk::pages(&chapter_id).await {
            Ok(urls) => match urls.get(idx) {
                Some(url) => proxy_mk_image(url).await,
                None => err(StatusCode::NOT_FOUND, "no such page"),
            },
            Err(e) => {
                eprintln!("[manga] mangakatana pages failed for {chapter_id}: {e}");
                err(StatusCode::BAD_GATEWAY, "chapter unavailable")
            }
        };
    }

    let ah = match at_home(&chapter_id).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[manga] at-home failed for {chapter_id}: {e}");
            return err(StatusCode::BAD_GATEWAY, "chapter unavailable");
        }
    };
    let Some(file) = ah.chapter.data.get(idx) else {
        return err(StatusCode::NOT_FOUND, "no such page");
    };

    let url = format!("{}/data/{}/{}", ah.base_url, ah.chapter.hash, file);
    proxy_image(&url, ext_mime(file)).await
}

async fn handle_cover(Path((md_id, file)): Path<(String, String)>) -> impl IntoResponse {
    let url = format!("{MD_COVERS}/{md_id}/{file}");
    proxy_image(&url, ext_mime(&file)).await
}

fn ext_mime(file: &str) -> &'static str {
    match file.rsplit('.').next() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

async fn proxy_image(url: &str, mime: &'static str) -> axum::response::Response {
    let resp = match client().get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("[manga] image fetch -> {} for {url}", r.status());
            return err(StatusCode::BAD_GATEWAY, "image fetch failed");
        }
        Err(e) => {
            eprintln!("[manga] image fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "image fetch failed");
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
            eprintln!("[manga] image body failed: {e}");
            err(StatusCode::BAD_GATEWAY, "image fetch failed")
        }
    }
}

async fn proxy_mk_image(url: &str) -> axum::response::Response {
    let mime = ext_mime(url);
    let resp = match mk_client().get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("[manga] mk image fetch -> {} for {url}", r.status());
            return err(StatusCode::BAD_GATEWAY, "image fetch failed");
        }
        Err(e) => {
            eprintln!("[manga] mk image fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "image fetch failed");
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
            eprintln!("[manga] mk image body failed: {e}");
            err(StatusCode::BAD_GATEWAY, "image fetch failed")
        }
    }
}

async fn proxy_ck_image(url: &str) -> axum::response::Response {
    let mime = ext_mime(url);
    let resp = match ck_client()
        .get(url)
        .header("Referer", manga_ck::CK_REF)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("[manga] ck image fetch -> {} for {url}", r.status());
            return err(StatusCode::BAD_GATEWAY, "image fetch failed");
        }
        Err(e) => {
            eprintln!("[manga] ck image fetch failed: {e}");
            return err(StatusCode::BAD_GATEWAY, "image fetch failed");
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
            eprintln!("[manga] ck image body failed: {e}");
            err(StatusCode::BAD_GATEWAY, "image fetch failed")
        }
    }
}

#[derive(Deserialize)]
struct ProgressBody {
    md_id: String,
    chapter_id: String,
    chapter: Option<String>,
    page: i64,
    pages: i64,
}

async fn handle_progress(
    Extension(auth): Extension<AuthUser>,
    State(state): State<std::sync::Arc<AppState>>,
    Json(b): Json<ProgressBody>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    if let Err(e) = db.upsert_manga_progress(
        &auth.id,
        &b.md_id,
        &b.chapter_id,
        b.chapter.as_deref(),
        b.page,
        b.pages,
    ) {
        eprintln!("[manga] progress save failed: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "save failed");
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_continue(
    Extension(auth): Extension<AuthUser>,
    State(state): State<std::sync::Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.list_manga_continue(&auth.id, 12) {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            eprintln!("[manga] continue list failed: {e}");
            err(StatusCode::INTERNAL_SERVER_ERROR, "list failed")
        }
    }
}

async fn md_search(query: &str) -> reqwest::Result<Vec<MangaHit>> {
    let r: MdList = client()
        .get(format!("{MD_API}/manga"))
        .query(&[
            ("title", query),
            ("limit", "24"),
            ("includes[]", "cover_art"),
            ("order[relevance]", "desc"),
            ("hasAvailableChapters", "true"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.data.iter().map(map_manga_hit).collect())
}

async fn md_popular() -> reqwest::Result<Vec<MangaHit>> {
    let r: MdList = client()
        .get(format!("{MD_API}/manga"))
        .query(&[
            ("limit", "24"),
            ("includes[]", "cover_art"),
            ("order[followedCount]", "desc"),
            ("hasAvailableChapters", "true"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.data.iter().map(map_manga_hit).collect())
}

async fn md_manga(md_id: &str) -> reqwest::Result<Option<(MangaHit, MangaExtras)>> {
    let r = client()
        .get(format!("{MD_API}/manga/{md_id}"))
        .query(&[
            ("includes[]", "cover_art"),
            ("includes[]", "author"),
            ("includes[]", "artist"),
        ])
        .send()
        .await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let e: MdEntity = r.error_for_status()?.json().await?;
    let hit = map_manga_hit(&e.data);
    let ex = extract_extras(&e.data);
    Ok(Some((hit, ex)))
}

fn enrich_claims() -> &'static Mutex<HashMap<String, Instant>> {
    ENRICHING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn spawn_enrich(state: &Arc<AppState>, md_id: &str) {
    {
        let mut m = enrich_claims().lock().unwrap();
        if m.get(md_id).is_some_and(|t| t.elapsed() < ENRICH_RETRY) {
            return;
        }
        m.insert(md_id.to_string(), Instant::now());
    }
    tokio::spawn(enrich_manga(state.clone(), md_id.to_string()));
}

async fn enrich_manga(state: Arc<AppState>, md_id: String) {
    let row = {
        let db = state.db.lock().await;
        db.find_manga_by_md(&md_id).ok().flatten()
    };
    let Some(row) = row else {
        enrich_claims().lock().unwrap().remove(&md_id);
        return;
    };
    if row.enriched_at.is_some() {
        enrich_claims().lock().unwrap().remove(&md_id);
        return;
    }
    println!("[manga] enriching '{}'", row.title);

    let extras = match md_manga(&md_id).await {
        Ok(Some((_, ex))) => Some(ex),
        Ok(None) => None,
        Err(e) => {
            eprintln!("[manga] enrich detail failed: {e}");
            None
        }
    };

    let stats = match md_statistics(&md_id).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[manga] enrich stats failed: {e}");
            None
        }
    };

    let mut anilist_id = row
        .anilist_id
        .or_else(|| extras.as_ref().and_then(|e| e.anilist_id));
    let mut mal_id = row
        .mal_id
        .or_else(|| extras.as_ref().and_then(|e| e.mal_id));

    if anilist_id.is_none() {
        if let Some(ex) = extras.as_ref() {
            anilist_id = ex.anilist_id;
            mal_id = mal_id.or(ex.mal_id);
        }
    }

    let tags_json = extras.as_ref().and_then(|e| {
        if e.tags.is_empty() {
            None
        } else {
            serde_json::to_string(&e.tags).ok()
        }
    });
    let authors_json = extras.as_ref().and_then(|e| {
        if e.authors.is_empty() {
            None
        } else {
            serde_json::to_string(&e.authors).ok()
        }
    });
    let artists_json = extras.as_ref().and_then(|e| {
        if e.artists.is_empty() {
            None
        } else {
            serde_json::to_string(&e.artists).ok()
        }
    });

    let (score, score_count) = match stats.as_ref().and_then(|s| s.rating.as_ref()) {
        Some(r) => {
            let count: i64 = r.distribution.values().sum();
            (r.bayesian, if count > 0 { Some(count) } else { None })
        }
        None => (None, None),
    };
    let follows = stats.as_ref().and_then(|s| s.follows);

    let demographic = extras.as_ref().and_then(|e| e.demographic.clone());
    let content_rating = extras.as_ref().and_then(|e| e.content_rating.clone());
    let original_language = extras.as_ref().and_then(|e| e.original_language.clone());
    let last_chapter = extras.as_ref().and_then(|e| e.last_chapter.clone());
    let links_json = extras.as_ref().and_then(|e| e.links_json.clone());

    let e = MangaEnrichment {
        tags: tags_json.as_deref(),
        demographic: demographic.as_deref(),
        content_rating: content_rating.as_deref(),
        original_language: original_language.as_deref(),
        authors: authors_json.as_deref(),
        artists: artists_json.as_deref(),
        score,
        score_count,
        follow_count: follows,
        last_chapter: last_chapter.as_deref(),
        anilist_id,
        mal_id,
        links_json: links_json.as_deref(),
    };

    let got = [
        e.tags.is_some(),
        e.demographic.is_some(),
        e.authors.is_some(),
        e.score.is_some(),
        e.follow_count.is_some(),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    if got == 0 {
        eprintln!("[manga] enrich got nothing for '{}'", row.title);
        return;
    }

    {
        let db = state.db.lock().await;
        if let Err(err) = db.update_manga_enrichment(&md_id, &e) {
            eprintln!("[manga] enrich save failed: {err}");
            return;
        }
    }

    enrich_claims().lock().unwrap().remove(&md_id);
    println!("[manga] enrich done: {got} fields for '{}'", row.title);

    if let Some(al) = anilist_id {
        tokio::spawn(async move {
            if let Err(err) = anilist::fetch(al).await {
                eprintln!("[manga] anilist warm failed for {al}: {err}");
            }
        });
    }
}

async fn md_statistics(md_id: &str) -> reqwest::Result<Option<MdStats>> {
    let r: MdStatsResp = client()
        .get(format!("{MD_API}/statistics/manga/{md_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(r.statistics.into_values().next())
}

#[derive(Deserialize)]
struct MdStatsResp {
    #[serde(default)]
    statistics: HashMap<String, MdStats>,
}

#[derive(Deserialize, Clone)]
pub struct MdStats {
    #[serde(default)]
    pub follows: Option<i64>,
    #[serde(default)]
    pub rating: Option<MdRating>,
}

#[derive(Deserialize, Clone)]
pub struct MdRating {
    #[serde(default)]
    pub bayesian: Option<f64>,
    #[serde(default)]
    pub distribution: HashMap<String, i64>,
}

#[derive(Default, Clone)]
pub struct ChaptersReport {
    pub chapters: Vec<MangaChapter>,
    pub restricted_langs: Vec<String>,
}

async fn md_chapters(md_id: &str, langs: &[&str]) -> reqwest::Result<ChaptersReport> {
    let mut out: Vec<MangaChapter> = Vec::new();
    let mut seen = HashSet::new();
    let mut offset: i64 = 0;
    let mut tally: HashMap<String, (i64, i64)> = HashMap::new();

    loop {
        let mut params: Vec<(&str, String)> = vec![
            ("limit", "500".to_string()),
            ("offset", offset.to_string()),
            ("order[chapter]", "asc".to_string()),
        ];
        for l in langs {
            params.push(("translatedLanguage[]", (*l).to_string()));
        }

        let r: MdFeed = client()
            .get(format!("{MD_API}/manga/{md_id}/feed"))
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let n = r.data.len() as i64;
        for c in r.data {
            let lang = c.attributes.translated_language.clone();
            let entry = tally.entry(lang.clone()).or_insert((0, 0));

            let external = c.attributes.external_url.is_some() || c.attributes.pages == 0;
            if external {
                entry.1 += 1;
                continue;
            }
            entry.0 += 1;

            let key = (
                c.attributes.chapter.clone().unwrap_or_default(),
                lang.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            out.push(MangaChapter {
                id: c.id,
                chapter: c.attributes.chapter,
                title: c.attributes.title,
                volume: c.attributes.volume,
                lang,
                pages: c.attributes.pages,
                published_at: c.attributes.publish_at,
            });
        }

        offset += n;
        if n == 0 || offset >= r.total || offset >= 4000 {
            break;
        }
    }

    let restricted_langs: Vec<String> = tally
        .into_iter()
        .filter_map(|(l, (kept, dropped))| {
            if kept == 0 && dropped > 0 {
                Some(l)
            } else {
                None
            }
        })
        .collect();

    Ok(ChaptersReport {
        chapters: out,
        restricted_langs,
    })
}

async fn at_home(chapter_id: &str) -> reqwest::Result<AtHome> {
    let key = chapter_id.to_string();
    if let Some(hit) = cache_get(&ATHOME_CACHE, &key, ATHOME_TTL) {
        return Ok(hit);
    }
    let a: AtHome = client()
        .get(format!("{MD_API}/at-home/server/{chapter_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    cache_put(&ATHOME_CACHE, key, a.clone());
    Ok(a)
}

fn map_manga_hit(m: &MdManga) -> MangaHit {
    let a = &m.attributes;
    let title = a
        .title
        .get("en")
        .or_else(|| a.alt_titles.iter().find_map(|t| t.get("en")))
        .or_else(|| a.title.values().next())
        .cloned()
        .unwrap_or_else(|| "untitled".to_string());

    let description = a
        .description
        .get("en")
        .or_else(|| a.description.get("pl"))
        .or_else(|| a.description.values().next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let cover_url = m
        .relationships
        .iter()
        .find(|r| r.kind == "cover_art")
        .and_then(|r| r.attributes.as_ref())
        .and_then(|a| a.file_name.clone())
        .map(|f| format!("/api/manga/cover/{}/{}.512.jpg", m.id, f));

    MangaHit {
        md_id: m.id.clone(),
        title,
        description,
        cover_url,
        year: a.year,
        status: a.status.clone(),
    }
}

#[derive(Clone, Default)]
pub struct MangaExtras {
    pub available_langs: Vec<String>,
    pub links_json: Option<String>,
    pub anilist_id: Option<i64>,
    pub mal_id: Option<i64>,
    pub tags: Vec<String>,
    pub demographic: Option<String>,
    pub content_rating: Option<String>,
    pub original_language: Option<String>,
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    pub last_chapter: Option<String>,
}

fn extract_extras(m: &MdManga) -> MangaExtras {
    let a = &m.attributes;
    let available_langs: Vec<String> = a
        .available_translated_languages
        .iter()
        .filter_map(|x| x.clone())
        .collect();

    let links_json = if a.links.is_empty() {
        None
    } else {
        serde_json::to_string(&a.links).ok()
    };
    let anilist_id = a.links.get("al").and_then(|s| s.parse().ok());
    let mal_id = a.links.get("mal").and_then(|s| s.parse().ok());

    let tags: Vec<String> = a
        .tags
        .iter()
        .filter_map(|t| t.attributes.name.get("en").cloned())
        .collect();

    let authors = relationship_names(m, "author");
    let artists = relationship_names(m, "artist");

    MangaExtras {
        available_langs,
        links_json,
        anilist_id,
        mal_id,
        tags,
        demographic: a.publication_demographic.clone(),
        content_rating: a.content_rating.clone(),
        original_language: a.original_language.clone(),
        authors,
        artists,
        last_chapter: a.last_chapter.clone().filter(|s| !s.is_empty()),
    }
}

fn relationship_names(m: &MdManga, kind: &str) -> Vec<String> {
    let mut out: Vec<String> = m
        .relationships
        .iter()
        .filter(|r| r.kind == kind)
        .filter_map(|r| r.attributes.as_ref())
        .filter_map(|a| a.name.clone())
        .filter(|s| !s.trim().is_empty())
        .collect();
    out.dedup();
    out
}

#[derive(Deserialize)]
struct MdList {
    data: Vec<MdManga>,
}

#[derive(Deserialize)]
struct MdEntity {
    data: MdManga,
}

#[derive(Deserialize)]
struct MdManga {
    id: String,
    attributes: MdMangaAttrs,
    #[serde(default)]
    relationships: Vec<MdRel>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MdMangaAttrs {
    #[serde(default)]
    title: HashMap<String, String>,
    #[serde(default)]
    alt_titles: Vec<HashMap<String, String>>,
    #[serde(default)]
    description: HashMap<String, String>,
    year: Option<i32>,
    status: Option<String>,
    #[serde(default)]
    links: HashMap<String, String>,
    #[serde(default)]
    available_translated_languages: Vec<Option<String>>,
    #[serde(default)]
    tags: Vec<MdTag>,
    publication_demographic: Option<String>,
    content_rating: Option<String>,
    original_language: Option<String>,
    last_chapter: Option<String>,
}

#[derive(Deserialize)]
struct MdTag {
    attributes: MdTagAttrs,
}

#[derive(Deserialize)]
struct MdTagAttrs {
    #[serde(default)]
    name: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MdRel {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    attributes: Option<MdRelAttrs>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MdRelAttrs {
    file_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct MdFeed {
    data: Vec<MdChapter>,
    #[serde(default)]
    total: i64,
}

#[derive(Deserialize)]
struct MdChapter {
    id: String,
    attributes: MdChapterAttrs,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MdChapterAttrs {
    volume: Option<String>,
    chapter: Option<String>,
    title: Option<String>,
    translated_language: String,
    #[serde(default)]
    pages: i64,
    publish_at: Option<String>,
    external_url: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AtHome {
    base_url: String,
    chapter: AtHomeChapter,
}

#[derive(Deserialize, Clone)]
struct AtHomeChapter {
    hash: String,
    data: Vec<String>,
}

#[derive(Deserialize)]
struct DiscoverQuery {
    kind: Option<String>,
}

async fn handle_discover(Query(q): Query<DiscoverQuery>) -> impl IntoResponse {
    let kind = q.kind.as_deref().unwrap_or("latest");
    match md_discover(kind).await {
        Ok(hits) => Json(hits).into_response(),
        Err(e) => {
            eprintln!("[manga] discover '{kind}' failed: {e}");
            err(StatusCode::BAD_GATEWAY, "mangadex unavailable")
        }
    }
}

static DISCOVER_CACHE: Cache<String, Vec<MangaHit>> = OnceLock::new();
const DISCOVER_TTL: Duration = Duration::from_secs(900);

async fn md_discover(kind: &str) -> reqwest::Result<Vec<MangaHit>> {
    if let Some(hit) = cache_get(&DISCOVER_CACHE, &kind.to_string(), DISCOVER_TTL) {
        return Ok(hit);
    }
    let order_param = match kind {
        "popular" => ("order[followedCount]", "desc"),
        "toprated" => ("order[rating]", "desc"),
        "new" => ("order[createdAt]", "desc"),
        _ => ("order[latestUploadedChapter]", "desc"),
    };
    let r: MdList = client()
        .get(format!("{MD_API}/manga"))
        .query(&[
            ("limit", "24"),
            ("includes[]", "cover_art"),
            ("hasAvailableChapters", "true"),
            order_param,
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let hits: Vec<MangaHit> = r.data.iter().map(map_manga_hit).collect();
    cache_put(&DISCOVER_CACHE, kind.to_string(), hits.clone());
    Ok(hits)
}

async fn handle_related(
    State(state): State<std::sync::Arc<AppState>>,
    Path(md_id): Path<String>,
) -> impl IntoResponse {
    match anilist_bundle(&state, &md_id).await {
        Ok(Some(b)) => Json(b.relations).into_response(),
        Ok(None) => Json(Vec::<anilist::RelatedWork>::new()).into_response(),
        Err(e) => {
            eprintln!("[manga] related failed for {md_id}: {e}");
            err(StatusCode::BAD_GATEWAY, "anilist unavailable")
        }
    }
}

async fn handle_recommendations(
    State(state): State<std::sync::Arc<AppState>>,
    Path(md_id): Path<String>,
) -> impl IntoResponse {
    match anilist_bundle(&state, &md_id).await {
        Ok(Some(b)) => Json(b.recommendations).into_response(),
        Ok(None) => Json(Vec::<anilist::Recommended>::new()).into_response(),
        Err(e) => {
            eprintln!("[manga] recommendations failed for {md_id}: {e}");
            err(StatusCode::BAD_GATEWAY, "anilist unavailable")
        }
    }
}

async fn handle_anime(
    State(state): State<std::sync::Arc<AppState>>,
    Path(md_id): Path<String>,
) -> impl IntoResponse {
    let adaptation = match anilist_bundle(&state, &md_id).await {
        Ok(Some(b)) => b.anime_adaptation,
        Ok(None) => None,
        Err(e) => {
            eprintln!("[manga] anime resolve failed for {md_id}: {e}");
            return err(StatusCode::BAD_GATEWAY, "anilist unavailable");
        }
    };
    let Some(ad) = adaptation else {
        return Json(serde_json::json!(null)).into_response();
    };

    let key = state.tmdb_key().await;
    if key.is_empty() {
        return Json(serde_json::json!({
            "anilist_id": ad.anilist_id,
            "title":      ad.title,
            "cover_url":  ad.cover_url,
            "format":     ad.format,
            "tmdb":       null,
        }))
        .into_response();
    }

    let tmdb_match = match crate::tmdb::search(&key, &ad.title).await {
        Ok(items) => items
            .into_iter()
            .find(|it| it.media_type == "tv" || it.media_type == "movie"),
        Err(e) => {
            eprintln!("[manga] tmdb search '{}' failed: {e}", ad.title);
            None
        }
    };

    Json(serde_json::json!({
        "anilist_id": ad.anilist_id,
        "title":      ad.title,
        "cover_url":  ad.cover_url,
        "format":     ad.format,
        "tmdb":       tmdb_match,
    }))
    .into_response()
}

async fn anilist_bundle(
    state: &std::sync::Arc<AppState>,
    md_id: &str,
) -> reqwest::Result<Option<anilist::Bundle>> {
    let row_al = {
        let db = state.db.lock().await;
        db.find_manga_by_md(md_id)
            .ok()
            .flatten()
            .and_then(|m| m.anilist_id)
    };
    let al = match row_al {
        Some(a) => Some(a),
        None => match md_manga(md_id).await? {
            Some((_, ex)) => ex.anilist_id,
            None => None,
        },
    };
    let Some(al) = al else { return Ok(None) };
    anilist::fetch(al).await
}

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(ApiError { error: msg.into() })).into_response()
}
