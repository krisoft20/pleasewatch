use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Pending,
    Admin,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Pending => "pending",
            Role::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
    pub approved_at: Option<String>,
    pub approved_by: Option<String>,
}

impl User {
    pub fn is_pending(&self) -> bool {
        self.role == Role::Pending.as_str()
    }

    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin.as_str()
    }
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            role: u.role,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: UserPublic,
}

#[derive(Debug, Serialize)]
pub struct ApiMessage {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub message: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    pub id: String,
    pub tmdb_id: Option<i64>,
    pub media_type: String,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub file_path: Option<String>,
    pub duration: Option<i64>,
    pub status: String,
    pub added_by: Option<String>,
    pub added_at: String,
    #[serde(default)]
    pub activity_at: Option<String>,
    #[serde(default)]
    pub activity_label: Option<String>,
    #[serde(default)]
    pub is_anime: bool,
    #[serde(default)]
    pub source_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manga {
    pub id: String,
    pub md_id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<i32>,
    pub status: Option<String>,
    pub added_by: Option<String>,
    pub added_at: String,
    pub restricted: bool,
    pub restricted_langs: Option<String>,
    pub comick_hid: Option<String>,
    pub anilist_id: Option<i64>,
    pub mal_id: Option<i64>,
    pub links_json: Option<String>,
    pub tags: Option<String>,
    pub demographic: Option<String>,
    pub content_rating: Option<String>,
    pub original_language: Option<String>,
    pub authors: Option<String>,
    pub artists: Option<String>,
    pub score: Option<f64>,
    pub score_count: Option<i64>,
    pub follow_count: Option<i64>,
    pub last_chapter: Option<String>,
    pub enriched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaHit {
    pub md_id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaChapter {
    pub id: String,
    pub chapter: Option<String>,
    pub title: Option<String>,
    pub volume: Option<String>,
    pub lang: String,
    pub pages: i64,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaProgress {
    pub md_id: String,
    pub chapter_id: String,
    pub chapter: Option<String>,
    pub page: i64,
    pub pages: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaContinueItem {
    pub md_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub chapter_id: String,
    pub chapter: Option<String>,
    pub page: i64,
    pub pages: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String,
    pub ol_key: String,
    pub title: String,
    pub authors: Option<String>,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<i32>,
    pub language: Option<String>,
    pub file_path: Option<String>,
    pub ext: Option<String>,
    pub status: String,
    pub added_by: Option<String>,
    pub added_at: String,
    pub pages: Option<i64>,
    pub subjects: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub rating: Option<f64>,
    pub rating_count: Option<i64>,
    pub enriched_at: Option<String>,
    pub series: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookHit {
    pub ol_key: String,
    pub title: String,
    pub authors: Option<String>,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<i32>,
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub author_keys: Vec<String>,
    #[serde(default)]
    pub in_library: bool,
    #[serde(default)]
    pub ready: bool,
    #[serde(default = "default_hit_kind", skip_serializing_if = "is_book_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series_covers: Vec<String>,
}

fn default_hit_kind() -> String {
    "book".to_string()
}
fn is_book_kind(s: &str) -> bool {
    s == "book"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSource {
    pub md5: String,
    pub title: String,
    pub authors: Option<String>,
    pub publisher: Option<String>,
    pub ext: String,
    pub language: Option<String>,
    pub size: Option<i64>,
    pub year: Option<i32>,
    pub pages: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookProgress {
    pub ol_key: String,
    pub cfi: Option<String>,
    pub percent: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookContinueItem {
    pub ol_key: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub cfi: Option<String>,
    pub percent: f64,
    pub updated_at: String,
    pub authors: Option<String>,
    pub pages: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookShelfItem {
    pub ol_key: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub authors: Option<String>,
    pub pages: Option<i64>,
    pub subjects: Option<String>,
    pub status: String,
    pub finished_at: Option<String>,
    pub percent: Option<f64>,
    pub showcased: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionItem {
    pub id: String,
    pub tmdb_id: i64,
    pub kind: String,
    pub title: String,
    pub year: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub status: String,
    pub showcased: bool,
    pub added_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMark {
    pub id: String,
    pub ol_key: String,
    pub kind: String,
    pub cfi: String,
    pub color: Option<String>,
    pub note: Option<String>,
    pub snippet: Option<String>,
    pub chapter: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub media_id: String,
    pub season: i32,
    pub episode: i32,
    pub title: Option<String>,
    pub file_path: Option<String>,
    pub duration: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub intro_start: Option<i64>,
    #[serde(default)]
    pub intro_end: Option<i64>,
    #[serde(default)]
    pub credits_start: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtitle {
    pub id: String,
    pub owner_id: String,
    pub language: String,
    pub label: String,
    pub format: String,
    pub file_path: String,
    pub is_default: bool,
    pub media_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchProgress {
    pub id: String,
    pub user_id: String,
    pub media_id: String,
    pub episode_id: Option<String>,
    pub position: i64,
    pub duration: i64,
    pub completed: bool,
    pub dismissed: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub media_id: String,
    pub episode_id: Option<String>,
    pub start_sec: f64,
    pub end_sec: f64,
    pub subtitle_id: Option<String>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContinueItem {
    pub media_id: String,
    pub media_title: String,
    pub media_type: String,
    pub is_anime: bool,
    pub tmdb_id: Option<i64>,
    pub poster_url: Option<String>,
    pub episode_id: Option<String>,
    pub episode_season: Option<i32>,
    pub episode_number: Option<i32>,
    pub episode_title: Option<String>,
    pub position: i64,
    pub duration: i64,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_still_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressSummary {
    pub media_id: String,
    pub position: i64,
    pub duration: i64,
}

#[derive(Debug, Deserialize)]
pub struct AddMediaRequest {
    pub tmdb_id: i64,
    pub media_type: String,
}

#[derive(Debug, Serialize)]
pub struct MediaWithEpisodes {
    #[serde(flatten)]
    pub media: Media,
    pub episodes: Vec<Episode>,
    pub subs_processing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbSearchItem {
    pub tmdb_id: i64,
    pub media_type: String,
    pub title: String,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub vote_average: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbGenre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverGenresResponse {
    pub movie: Vec<TmdbGenre>,
    pub tv: Vec<TmdbGenre>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbDetail {
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub media_type: String,
    pub title: String,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub vote_average: Option<f64>,
    pub runtime: Option<i32>,
    pub genres: Vec<String>,
    pub is_anime: bool,
    pub seasons: Option<Vec<TmdbSeason>>,
    pub cast: Vec<TmdbCastMember>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omdb_seasons: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belongs_to_collection: Option<TmdbCollectionRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbCollectionRef {
    pub id: i64,
    pub name: String,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbCollection {
    pub id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub parts: Vec<TmdbCollectionPart>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbCollectionPart {
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<String>,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub vote_average: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbCastMember {
    pub id: i64,
    pub name: String,
    pub character: String,
    pub photo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverResponse {
    pub trending: Vec<TmdbSearchItem>,
    pub popular_movies: Vec<TmdbSearchItem>,
    pub popular_tv: Vec<TmdbSearchItem>,
    pub top_rated_movies: Vec<TmdbSearchItem>,
    pub top_rated_tv: Vec<TmdbSearchItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbPersonCredit {
    pub tmdb_id: i64,
    pub media_type: String,
    pub title: String,
    pub year: Option<String>,
    pub poster_url: Option<String>,
    pub character: Option<String>,
    pub vote_average: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbPersonDetail {
    pub id: i64,
    pub name: String,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub place_of_birth: Option<String>,
    pub photo_url: Option<String>,
    pub known_for_department: Option<String>,
    pub also_known_as: Vec<String>,
    pub total_credits: i32,
    pub career_start: Option<i32>,
    pub career_end: Option<i32>,
    pub credits: Vec<TmdbPersonCredit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbVideo {
    pub key: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentOption {
    pub provider: String,
    pub provider_id: String,
    pub title: String,
    pub magnet: String,
    pub quality: Option<String>,
    pub size: i64,
    pub seeds: i32,
    pub peers: i32,
    pub audio: Vec<String>,
    pub video_codec: Option<String>,
    pub subtitle_info: Option<String>,
    pub release_group: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub pref_score: f64,
    #[serde(default)]
    pub aggregator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: String,
    pub media_id: String,
    pub episode_id: Option<String>,
    pub magnet: String,
    pub qbit_hash: Option<String>,
    pub status: String,
    pub save_path: String,
    pub title: Option<String>,
    pub requested_by: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub magnet: String,
    pub media_id: Option<String>,
    pub tmdb_id: Option<i64>,
    pub media_type: Option<String>,
    pub episode_id: Option<String>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub title: Option<String>,
    pub torrent: Option<TorrentOption>,
}

#[derive(Debug, Serialize)]
pub struct DownloadStatus {
    #[serde(flatten)]
    pub download: Download,
    pub progress: f64,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbSeason {
    pub season_number: i32,
    pub name: String,
    pub episode_count: i32,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbEpisode {
    pub episode_number: i32,
    pub season_number: i32,
    pub name: String,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub still_url: Option<String>,
    pub runtime: Option<i32>,
    pub vote_average: Option<f64>,
}
