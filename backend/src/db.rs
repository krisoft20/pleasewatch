use crate::models::{
    Book, BookContinueItem, BookMark, BookProgress, BookShelfItem, CollectionItem, Download,
    Episode, Manga, MangaContinueItem, MangaProgress, Media, User,
};
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::path::Path;

const SESSION_MAX_AGE: &str = "-14 days";

#[derive(Default)]
pub struct MangaEnrichment<'a> {
    pub tags: Option<&'a str>,
    pub demographic: Option<&'a str>,
    pub content_rating: Option<&'a str>,
    pub original_language: Option<&'a str>,
    pub authors: Option<&'a str>,
    pub artists: Option<&'a str>,
    pub score: Option<f64>,
    pub score_count: Option<i64>,
    pub follow_count: Option<i64>,
    pub last_chapter: Option<&'a str>,
    pub anilist_id: Option<i64>,
    pub mal_id: Option<i64>,
    pub links_json: Option<&'a str>,
}

pub struct Database {
    conn: Connection,
}

#[derive(Debug)]
pub enum CollectionError {
    NotFound,
    ShowcaseLimit,
    ShowcaseRequiresCompletion,
    Database(rusqlite::Error),
}

impl std::fmt::Display for CollectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("collection item not found"),
            Self::ShowcaseLimit => f.write_str("showcase is limited to five items"),
            Self::ShowcaseRequiresCompletion => {
                f.write_str("only completed items can be showcased")
            }
            Self::Database(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for CollectionError {}

impl From<rusqlite::Error> for CollectionError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;",
        )?;
        Ok(Self { conn })
    }

    pub fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                email TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT DEFAULT (datetime('now')),
                approved_at TEXT DEFAULT NULL,
                approved_by TEXT DEFAULT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);

            CREATE TABLE IF NOT EXISTS media (
                id TEXT PRIMARY KEY,
                tmdb_id INTEGER,
                media_type TEXT NOT NULL,
                title TEXT NOT NULL,
                year INTEGER,
                overview TEXT,
                poster_url TEXT,
                backdrop_url TEXT,
                file_path TEXT,
                duration INTEGER,
                status TEXT NOT NULL DEFAULT 'pending',
                added_by TEXT,
                added_at TEXT DEFAULT (datetime('now')),
                is_anime INTEGER NOT NULL DEFAULT 0
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_media_tmdb ON media(tmdb_id, media_type)
                WHERE tmdb_id IS NOT NULL;

            CREATE TABLE IF NOT EXISTS episodes (
                id TEXT PRIMARY KEY,
                media_id TEXT NOT NULL,
                season INTEGER NOT NULL,
                episode INTEGER NOT NULL,
                title TEXT,
                file_path TEXT,
                duration INTEGER,
                status TEXT NOT NULL DEFAULT 'pending',
                FOREIGN KEY (media_id) REFERENCES media(id),
                UNIQUE(media_id, season, episode)
            );

            CREATE INDEX IF NOT EXISTS idx_episodes_media ON episodes(media_id);

            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                media_id TEXT NOT NULL,
                episode_id TEXT,
                magnet TEXT NOT NULL,
                qbit_hash TEXT,
                status TEXT NOT NULL DEFAULT 'queued',
                save_path TEXT NOT NULL,
                title TEXT,
                requested_by TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                completed_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
            CREATE INDEX IF NOT EXISTS idx_downloads_media ON downloads(media_id);
            CREATE INDEX IF NOT EXISTS idx_downloads_hash ON downloads(qbit_hash);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS subtitles (
                id TEXT PRIMARY KEY,
                owner_id TEXT NOT NULL,
                language TEXT NOT NULL,
                label TEXT NOT NULL,
                format TEXT NOT NULL,
                file_path TEXT NOT NULL,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_subtitles_owner ON subtitles(owner_id);",
        )?;

        if !self.has_column("media", "is_anime")? {
            self.conn.execute(
                "ALTER TABLE media ADD COLUMN is_anime INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !self.has_column("episodes", "source_name")? {
            self.conn
                .execute("ALTER TABLE episodes ADD COLUMN source_name TEXT", [])?;
        }
        if !self.has_column("episodes", "still_url")? {
            self.conn
                .execute("ALTER TABLE episodes ADD COLUMN still_url TEXT", [])?;
        }
        if !self.has_column("episodes", "ready_at")? {
            self.conn
                .execute("ALTER TABLE episodes ADD COLUMN ready_at TEXT", [])?;
        }
        if !self.has_column("media", "source_name")? {
            self.conn
                .execute("ALTER TABLE media ADD COLUMN source_name TEXT", [])?;
        }

        for col in ["intro_start", "intro_end", "credits_start"] {
            if !self.has_column("episodes", col)? {
                self.conn.execute(
                    &format!("ALTER TABLE episodes ADD COLUMN {col} INTEGER"),
                    [],
                )?;
            }
        }

        let dropped = self.conn.execute(
            "UPDATE episodes SET intro_start = NULL, intro_end = NULL
             WHERE intro_start IS NOT NULL
               AND (intro_end - intro_start < 18 OR intro_start < 30)",
            [],
        )?;
        if dropped > 0 {
            println!("[db] nulled {dropped} dodgy intro markers (too short or too early)");
        }

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watch_progress (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL,
                media_id    TEXT NOT NULL,
                episode_id  TEXT,
                position    INTEGER NOT NULL DEFAULT 0,
                duration    INTEGER NOT NULL DEFAULT 0,
                completed   INTEGER NOT NULL DEFAULT 0,
                dismissed   INTEGER NOT NULL DEFAULT 0,
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_watch_progress_unique
                ON watch_progress(user_id, media_id, COALESCE(episode_id, ''));

            CREATE INDEX IF NOT EXISTS idx_watch_progress_user
                ON watch_progress(user_id, updated_at DESC);

            CREATE TABLE IF NOT EXISTS watch_time (
                user_id     TEXT PRIMARY KEY,
                seconds     INTEGER NOT NULL DEFAULT 0,
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;

        self.conn.execute(
            "INSERT INTO watch_time (user_id, seconds, updated_at)
             SELECT user_id,
                    COALESCE(SUM(CASE WHEN position > duration THEN duration ELSE position END), 0),
                    MAX(updated_at)
             FROM watch_progress
             WHERE duration > 0
             GROUP BY user_id
             ON CONFLICT(user_id) DO NOTHING",
            [],
        )?;

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watch_sessions (
                id            TEXT PRIMARY KEY,
                code          TEXT NOT NULL UNIQUE,
                host_user_id  TEXT NOT NULL,
                media_id      TEXT NOT NULL,
                episode_id    TEXT,
                active        INTEGER NOT NULL DEFAULT 1,
                created_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_ws_code ON watch_sessions(code);",
        )?;

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clips (
                id           TEXT PRIMARY KEY,
                media_id     TEXT NOT NULL,
                episode_id   TEXT,
                start_sec    REAL NOT NULL,
                end_sec      REAL NOT NULL,
                subtitle_id  TEXT,
                file_path    TEXT NOT NULL,
                file_size    INTEGER,
                created_by   TEXT NOT NULL,
                created_at   TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_clips_created ON clips(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_clips_media   ON clips(media_id);

            CREATE TABLE IF NOT EXISTS user_torrent_prefs (
                user_id    TEXT NOT NULL,
                kind       TEXT NOT NULL,
                value      TEXT NOT NULL,
                count      INTEGER NOT NULL DEFAULT 0,
                last_used  TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (user_id, kind, value)
            );
            CREATE INDEX IF NOT EXISTS idx_torrprefs_user ON user_torrent_prefs(user_id);",
        )?;

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS manga (
                id          TEXT PRIMARY KEY,
                md_id       TEXT UNIQUE NOT NULL,
                title       TEXT NOT NULL,
                description TEXT,
                cover_url   TEXT,
                year        INTEGER,
                status      TEXT,
                added_by    TEXT,
                added_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS manga_progress (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL,
                md_id       TEXT NOT NULL,
                chapter_id  TEXT NOT NULL,
                chapter     TEXT,
                page        INTEGER NOT NULL DEFAULT 0,
                pages       INTEGER NOT NULL DEFAULT 0,
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(user_id, md_id)
            );

            CREATE INDEX IF NOT EXISTS idx_manga_progress_user
                ON manga_progress(user_id, updated_at DESC);",
        )?;

        for (col, ty) in [
            ("restricted", "INTEGER NOT NULL DEFAULT 0"),
            ("restricted_langs", "TEXT"),
            ("comick_hid", "TEXT"),
            ("anilist_id", "INTEGER"),
            ("mal_id", "INTEGER"),
            ("links_json", "TEXT"),
            ("tags", "TEXT"),
            ("demographic", "TEXT"),
            ("content_rating", "TEXT"),
            ("original_language", "TEXT"),
            ("authors", "TEXT"),
            ("artists", "TEXT"),
            ("score", "REAL"),
            ("score_count", "INTEGER"),
            ("follow_count", "INTEGER"),
            ("last_chapter", "TEXT"),
            ("enriched_at", "TEXT"),
        ] {
            if !self.has_column("manga", col)? {
                self.conn
                    .execute(&format!("ALTER TABLE manga ADD COLUMN {col} {ty}"), [])?;
            }
        }

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS books (
                id           TEXT PRIMARY KEY,
                ol_key       TEXT UNIQUE NOT NULL,
                title        TEXT NOT NULL,
                authors      TEXT,
                description  TEXT,
                cover_url    TEXT,
                year         INTEGER,
                language     TEXT,
                file_path    TEXT,
                ext          TEXT,
                status       TEXT NOT NULL DEFAULT 'pending',
                added_by     TEXT,
                added_at     TEXT NOT NULL DEFAULT (datetime('now')),
                pages        INTEGER,
                subjects     TEXT,
                isbn         TEXT,
                publisher    TEXT,
                rating       REAL,
                rating_count INTEGER,
                enriched_at  TEXT
            );

            CREATE TABLE IF NOT EXISTS book_progress (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL,
                ol_key      TEXT NOT NULL,
                cfi         TEXT,
                percent     REAL NOT NULL DEFAULT 0,
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(user_id, ol_key)
            );

            CREATE INDEX IF NOT EXISTS idx_book_progress_user
                ON book_progress(user_id, updated_at DESC);

            CREATE TABLE IF NOT EXISTS book_marks (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL,
                ol_key      TEXT NOT NULL,
                kind        TEXT NOT NULL,
                cfi         TEXT NOT NULL,
                color       TEXT,
                note        TEXT,
                snippet     TEXT,
                chapter     TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_book_marks_user
                ON book_marks(user_id, ol_key);",
        )?;

        for (col, ty) in [
            ("pages", "INTEGER"),
            ("subjects", "TEXT"),
            ("isbn", "TEXT"),
            ("publisher", "TEXT"),
            ("rating", "REAL"),
            ("rating_count", "INTEGER"),
            ("enriched_at", "TEXT"),
            ("series", "TEXT"),
            ("author_keys", "TEXT"),
            ("qbit_hash", "TEXT"),
            ("download_progress", "REAL"),
        ] {
            if !self.has_column("books", col)? {
                self.conn
                    .execute(&format!("ALTER TABLE books ADD COLUMN {col} {ty}"), [])?;
            }
        }

        if !self.has_column("users", "book_goal")? {
            self.conn
                .execute("ALTER TABLE users ADD COLUMN book_goal INTEGER", [])?;
        }

        let had_shelf = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'book_shelf'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n > 0)?;

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS book_shelf (
                id          TEXT PRIMARY KEY,
                user_id     TEXT NOT NULL,
                ol_key      TEXT NOT NULL,
                status      TEXT NOT NULL,
                added_at    TEXT NOT NULL DEFAULT (datetime('now')),
                finished_at TEXT,
                showcased   INTEGER NOT NULL DEFAULT 0,
                UNIQUE(user_id, ol_key)
            );

            CREATE INDEX IF NOT EXISTS idx_book_shelf_user ON book_shelf(user_id, status);",
        )?;

        if !self.has_column("book_shelf", "showcased")? {
            self.conn.execute(
                "ALTER TABLE book_shelf ADD COLUMN showcased INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        if !had_shelf {
            let seeded = self.conn.execute(
                "INSERT INTO book_shelf (id, user_id, ol_key, status, added_at, finished_at)
                 SELECT lower(hex(randomblob(16))), user_id, ol_key,
                        CASE WHEN percent >= 0.97 THEN 'read' ELSE 'reading' END,
                        updated_at,
                        CASE WHEN percent >= 0.97 THEN updated_at ELSE NULL END
                 FROM book_progress WHERE percent > 0",
                [],
            )?;
            if seeded > 0 {
                println!("[db] backfilled {seeded} shelf rows from progress");
            }
        }

        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS collection_items (
                id            TEXT PRIMARY KEY,
                user_id       TEXT NOT NULL,
                tmdb_id       INTEGER NOT NULL,
                kind          TEXT NOT NULL CHECK(kind IN ('movie', 'tv', 'anime')),
                title         TEXT NOT NULL,
                year          TEXT,
                poster_url    TEXT,
                backdrop_url  TEXT,
                status        TEXT NOT NULL CHECK(status IN ('planned', 'in_progress', 'completed')),
                showcased     INTEGER NOT NULL DEFAULT 0 CHECK(showcased IN (0, 1)),
                auto_completed INTEGER NOT NULL DEFAULT 0,
                added_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
                completed_at  TEXT,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                UNIQUE(user_id, kind, tmdb_id)
            );

            CREATE INDEX IF NOT EXISTS idx_collection_user_status
                ON collection_items(user_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_collection_user_kind
                ON collection_items(user_id, kind, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_collection_user_showcase
                ON collection_items(user_id, showcased, updated_at DESC);",
        )?;

        if !self.has_column("collection_items", "auto_completed")? {
            self.conn.execute(
                "ALTER TABLE collection_items ADD COLUMN auto_completed INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        self.conn.execute_batch(
            "DELETE FROM book_marks
             WHERE NOT EXISTS (SELECT 1 FROM books WHERE books.ol_key = book_marks.ol_key);
             DELETE FROM book_progress
             WHERE NOT EXISTS (SELECT 1 FROM books WHERE books.ol_key = book_progress.ol_key);
             DELETE FROM book_shelf
             WHERE NOT EXISTS (SELECT 1 FROM books WHERE books.ol_key = book_shelf.ol_key);

             UPDATE collection_items
             SET showcased = 0,
                 auto_completed = 0
             WHERE showcased = 1 AND status != 'completed';

             UPDATE collection_items
             SET auto_completed = 0
             WHERE status != 'completed';

             UPDATE book_shelf
             SET showcased = 0
             WHERE showcased = 1 AND status != 'read';",
        )?;

        let backfilled = self.conn.execute(
            "WITH watched AS (
                SELECT
                    wp.user_id,
                    m.tmdb_id,
                    CASE
                        WHEN m.media_type = 'movie' THEN 'movie'
                        WHEN m.is_anime = 1 THEN 'anime'
                        ELSE 'tv'
                    END AS kind,
                    m.title,
                    CAST(m.year AS TEXT) AS year,
                    m.poster_url,
                    m.backdrop_url,
                    CASE
                        WHEN m.media_type = 'movie' AND MAX(wp.completed) = 1
                            THEN 'completed'
                        ELSE 'in_progress'
                    END AS status,
                    MIN(wp.updated_at) AS added_at,
                    MAX(wp.updated_at) AS updated_at,
                    CASE
                        WHEN m.media_type = 'movie' AND MAX(wp.completed) = 1
                            THEN MAX(CASE WHEN wp.completed = 1 THEN wp.updated_at END)
                        ELSE NULL
                    END AS completed_at
                FROM watch_progress wp
                JOIN media m ON m.id = wp.media_id
                JOIN users u ON u.id = wp.user_id
                WHERE m.tmdb_id IS NOT NULL
                  AND (wp.position > 30 OR wp.completed = 1)
                GROUP BY wp.user_id, m.id
             )
             INSERT INTO collection_items
                (id, user_id, tmdb_id, kind, title, year, poster_url, backdrop_url, status,
                 showcased, added_at, updated_at, completed_at)
             SELECT
                lower(hex(randomblob(16))), w.user_id, w.tmdb_id, w.kind, w.title, w.year,
                w.poster_url, w.backdrop_url, w.status, 0, w.added_at, w.updated_at,
                w.completed_at
             FROM watched w
             WHERE NOT EXISTS (
                SELECT 1
                FROM collection_items c
                WHERE c.user_id = w.user_id
                  AND c.tmdb_id = w.tmdb_id
                  AND (
                       c.kind = w.kind
                       OR (c.kind IN ('tv', 'anime') AND w.kind IN ('tv', 'anime'))
                  )
             )
             ON CONFLICT DO NOTHING",
            [],
        )?;
        if backfilled > 0 {
            println!("[db] backfilled {backfilled} collection rows from watch progress");
        }

        let series_aliases = {
            let mut stmt = self.conn.prepare(
                "SELECT
                    c.user_id,
                    c.tmdb_id,
                    CASE
                        WHEN EXISTS (
                            SELECT 1 FROM media m
                            WHERE m.tmdb_id = c.tmdb_id
                              AND m.media_type = 'tv'
                              AND m.is_anime = 1
                        ) THEN 'anime'
                        WHEN EXISTS (
                            SELECT 1 FROM media m
                            WHERE m.tmdb_id = c.tmdb_id
                              AND m.media_type = 'tv'
                              AND m.is_anime = 0
                        ) THEN 'tv'
                        ELSE 'anime'
                    END AS canonical_kind
                 FROM collection_items c
                 WHERE c.kind IN ('tv', 'anime')
                   AND EXISTS (
                       SELECT 1 FROM media m
                       WHERE m.tmdb_id = c.tmdb_id
                         AND m.media_type = 'tv'
                   )
                 GROUP BY c.user_id, c.tmdb_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>>>()?
        };
        for (user_id, tmdb_id, kind) in series_aliases {
            reconcile_collection_alias(&self.conn, &user_id, &kind, tmdb_id)?;
        }

        self.conn.execute(
            "UPDATE collection_items AS c
             SET status = 'completed',
                 completed_at = COALESCE(
                     completed_at,
                     (
                         SELECT MAX(wp.updated_at)
                         FROM watch_progress wp
                         JOIN media m ON m.id = wp.media_id
                         WHERE wp.user_id = c.user_id
                           AND m.tmdb_id = c.tmdb_id
                           AND m.media_type = 'movie'
                           AND wp.completed = 1
                     )
                 ),
                 updated_at = COALESCE(
                     (
                         SELECT MAX(wp.updated_at)
                         FROM watch_progress wp
                         JOIN media m ON m.id = wp.media_id
                         WHERE wp.user_id = c.user_id
                           AND m.tmdb_id = c.tmdb_id
                           AND m.media_type = 'movie'
                           AND wp.completed = 1
                     ),
                     updated_at
                 )
             WHERE c.kind = 'movie'
               AND c.status IN ('planned', 'in_progress')
               AND EXISTS (
                   SELECT 1
                   FROM watch_progress wp
                   JOIN media m ON m.id = wp.media_id
                   WHERE wp.user_id = c.user_id
                     AND m.tmdb_id = c.tmdb_id
                     AND m.media_type = 'movie'
                     AND wp.completed = 1
               )",
            [],
        )?;

        self.conn.execute(
            "UPDATE collection_items AS c
             SET status = 'in_progress',
                 completed_at = NULL,
                 updated_at = COALESCE(
                     (
                         SELECT MAX(wp.updated_at)
                         FROM watch_progress wp
                         JOIN media m ON m.id = wp.media_id
                         WHERE wp.user_id = c.user_id
                           AND m.tmdb_id = c.tmdb_id
                           AND (
                               (c.kind = 'movie' AND m.media_type = 'movie')
                               OR (c.kind IN ('tv', 'anime') AND m.media_type = 'tv')
                           )
                           AND (wp.position > 30 OR wp.completed = 1)
                     ),
                     updated_at
                 )
             WHERE c.status = 'planned'
               AND EXISTS (
                   SELECT 1
                   FROM watch_progress wp
                   JOIN media m ON m.id = wp.media_id
                   WHERE wp.user_id = c.user_id
                     AND m.tmdb_id = c.tmdb_id
                     AND (
                         (c.kind = 'movie' AND m.media_type = 'movie')
                         OR (c.kind IN ('tv', 'anime') AND m.media_type = 'tv')
                     )
                     AND (wp.position > 30 OR wp.completed = 1)
               )",
            [],
        )?;

        let shows = {
            let mut stmt = self
                .conn
                .prepare("SELECT id FROM media WHERE media_type = 'tv' AND tmdb_id IS NOT NULL")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>>>()?
        };
        let mut series_done = 0;
        for media_id in shows {
            series_done += refresh_series_for_media(&self.conn, &media_id)?;
        }
        if series_done > 0 {
            println!("[db] refreshed {series_done} series collection rows");
        }

        self.conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_collection_user_canonical_tmdb
                ON collection_items(
                    user_id,
                    CASE WHEN kind IN ('tv', 'anime') THEN 'tv' ELSE kind END,
                    tmdb_id
                );

             CREATE TRIGGER IF NOT EXISTS trg_collection_showcase_completed_insert
             BEFORE INSERT ON collection_items
             WHEN NEW.showcased = 1 AND NEW.status != 'completed'
             BEGIN
                SELECT RAISE(ABORT, 'only completed items can be showcased');
             END;

             CREATE TRIGGER IF NOT EXISTS trg_collection_showcase_completed_update
             BEFORE UPDATE OF status, showcased ON collection_items
             WHEN NEW.showcased = 1 AND NEW.status != 'completed'
             BEGIN
                SELECT RAISE(ABORT, 'only completed items can be showcased');
             END;

             CREATE TRIGGER IF NOT EXISTS trg_book_showcase_read_insert
             BEFORE INSERT ON book_shelf
             WHEN NEW.showcased = 1 AND NEW.status != 'read'
             BEGIN
                SELECT RAISE(ABORT, 'only completed items can be showcased');
             END;

             CREATE TRIGGER IF NOT EXISTS trg_book_showcase_read_update
             BEFORE UPDATE OF status, showcased ON book_shelf
             WHEN NEW.showcased = 1 AND NEW.status != 'read'
             BEGIN
                SELECT RAISE(ABORT, 'only completed items can be showcased');
             END;",
        )?;

        let stuck = self.conn.execute(
            "UPDATE books SET status = 'error' WHERE status = 'processing'",
            [],
        )?;
        if stuck > 0 {
            println!("[db] {stuck} books stuck in processing -> error (restart mid-convert)");
        }

        let rehosted = self.conn.execute(
            "UPDATE manga
             SET cover_url = replace(cover_url, 'https://uploads.mangadex.org/covers/', '/api/manga/cover/')
             WHERE cover_url LIKE 'https://uploads.mangadex.org/covers/%'",
            [],
        )?;
        if rehosted > 0 {
            println!(
                "[db] rewrote {rehosted} manga covers to the proxy route (mangadex hotlink block)"
            );
        }

        self.conn.execute(
            "DELETE FROM subtitles
             WHERE owner_id NOT IN (SELECT id FROM media)
               AND owner_id NOT IN (SELECT id FROM episodes)",
            [],
        )?;

        self.conn.execute(
            "DELETE FROM downloads
             WHERE status IN ('complete', 'cancelled')
               AND created_at < datetime('now', '-30 days')",
            [],
        )?;

        Ok(())
    }

    fn has_column(&self, table: &str, column: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
        for name in rows {
            if name? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn create_user(
        &self,
        id: &str,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO users (id, username, email, password_hash, role) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, username, email, password_hash, role],
        )?;
        Ok(())
    }

    pub fn find_user_by_id(&self, id: &str) -> Result<Option<User>> {
        self.conn
            .query_row(
                "SELECT id, username, email, password_hash, role, created_at, approved_at, approved_by
                 FROM users WHERE id = ?1",
                params![id],
                map_user,
            )
            .optional()
    }

    pub fn find_user_by_username(&self, username: &str) -> Result<Option<User>> {
        self.conn
            .query_row(
                "SELECT id, username, email, password_hash, role, created_at, approved_at, approved_by
                 FROM users WHERE username = ?1",
                params![username],
                map_user,
            )
            .optional()
    }

    pub fn count_users(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
    }

    pub fn count_media_by_status(&self) -> Result<(i64, i64, i64)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM media", [], |r| r.get(0))?;
        let ready: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM media WHERE status = 'ready'",
            [],
            |r| r.get(0),
        )?;
        let error: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM media WHERE status = 'error'",
            [],
            |r| r.get(0),
        )?;
        Ok((total, ready, error))
    }

    pub fn count_episodes(&self) -> Result<(i64, i64)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM episodes", [], |r| r.get(0))?;
        let ready: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM episodes WHERE status = 'ready'",
            [],
            |r| r.get(0),
        )?;
        Ok((total, ready))
    }

    pub fn count_downloads(&self) -> Result<(i64, i64)> {
        let active: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE status IN ('queued','downloading','processing')",
            [],
            |r| r.get(0),
        )?;
        let errored: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE status = 'error'",
            [],
            |r| r.get(0),
        )?;
        Ok((active, errored))
    }

    pub fn subtitle_stats(&self) -> Result<(i64, Vec<(String, i64)>, Vec<(String, i64)>)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM subtitles", [], |r| r.get(0))?;
        let mut langs = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(language,''), COUNT(*) FROM subtitles
             GROUP BY language ORDER BY COUNT(*) DESC",
        )?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            langs.push(row?);
        }
        let mut srcs = Vec::new();
        let mut s2 = self.conn.prepare(
            "SELECT COALESCE(format,''), COUNT(*) FROM subtitles
             GROUP BY format ORDER BY COUNT(*) DESC",
        )?;
        for row in s2.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            srcs.push(row?);
        }
        Ok((total, langs, srcs))
    }

    pub fn watch_summary(&self) -> Result<(i64, i64, i64)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM watch_progress", [], |r| r.get(0))?;
        let completed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM watch_progress WHERE completed = 1",
            [],
            |r| r.get(0),
        )?;
        let active: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM watch_time
             WHERE updated_at > datetime('now','-24 hours')",
            [],
            |r| r.get(0),
        )?;
        Ok((total, completed, active))
    }

    pub fn watch_stats_aggregate(&self) -> Result<(i64, i64, Vec<(String, String, i64, i64)>)> {
        let total_secs: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(seconds), 0) FROM watch_time",
            [],
            |r| r.get(0),
        )?;
        let total_done: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM watch_progress WHERE completed = 1",
            [],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(
            "SELECT u.id, u.username,
                COALESCE(wt.seconds, 0) AS secs,
                COALESCE(SUM(CASE WHEN wp.completed = 1 THEN 1 ELSE 0 END), 0) AS done
             FROM users u
             LEFT JOIN watch_time wt ON wt.user_id = u.id
             LEFT JOIN watch_progress wp ON wp.user_id = u.id
             WHERE u.role IN ('user', 'admin')
             GROUP BY u.id, u.username, wt.seconds
             HAVING secs > 0 OR done > 0
             ORDER BY secs DESC",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<Vec<_>>>()?;
        Ok((total_secs, total_done, rows))
    }

    pub fn add_watch_seconds(&self, user_id: &str, seconds: i64) -> Result<()> {
        let secs = seconds.clamp(1, 60);
        self.conn.execute(
            "INSERT INTO watch_time (user_id, seconds, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(user_id) DO UPDATE SET
                seconds = watch_time.seconds + excluded.seconds,
                updated_at = datetime('now')",
            params![user_id, secs],
        )?;
        Ok(())
    }

    pub fn list_users_by_role(&self, role: &str) -> Result<Vec<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, email, password_hash, role, created_at, approved_at, approved_by
             FROM users WHERE role = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![role], map_user)?;
        rows.collect()
    }

    pub fn list_all_users(&self) -> Result<Vec<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, email, password_hash, role, created_at, approved_at, approved_by
             FROM users
             ORDER BY CASE role WHEN 'pending' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, created_at DESC",
        )?;
        let rows = stmt.query_map([], map_user)?;
        rows.collect()
    }

    pub fn set_user_role(&self, user_id: &str, role: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE users SET role = ?1 WHERE id = ?2",
            params![role, user_id],
        )?;
        Ok(n > 0)
    }

    pub fn list_admin_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM users WHERE role = 'admin'")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn bump_torrent_prefs(&self, user_id: &str, attrs: &[(String, String)]) -> Result<()> {
        for (kind, value) in attrs {
            self.conn.execute(
                "INSERT INTO user_torrent_prefs (user_id, kind, value, count, last_used)
                 VALUES (?1, ?2, ?3, 1, datetime('now'))
                 ON CONFLICT(user_id, kind, value)
                 DO UPDATE SET count = count + 1, last_used = datetime('now')",
                params![user_id, kind, value],
            )?;
        }
        Ok(())
    }

    pub fn get_torrent_prefs(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<(String, String, String, f64)>> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<&str> = user_ids.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT user_id, kind, value, count,
                    CAST((julianday('now') - julianday(last_used)) AS REAL) AS days
             FROM user_torrent_prefs
             WHERE user_id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params_dyn: Vec<&dyn rusqlite::ToSql> =
            user_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(params_dyn.iter()), |r| {
            let user_id: String = r.get(0)?;
            let kind: String = r.get(1)?;
            let value: String = r.get(2)?;
            let count: i64 = r.get(3)?;
            let days: f64 = r.get(4)?;
            let decay = (-days / 30.0).exp();
            Ok((user_id, kind, value, count as f64 * decay))
        })?;
        rows.collect()
    }

    pub fn approve_user(&self, user_id: &str, admin_id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE users SET role = 'user', approved_at = datetime('now'), approved_by = ?1
             WHERE id = ?2 AND role = 'pending'",
            params![admin_id, user_id],
        )?;
        Ok(n > 0)
    }

    pub fn delete_user(&self, user_id: &str) -> Result<bool> {
        self.conn
            .execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id])?;
        let n = self
            .conn
            .execute("DELETE FROM users WHERE id = ?1", params![user_id])?;
        Ok(n > 0)
    }

    pub fn create_session(&self, token: &str, user_id: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (token, user_id) VALUES (?1, ?2)",
            params![token, user_id],
        )?;
        Ok(())
    }

    pub fn find_user_by_session(&self, token: &str) -> Result<Option<User>> {
        self.conn
            .query_row(
                "SELECT u.id, u.username, u.email, u.password_hash, u.role,
                        u.created_at, u.approved_at, u.approved_by
                 FROM sessions s JOIN users u ON u.id = s.user_id
                 WHERE s.token = ?1 AND s.created_at > datetime('now', ?2)",
                params![token, SESSION_MAX_AGE],
                map_user,
            )
            .optional()
    }

    pub fn delete_session(&self, token: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    pub fn cleanup_sessions(&self) -> Result<()> {
        let n = self.conn.execute(
            "DELETE FROM sessions WHERE created_at < datetime('now', ?1)",
            params![SESSION_MAX_AGE],
        )?;
        if n > 0 {
            println!("[db] cleaned {n} expired sessions");
        }
        Ok(())
    }

    pub fn create_media(&self, m: &Media) -> Result<()> {
        self.conn.execute(
            "INSERT INTO media (id, tmdb_id, media_type, title, year, overview, poster_url, backdrop_url,
                                file_path, duration, status, added_by, added_at, is_anime, source_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                m.id, m.tmdb_id, m.media_type, m.title, m.year, m.overview, m.poster_url,
                m.backdrop_url, m.file_path, m.duration, m.status, m.added_by, m.added_at,
                m.is_anime as i32, m.source_name,
            ],
        )?;
        Ok(())
    }

    pub fn find_media_by_id(&self, id: &str) -> Result<Option<Media>> {
        self.conn
            .query_row(
                "SELECT id, tmdb_id, media_type, title, year, overview, poster_url, backdrop_url,
                        file_path, duration, status, added_by, added_at, is_anime, source_name
                 FROM media WHERE id = ?1",
                params![id],
                map_media,
            )
            .optional()
    }

    pub fn find_media_by_tmdb(&self, tmdb_id: i64, media_type: &str) -> Result<Option<Media>> {
        self.conn
            .query_row(
                "SELECT id, tmdb_id, media_type, title, year, overview, poster_url, backdrop_url,
                        file_path, duration, status, added_by, added_at, is_anime, source_name
                 FROM media WHERE tmdb_id = ?1 AND media_type = ?2",
                params![tmdb_id, media_type],
                map_media,
            )
            .optional()
    }

    pub fn list_media(&self) -> Result<Vec<Media>> {
        self.list_media_for(None, None)
    }

    pub fn list_media_for_viewer(&self, user_id: &str) -> Result<Vec<Media>> {
        self.list_media_for(Some(user_id), None)
    }

    pub fn list_media_by_user(&self, user_id: &str) -> Result<Vec<Media>> {
        self.list_media_for(Some(user_id), Some(user_id))
    }

    fn list_media_for(
        &self,
        viewer_id: Option<&str>,
        owner_id: Option<&str>,
    ) -> Result<Vec<Media>> {
        let viewer_id = viewer_id.unwrap_or("");
        let filter = if owner_id.is_some() {
            " WHERE added_by = ?2"
        } else {
            ""
        };
        let sql = format!(
            "SELECT id, tmdb_id, media_type, title, year, overview, poster_url, backdrop_url,
                    file_path, duration, status, added_by, added_at, is_anime, source_name,
                    COALESCE((
                        SELECT MAX(COALESCE(e.ready_at, d.completed_at))
                        FROM episodes e
                        LEFT JOIN downloads d
                            ON d.episode_id = e.id
                           AND d.status = 'complete'
                           AND d.completed_at IS NOT NULL
                        WHERE e.media_id = media.id
                          AND e.status = 'ready'
                          AND (e.ready_at IS NOT NULL OR d.completed_at IS NOT NULL)
                    ), added_at) AS activity_at,
                    (
                        SELECT CASE
                            WHEN ?1 != '' AND EXISTS (
                                SELECT 1
                                FROM watch_progress wp
                                WHERE wp.user_id = ?1
                                  AND wp.episode_id = e.id
                                  AND wp.completed = 1
                            )
                            THEN NULL
                            ELSE 'S' || printf('%02d', e.season) || 'E' || printf('%02d', e.episode)
                        END
                        FROM episodes e
                        LEFT JOIN downloads d
                            ON d.episode_id = e.id
                           AND d.status = 'complete'
                           AND d.completed_at IS NOT NULL
                        WHERE e.media_id = media.id
                          AND e.status = 'ready'
                          AND (e.ready_at IS NOT NULL OR d.completed_at IS NOT NULL)
                        ORDER BY COALESCE(e.ready_at, d.completed_at) DESC
                        LIMIT 1
                    ) AS activity_label
             FROM media{filter} ORDER BY activity_at DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        match owner_id {
            Some(owner_id) => {
                let rows = stmt.query_map(params![viewer_id, owner_id], map_media)?;
                rows.collect()
            }
            None => {
                let rows = stmt.query_map(params![viewer_id], map_media)?;
                rows.collect()
            }
        }
    }

    pub fn delete_media(&self, id: &str) -> Result<bool> {
        self.conn.execute(
            "DELETE FROM subtitles
             WHERE owner_id = ?1
                OR owner_id IN (SELECT id FROM episodes WHERE media_id = ?1)",
            params![id],
        )?;
        self.conn
            .execute("DELETE FROM episodes WHERE media_id = ?1", params![id])?;
        let n = self
            .conn
            .execute("DELETE FROM media WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    pub fn list_collection(
        &self,
        user_id: &str,
        kind: Option<&str>,
        status: Option<&str>,
        showcased: Option<bool>,
    ) -> Result<Vec<CollectionItem>> {
        let showcased = showcased.map(|value| value as i64);
        let mut stmt = self.conn.prepare(
            "SELECT id, tmdb_id, kind, title, year, poster_url, backdrop_url, status,
                    showcased, added_at, updated_at, completed_at
             FROM collection_items
             WHERE user_id = ?1
               AND (?2 IS NULL OR kind = ?2)
               AND (?3 IS NULL OR status = ?3)
               AND (?4 IS NULL OR showcased = ?4)
             ORDER BY showcased DESC, updated_at DESC, title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(
            params![user_id, kind, status, showcased],
            map_collection_item,
        )?;
        rows.collect()
    }

    pub fn get_collection_item(
        &self,
        user_id: &str,
        kind: &str,
        tmdb_id: i64,
    ) -> Result<Option<CollectionItem>> {
        get_collection_item_from(&self.conn, user_id, kind, tmdb_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_collection_item(
        &self,
        user_id: &str,
        tmdb_id: i64,
        kind: &str,
        title: &str,
        year: Option<&str>,
        poster_url: Option<&str>,
        backdrop_url: Option<&str>,
        status: &str,
        showcased: Option<bool>,
    ) -> std::result::Result<CollectionItem, CollectionError> {
        if showcased == Some(true) && status != "completed" {
            return Err(CollectionError::ShowcaseRequiresCompletion);
        }

        let tx = self.conn.unchecked_transaction()?;
        reconcile_collection_alias(&tx, user_id, kind, tmdb_id)?;
        if showcased == Some(true)
            && !get_collection_item_from(&tx, user_id, kind, tmdb_id)?
                .is_some_and(|item| item.showcased)
        {
            ensure_showcase_slot_from(&tx, user_id)?;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let showcased = showcased.map(|value| value as i64);
        tx.execute(
            "INSERT INTO collection_items
                (id, user_id, tmdb_id, kind, title, year, poster_url, backdrop_url, status,
                 showcased, auto_completed, added_at, updated_at, completed_at)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, COALESCE(?10, 0), 0,
                 datetime('now'), datetime('now'),
                 CASE WHEN ?9 = 'completed' THEN datetime('now') ELSE NULL END)
             ON CONFLICT(user_id, kind, tmdb_id) DO UPDATE SET
                title = excluded.title,
                year = excluded.year,
                poster_url = excluded.poster_url,
                backdrop_url = excluded.backdrop_url,
                status = excluded.status,
                auto_completed = 0,
                showcased = CASE
                    WHEN excluded.status != 'completed' THEN 0
                    WHEN ?10 IS NULL THEN collection_items.showcased
                    ELSE ?10
                END,
                updated_at = datetime('now'),
                completed_at = CASE
                    WHEN excluded.status = 'completed'
                        THEN COALESCE(collection_items.completed_at, datetime('now'))
                    ELSE NULL
                END",
            params![
                id,
                user_id,
                tmdb_id,
                kind,
                title,
                year,
                poster_url,
                backdrop_url,
                status,
                showcased
            ],
        )?;
        let item = get_collection_item_from(&tx, user_id, kind, tmdb_id)?
            .ok_or(CollectionError::NotFound)?;
        tx.commit()?;
        Ok(item)
    }

    pub fn patch_collection_item(
        &self,
        user_id: &str,
        kind: &str,
        tmdb_id: i64,
        status: Option<&str>,
        showcased: Option<bool>,
    ) -> std::result::Result<CollectionItem, CollectionError> {
        let existing = self
            .get_collection_item(user_id, kind, tmdb_id)?
            .ok_or(CollectionError::NotFound)?;
        let desired_status = status.unwrap_or(&existing.status);
        if showcased == Some(true) && desired_status != "completed" {
            return Err(CollectionError::ShowcaseRequiresCompletion);
        }
        if showcased == Some(true) && !existing.showcased {
            self.ensure_showcase_slot(user_id)?;
        }

        let showcased = showcased.map(|value| value as i64);
        self.conn.execute(
            "UPDATE collection_items SET
                status = COALESCE(?4, status),
                auto_completed = CASE
                    WHEN ?4 IS NULL THEN auto_completed
                    ELSE 0
                END,
                showcased = CASE
                    WHEN COALESCE(?4, status) != 'completed' THEN 0
                    ELSE COALESCE(?5, showcased)
                END,
                updated_at = datetime('now'),
                completed_at = CASE
                    WHEN COALESCE(?4, status) = 'completed'
                        THEN COALESCE(completed_at, datetime('now'))
                    ELSE NULL
                END
             WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
            params![user_id, kind, tmdb_id, status, showcased],
        )?;
        self.get_collection_item(user_id, kind, tmdb_id)?
            .ok_or(CollectionError::NotFound)
    }

    pub fn delete_collection_item(&self, user_id: &str, kind: &str, tmdb_id: i64) -> Result<bool> {
        let changed = if matches!(kind, "tv" | "anime") {
            self.conn.execute(
                "DELETE FROM collection_items
                 WHERE user_id = ?1 AND tmdb_id = ?2 AND kind IN ('tv', 'anime')",
                params![user_id, tmdb_id],
            )?
        } else {
            self.conn.execute(
                "DELETE FROM collection_items
                 WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
                params![user_id, kind, tmdb_id],
            )?
        };
        Ok(changed > 0)
    }

    pub fn sync_collection_from_watch(
        &self,
        user_id: &str,
        media_id: &str,
        completed: bool,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        sync_collection_from_watch_in(&tx, user_id, media_id, completed)?;
        tx.commit()?;
        Ok(())
    }

    fn ensure_showcase_slot(&self, user_id: &str) -> std::result::Result<(), CollectionError> {
        ensure_showcase_slot_from(&self.conn, user_id)
    }

    pub fn delete_subtitles_by_owner(&self, owner_id: &str) -> Result<usize> {
        self.conn.execute(
            "DELETE FROM subtitles WHERE owner_id = ?1",
            params![owner_id],
        )
    }

    pub fn create_manga(&self, m: &Manga) -> Result<()> {
        self.conn.execute(
            "INSERT INTO manga
                (id, md_id, title, description, cover_url, year, status, added_by, added_at,
                 restricted, restricted_langs, comick_hid, anilist_id, mal_id, links_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                m.id,
                m.md_id,
                m.title,
                m.description,
                m.cover_url,
                m.year,
                m.status,
                m.added_by,
                m.added_at,
                m.restricted as i64,
                m.restricted_langs,
                m.comick_hid,
                m.anilist_id,
                m.mal_id,
                m.links_json,
            ],
        )?;
        Ok(())
    }

    pub fn find_manga_by_md(&self, md_id: &str) -> Result<Option<Manga>> {
        self.conn
            .query_row(
                "SELECT id, md_id, title, description, cover_url, year, status, added_by, added_at,
                        restricted, restricted_langs, comick_hid, anilist_id, mal_id, links_json,
                        tags, demographic, content_rating, original_language, authors, artists,
                        score, score_count, follow_count, last_chapter, enriched_at
                 FROM manga WHERE md_id = ?1",
                params![md_id],
                map_manga,
            )
            .optional()
    }

    pub fn list_manga(&self) -> Result<Vec<Manga>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, md_id, title, description, cover_url, year, status, added_by, added_at,
                    restricted, restricted_langs, comick_hid, anilist_id, mal_id, links_json,
                    tags, demographic, content_rating, original_language, authors, artists,
                    score, score_count, follow_count, last_chapter, enriched_at
             FROM manga ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], map_manga)?;
        rows.collect()
    }

    pub fn list_manga_by_user(&self, user_id: &str) -> Result<Vec<Manga>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, md_id, title, description, cover_url, year, status, added_by, added_at,
                    restricted, restricted_langs, comick_hid, anilist_id, mal_id, links_json,
                    tags, demographic, content_rating, original_language, authors, artists,
                    score, score_count, follow_count, last_chapter, enriched_at
             FROM manga WHERE added_by = ?1 ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map(params![user_id], map_manga)?;
        rows.collect()
    }

    pub fn delete_manga_by_md(&self, md_id: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM manga WHERE md_id = ?1", params![md_id])?;
        Ok(n > 0)
    }

    pub fn update_manga_restricted(
        &self,
        md_id: &str,
        restricted: bool,
        restricted_langs: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE manga SET restricted = ?2, restricted_langs = ?3 WHERE md_id = ?1",
            params![md_id, restricted as i64, restricted_langs],
        )?;
        Ok(())
    }

    pub fn update_manga_comick_hid(&self, md_id: &str, hid: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE manga SET comick_hid = ?2 WHERE md_id = ?1",
            params![md_id, hid],
        )?;
        Ok(())
    }

    pub fn update_manga_enrichment(&self, md_id: &str, e: &MangaEnrichment) -> Result<()> {
        self.conn.execute(
            "UPDATE manga SET
                tags              = COALESCE(?2,  tags),
                demographic       = COALESCE(?3,  demographic),
                content_rating    = COALESCE(?4,  content_rating),
                original_language = COALESCE(?5,  original_language),
                authors           = COALESCE(?6,  authors),
                artists           = COALESCE(?7,  artists),
                score             = COALESCE(?8,  score),
                score_count       = COALESCE(?9,  score_count),
                follow_count      = COALESCE(?10, follow_count),
                last_chapter      = COALESCE(?11, last_chapter),
                anilist_id        = COALESCE(?12, anilist_id),
                mal_id            = COALESCE(?13, mal_id),
                links_json        = COALESCE(?14, links_json),
                enriched_at       = datetime('now')
             WHERE md_id = ?1",
            params![
                md_id,
                e.tags,
                e.demographic,
                e.content_rating,
                e.original_language,
                e.authors,
                e.artists,
                e.score,
                e.score_count,
                e.follow_count,
                e.last_chapter,
                e.anilist_id,
                e.mal_id,
                e.links_json,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_manga_progress(
        &self,
        user_id: &str,
        md_id: &str,
        chapter_id: &str,
        chapter: Option<&str>,
        page: i64,
        pages: i64,
    ) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO manga_progress (id, user_id, md_id, chapter_id, chapter, page, pages, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(user_id, md_id) DO UPDATE SET
                chapter_id = excluded.chapter_id,
                chapter = excluded.chapter,
                page = excluded.page,
                pages = excluded.pages,
                updated_at = datetime('now')",
            params![id, user_id, md_id, chapter_id, chapter, page, pages],
        )?;
        Ok(())
    }

    pub fn get_manga_progress(&self, user_id: &str, md_id: &str) -> Result<Option<MangaProgress>> {
        self.conn
            .query_row(
                "SELECT md_id, chapter_id, chapter, page, pages, updated_at
                 FROM manga_progress WHERE user_id = ?1 AND md_id = ?2",
                params![user_id, md_id],
                map_manga_progress,
            )
            .optional()
    }

    pub fn list_manga_continue(&self, user_id: &str, limit: i64) -> Result<Vec<MangaContinueItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.md_id, m.title, m.cover_url, p.chapter_id, p.chapter, p.page, p.pages, p.updated_at
             FROM manga_progress p
             JOIN manga m ON m.md_id = p.md_id
             WHERE p.user_id = ?1
             ORDER BY p.updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![user_id, limit], |row| {
            Ok(MangaContinueItem {
                md_id: row.get(0)?,
                title: row.get(1)?,
                cover_url: row.get(2)?,
                chapter_id: row.get(3)?,
                chapter: row.get(4)?,
                page: row.get(5)?,
                pages: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn create_book(&self, b: &Book) -> Result<()> {
        self.conn.execute(
            "INSERT INTO books (id, ol_key, title, authors, description, cover_url, year, language, file_path, ext, status, added_by, added_at, pages, subjects, isbn, publisher, rating, rating_count, enriched_at, series)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                b.id, b.ol_key, b.title, b.authors, b.description, b.cover_url, b.year,
                b.language, b.file_path, b.ext, b.status, b.added_by, b.added_at,
                b.pages, b.subjects, b.isbn, b.publisher, b.rating, b.rating_count, b.enriched_at,
                b.series,
            ],
        )?;
        Ok(())
    }

    pub fn find_book_by_key(&self, ol_key: &str) -> Result<Option<Book>> {
        self.conn
            .query_row(
                "SELECT id, ol_key, title, authors, description, cover_url, year, language,
                        file_path, ext, status, added_by, added_at,
                        pages, subjects, isbn, publisher, rating, rating_count, enriched_at, series
                 FROM books WHERE ol_key = ?1",
                params![ol_key],
                map_book,
            )
            .optional()
    }

    pub fn get_book_author_keys(&self, ol_key: &str) -> Result<Vec<String>> {
        let raw = self
            .conn
            .query_row(
                "SELECT author_keys FROM books WHERE ol_key = ?1",
                params![ol_key],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(raw
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default())
    }

    pub fn update_book_author_keys(&self, ol_key: &str, author_keys: &[String]) -> Result<()> {
        if author_keys.is_empty() {
            return Ok(());
        }
        let value = serde_json::to_string(author_keys).unwrap_or_default();
        self.conn.execute(
            "UPDATE books SET author_keys = ?1 WHERE ol_key = ?2",
            params![value, ol_key],
        )?;
        Ok(())
    }

    pub fn fill_missing_book_cover(&self, ol_key: &str, cover_url: &str) -> Result<bool> {
        let changed = self.conn.execute(
            "UPDATE books
             SET cover_url = ?1
             WHERE ol_key = ?2
               AND (cover_url IS NULL OR trim(cover_url) = '')",
            params![cover_url, ol_key],
        )?;
        Ok(changed > 0)
    }

    pub fn list_books(&self) -> Result<Vec<Book>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ol_key, title, authors, description, cover_url, year, language,
                    file_path, ext, status, added_by, added_at,
                    pages, subjects, isbn, publisher, rating, rating_count, enriched_at, series
             FROM books ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], map_book)?;
        rows.collect()
    }

    pub fn list_books_by_user(&self, user_id: &str) -> Result<Vec<Book>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ol_key, title, authors, description, cover_url, year, language,
                    file_path, ext, status, added_by, added_at,
                    pages, subjects, isbn, publisher, rating, rating_count, enriched_at, series
             FROM books WHERE added_by = ?1 ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map(params![user_id], map_book)?;
        rows.collect()
    }

    pub fn list_book_keys_owned(
        &self,
        ol_keys: &[String],
    ) -> Result<std::collections::HashMap<String, bool>> {
        if ol_keys.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = (1..=ol_keys.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT ol_key, file_path FROM books WHERE ol_key IN ({placeholders})");
        let mut stmt = self.conn.prepare(&sql)?;
        let params_vec: Vec<&dyn rusqlite::ToSql> =
            ol_keys.iter().map(|k| k as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params_vec.as_slice(), |r| {
            let key: String = r.get(0)?;
            let fp: Option<String> = r.get(1)?;
            Ok((key, fp.is_some()))
        })?;
        rows.collect::<Result<std::collections::HashMap<_, _>>>()
    }

    pub fn delete_book_by_key(&self, ol_key: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM book_marks WHERE ol_key = ?1", params![ol_key])?;
        tx.execute(
            "DELETE FROM book_progress WHERE ol_key = ?1",
            params![ol_key],
        )?;
        tx.execute("DELETE FROM book_shelf WHERE ol_key = ?1", params![ol_key])?;
        let n = tx.execute("DELETE FROM books WHERE ol_key = ?1", params![ol_key])?;
        tx.commit()?;
        Ok(n > 0)
    }

    pub fn update_book_file(&self, ol_key: &str, path: &str, ext: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET file_path = ?1, ext = ?2, status = 'ready' WHERE ol_key = ?3",
            params![path, ext, ol_key],
        )?;
        Ok(())
    }

    pub fn update_book_status(&self, ol_key: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET status = ?1 WHERE ol_key = ?2",
            params![status, ol_key],
        )?;
        Ok(())
    }

    pub fn clear_book_file(&self, ol_key: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET file_path = NULL, ext = NULL, status = 'pending' WHERE ol_key = ?1",
            params![ol_key],
        )?;
        Ok(())
    }

    pub fn set_book_qbit(&self, ol_key: &str, hash: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET qbit_hash = ?1, download_progress = 0, status = 'processing' WHERE ol_key = ?2",
            params![hash, ol_key],
        )?;
        Ok(())
    }

    pub fn update_book_download_progress(&self, ol_key: &str, progress: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET download_progress = ?1 WHERE ol_key = ?2",
            params![progress, ol_key],
        )?;
        Ok(())
    }

    pub fn clear_book_qbit(&self, ol_key: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET qbit_hash = NULL, download_progress = NULL WHERE ol_key = ?1",
            params![ol_key],
        )?;
        Ok(())
    }

    pub fn upsert_book_progress(
        &self,
        user_id: &str,
        ol_key: &str,
        cfi: Option<&str>,
        percent: f64,
    ) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO book_progress (id, user_id, ol_key, cfi, percent, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
             ON CONFLICT(user_id, ol_key) DO UPDATE SET
                cfi = excluded.cfi,
                percent = excluded.percent,
                updated_at = datetime('now')",
            params![id, user_id, ol_key, cfi, percent],
        )?;
        Ok(())
    }

    pub fn get_book_progress(&self, user_id: &str, ol_key: &str) -> Result<Option<BookProgress>> {
        self.conn
            .query_row(
                "SELECT ol_key, cfi, percent, updated_at
                 FROM book_progress WHERE user_id = ?1 AND ol_key = ?2",
                params![user_id, ol_key],
                |row| {
                    Ok(BookProgress {
                        ol_key: row.get(0)?,
                        cfi: row.get(1)?,
                        percent: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
    }

    pub fn list_book_continue(&self, user_id: &str, limit: i64) -> Result<Vec<BookContinueItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT b.ol_key, b.title, b.cover_url, p.cfi, p.percent, p.updated_at, b.authors, b.pages
             FROM book_progress p
             JOIN books b ON b.ol_key = p.ol_key
             WHERE p.user_id = ?1
               AND p.percent > 0
               AND p.percent < 0.97
               AND NOT EXISTS (
                    SELECT 1
                    FROM book_shelf s
                    WHERE s.user_id = p.user_id
                      AND s.ol_key = p.ol_key
                      AND s.status = 'read'
               )
             ORDER BY p.updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![user_id, limit], |row| {
            Ok(BookContinueItem {
                ol_key: row.get(0)?,
                title: row.get(1)?,
                cover_url: row.get(2)?,
                cfi: row.get(3)?,
                percent: row.get(4)?,
                updated_at: row.get(5)?,
                authors: row.get(6)?,
                pages: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_book_marks(&self, user_id: &str, ol_key: &str) -> Result<Vec<BookMark>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ol_key, kind, cfi, color, note, snippet, chapter, created_at
             FROM book_marks WHERE user_id = ?1 AND ol_key = ?2
             ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![user_id, ol_key], |row| {
            Ok(BookMark {
                id: row.get(0)?,
                ol_key: row.get(1)?,
                kind: row.get(2)?,
                cfi: row.get(3)?,
                color: row.get(4)?,
                note: row.get(5)?,
                snippet: row.get(6)?,
                chapter: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn create_book_mark(
        &self,
        user_id: &str,
        ol_key: &str,
        kind: &str,
        cfi: &str,
        color: Option<&str>,
        note: Option<&str>,
        snippet: Option<&str>,
        chapter: Option<&str>,
    ) -> Result<BookMark> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO book_marks (id, user_id, ol_key, kind, cfi, color, note, snippet, chapter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, user_id, ol_key, kind, cfi, color, note, snippet, chapter],
        )?;
        self.conn.query_row(
            "SELECT id, ol_key, kind, cfi, color, note, snippet, chapter, created_at
             FROM book_marks WHERE id = ?1",
            params![id],
            |row| {
                Ok(BookMark {
                    id: row.get(0)?,
                    ol_key: row.get(1)?,
                    kind: row.get(2)?,
                    cfi: row.get(3)?,
                    color: row.get(4)?,
                    note: row.get(5)?,
                    snippet: row.get(6)?,
                    chapter: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        )
    }

    pub fn update_book_mark(
        &self,
        id: &str,
        user_id: &str,
        color: Option<&str>,
        note: Option<&str>,
    ) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE book_marks SET color = COALESCE(?1, color), note = COALESCE(?2, note)
             WHERE id = ?3 AND user_id = ?4",
            params![color, note, id, user_id],
        )?;
        Ok(n > 0)
    }

    pub fn delete_book_mark(&self, id: &str, user_id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM book_marks WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )?;
        Ok(n > 0)
    }

    pub fn set_book_shelf(&self, user_id: &str, ol_key: &str, status: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        if status == "read" {
            self.conn.execute(
                "INSERT INTO book_shelf (id, user_id, ol_key, status, added_at, finished_at)
                 VALUES (?1, ?2, ?3, 'read', datetime('now'), datetime('now'))
                 ON CONFLICT(user_id, ol_key) DO UPDATE SET
                    status = 'read',
                    finished_at = COALESCE(finished_at, excluded.finished_at)",
                params![id, user_id, ol_key],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO book_shelf (id, user_id, ol_key, status, added_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))
                 ON CONFLICT(user_id, ol_key) DO UPDATE SET
                    status = excluded.status,
                    finished_at = NULL,
                    showcased = 0",
                params![id, user_id, ol_key, status],
            )?;
        }
        Ok(())
    }

    pub fn touch_book_shelf_reading(&self, user_id: &str, ol_key: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO book_shelf (id, user_id, ol_key, status, added_at)
             VALUES (?1, ?2, ?3, 'reading', datetime('now'))
             ON CONFLICT(user_id, ol_key) DO UPDATE SET
                status = 'reading',
                showcased = 0
             WHERE book_shelf.status = 'want'",
            params![id, user_id, ol_key],
        )?;
        Ok(())
    }

    pub fn touch_book_shelf_want(&self, user_id: &str, ol_key: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO book_shelf (id, user_id, ol_key, status, added_at)
             VALUES (?1, ?2, ?3, 'want', datetime('now'))
             ON CONFLICT(user_id, ol_key) DO UPDATE SET
                status = 'want',
                finished_at = NULL,
                showcased = 0
             WHERE book_shelf.status != 'read'",
            params![id, user_id, ol_key],
        )?;
        Ok(())
    }

    pub fn list_daily_quote_candidates(
        &self,
        user_id: &str,
    ) -> Result<Vec<(BookMark, String, Option<String>, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.ol_key, m.kind, m.cfi, m.color, m.note, m.snippet, m.chapter, m.created_at,
                    b.title, b.authors, b.cover_url
             FROM book_marks m
             JOIN books b ON b.ol_key = m.ol_key
             WHERE m.user_id = ?1
               AND m.kind = 'highlight'
               AND m.snippet IS NOT NULL
               AND length(trim(m.snippet)) > 10",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok((
                BookMark {
                    id: row.get(0)?,
                    ol_key: row.get(1)?,
                    kind: row.get(2)?,
                    cfi: row.get(3)?,
                    color: row.get(4)?,
                    note: row.get(5)?,
                    snippet: row.get(6)?,
                    chapter: row.get(7)?,
                    created_at: row.get(8)?,
                },
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })?;
        rows.collect()
    }

    pub fn get_book_shelf_status(&self, user_id: &str, ol_key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT status FROM book_shelf WHERE user_id = ?1 AND ol_key = ?2",
                params![user_id, ol_key],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn delete_book_shelf(&self, user_id: &str, ol_key: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM book_shelf WHERE user_id = ?1 AND ol_key = ?2",
            params![user_id, ol_key],
        )?;
        Ok(n > 0)
    }

    pub fn set_book_showcased(
        &self,
        user_id: &str,
        ol_key: &str,
        showcased: bool,
    ) -> std::result::Result<(), CollectionError> {
        let state = self
            .conn
            .query_row(
                "SELECT
                    CASE
                        WHEN s.status = 'read' THEN 'read'
                        WHEN p.percent >= 0.97 THEN 'read'
                        WHEN p.percent > 0 THEN 'reading'
                        WHEN p.percent IS NOT NULL THEN 'want'
                        ELSE COALESCE(s.status, 'want')
                    END,
                    COALESCE(s.showcased, 0)
                 FROM books b
                 LEFT JOIN book_shelf s ON s.ol_key = b.ol_key AND s.user_id = ?1
                 LEFT JOIN book_progress p ON p.ol_key = b.ol_key AND p.user_id = ?1
                 WHERE b.ol_key = ?2
                   AND (b.added_by = ?1 OR s.user_id IS NOT NULL)",
                params![user_id, ol_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0)),
            )
            .optional()?
            .ok_or(CollectionError::NotFound)?;
        let (effective_status, already_showcased) = state;
        if showcased && effective_status != "read" {
            return Err(CollectionError::ShowcaseRequiresCompletion);
        }
        if showcased && !already_showcased {
            self.ensure_showcase_slot(user_id)?;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let changed = self.conn.execute(
            "INSERT INTO book_shelf
                (id, user_id, ol_key, status, added_at, finished_at, showcased)
             SELECT
                ?1, ?2, b.ol_key,
                CASE
                    WHEN s.status = 'read' THEN 'read'
                    WHEN p.percent >= 0.97 THEN 'read'
                    WHEN p.percent > 0 THEN 'reading'
                    ELSE 'want'
                END,
                datetime('now'),
                CASE
                    WHEN s.status = 'read'
                        THEN COALESCE(s.finished_at, p.updated_at, datetime('now'))
                    WHEN p.percent >= 0.97 THEN p.updated_at
                    ELSE NULL
                END,
                ?4
             FROM books b
             LEFT JOIN book_shelf s ON s.ol_key = b.ol_key AND s.user_id = ?2
             LEFT JOIN book_progress p ON p.ol_key = b.ol_key AND p.user_id = ?2
             WHERE b.ol_key = ?3
               AND (b.added_by = ?2 OR s.user_id IS NOT NULL)
             ON CONFLICT(user_id, ol_key) DO UPDATE SET
                status = CASE
                    WHEN excluded.showcased = 1 THEN 'read'
                    ELSE book_shelf.status
                END,
                finished_at = CASE
                    WHEN excluded.showcased = 1
                        THEN COALESCE(book_shelf.finished_at, excluded.finished_at, datetime('now'))
                    ELSE book_shelf.finished_at
                END,
                showcased = excluded.showcased",
            params![id, user_id, ol_key, showcased as i64],
        )?;
        if changed == 0 {
            return Err(CollectionError::NotFound);
        }
        Ok(())
    }

    pub fn list_book_shelf(&self, user_id: &str) -> Result<Vec<BookShelfItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT b.ol_key, b.title, b.cover_url, b.authors, b.pages, b.subjects,
                    CASE
                        WHEN s.status = 'read' THEN 'read'
                        WHEN p.percent >= 0.97 THEN 'read'
                        WHEN p.percent > 0 THEN 'reading'
                        WHEN p.percent IS NOT NULL THEN 'want'
                        ELSE COALESCE(s.status, 'want')
                    END AS effective_status,
                    CASE
                        WHEN s.status = 'read' THEN COALESCE(s.finished_at, p.updated_at)
                        WHEN p.percent >= 0.97 THEN COALESCE(s.finished_at, p.updated_at)
                        ELSE NULL
                    END AS effective_finished_at,
                    p.percent,
                    COALESCE(s.showcased, 0)
             FROM books b
             LEFT JOIN book_shelf s ON s.ol_key = b.ol_key AND s.user_id = ?1
             LEFT JOIN book_progress p ON p.ol_key = b.ol_key AND p.user_id = ?1
             WHERE b.added_by = ?1 OR s.user_id IS NOT NULL
             ORDER BY COALESCE(s.added_at, b.added_at) DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(BookShelfItem {
                ol_key: row.get(0)?,
                title: row.get(1)?,
                cover_url: row.get(2)?,
                authors: row.get(3)?,
                pages: row.get(4)?,
                subjects: row.get(5)?,
                status: row.get(6)?,
                finished_at: row.get(7)?,
                percent: row.get(8)?,
                showcased: row.get::<_, i64>(9)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn get_book_goal(&self, user_id: &str) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT book_goal FROM users WHERE id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .optional()
            .map(|v| v.flatten())
    }

    pub fn set_book_goal(&self, user_id: &str, goal: Option<i64>) -> Result<()> {
        self.conn.execute(
            "UPDATE users SET book_goal = ?1 WHERE id = ?2",
            params![goal, user_id],
        )?;
        Ok(())
    }

    pub fn count_book_shelf_read(&self, user_id: &str) -> Result<(i64, i64)> {
        let year = chrono::Utc::now().format("%Y").to_string();
        let items = self.list_book_shelf(user_id)?;
        let total = items.iter().filter(|item| item.status == "read").count() as i64;
        let this_year = items
            .iter()
            .filter(|item| {
                item.status == "read"
                    && item
                        .finished_at
                        .as_deref()
                        .is_some_and(|finished| finished.starts_with(&year))
            })
            .count() as i64;
        Ok((total, this_year))
    }

    pub fn update_book_enrichment(
        &self,
        ol_key: &str,
        pages: Option<i64>,
        subjects: Option<&str>,
        isbn: Option<&str>,
        publisher: Option<&str>,
        rating: Option<f64>,
        rating_count: Option<i64>,
        series: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET
                pages = COALESCE(?1, pages),
                subjects = COALESCE(?2, subjects),
                isbn = COALESCE(?3, isbn),
                publisher = COALESCE(?4, publisher),
                rating = COALESCE(?5, rating),
                rating_count = COALESCE(?6, rating_count),
                series = COALESCE(?7, series),
                description = COALESCE(NULLIF(description, ''), ?8, description),
                enriched_at = datetime('now')
             WHERE ol_key = ?9",
            params![
                pages,
                subjects,
                isbn,
                publisher,
                rating,
                rating_count,
                series,
                description,
                ol_key
            ],
        )?;
        Ok(())
    }

    pub fn find_episode_by_id(&self, id: &str) -> Result<Option<Episode>> {
        self.conn
            .query_row(
                "SELECT id, media_id, season, episode, title, file_path, duration, status, source_name, intro_start, intro_end, credits_start
                 FROM episodes WHERE id = ?1",
                params![id],
                map_episode,
            )
            .optional()
    }

    pub fn find_next_episode(
        &self,
        media_id: &str,
        season: i32,
        episode: i32,
    ) -> Result<Option<Episode>> {
        self.conn
            .query_row(
                "SELECT id, media_id, season, episode, title, file_path, duration, status, source_name, intro_start, intro_end, credits_start
                 FROM episodes
                 WHERE media_id = ?1
                   AND file_path IS NOT NULL
                   AND (season > ?2 OR (season = ?2 AND episode > ?3))
                 ORDER BY season ASC, episode ASC
                 LIMIT 1",
                params![media_id, season, episode],
                map_episode,
            )
            .optional()
    }

    pub fn find_or_create_episode(
        &self,
        media_id: &str,
        season: i32,
        episode: i32,
    ) -> Result<Episode> {
        if let Some(existing) = self
            .conn
            .query_row(
                "SELECT id, media_id, season, episode, title, file_path, duration, status, source_name, intro_start, intro_end, credits_start
                 FROM episodes WHERE media_id = ?1 AND season = ?2 AND episode = ?3",
                params![media_id, season, episode],
                map_episode,
            )
            .optional()?
        {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO episodes (id, media_id, season, episode, title, file_path, duration, status)
             VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, 'pending')",
            params![id, media_id, season, episode],
        )?;
        Ok(Episode {
            id,
            media_id: media_id.to_string(),
            season,
            episode,
            title: None,
            file_path: None,
            duration: None,
            status: "pending".into(),
            source_name: None,
            intro_start: None,
            intro_end: None,
            credits_start: None,
        })
    }

    pub fn list_episodes_for_media(&self, media_id: &str) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, media_id, season, episode, title, file_path, duration, status, source_name, intro_start, intro_end, credits_start
             FROM episodes WHERE media_id = ?1 ORDER BY season, episode",
        )?;
        let rows = stmt.query_map(params![media_id], map_episode)?;
        rows.collect()
    }

    pub fn create_download(&self, d: &Download) -> Result<()> {
        self.conn.execute(
            "INSERT INTO downloads (id, media_id, episode_id, magnet, qbit_hash, status, save_path, title, requested_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                d.id, d.media_id, d.episode_id, d.magnet, d.qbit_hash, d.status,
                d.save_path, d.title, d.requested_by, d.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn set_download_hash(&self, id: &str, hash: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE downloads SET qbit_hash = ?1 WHERE id = ?2",
            params![hash, id],
        )?;
        Ok(())
    }

    pub fn set_download_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE downloads SET status = ?1,
                 completed_at = CASE WHEN ?1 IN ('complete', 'error', 'cancelled') THEN datetime('now') ELSE completed_at END
             WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn list_downloads(&self) -> Result<Vec<Download>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, media_id, episode_id, magnet, qbit_hash, status, save_path, title, requested_by, created_at, completed_at
             FROM downloads
             WHERE status NOT IN ('complete', 'cancelled')
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_download)?;
        rows.collect()
    }

    pub fn list_active_downloads(&self) -> Result<Vec<Download>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, media_id, episode_id, magnet, qbit_hash, status, save_path, title, requested_by, created_at, completed_at
             FROM downloads WHERE status IN ('queued', 'downloading') ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_download)?;
        rows.collect()
    }

    pub fn downloads_for_media(&self, media_id: &str) -> Result<Vec<Download>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, media_id, episode_id, magnet, qbit_hash, status, save_path, title, requested_by, created_at, completed_at
             FROM downloads WHERE media_id = ?1",
        )?;
        let rows = stmt.query_map(params![media_id], map_download)?;
        rows.collect()
    }

    pub fn find_download(&self, id: &str) -> Result<Option<Download>> {
        self.conn
            .query_row(
                "SELECT id, media_id, episode_id, magnet, qbit_hash, status, save_path, title, requested_by, created_at, completed_at
                 FROM downloads WHERE id = ?1",
                params![id],
                map_download,
            )
            .optional()
    }

    pub fn delete_download(&self, id: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    pub fn purge_dead_downloads(&self) -> Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM downloads WHERE status IN ('error', 'cancelled')",
            [],
        )?;
        Ok(n)
    }

    pub fn update_media_ready(&self, id: &str, file_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE media SET file_path = ?1, status = 'ready' WHERE id = ?2",
            params![file_path, id],
        )?;
        Ok(())
    }

    pub fn update_media_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE media SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn update_episode_ready(&self, id: &str, file_path: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let media_id = tx
            .query_row(
                "SELECT media_id FROM episodes WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        tx.execute(
            "UPDATE episodes SET file_path = ?1, status = 'ready', ready_at = datetime('now') WHERE id = ?2",
            params![file_path, id],
        )?;
        if let Some(media_id) = media_id {
            refresh_series_for_media(&tx, &media_id)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_episode_duration(&self, id: &str, duration: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET duration = ?1 WHERE id = ?2",
            params![duration, id],
        )?;
        Ok(())
    }

    pub fn update_episode_status(&self, id: &str, status: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let media_id = tx
            .query_row(
                "SELECT media_id FROM episodes WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        tx.execute(
            "UPDATE episodes SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        if let Some(media_id) = media_id {
            refresh_series_for_media(&tx, &media_id)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn clear_episode_file(&self, id: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let media_id = tx
            .query_row(
                "SELECT media_id FROM episodes WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        tx.execute(
            "UPDATE episodes SET file_path = NULL, duration = NULL, status = 'pending' WHERE id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM subtitles WHERE owner_id = ?1", params![id])?;
        if let Some(media_id) = media_id {
            refresh_series_for_media(&tx, &media_id)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_media_duration(&self, id: &str, duration: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE media SET duration = ?1 WHERE id = ?2",
            params![duration, id],
        )?;
        Ok(())
    }

    pub fn update_episode_source(&self, id: &str, source_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET source_name = ?1 WHERE id = ?2",
            params![source_name, id],
        )?;
        Ok(())
    }

    pub fn update_episode_still(&self, id: &str, url: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET still_url = ?1 WHERE id = ?2",
            params![url, id],
        )?;
        Ok(())
    }

    pub fn update_media_source(&self, id: &str, source_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE media SET source_name = ?1 WHERE id = ?2",
            params![source_name, id],
        )?;
        Ok(())
    }

    pub fn create_subtitle(&self, sub: &crate::models::Subtitle) -> Result<()> {
        self.conn.execute(
            "INSERT INTO subtitles (id, owner_id, language, label, format, file_path, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                sub.id,
                sub.owner_id,
                sub.language,
                sub.label,
                sub.format,
                sub.file_path,
                sub.is_default as i32
            ],
        )?;
        Ok(())
    }

    pub fn find_subtitle_by_id(&self, id: &str) -> Result<Option<crate::models::Subtitle>> {
        self.conn
            .query_row(
                "SELECT id, owner_id, language, label, format, file_path, is_default
             FROM subtitles WHERE id = ?1",
                params![id],
                |r| {
                    let def: i64 = r.get(6)?;
                    Ok(crate::models::Subtitle {
                        id: r.get(0)?,
                        owner_id: r.get(1)?,
                        language: r.get(2)?,
                        label: r.get(3)?,
                        format: r.get(4)?,
                        file_path: r.get(5)?,
                        is_default: def != 0,
                        media_id: None,
                    })
                },
            )
            .optional()
    }

    pub fn subtitle_paths_for_media(&self, media_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_path FROM subtitles
             WHERE owner_id = ?1
                OR owner_id IN (SELECT id FROM episodes WHERE media_id = ?1)",
        )?;
        let rows = stmt.query_map(params![media_id], |r| r.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn subtitle_paths_for_owner(&self, owner_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file_path FROM subtitles WHERE owner_id = ?1")?;
        let rows = stmt.query_map(params![owner_id], |r| r.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn list_subtitles_for_owner(&self, owner_id: &str) -> Result<Vec<crate::models::Subtitle>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner_id, language, label, format, file_path, is_default
             FROM subtitles WHERE owner_id = ?1 ORDER BY is_default DESC, language",
        )?;
        let rows = stmt.query_map(params![owner_id], |r| {
            let def: i64 = r.get(6)?;
            Ok(crate::models::Subtitle {
                id: r.get(0)?,
                owner_id: r.get(1)?,
                language: r.get(2)?,
                label: r.get(3)?,
                format: r.get(4)?,
                file_path: r.get(5)?,
                is_default: def != 0,
                media_id: None,
            })
        })?;
        rows.collect()
    }

    pub fn subtitle_exists(&self, owner_id: &str, language: &str, label: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM subtitles WHERE owner_id = ?1 AND language = ?2 AND label = ?3",
            params![owner_id, language, label],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn find_subtitle_path(&self, id: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT file_path FROM subtitles WHERE id = ?1",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .optional()
    }

    pub fn find_subtitle_owner(&self, id: &str) -> Result<Option<(String, String)>> {
        self.conn
            .query_row(
                "SELECT owner_id, file_path FROM subtitles WHERE id = ?1",
                params![id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
    }

    pub fn delete_subtitle(&self, id: &str) -> Result<Option<String>> {
        let path = self
            .conn
            .query_row(
                "SELECT file_path FROM subtitles WHERE id = ?1",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        if path.is_some() {
            self.conn
                .execute("DELETE FROM subtitles WHERE id = ?1", params![id])?;
        }
        Ok(path)
    }

    pub fn upsert_progress(
        &self,
        user_id: &str,
        media_id: &str,
        episode_id: Option<&str>,
        position: i64,
        duration: i64,
    ) -> Result<()> {
        let completed = if duration > 0 && position * 100 / duration >= 90 {
            1
        } else {
            0
        };
        let id = uuid::Uuid::new_v4().to_string();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO watch_progress (id, user_id, media_id, episode_id, position, duration, completed, dismissed, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, datetime('now'))
             ON CONFLICT(user_id, media_id, COALESCE(episode_id, '')) DO UPDATE SET
                position = excluded.position,
                duration = excluded.duration,
                completed = excluded.completed,
                dismissed = 0,
                updated_at = datetime('now')",
            params![id, user_id, media_id, episode_id, position, duration, completed],
        )?;
        sync_collection_from_watch_in(&tx, user_id, media_id, completed != 0)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_progress(
        &self,
        user_id: &str,
        media_id: &str,
        episode_id: Option<&str>,
    ) -> Result<Option<crate::models::WatchProgress>> {
        let sql = if episode_id.is_some() {
            "SELECT id, user_id, media_id, episode_id, position, duration, completed, dismissed, updated_at
             FROM watch_progress WHERE user_id = ?1 AND media_id = ?2 AND episode_id = ?3"
        } else {
            "SELECT id, user_id, media_id, episode_id, position, duration, completed, dismissed, updated_at
             FROM watch_progress WHERE user_id = ?1 AND media_id = ?2 AND episode_id IS NULL"
        };
        match episode_id {
            Some(eid) => self
                .conn
                .query_row(sql, params![user_id, media_id, eid], map_progress)
                .optional(),
            None => self
                .conn
                .query_row(sql, params![user_id, media_id], map_progress)
                .optional(),
        }
    }

    pub fn list_progress_for_media(
        &self,
        user_id: &str,
        media_id: &str,
    ) -> Result<Vec<crate::models::WatchProgress>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_id, media_id, episode_id, position, duration, completed, dismissed, updated_at
             FROM watch_progress WHERE user_id = ?1 AND media_id = ?2",
        )?;
        let rows = stmt.query_map(params![user_id, media_id], map_progress)?;
        rows.collect()
    }

    pub fn list_continue_watching(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<crate::models::ContinueItem>> {
        let mut stmt = self.conn.prepare(
            "WITH ranked AS (
                SELECT wp.*,
                    ROW_NUMBER() OVER (PARTITION BY wp.media_id ORDER BY wp.updated_at DESC) AS rn
                FROM watch_progress wp
                WHERE wp.user_id = ?1
                  AND wp.dismissed = 0
                  AND (wp.completed = 1 OR wp.position > 30)
            )
            SELECT
                m.id, m.title, m.media_type, m.is_anime, m.tmdb_id, m.poster_url,
                e.id, e.season, e.episode, e.title,
                ranked.position, ranked.duration, ranked.updated_at, ranked.completed,
                e.still_url
             FROM ranked
             JOIN media m ON m.id = ranked.media_id
             LEFT JOIN episodes e ON e.id = ranked.episode_id
             WHERE ranked.rn = 1
               AND (
                    (ranked.completed = 1 AND ranked.episode_id IS NOT NULL)
                 OR (ranked.episode_id IS NULL AND m.status = 'ready' AND m.file_path IS NOT NULL)
                 OR (ranked.episode_id IS NOT NULL AND e.status = 'ready' AND e.file_path IS NOT NULL)
               )
             ORDER BY ranked.updated_at DESC
             LIMIT ?2",
        )?;
        let raw: Vec<(crate::models::ContinueItem, bool)> = stmt
            .query_map(params![user_id, limit * 2], |r| {
                let is_anime: i64 = r.get(3)?;
                let completed: i64 = r.get(13)?;
                Ok((
                    crate::models::ContinueItem {
                        media_id: r.get(0)?,
                        media_title: r.get(1)?,
                        media_type: r.get(2)?,
                        is_anime: is_anime != 0,
                        tmdb_id: r.get(4)?,
                        poster_url: r.get(5)?,
                        episode_id: r.get(6)?,
                        episode_season: r.get(7)?,
                        episode_number: r.get(8)?,
                        episode_title: r.get(9)?,
                        position: r.get(10)?,
                        duration: r.get(11)?,
                        updated_at: r.get(12)?,
                        episode_still_url: r.get(14)?,
                    },
                    completed != 0,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;

        let mut out = Vec::with_capacity(raw.len());
        for (mut item, completed) in raw {
            if !completed {
                out.push(item);
                continue;
            }
            let (Some(s), Some(e)) = (item.episode_season, item.episode_number) else {
                continue;
            };
            let Some(next) = self.find_next_episode(&item.media_id, s, e)? else {
                continue;
            };
            item.episode_id = Some(next.id);
            item.episode_season = Some(next.season);
            item.episode_number = Some(next.episode);
            item.episode_title = next.title;
            item.episode_still_url = None;
            item.position = 0;
            item.duration = next.duration.unwrap_or(0);
            out.push(item);
            if out.len() as i64 >= limit {
                break;
            }
        }
        Ok(out)
    }

    pub fn list_progress_summary(
        &self,
        user_id: &str,
    ) -> Result<Vec<crate::models::ProgressSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT media_id, position, duration FROM (
                SELECT media_id, position, duration,
                    ROW_NUMBER() OVER (PARTITION BY media_id ORDER BY updated_at DESC) AS rn
                FROM watch_progress
                WHERE user_id = ?1 AND completed = 0 AND position > 30
            ) WHERE rn = 1",
        )?;
        let rows = stmt.query_map(params![user_id], |r| {
            Ok(crate::models::ProgressSummary {
                media_id: r.get(0)?,
                position: r.get(1)?,
                duration: r.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn dismiss_progress(&self, user_id: &str, media_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE watch_progress SET dismissed = 1 WHERE user_id = ?1 AND media_id = ?2",
            params![user_id, media_id],
        )?;
        Ok(())
    }

    pub fn mark_watched(
        &self,
        user_id: &str,
        media_id: &str,
        episode_id: Option<&str>,
        duration: i64,
        watched: bool,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        if !watched {
            let sql = if episode_id.is_some() {
                "DELETE FROM watch_progress WHERE user_id = ?1 AND media_id = ?2 AND episode_id = ?3"
            } else {
                "DELETE FROM watch_progress WHERE user_id = ?1 AND media_id = ?2 AND episode_id IS NULL"
            };
            match episode_id {
                Some(eid) => {
                    tx.execute(sql, params![user_id, media_id, eid])?;
                }
                None => {
                    tx.execute(sql, params![user_id, media_id])?;
                }
            };
            sync_collection_from_watch_in(&tx, user_id, media_id, false)?;
            tx.commit()?;
            return Ok(());
        }
        let dur = duration.max(1);
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO watch_progress (id, user_id, media_id, episode_id, position, duration, completed, dismissed, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1, 0, datetime('now'))
             ON CONFLICT(user_id, media_id, COALESCE(episode_id, '')) DO UPDATE SET
                position = excluded.position,
                duration = excluded.duration,
                completed = 1,
                updated_at = datetime('now')",
            params![id, user_id, media_id, episode_id, dur],
        )?;
        sync_collection_from_watch_in(&tx, user_id, media_id, true)?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_episode_intro(&self, id: &str, start: i64, end: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET intro_start = ?1, intro_end = ?2 WHERE id = ?3",
            params![start, end, id],
        )?;
        Ok(())
    }

    pub fn update_episode_credits(&self, id: &str, start: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET credits_start = ?1 WHERE id = ?2",
            params![start, id],
        )?;
        Ok(())
    }

    pub fn list_seasons_for_media(&self, media_id: &str) -> Result<Vec<i32>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT season FROM episodes
             WHERE media_id = ?1 AND status = 'ready' AND file_path IS NOT NULL
             ORDER BY season",
        )?;
        let rows = stmt.query_map(params![media_id], |r| r.get::<_, i32>(0))?;
        rows.collect()
    }

    pub fn list_ready_episodes_without_intro(
        &self,
        media_id: &str,
        season: i32,
        limit: i64,
    ) -> Result<Vec<(String, String, i32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, episode FROM episodes
             WHERE media_id = ?1 AND season = ?2 AND status = 'ready'
               AND file_path IS NOT NULL AND intro_start IS NULL
             ORDER BY episode LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![media_id, season, limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i32>(2)?,
            ))
        })?;
        rows.collect()
    }

    pub fn create_party(
        &self,
        id: &str,
        code: &str,
        host: &str,
        media_id: &str,
        episode_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO watch_sessions (id, code, host_user_id, media_id, episode_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, code, host, media_id, episode_id],
        )?;
        Ok(())
    }

    pub fn find_party_by_code(
        &self,
        code: &str,
    ) -> Result<Option<(String, String, Option<String>, String)>> {
        self.conn.query_row(
            "SELECT id, media_id, episode_id, host_user_id FROM watch_sessions WHERE code = ?1 AND active = 1",
            params![code],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).optional()
    }

    pub fn deactivate_party(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE watch_sessions SET active = 0 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn update_party_episode(&self, code: &str, episode_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE watch_sessions SET episode_id = ?1 WHERE code = ?2 AND active = 1",
            params![episode_id, code],
        )?;
        Ok(())
    }

    pub fn create_clip(&self, clip: &crate::models::Clip) -> Result<()> {
        self.conn.execute(
            "INSERT INTO clips (id, media_id, episode_id, start_sec, end_sec, subtitle_id, file_path, file_size, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                clip.id, clip.media_id, clip.episode_id,
                clip.start_sec, clip.end_sec, clip.subtitle_id,
                clip.file_path, clip.file_size, clip.created_by,
            ],
        )?;
        Ok(())
    }

    pub fn find_clip_by_id(&self, id: &str) -> Result<Option<crate::models::Clip>> {
        self.conn
            .query_row(
                "SELECT id, media_id, episode_id, start_sec, end_sec, subtitle_id,
                    file_path, file_size, created_by, created_at
             FROM clips WHERE id = ?1",
                params![id],
                map_clip,
            )
            .optional()
    }

    pub fn list_clips(&self, limit: i64) -> Result<Vec<crate::models::Clip>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, media_id, episode_id, start_sec, end_sec, subtitle_id,
                    file_path, file_size, created_by, created_at
             FROM clips ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_clip)?;
        rows.collect()
    }

    pub fn delete_clip(&self, id: &str) -> Result<Option<String>> {
        let path: Option<String> = self
            .conn
            .query_row(
                "SELECT file_path FROM clips WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        if path.is_some() {
            self.conn
                .execute("DELETE FROM clips WHERE id = ?1", params![id])?;
        }
        Ok(path)
    }
}

fn map_clip(row: &rusqlite::Row) -> Result<crate::models::Clip> {
    Ok(crate::models::Clip {
        id: row.get(0)?,
        media_id: row.get(1)?,
        episode_id: row.get(2)?,
        start_sec: row.get(3)?,
        end_sec: row.get(4)?,
        subtitle_id: row.get(5)?,
        file_path: row.get(6)?,
        file_size: row.get(7)?,
        created_by: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn map_progress(row: &rusqlite::Row) -> Result<crate::models::WatchProgress> {
    let completed: i64 = row.get(6)?;
    let dismissed: i64 = row.get(7)?;
    Ok(crate::models::WatchProgress {
        id: row.get(0)?,
        user_id: row.get(1)?,
        media_id: row.get(2)?,
        episode_id: row.get(3)?,
        position: row.get(4)?,
        duration: row.get(5)?,
        completed: completed != 0,
        dismissed: dismissed != 0,
        updated_at: row.get(8)?,
    })
}

fn map_user(row: &rusqlite::Row) -> Result<User> {
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        email: row.get(2)?,
        password_hash: row.get(3)?,
        role: row.get(4)?,
        created_at: row.get(5)?,
        approved_at: row.get(6)?,
        approved_by: row.get(7)?,
    })
}

fn map_manga(row: &rusqlite::Row) -> Result<Manga> {
    let restricted: i64 = row.get(9)?;
    Ok(Manga {
        id: row.get(0)?,
        md_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        cover_url: row.get(4)?,
        year: row.get(5)?,
        status: row.get(6)?,
        added_by: row.get(7)?,
        added_at: row.get(8)?,
        restricted: restricted != 0,
        restricted_langs: row.get(10)?,
        comick_hid: row.get(11)?,
        anilist_id: row.get(12)?,
        mal_id: row.get(13)?,
        links_json: row.get(14)?,
        tags: row.get(15)?,
        demographic: row.get(16)?,
        content_rating: row.get(17)?,
        original_language: row.get(18)?,
        authors: row.get(19)?,
        artists: row.get(20)?,
        score: row.get(21)?,
        score_count: row.get(22)?,
        follow_count: row.get(23)?,
        last_chapter: row.get(24)?,
        enriched_at: row.get(25)?,
    })
}

fn map_book(row: &rusqlite::Row) -> Result<Book> {
    Ok(Book {
        id: row.get(0)?,
        ol_key: row.get(1)?,
        title: row.get(2)?,
        authors: row.get(3)?,
        description: row.get(4)?,
        cover_url: row.get(5)?,
        year: row.get(6)?,
        language: row.get(7)?,
        file_path: row.get(8)?,
        ext: row.get(9)?,
        status: row.get(10)?,
        added_by: row.get(11)?,
        added_at: row.get(12)?,
        pages: row.get(13)?,
        subjects: row.get(14)?,
        isbn: row.get(15)?,
        publisher: row.get(16)?,
        rating: row.get(17)?,
        rating_count: row.get(18)?,
        enriched_at: row.get(19)?,
        series: row.get(20)?,
    })
}

fn get_collection_item_from(
    conn: &Connection,
    user_id: &str,
    kind: &str,
    tmdb_id: i64,
) -> Result<Option<CollectionItem>> {
    conn.query_row(
        "SELECT id, tmdb_id, kind, title, year, poster_url, backdrop_url, status,
                showcased, added_at, updated_at, completed_at
         FROM collection_items
         WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
        params![user_id, kind, tmdb_id],
        map_collection_item,
    )
    .optional()
}

fn ensure_showcase_slot_from(
    conn: &Connection,
    user_id: &str,
) -> std::result::Result<(), CollectionError> {
    let count: i64 = conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM collection_items
             WHERE user_id = ?1 AND showcased = 1)
            +
            (SELECT COUNT(*)
             FROM book_shelf s
             JOIN books b ON b.ol_key = s.ol_key
             WHERE s.user_id = ?1 AND s.showcased = 1)",
        params![user_id],
        |row| row.get(0),
    )?;
    if count >= 5 {
        return Err(CollectionError::ShowcaseLimit);
    }
    Ok(())
}

fn reconcile_collection_alias(
    conn: &Connection,
    user_id: &str,
    canonical_kind: &str,
    tmdb_id: i64,
) -> Result<()> {
    let alias_kind = match canonical_kind {
        "tv" => "anime",
        "anime" => "tv",
        _ => return Ok(()),
    };
    let canonical = get_collection_item_from(conn, user_id, canonical_kind, tmdb_id)?;
    let alias = get_collection_item_from(conn, user_id, alias_kind, tmdb_id)?;

    match (canonical, alias) {
        (None, Some(alias)) => {
            conn.execute(
                "UPDATE collection_items
                 SET kind = ?1
                 WHERE user_id = ?2 AND kind = ?3 AND tmdb_id = ?4",
                params![canonical_kind, user_id, alias.kind, tmdb_id],
            )?;
        }
        (Some(canonical), Some(alias)) => {
            let canonical_auto: i64 = conn.query_row(
                "SELECT auto_completed FROM collection_items WHERE id = ?1",
                params![canonical.id],
                |row| row.get(0),
            )?;
            let alias_auto: i64 = conn.query_row(
                "SELECT auto_completed FROM collection_items WHERE id = ?1",
                params![alias.id],
                |row| row.get(0),
            )?;
            let status = if canonical.status == "completed" || alias.status == "completed" {
                "completed"
            } else if canonical.status == "in_progress" || alias.status == "in_progress" {
                "in_progress"
            } else {
                "planned"
            };
            let manual_completion = (canonical.status == "completed" && canonical_auto == 0)
                || (alias.status == "completed" && alias_auto == 0);
            let auto_completed = if status == "completed" && !manual_completion {
                1
            } else {
                0
            };
            let showcased = status == "completed" && (canonical.showcased || alias.showcased);
            let completed_at = if status == "completed" {
                earliest_optional(
                    canonical.completed_at.as_deref(),
                    alias.completed_at.as_deref(),
                )
            } else {
                None
            };
            let added_at = canonical.added_at.min(alias.added_at);
            let updated_at = canonical.updated_at.max(alias.updated_at);
            let year = canonical.year.or(alias.year);
            let poster_url = canonical.poster_url.or(alias.poster_url);
            let backdrop_url = canonical.backdrop_url.or(alias.backdrop_url);

            conn.execute(
                "UPDATE collection_items SET
                    year = ?1,
                    poster_url = ?2,
                    backdrop_url = ?3,
                    status = ?4,
                    showcased = ?5,
                    added_at = ?6,
                    updated_at = ?7,
                    completed_at = ?8,
                    auto_completed = ?9
                 WHERE id = ?10",
                params![
                    year,
                    poster_url,
                    backdrop_url,
                    status,
                    showcased as i64,
                    added_at,
                    updated_at,
                    completed_at,
                    auto_completed,
                    canonical.id
                ],
            )?;
            conn.execute(
                "DELETE FROM collection_items WHERE id = ?1",
                params![alias.id],
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn earliest_optional(left: Option<&str>, right: Option<&str>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right).to_string()),
        (Some(value), None) | (None, Some(value)) => Some(value.to_string()),
        (None, None) => None,
    }
}

fn sync_collection_from_watch_in(
    conn: &Connection,
    user_id: &str,
    media_id: &str,
    completed: bool,
) -> Result<()> {
    let identity = conn
        .query_row(
            "SELECT CASE
                WHEN media_type = 'movie' THEN 'movie'
                WHEN is_anime = 1 THEN 'anime'
                ELSE 'tv'
             END,
             tmdb_id,
             media_type
             FROM media
             WHERE id = ?1 AND tmdb_id IS NOT NULL",
            params![media_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((canonical, tmdb_id, media_type)) = identity else {
        return Ok(());
    };

    reconcile_collection_alias(conn, user_id, &canonical, tmdb_id)?;
    let current_auto = conn
        .query_row(
            "SELECT auto_completed
             FROM collection_items
             WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
            params![user_id, canonical, tmdb_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let active: i64 = conn.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM watch_progress
            WHERE user_id = ?1
              AND media_id = ?2
              AND (completed = 1 OR position > 30)
         )",
        params![user_id, media_id],
        |row| row.get(0),
    )?;
    if active == 0 && current_auto == 0 && !completed {
        return Ok(());
    }

    let finished = if media_type == "tv" {
        series_completed_at(conn, user_id, media_id)?
    } else {
        None
    };
    let status = if (media_type == "movie" && completed) || finished.is_some() {
        "completed"
    } else {
        "in_progress"
    };
    let auto_completed = (media_type == "tv" && finished.is_some()) as i64;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO collection_items
            (id, user_id, tmdb_id, kind, title, year, poster_url, backdrop_url, status,
             showcased, auto_completed, added_at, updated_at, completed_at)
         SELECT
            ?1, ?2, m.tmdb_id,
            CASE
                WHEN m.media_type = 'movie' THEN 'movie'
                WHEN m.is_anime = 1 THEN 'anime'
                ELSE 'tv'
            END,
            m.title, CAST(m.year AS TEXT), m.poster_url, m.backdrop_url,
            ?4, 0, ?5, datetime('now'), datetime('now'),
            CASE WHEN ?4 = 'completed' THEN COALESCE(?6, datetime('now')) ELSE NULL END
         FROM media m
         WHERE m.id = ?3 AND m.tmdb_id IS NOT NULL
         ON CONFLICT(user_id, kind, tmdb_id) DO UPDATE SET
            title = excluded.title,
            year = excluded.year,
            poster_url = excluded.poster_url,
            backdrop_url = excluded.backdrop_url,
            status = CASE
                WHEN collection_items.status = 'completed'
                 AND collection_items.auto_completed = 0 THEN 'completed'
                ELSE excluded.status
            END,
            showcased = CASE
                WHEN collection_items.status = 'completed'
                 AND collection_items.auto_completed = 0
                    THEN collection_items.showcased
                WHEN excluded.status != 'completed' THEN 0
                ELSE collection_items.showcased
            END,
            auto_completed = CASE
                WHEN collection_items.status = 'completed'
                 AND collection_items.auto_completed = 0 THEN 0
                ELSE excluded.auto_completed
            END,
            updated_at = CASE
                WHEN collection_items.status = 'completed'
                 AND collection_items.auto_completed = 0
                    THEN collection_items.updated_at
                WHEN collection_items.status = excluded.status
                 AND collection_items.auto_completed = excluded.auto_completed
                    THEN collection_items.updated_at
                ELSE datetime('now')
            END,
            completed_at = CASE
                WHEN collection_items.status = 'completed'
                 AND collection_items.auto_completed = 0
                    THEN collection_items.completed_at
                WHEN excluded.status = 'completed'
                    THEN COALESCE(
                        collection_items.completed_at,
                        excluded.completed_at,
                        datetime('now')
                    )
                ELSE NULL
            END",
        params![
            id,
            user_id,
            media_id,
            status,
            auto_completed,
            finished.as_deref()
        ],
    )?;
    Ok(())
}

fn series_completed_at(conn: &Connection, user_id: &str, media_id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT COUNT(*), COUNT(wp.id), MAX(wp.updated_at)
         FROM episodes e
         LEFT JOIN watch_progress wp
            ON wp.user_id = ?1
           AND wp.media_id = e.media_id
           AND wp.episode_id = e.id
           AND wp.completed = 1
         WHERE e.media_id = ?2
           AND e.status = 'ready'
           AND e.file_path IS NOT NULL",
        params![user_id, media_id],
        |row| {
            let total: i64 = row.get(0)?;
            let done: i64 = row.get(1)?;
            let finished: Option<String> = row.get(2)?;
            if total > 0 && total == done {
                Ok(finished)
            } else {
                Ok(None)
            }
        },
    )
}

fn refresh_series_for_media(conn: &Connection, media_id: &str) -> Result<usize> {
    let tmdb_id = conn
        .query_row(
            "SELECT tmdb_id
             FROM media
             WHERE id = ?1 AND media_type = 'tv' AND tmdb_id IS NOT NULL",
            params![media_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(tmdb_id) = tmdb_id else {
        return Ok(0);
    };

    let rows = {
        let mut stmt = conn.prepare(
            "SELECT user_id, kind, status, auto_completed, completed_at
             FROM collection_items
             WHERE tmdb_id = ?1 AND kind IN ('tv', 'anime')",
        )?;
        let found = stmt.query_map(params![tmdb_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        found.collect::<Result<Vec<_>>>()?
    };

    let mut changed = 0;
    for (user_id, kind, status, auto, completed_at) in rows {
        if status == "completed" && auto == 0 {
            continue;
        }

        match series_completed_at(conn, &user_id, media_id)? {
            Some(finished) => {
                if status == "completed" && auto != 0 && completed_at.is_some() {
                    continue;
                }
                changed += conn.execute(
                    "UPDATE collection_items SET
                        status = 'completed',
                        auto_completed = 1,
                        completed_at = COALESCE(completed_at, ?4),
                        updated_at = datetime('now')
                     WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
                    params![user_id, kind, tmdb_id, finished],
                )?;
            }
            None if auto != 0 => {
                changed += conn.execute(
                    "UPDATE collection_items SET
                        status = 'in_progress',
                        showcased = 0,
                        auto_completed = 0,
                        completed_at = NULL,
                        updated_at = datetime('now')
                     WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
                    params![user_id, kind, tmdb_id],
                )?;
            }
            None => {}
        }
    }
    Ok(changed)
}

fn map_collection_item(row: &rusqlite::Row) -> Result<CollectionItem> {
    Ok(CollectionItem {
        id: row.get(0)?,
        tmdb_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        year: row.get(4)?,
        poster_url: row.get(5)?,
        backdrop_url: row.get(6)?,
        status: row.get(7)?,
        showcased: row.get::<_, i64>(8)? != 0,
        added_at: row.get(9)?,
        updated_at: row.get(10)?,
        completed_at: row.get(11)?,
    })
}

fn map_manga_progress(row: &rusqlite::Row) -> Result<MangaProgress> {
    Ok(MangaProgress {
        md_id: row.get(0)?,
        chapter_id: row.get(1)?,
        chapter: row.get(2)?,
        page: row.get(3)?,
        pages: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_media(row: &rusqlite::Row) -> Result<Media> {
    let anime: i64 = row.get(13).unwrap_or(0);
    let source_name: Option<String> = row.get(14).ok();
    let activity_at: Option<String> = row.get(15).ok();
    let activity_label: Option<String> = row.get(16).ok();
    Ok(Media {
        id: row.get(0)?,
        tmdb_id: row.get(1)?,
        media_type: row.get(2)?,
        title: row.get(3)?,
        year: row.get(4)?,
        overview: row.get(5)?,
        poster_url: row.get(6)?,
        backdrop_url: row.get(7)?,
        file_path: row.get(8)?,
        duration: row.get(9)?,
        status: row.get(10)?,
        added_by: row.get(11)?,
        added_at: row.get(12)?,
        activity_at,
        activity_label,
        is_anime: anime != 0,
        source_name,
    })
}

fn map_episode(row: &rusqlite::Row) -> Result<Episode> {
    let source_name: Option<String> = row.get(8).ok();
    let intro_start: Option<i64> = row.get(9).ok();
    let intro_end: Option<i64> = row.get(10).ok();
    let credits_start: Option<i64> = row.get(11).ok();
    Ok(Episode {
        id: row.get(0)?,
        media_id: row.get(1)?,
        season: row.get(2)?,
        episode: row.get(3)?,
        title: row.get(4)?,
        file_path: row.get(5)?,
        duration: row.get(6)?,
        status: row.get(7)?,
        source_name,
        intro_start,
        intro_end,
        credits_start,
    })
}

fn map_download(row: &rusqlite::Row) -> Result<Download> {
    Ok(Download {
        id: row.get(0)?,
        media_id: row.get(1)?,
        episode_id: row.get(2)?,
        magnet: row.get(3)?,
        qbit_hash: row.get(4)?,
        status: row.get(5)?,
        save_path: row.get(6)?,
        title: row.get(7)?,
        requested_by: row.get(8)?,
        created_at: row.get(9)?,
        completed_at: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn book_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE books (
                ol_key TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                cover_url TEXT,
                authors TEXT,
                pages INTEGER,
                subjects TEXT,
                added_by TEXT,
                added_at TEXT NOT NULL
            );
            CREATE TABLE book_shelf (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                ol_key TEXT NOT NULL,
                status TEXT NOT NULL,
                added_at TEXT NOT NULL,
                finished_at TEXT,
                showcased INTEGER NOT NULL DEFAULT 0,
                UNIQUE(user_id, ol_key)
            );
            CREATE TABLE book_progress (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                ol_key TEXT NOT NULL,
                cfi TEXT,
                percent REAL NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, ol_key)
            );",
        )
        .unwrap();
        Database { conn }
    }

    fn collection_db() -> Database {
        let db = Database::open(":memory:").unwrap();
        db.migrate().unwrap();
        db
    }

    fn add_user(db: &Database, id: &str) {
        db.conn
            .execute(
                "INSERT INTO users (id, username, email, password_hash, role)
                 VALUES (?1, ?2, ?3, 'hash', 'admin')",
                params![id, format!("{id}-name"), format!("{id}@example.com")],
            )
            .unwrap();
    }

    fn collection_auto(db: &Database, user_id: &str, kind: &str, tmdb_id: i64) -> i64 {
        db.conn
            .query_row(
                "SELECT auto_completed
                 FROM collection_items
                 WHERE user_id = ?1 AND kind = ?2 AND tmdb_id = ?3",
                params![user_id, kind, tmdb_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn book_library_uses_progress_for_automatic_shelves() {
        let db = book_db();
        db.conn
            .execute_batch(
                "INSERT INTO books (ol_key, title, added_by, added_at) VALUES
                    ('unopened', 'Unopened', 'user-1', '2026-01-01 00:00:00'),
                    ('zero', 'At the cover', 'user-1', '2026-01-02 00:00:00'),
                    ('started', 'Started', 'user-1', '2026-01-03 00:00:00'),
                    ('done', 'Done', 'user-1', '2026-01-04 00:00:00'),
                    ('reread', 'Reread', 'user-1', '2026-01-05 00:00:00');
                INSERT INTO book_shelf (id, user_id, ol_key, status, added_at, finished_at)
                    VALUES
                    ('s-zero', 'user-1', 'zero', 'reading', '2026-01-02 00:00:00', NULL),
                    ('s-reread', 'user-1', 'reread', 'read', '2026-01-05 00:00:00', '2026-01-05 00:00:00');
                INSERT INTO book_progress (id, user_id, ol_key, cfi, percent, updated_at) VALUES
                    ('p-zero', 'user-1', 'zero', NULL, 0, '2026-02-01 00:00:00'),
                    ('p-started', 'user-1', 'started', 'chapter-2', 0.4, '2026-02-02 00:00:00'),
                    ('p-done', 'user-1', 'done', 'the-end', 0.97, '2026-02-03 00:00:00'),
                    ('p-reread', 'user-1', 'reread', 'chapter-3', 0.5, '2026-02-04 00:00:00');",
            )
            .unwrap();

        let items = db.list_book_shelf("user-1").unwrap();
        let by_key: HashMap<_, _> = items
            .iter()
            .map(|item| (item.ol_key.as_str(), item))
            .collect();

        assert_eq!(by_key["unopened"].status, "want");
        assert_eq!(by_key["zero"].status, "want");
        assert_eq!(by_key["started"].status, "reading");
        assert_eq!(by_key["done"].status, "read");
        assert_eq!(by_key["reread"].status, "read");
        assert_eq!(
            by_key["done"].finished_at.as_deref(),
            Some("2026-02-03 00:00:00")
        );

        db.touch_book_shelf_want("user-1", "zero").unwrap();
        db.touch_book_shelf_reading("user-1", "reread").unwrap();
        let zero_status: String = db
            .conn
            .query_row(
                "SELECT status FROM book_shelf WHERE user_id = 'user-1' AND ol_key = 'zero'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let reread_status: String = db
            .conn
            .query_row(
                "SELECT status FROM book_shelf WHERE user_id = 'user-1' AND ol_key = 'reread'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(zero_status, "want");
        assert_eq!(reread_status, "read");

        let continuing = db.list_book_continue("user-1", 12).unwrap();
        assert_eq!(continuing.len(), 1);
        assert_eq!(continuing[0].ol_key, "started");

        let (read_total, _) = db.count_book_shelf_read("user-1").unwrap();
        assert_eq!(read_total, 2);
    }

    #[test]
    fn collection_upsert_is_user_scoped_and_tracks_completion() {
        let db = collection_db();
        add_user(&db, "user-1");
        add_user(&db, "user-2");

        let planned = db
            .upsert_collection_item(
                "user-1",
                101,
                "movie",
                "A Movie",
                Some("2026"),
                Some("/poster.jpg"),
                Some("/backdrop.jpg"),
                "planned",
                None,
            )
            .unwrap();
        assert_eq!(planned.status, "planned");
        assert!(planned.completed_at.is_none());
        assert!(db
            .get_collection_item("user-2", "movie", 101)
            .unwrap()
            .is_none());

        let completed = db
            .patch_collection_item("user-1", "movie", 101, Some("completed"), Some(true))
            .unwrap();
        let first_completed_at = completed.completed_at.clone().unwrap();
        assert_eq!(completed.id, planned.id);
        assert!(completed.showcased);

        let repeated = db
            .patch_collection_item("user-1", "movie", 101, Some("completed"), None)
            .unwrap();
        assert_eq!(
            repeated.completed_at.as_deref(),
            Some(first_completed_at.as_str())
        );

        let resumed = db
            .patch_collection_item("user-1", "movie", 101, Some("in_progress"), None)
            .unwrap();
        assert!(resumed.completed_at.is_none());
        assert!(!resumed.showcased);

        let filtered = db
            .list_collection("user-1", Some("movie"), Some("in_progress"), Some(false))
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].tmdb_id, 101);
    }

    #[test]
    fn showcase_limit_is_shared_with_books() {
        let db = collection_db();
        add_user(&db, "user-1");

        for tmdb_id in 1..=4 {
            db.upsert_collection_item(
                "user-1",
                tmdb_id,
                "movie",
                &format!("Movie {tmdb_id}"),
                None,
                None,
                None,
                "completed",
                Some(true),
            )
            .unwrap();
        }

        db.conn
            .execute(
                "INSERT INTO books (id, ol_key, title, status, added_by)
                 VALUES ('book-id', 'proud-book', 'Proud Book', 'ready', 'user-1')",
                [],
            )
            .unwrap();
        db.set_book_shelf("user-1", "proud-book", "read").unwrap();
        db.set_book_showcased("user-1", "proud-book", true).unwrap();
        db.upsert_book_progress("user-1", "proud-book", Some("chapter-1"), 1.0)
            .unwrap();
        db.create_book_mark(
            "user-1",
            "proud-book",
            "highlight",
            "chapter-1",
            None,
            None,
            Some("A highlight worth keeping until the book is deleted."),
            None,
        )
        .unwrap();
        let shelf = db.list_book_shelf("user-1").unwrap();
        assert_eq!(shelf.len(), 1);
        assert!(shelf[0].showcased);

        let error = db
            .upsert_collection_item(
                "user-1",
                5,
                "movie",
                "One Too Many",
                None,
                None,
                None,
                "completed",
                Some(true),
            )
            .unwrap_err();
        assert!(matches!(error, CollectionError::ShowcaseLimit));

        assert!(db.delete_book_by_key("proud-book").unwrap());
        for table in ["book_shelf", "book_progress", "book_marks"] {
            let count: i64 = db
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE ol_key = 'proud-book'"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table} should not retain an orphan");
        }
        db.upsert_collection_item(
            "user-1",
            5,
            "movie",
            "Now It Fits",
            None,
            None,
            None,
            "completed",
            Some(true),
        )
        .unwrap();
        assert_eq!(
            db.list_collection("user-1", None, None, Some(true))
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn showcase_requires_completion_and_demotion_clears_it() {
        let db = collection_db();
        add_user(&db, "user-1");

        let error = db
            .upsert_collection_item(
                "user-1",
                10,
                "movie",
                "Unfinished",
                None,
                None,
                None,
                "in_progress",
                Some(true),
            )
            .unwrap_err();
        assert!(matches!(error, CollectionError::ShowcaseRequiresCompletion));

        db.upsert_collection_item(
            "user-1",
            11,
            "movie",
            "Finished",
            None,
            None,
            None,
            "completed",
            Some(true),
        )
        .unwrap();
        let error = db
            .patch_collection_item("user-1", "movie", 11, Some("planned"), Some(true))
            .unwrap_err();
        assert!(matches!(error, CollectionError::ShowcaseRequiresCompletion));
        let still_completed = db
            .get_collection_item("user-1", "movie", 11)
            .unwrap()
            .unwrap();
        assert_eq!(still_completed.status, "completed");
        assert!(still_completed.showcased);

        let planned = db
            .patch_collection_item("user-1", "movie", 11, Some("planned"), None)
            .unwrap();
        assert_eq!(planned.status, "planned");
        assert!(!planned.showcased);
        assert!(planned.completed_at.is_none());

        db.conn
            .execute(
                "INSERT INTO books (id, ol_key, title, status, added_by)
                 VALUES ('unfinished-book-id', 'unfinished-book', 'Unfinished Book', 'ready', 'user-1')",
                [],
            )
            .unwrap();
        let error = db
            .set_book_showcased("user-1", "unfinished-book", true)
            .unwrap_err();
        assert!(matches!(error, CollectionError::ShowcaseRequiresCompletion));

        db.set_book_shelf("user-1", "unfinished-book", "read")
            .unwrap();
        db.set_book_showcased("user-1", "unfinished-book", true)
            .unwrap();
        db.set_book_shelf("user-1", "unfinished-book", "reading")
            .unwrap();
        let book = db
            .list_book_shelf("user-1")
            .unwrap()
            .into_iter()
            .find(|item| item.ol_key == "unfinished-book")
            .unwrap();
        assert_eq!(book.status, "reading");
        assert!(!book.showcased);
    }

    #[test]
    fn migration_repairs_invalid_showcases_and_orphaned_book_rows() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "DROP TRIGGER trg_collection_showcase_completed_insert;
                 DROP TRIGGER trg_collection_showcase_completed_update;
                 DROP TRIGGER trg_book_showcase_read_insert;
                 DROP TRIGGER trg_book_showcase_read_update;

                 INSERT INTO collection_items
                    (id, user_id, tmdb_id, kind, title, status, showcased)
                 VALUES
                    ('invalid-media', 'user-1', 71, 'movie', 'Invalid Media', 'planned', 1);

                 INSERT INTO books (id, ol_key, title, status, added_by)
                 VALUES ('valid-book-id', 'valid-book', 'Valid Book', 'ready', 'user-1');

                 INSERT INTO book_shelf
                    (id, user_id, ol_key, status, added_at, showcased)
                 VALUES
                    ('invalid-book-shelf', 'user-1', 'valid-book', 'reading', datetime('now'), 1),
                    ('orphan-book-shelf', 'user-1', 'missing-book', 'read', datetime('now'), 1);

                 INSERT INTO book_progress
                    (id, user_id, ol_key, percent, updated_at)
                 VALUES
                    ('orphan-progress', 'user-1', 'missing-book', 1.0, datetime('now'));

                 INSERT INTO book_marks
                    (id, user_id, ol_key, kind, cfi)
                 VALUES
                    ('orphan-mark', 'user-1', 'missing-book', 'highlight', 'chapter-1');",
            )
            .unwrap();

        db.migrate().unwrap();

        let media = db
            .get_collection_item("user-1", "movie", 71)
            .unwrap()
            .unwrap();
        assert!(!media.showcased);
        let book = db.list_book_shelf("user-1").unwrap().pop().unwrap();
        assert!(!book.showcased);
        for table in ["book_shelf", "book_progress", "book_marks"] {
            let count: i64 = db
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE ol_key = 'missing-book'"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }

        let trigger_error = db.conn.execute(
            "INSERT INTO collection_items
                (id, user_id, tmdb_id, kind, title, status, showcased)
             VALUES ('still-invalid', 'user-1', 72, 'movie', 'Still Invalid', 'planned', 1)",
            [],
        );
        assert!(trigger_error.is_err());
    }

    #[test]
    fn migration_backfills_and_reconciles_watch_progress_once() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "INSERT INTO media
                    (id, tmdb_id, media_type, title, year, poster_url, status, is_anime)
                 VALUES
                    ('old-movie', 401, 'movie', 'Old Movie', 2020, '/old-movie.jpg', 'ready', 0),
                    ('old-show', 402, 'tv', 'Old Show', 2021, '/old-show.jpg', 'ready', 0),
                    ('old-anime', 403, 'tv', 'Old Anime', 2022, '/old-anime.jpg', 'ready', 1),
                    ('short-open', 404, 'movie', 'Short Open', 2023, NULL, 'ready', 0),
                    ('manual-movie', 405, 'movie', 'Manual Movie', 2024, NULL, 'ready', 0);

                 INSERT INTO episodes
                    (id, media_id, season, episode, file_path, status)
                 VALUES
                    ('episode-1', 'old-show', 1, 1, 'show-s1e1.mkv', 'ready'),
                    ('show-pending', 'old-show', 1, 2, NULL, 'pending'),
                    ('episode-2', 'old-anime', 1, 1, 'anime-s1e1.mkv', 'ready');

                 INSERT INTO watch_progress
                    (id, user_id, media_id, episode_id, position, duration, completed, updated_at)
                 VALUES
                    ('wp-movie', 'user-1', 'old-movie', NULL, 95, 100, 1, '2026-01-01 10:00:00'),
                    ('wp-show', 'user-1', 'old-show', 'episode-1', 45, 50, 1, '2026-01-02 10:00:00'),
                    ('wp-anime', 'user-1', 'old-anime', 'episode-2', 40, 100, 0, '2026-01-03 10:00:00'),
                    ('wp-short', 'user-1', 'short-open', NULL, 10, 100, 0, '2026-01-04 10:00:00'),
                    ('wp-manual', 'user-1', 'manual-movie', NULL, 50, 100, 0, '2026-01-05 10:00:00');",
            )
            .unwrap();
        let existing_movie = db
            .upsert_collection_item(
                "user-1",
                401,
                "movie",
                "Old Movie",
                Some("2020"),
                Some("/old-movie.jpg"),
                None,
                "in_progress",
                None,
            )
            .unwrap();
        let existing_show = db
            .upsert_collection_item(
                "user-1",
                402,
                "tv",
                "Old Show",
                Some("2021"),
                Some("/old-show.jpg"),
                None,
                "planned",
                None,
            )
            .unwrap();
        let manual = db
            .upsert_collection_item(
                "user-1",
                405,
                "movie",
                "Manual Movie",
                Some("2024"),
                None,
                None,
                "planned",
                None,
            )
            .unwrap();

        db.migrate().unwrap();

        let movie = db
            .get_collection_item("user-1", "movie", 401)
            .unwrap()
            .unwrap();
        assert_eq!(movie.id, existing_movie.id);
        assert_eq!(movie.status, "completed");
        assert_eq!(movie.completed_at.as_deref(), Some("2026-01-01 10:00:00"));
        let show = db
            .get_collection_item("user-1", "tv", 402)
            .unwrap()
            .unwrap();
        assert_eq!(show.id, existing_show.id);
        assert_eq!(show.status, "completed");
        assert_eq!(collection_auto(&db, "user-1", "tv", 402), 1);
        assert_eq!(
            db.get_collection_item("user-1", "tv", 402)
                .unwrap()
                .unwrap()
                .completed_at
                .as_deref(),
            Some("2026-01-02 10:00:00")
        );
        assert_eq!(
            db.get_collection_item("user-1", "anime", 403)
                .unwrap()
                .unwrap()
                .status,
            "in_progress"
        );
        assert!(db
            .get_collection_item("user-1", "movie", 404)
            .unwrap()
            .is_none());
        let preserved = db
            .get_collection_item("user-1", "movie", 405)
            .unwrap()
            .unwrap();
        assert_eq!(preserved.id, manual.id);
        assert_eq!(preserved.status, "in_progress");

        let before = db.list_collection("user-1", None, None, None).unwrap();
        db.migrate().unwrap();
        let after = db.list_collection("user-1", None, None, None).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn tv_and_anime_are_reconciled_as_one_collection_identity() {
        let db = collection_db();
        add_user(&db, "user-1");

        let tv = db
            .upsert_collection_item(
                "user-1",
                501,
                "tv",
                "Alias Show",
                None,
                None,
                None,
                "completed",
                Some(true),
            )
            .unwrap();
        let anime = db
            .upsert_collection_item(
                "user-1",
                501,
                "anime",
                "Alias Show",
                None,
                None,
                None,
                "completed",
                None,
            )
            .unwrap();
        assert_eq!(anime.id, tv.id);
        assert!(anime.showcased);
        assert!(db
            .get_collection_item("user-1", "tv", 501)
            .unwrap()
            .is_none());
        assert_eq!(
            db.list_collection("user-1", None, None, None)
                .unwrap()
                .len(),
            1
        );

        db.conn
            .execute(
                "INSERT INTO media
                    (id, tmdb_id, media_type, title, status, is_anime)
                 VALUES ('alias-media', 501, 'tv', 'Alias Show', 'ready', 0)",
                [],
            )
            .unwrap();
        db.sync_collection_from_watch("user-1", "alias-media", false)
            .unwrap();
        let canonical = db
            .get_collection_item("user-1", "tv", 501)
            .unwrap()
            .unwrap();
        assert_eq!(canonical.id, tv.id);
        assert_eq!(canonical.status, "completed");
        assert!(canonical.showcased);
        assert!(db
            .get_collection_item("user-1", "anime", 501)
            .unwrap()
            .is_none());
    }

    #[test]
    fn alias_reconciliation_keeps_manual_completion_sticky() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "DROP INDEX idx_collection_user_canonical_tmdb;

                 INSERT INTO media
                    (id, tmdb_id, media_type, title, status, is_anime)
                 VALUES
                    ('legacy-alias', 551, 'tv', 'Legacy Alias', 'ready', 0),
                    ('legacy-single', 552, 'tv', 'Legacy Single', 'ready', 0);

                 INSERT INTO episodes
                    (id, media_id, season, episode, file_path, status)
                 VALUES ('legacy-e1', 'legacy-alias', 1, 1, NULL, 'pending');

                 INSERT INTO collection_items
                    (id, user_id, tmdb_id, kind, title, status, auto_completed)
                 VALUES
                    ('legacy-tv', 'user-1', 551, 'tv', 'Legacy Alias', 'completed', 1),
                    ('legacy-anime', 'user-1', 551, 'anime', 'Legacy Alias', 'completed', 0),
                    ('legacy-single-row', 'user-1', 552, 'anime', 'Legacy Single', 'planned', 0);",
            )
            .unwrap();

        db.migrate().unwrap();

        let merged = db
            .get_collection_item("user-1", "tv", 551)
            .unwrap()
            .unwrap();
        assert_eq!(merged.status, "completed");
        assert_eq!(collection_auto(&db, "user-1", "tv", 551), 0);
        assert!(db
            .get_collection_item("user-1", "anime", 551)
            .unwrap()
            .is_none());
        assert!(db
            .get_collection_item("user-1", "tv", 552)
            .unwrap()
            .is_some());
        assert!(db
            .get_collection_item("user-1", "anime", 552)
            .unwrap()
            .is_none());

        db.update_episode_ready("legacy-e1", "legacy-e1.mkv")
            .unwrap();
        let unchanged = db
            .get_collection_item("user-1", "tv", 551)
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.status, "completed");
        assert_eq!(collection_auto(&db, "user-1", "tv", 551), 0);
    }

    #[test]
    fn series_auto_completion_uses_only_playable_episodes() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "INSERT INTO media
                    (id, tmdb_id, media_type, title, status, is_anime)
                 VALUES
                    ('coverage-show', 601, 'tv', 'Coverage Show', 'ready', 0),
                    ('empty-show', 602, 'tv', 'Empty Show', 'ready', 0);

                 INSERT INTO episodes
                    (id, media_id, season, episode, file_path, status)
                 VALUES
                    ('coverage-e1', 'coverage-show', 1, 1, 'e1.mkv', 'ready'),
                    ('coverage-e2', 'coverage-show', 1, 2, 'e2.mkv', 'ready'),
                    ('coverage-e3', 'coverage-show', 1, 3, NULL, 'pending');",
            )
            .unwrap();

        db.upsert_progress("user-1", "coverage-show", Some("coverage-e1"), 95, 100)
            .unwrap();
        let partial = db
            .get_collection_item("user-1", "tv", 601)
            .unwrap()
            .unwrap();
        assert_eq!(partial.status, "in_progress");
        assert_eq!(collection_auto(&db, "user-1", "tv", 601), 0);

        db.upsert_progress("user-1", "coverage-show", Some("coverage-e2"), 95, 100)
            .unwrap();
        let completed = db
            .get_collection_item("user-1", "tv", 601)
            .unwrap()
            .unwrap();
        assert_eq!(completed.status, "completed");
        assert!(completed.completed_at.is_some());
        assert_eq!(collection_auto(&db, "user-1", "tv", 601), 1);

        let showcased = db
            .patch_collection_item("user-1", "tv", 601, None, Some(true))
            .unwrap();
        assert!(showcased.showcased);
        assert_eq!(collection_auto(&db, "user-1", "tv", 601), 1);

        db.mark_watched("user-1", "coverage-show", Some("coverage-e2"), 100, false)
            .unwrap();
        let reopened = db
            .get_collection_item("user-1", "tv", 601)
            .unwrap()
            .unwrap();
        assert_eq!(reopened.status, "in_progress");
        assert!(!reopened.showcased);
        assert!(reopened.completed_at.is_none());
        assert_eq!(collection_auto(&db, "user-1", "tv", 601), 0);

        db.sync_collection_from_watch("user-1", "empty-show", true)
            .unwrap();
        let empty = db
            .get_collection_item("user-1", "tv", 602)
            .unwrap()
            .unwrap();
        assert_eq!(empty.status, "in_progress");
        assert!(empty.completed_at.is_none());
        assert_eq!(collection_auto(&db, "user-1", "tv", 602), 0);
    }

    #[test]
    fn progress_and_collection_sync_commit_together() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "INSERT INTO media
                    (id, tmdb_id, media_type, title, status, is_anime)
                 VALUES ('atomic-movie', 651, 'movie', 'Atomic Movie', 'ready', 0);

                 CREATE TRIGGER fail_collection_insert
                 BEFORE INSERT ON collection_items
                 WHEN NEW.tmdb_id = 651
                 BEGIN
                    SELECT RAISE(ABORT, 'test collection failure');
                 END;",
            )
            .unwrap();

        assert!(db
            .upsert_progress("user-1", "atomic-movie", None, 31, 100)
            .is_err());
        assert!(db
            .get_progress("user-1", "atomic-movie", None)
            .unwrap()
            .is_none());

        db.conn
            .execute_batch("DROP TRIGGER fail_collection_insert;")
            .unwrap();
        db.upsert_progress("user-1", "atomic-movie", None, 31, 100)
            .unwrap();
        assert!(db
            .get_collection_item("user-1", "movie", 651)
            .unwrap()
            .is_some());

        db.conn
            .execute_batch(
                "CREATE TRIGGER fail_collection_update
                 BEFORE UPDATE ON collection_items
                 WHEN NEW.tmdb_id = 651
                 BEGIN
                    SELECT RAISE(ABORT, 'test collection failure');
                 END;",
            )
            .unwrap();
        assert!(db
            .mark_watched("user-1", "atomic-movie", None, 100, true)
            .is_err());
        assert!(
            !db.get_progress("user-1", "atomic-movie", None)
                .unwrap()
                .unwrap()
                .completed
        );
    }

    #[test]
    fn episode_availability_recomputes_auto_and_preserves_manual_completion() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "INSERT INTO media
                    (id, tmdb_id, media_type, title, status, is_anime)
                 VALUES ('changing-show', 701, 'tv', 'Changing Show', 'ready', 0);

                 INSERT INTO episodes
                    (id, media_id, season, episode, file_path, status)
                 VALUES
                    ('changing-e1', 'changing-show', 1, 1, 'e1.mkv', 'ready'),
                    ('changing-e2', 'changing-show', 1, 2, NULL, 'pending'),
                    ('changing-e3', 'changing-show', 1, 3, NULL, 'pending'),
                    ('changing-e4', 'changing-show', 1, 4, 'e4.mkv', 'pending');",
            )
            .unwrap();

        db.mark_watched("user-1", "changing-show", Some("changing-e1"), 100, true)
            .unwrap();
        db.patch_collection_item("user-1", "tv", 701, None, Some(true))
            .unwrap();
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 1);

        db.update_episode_status("changing-e4", "ready").unwrap();
        let status_expanded = db
            .get_collection_item("user-1", "tv", 701)
            .unwrap()
            .unwrap();
        assert_eq!(status_expanded.status, "in_progress");
        assert!(!status_expanded.showcased);
        assert!(status_expanded.completed_at.is_none());
        db.update_episode_status("changing-e4", "pending").unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "tv", 701)
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );
        db.patch_collection_item("user-1", "tv", 701, None, Some(true))
            .unwrap();

        db.update_episode_ready("changing-e2", "e2.mkv").unwrap();
        let expanded = db
            .get_collection_item("user-1", "tv", 701)
            .unwrap()
            .unwrap();
        assert_eq!(expanded.status, "in_progress");
        assert!(!expanded.showcased);
        assert!(expanded.completed_at.is_none());
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 0);

        db.mark_watched("user-1", "changing-show", Some("changing-e2"), 100, true)
            .unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "tv", 701)
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 1);

        db.clear_episode_file("changing-e2").unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "tv", 701)
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 1);

        db.clear_episode_file("changing-e1").unwrap();
        let empty = db
            .get_collection_item("user-1", "tv", 701)
            .unwrap()
            .unwrap();
        assert_eq!(empty.status, "in_progress");
        assert!(empty.completed_at.is_none());
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 0);

        db.update_episode_ready("changing-e1", "e1.mkv").unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "tv", 701)
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 1);

        db.patch_collection_item("user-1", "tv", 701, Some("completed"), None)
            .unwrap();
        db.patch_collection_item("user-1", "tv", 701, None, Some(true))
            .unwrap();
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 0);

        db.update_episode_ready("changing-e3", "e3.mkv").unwrap();
        let manual = db
            .get_collection_item("user-1", "tv", 701)
            .unwrap()
            .unwrap();
        assert_eq!(manual.status, "completed");
        assert!(manual.showcased);
        assert!(manual.completed_at.is_some());
        assert_eq!(collection_auto(&db, "user-1", "tv", 701), 0);
    }

    #[test]
    fn watch_sync_tracks_media_without_demoting_completed_titles() {
        let db = collection_db();
        add_user(&db, "user-1");
        db.conn
            .execute_batch(
                "INSERT INTO media
                    (id, tmdb_id, media_type, title, year, poster_url, backdrop_url, status, is_anime)
                 VALUES
                    ('movie-1', 201, 'movie', 'Movie One', 2024, '/m1.jpg', '/m1-bg.jpg', 'ready', 0),
                    ('movie-2', 202, 'movie', 'Movie Two', 2025, '/m2.jpg', NULL, 'ready', 0),
                    ('show-1', 301, 'tv', 'A Show', 2023, '/tv.jpg', NULL, 'ready', 0),
                    ('anime-1', 302, 'tv', 'An Anime', 2022, '/anime.jpg', NULL, 'ready', 1);",
            )
            .unwrap();

        db.upsert_progress("user-1", "movie-1", None, 31, 100)
            .unwrap();
        db.sync_collection_from_watch("user-1", "movie-1", false)
            .unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "movie", 201)
                .unwrap()
                .unwrap()
                .status,
            "in_progress"
        );

        db.patch_collection_item("user-1", "movie", 201, Some("completed"), None)
            .unwrap();
        db.sync_collection_from_watch("user-1", "movie-1", false)
            .unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "movie", 201)
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );

        db.sync_collection_from_watch("user-1", "movie-2", true)
            .unwrap();
        assert!(db
            .get_collection_item("user-1", "movie", 202)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());

        db.sync_collection_from_watch("user-1", "show-1", true)
            .unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "tv", 301)
                .unwrap()
                .unwrap()
                .status,
            "in_progress"
        );

        db.sync_collection_from_watch("user-1", "anime-1", true)
            .unwrap();
        assert_eq!(
            db.get_collection_item("user-1", "anime", 302)
                .unwrap()
                .unwrap()
                .kind,
            "anime"
        );
    }
}
