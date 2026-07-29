import { currentLang } from './i18n';

function tmdbLang(): string {
    switch (currentLang()) {
        case 'PL':
            return 'pl-PL';
        case 'DE':
            return 'de-DE';
        default:
            return 'en-US';
    }
}

function parseReleaseTokens(name: string): {
    se: string | null;
    source: string | null;
    codec: string | null;
    res: string | null;
    group: string | null;
} {
    const s = name.toLowerCase().replace(/\./g, ' ');
    const se = s.match(/s(\d{1,3})e(\d{1,3})/);
    const source = s.match(/\b(blu-?ray|web-?dl|web-?rip|hdtv|hdrip|dvdrip|bdrip|bdrip|remux)\b/);
    const codec = s.match(/\b(x265|x264|hevc|h[ \-.]?264|h[ \-.]?265|av1)\b/);
    const res = s.match(/\b(2160p|1440p|1080p|720p|480p|4k|uhd)\b/);
    const group = s.match(/-\s*([a-z0-9]+)(?:\s*\[|\s*$)/i);
    return {
        se: se
            ? `s${String(parseInt(se[1], 10)).padStart(2, '0')}e${String(parseInt(se[2], 10)).padStart(2, '0')}`
            : null,
        source: source ? source[1].replace(/[ \-]/g, '') : null,
        codec: codec
            ? (() => {
                  const c = codec[1].replace(/[ \-.]/g, '');
                  if (c === 'h264') return 'x264';
                  if (c === 'h265') return 'hevc';
                  if (c === 'x265') return 'hevc';
                  return c;
              })()
            : null,
        res: res ? (res[1] === 'uhd' || res[1] === '4k' ? '2160p' : res[1]) : null,
        group: group ? group[1] : null
    };
}

export type User = {
    id: string;
    username: string;
    email: string;
    role: string;
    created_at: string;
};

export type Media = {
    id: string;
    tmdb_id: number | null;
    media_type: string;
    title: string;
    year: number | null;
    overview: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    file_path: string | null;
    duration: number | null;
    status: string;
    added_by: string | null;
    added_at: string;
    activity_at?: string | null;
    activity_label?: string | null;
    is_anime?: boolean;
    source_name?: string | null;
};

export type Episode = {
    id: string;
    media_id: string;
    season: number;
    episode: number;
    title: string | null;
    file_path: string | null;
    duration: number | null;
    status: string;
    source_name?: string | null;
    intro_start?: number | null;
    intro_end?: number | null;
    credits_start?: number | null;
};

export type MediaWithEpisodes = Media & { episodes: Episode[]; subs_processing?: boolean };

export type Subtitle = {
    id: string;
    owner_id: string;
    language: string;
    label: string;
    format: string;
    file_path: string;
    is_default: boolean;
    media_id?: string;
};

export type AudioTrack = {
    index: number;
    language: string;
    label: string;
    codec: string;
};

export type EpisodeListItem = {
    season: number;
    episode: number;
    name: string;
    overview: string | null;
    still_url: string | null;
    episode_id: string | null;
    has_file: boolean;
};

export type EpisodeRecord = Episode & {
    download_id?: string | null;
    media_type?: string;
    duration?: number | null;
};

export type MediaSubtitle = Subtitle & { media_id?: string };

export type WyzieEntry = {
    id?: string;
    display?: string;
    language?: string;
    encoding?: string;
    format?: string;
    media?: string;
    source?: string;
    url: string;
    release?: string;
    origin?: string;
    fileName?: string;
    downloadCount?: number;
    matchedRelease?: string | null;
    isHearingImpaired?: boolean;
};

export type WatchProgress = {
    id: string;
    user_id: string;
    media_id: string;
    episode_id: string | null;
    position: number;
    duration: number;
    completed: boolean;
    dismissed: boolean;
    updated_at: string;
};

export type ContinueItem = {
    media_id: string;
    media_title: string;
    media_type: string;
    is_anime: boolean;
    tmdb_id: number | null;
    poster_url: string | null;
    episode_id: string | null;
    episode_season: number | null;
    episode_number: number | null;
    episode_title: string | null;
    position: number;
    duration: number;
    updated_at: string;
    episode_still_url?: string | null;
};

export type ProgressSummary = {
    media_id: string;
    position: number;
    duration: number;
};

export type SubSearchResult = {
    url: string;
    release: string;
    source: string;
    score: number;
    hearing_impaired: boolean;
    language: string;
    format?: string;
    encoding?: string;
    downloads?: number;
    origin?: string;
};

export type TmdbSearchItem = {
    tmdb_id: number;
    media_type: string;
    title: string;
    year: string | null;
    overview: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    vote_average: number | null;
    genre_ids?: number[] | null;
};

export type CollectionKind = 'movie' | 'tv' | 'anime';
export type CollectionStatus = 'planned' | 'in_progress' | 'completed';

export type CollectionItem = {
    id: string;
    tmdb_id: number;
    kind: CollectionKind;
    title: string;
    year: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    status: CollectionStatus;
    showcased: boolean;
    added_at: string;
    updated_at: string;
    completed_at: string | null;
};

export type CollectionUpsert = {
    tmdb_id: number;
    kind: CollectionKind;
    title: string;
    year: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    status: CollectionStatus;
    showcased?: boolean;
};

export type TmdbGenre = {
    id: number;
    name: string;
};

export type PartyInfo = {
    code: string;
    media_id: string;
    episode_id: string | null;
    media_title: string;
    media_type: string;
    poster_url: string | null;
    episode_title: string | null;
    episode_season: number | null;
    episode_number: number | null;
    participants: number;
    stream_id: string;
};

export type ClipInfo = {
    id: string;
    media_id: string;
    episode_id: string | null;
    start_sec: number;
    end_sec: number;
    subtitle_id: string | null;
    file_path: string;
    file_size: number | null;
    created_by: string;
    created_at: string;
};

export type DiscoverResponse = {
    trending: TmdbSearchItem[];
    popular_movies: TmdbSearchItem[];
    popular_tv: TmdbSearchItem[];
    top_rated_movies: TmdbSearchItem[];
    top_rated_tv: TmdbSearchItem[];
};

export type DiscoverGenresResponse = {
    movie: TmdbGenre[];
    tv: TmdbGenre[];
};

export type TmdbDetail = {
    tmdb_id: number;
    imdb_id: string | null;
    media_type: string;
    title: string;
    year: string | null;
    overview: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    vote_average: number | null;
    runtime: number | null;
    genres: string[];
    is_anime: boolean;
    seasons: TmdbSeason[] | null;
    cast: TmdbCastMember[];
    omdb_seasons?: number[] | null;
    belongs_to_collection?: TmdbCollectionRef | null;
};

export type TmdbCollectionRef = {
    id: number;
    name: string;
    poster_url: string | null;
};

export type TmdbCollection = {
    id: number;
    name: string;
    overview: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    parts: TmdbCollectionPart[];
};

export type TmdbCollectionPart = {
    tmdb_id: number;
    title: string;
    year: string | null;
    release_date: string | null;
    overview: string | null;
    poster_url: string | null;
    backdrop_url: string | null;
    vote_average: number | null;
};

export type TmdbCastMember = {
    id: number;
    name: string;
    character: string;
    photo_url: string | null;
};

export type TmdbPersonCredit = {
    tmdb_id: number;
    media_type: string;
    title: string;
    year: string | null;
    poster_url: string | null;
    character: string | null;
    vote_average: number | null;
};

export type TmdbPersonDetail = {
    id: number;
    name: string;
    biography: string | null;
    birthday: string | null;
    deathday: string | null;
    place_of_birth: string | null;
    photo_url: string | null;
    known_for_department: string | null;
    also_known_as: string[];
    total_credits: number;
    career_start: number | null;
    career_end: number | null;
    credits: TmdbPersonCredit[];
};

export type TmdbVideo = {
    key: string;
    name: string;
    kind: string;
};

export type TmdbSeason = {
    season_number: number;
    name: string;
    episode_count: number;
    overview: string | null;
    poster_url: string | null;
};

export type TmdbEpisode = {
    episode_number: number;
    season_number: number;
    name: string;
    overview: string | null;
    air_date: string | null;
    still_url: string | null;
    runtime: number | null;
    vote_average: number | null;
};

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const res = await fetch(path, {
        credentials: 'include',
        headers: { 'content-type': 'application/json', ...(init.headers ?? {}) },
        ...init
    });

    const text = await res.text();
    let body: any = null;
    if (text) {
        try {
            body = JSON.parse(text);
        } catch {
            body = text;
        }
    }

    if (!res.ok) {
        const msg = typeof body === 'string' ? body : (body?.error ?? res.statusText);
        throw new Error(msg);
    }
    return body as T;
}

function stub(reason: string) {
    return async (..._args: unknown[]): Promise<any> => {
        throw new Error(reason);
    };
}

export type Manga = {
    id: string;
    md_id: string;
    title: string;
    description: string | null;
    cover_url: string | null;
    year: number | null;
    status: string | null;
    added_by: string | null;
    added_at: string;
};

export type MangaHit = {
    md_id: string;
    title: string;
    description: string | null;
    cover_url: string | null;
    year: number | null;
    status: string | null;
};

export type MangaChapter = {
    id: string;
    chapter: string | null;
    title: string | null;
    volume: string | null;
    lang: string;
    pages: number;
    published_at: string | null;
};

export type MangaProgress = {
    md_id: string;
    chapter_id: string;
    chapter: string | null;
    page: number;
    pages: number;
    updated_at: string;
};

export type MangaContinueItem = {
    md_id: string;
    title: string;
    cover_url: string | null;
    chapter_id: string;
    chapter: string | null;
    page: number;
    pages: number;
    updated_at: string;
};

export type MangaDetail = {
    manga: MangaHit;
    in_library: boolean;
    progress: MangaProgress | null;
    restricted: boolean;
    restricted_langs: string[];
    available_langs: string[];
    links: Record<string, string>;
    anilist_id: number | null;
    mal_id: number | null;
    tags: string[];
    demographic: string | null;
    content_rating: string | null;
    authors: string[];
    artists: string[];
    score: number | null;
    score_count: number | null;
    follow_count: number | null;
    last_chapter: string | null;
};

export type MangaRelated = {
    anilist_id: number;
    relation: string;
    title: string;
    cover_url: string | null;
    kind: string;
    format: string | null;
};

export type MangaRecommended = {
    anilist_id: number;
    title: string;
    cover_url: string | null;
};

export type MangaAnimeLink = {
    anilist_id: number;
    title: string;
    cover_url: string | null;
    format: string | null;
    tmdb: TmdbSearchItem | null;
} | null;

export type Book = {
    id: string;
    ol_key: string;
    title: string;
    authors: string | null;
    description: string | null;
    cover_url: string | null;
    year: number | null;
    language: string | null;
    file_path: string | null;
    ext: string | null;
    status: string;
    added_by: string | null;
    added_at: string;
    pages: number | null;
    subjects: string | null;
    isbn: string | null;
    publisher: string | null;
    rating: number | null;
    rating_count: number | null;
    enriched_at: string | null;
    series: string | null;
};

export type BookHit = {
    ol_key: string;
    title: string;
    authors: string | null;
    description: string | null;
    cover_url: string | null;
    year: number | null;
    language: string | null;
    author_keys?: string[];
    in_library?: boolean;
    kind?: 'book' | 'series';
    series_count?: number;
    series_covers?: string[];
};

export type SeriesDetail = {
    name: string;
    author: string | null;
    cover_url: string | null;
    year_min: number | null;
    year_max: number | null;
    books: BookHit[];
};

export type BookSource = {
    md5: string;
    title: string;
    authors: string | null;
    publisher: string | null;
    ext: string;
    language: string | null;
    size: number | null;
    year: number | null;
    pages: number | null;
};

export type BookProgress = {
    ol_key: string;
    cfi: string | null;
    percent: number;
    updated_at: string;
};

export type BookContinueItem = {
    ol_key: string;
    title: string;
    cover_url: string | null;
    cfi: string | null;
    percent: number;
    updated_at: string;
    authors: string | null;
    pages: number | null;
};

export type BookDetail = {
    book: Book;
    in_library: boolean;
    progress: BookProgress | null;
    file_size: number | null;
    shelf: string | null;
    author_keys?: string[];
};

export type AuthorDetail = {
    olid: string;
    name: string;
    bio: string | null;
    birth_date: string | null;
    death_date: string | null;
    photo_url: string | null;
    top_works: BookHit[];
};

export type BookShelfItem = {
    ol_key: string;
    title: string;
    cover_url: string | null;
    authors: string | null;
    pages: number | null;
    subjects: string | null;
    status: 'want' | 'reading' | 'read';
    showcased: boolean;
    finished_at: string | null;
    percent: number | null;
};

export type DailyQuote = {
    id: string;
    ol_key: string;
    cfi: string;
    snippet: string;
    chapter: string | null;
    title: string;
    authors: string | null;
    cover_url: string | null;
};

export type BookMark = {
    id: string;
    ol_key: string;
    kind: 'highlight' | 'bookmark';
    cfi: string;
    color: string | null;
    note: string | null;
    snippet: string | null;
    chapter: string | null;
    created_at: string;
};

let mePromise: Promise<User> | null = null;

export const api = {
    register: (username: string, email: string, password: string) =>
        request<{ message: string; role: string }>('/api/auth/register', {
            method: 'POST',
            body: JSON.stringify({ username, email, password })
        }),

    login: (username: string, password: string) => {
        mePromise = null;
        return request<{ user: User }>('/api/auth/login', {
            method: 'POST',
            body: JSON.stringify({ username, password })
        });
    },

    logout: () => {
        mePromise = null;
        return request<{ message: string }>('/api/auth/logout', { method: 'POST' });
    },

    me: () => {
        if (!mePromise) {
            mePromise = request<User>('/api/auth/me');
            mePromise.catch(() => {
                mePromise = null;
            });
        }
        return mePromise;
    },

    listMedia: (opts?: { mine?: boolean }) => request<Media[]>(opts?.mine ? '/api/media?mine=true' : '/api/media'),

    getMedia: (id: string) => request<MediaWithEpisodes>(`/api/media/${id}`),

    getMediaByTmdb: (kind: string, tmdb_id: number) =>
        request<MediaWithEpisodes>(`/api/media/by-tmdb/${kind}/${tmdb_id}`),

    addMedia: (tmdb_id: number, media_type: string) =>
        request<Media>('/api/media', {
            method: 'POST',
            body: JSON.stringify({ tmdb_id, media_type })
        }),

    deleteMedia: (id: string) => request<void>(`/api/media/${id}`, { method: 'DELETE' }),

    collectionList: (opts?: { kind?: CollectionKind; status?: CollectionStatus; showcased?: boolean }) => {
        const q = new URLSearchParams();
        if (opts?.kind) q.set('kind', opts.kind);
        if (opts?.status) q.set('status', opts.status);
        if (opts?.showcased !== undefined) q.set('showcased', String(opts.showcased));
        const suffix = q.size > 0 ? `?${q}` : '';
        return request<CollectionItem[]>(`/api/collection${suffix}`);
    },

    collectionGet: (kind: CollectionKind, tmdb_id: number) =>
        request<CollectionItem>(`/api/collection/${kind}/${tmdb_id}`),

    collectionSave: (body: CollectionUpsert) =>
        request<CollectionItem>('/api/collection', {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    collectionUpdate: (
        kind: CollectionKind,
        tmdb_id: number,
        body: { status?: CollectionStatus; showcased?: boolean }
    ) =>
        request<CollectionItem>(`/api/collection/${kind}/${tmdb_id}`, {
            method: 'PATCH',
            body: JSON.stringify(body)
        }),

    collectionRemove: (kind: CollectionKind, tmdb_id: number) =>
        request<void>(`/api/collection/${kind}/${tmdb_id}`, { method: 'DELETE' }),

    listSubtitles: (owner_id: string) => request<Subtitle[]>(`/api/subtitles/${owner_id}`),

    audioTracks: (id: string) => request<AudioTrack[]>(`/api/stream/${id}/audio-tracks`),

    streamUrl: (id: string, audio?: number) =>
        audio && audio > 0 ? `/api/stream/${id}?audio=${audio}` : `/api/stream/${id}`,

    thumbUrl: (id: string, t: number, w = 480) => {
        const ts = Math.max(0, Math.floor(t / 10) * 10);
        return `/api/thumb/${id}?t=${ts}&v=${w}`;
    },

    deleteEpisodeFile: (episode_id: string) => request<void>(`/api/episodes/${episode_id}/file`, { method: 'DELETE' }),

    subtitleUrl: (_ownerId: string | undefined, subId: string) => `/api/subtitle/${subId}`,

    getSeasonEpisodes: (tmdbId: number, season: number) =>
        request<TmdbEpisode[]>(`/api/search/tv/${tmdbId}/season/${season}`),

    aiTranslateSub: (ownerId: string, target: string) =>
        request<Subtitle>(`/api/subtitles/translate/${ownerId}`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ target })
        }),

    searchSubtitles: async (
        mediaId: string,
        lang: string,
        videoRelease?: string | null
    ): Promise<SubSearchResult[]> => {
        const q = lang ? `?language=${encodeURIComponent(lang)}` : '';
        const entries = await request<WyzieEntry[]>(`/api/subtitles/search/${mediaId}${q}`);
        const target = parseReleaseTokens(videoRelease ?? '');
        const mapped = entries.map((e) => {
            const release =
                e.release || e.fileName?.replace(/\.(srt|vtt|ass|ssa)$/i, '') || e.media || 'unknown release';
            const tok = parseReleaseTokens(release);

            let score = 0;
            let hardMismatch = false;

            if (target.se && tok.se && target.se !== tok.se) {
                hardMismatch = true;
            }

            if (!hardMismatch) {
                score += 20;
                if (target.source && tok.source && target.source === tok.source) score += 25;
                if (target.codec && tok.codec && target.codec === tok.codec) score += 15;
                if (target.res && tok.res && target.res === tok.res) score += 10;
                if (target.group && tok.group && target.group === tok.group) score += 15;
                if (e.matchedRelease) score += 5;
                const dl = e.downloadCount ?? 0;
                score += Math.min(15, Math.round(Math.log10(dl + 1) * 5));
                if (e.isHearingImpaired) score -= 8;
            }

            score = Math.min(99, Math.max(0, Math.round(score)));
            return {
                url: e.url,
                release,
                source: e.source || 'wyzie',
                score,
                hearing_impaired: !!e.isHearingImpaired,
                language: e.language || lang,
                format: e.format,
                encoding: e.encoding,
                downloads: e.downloadCount ?? undefined,
                origin: e.origin
            };
        });
        mapped.sort((a, b) => b.score - a.score);
        return mapped;
    },

    downloadSubtitle: async (mediaId: string, url: string, language: string, label: string): Promise<MediaSubtitle> =>
        request<MediaSubtitle>(`/api/subtitle/fetch`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ owner_id: mediaId, url, language, label })
        }),

    autoDownloadNext: async (..._args: unknown[]) => ({}) as any,

    saveProgress: (body: { media_id: string; episode_id?: string | null; position: number; duration: number }) =>
        request<{ ok: boolean }>('/api/watch/save', {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    getProgress: (media_id: string, episode_id?: string) => {
        const q = new URLSearchParams({ media_id });
        if (episode_id) q.set('episode_id', episode_id);
        return request<WatchProgress | null>(`/api/watch/get?${q}`);
    },

    listMediaProgress: (media_id: string) => request<WatchProgress[]>(`/api/media/${media_id}/progress`),

    continueWatching: () => request<ContinueItem[]>('/api/watch/continue'),

    watchSummary: () => request<ProgressSummary[]>('/api/watch/summary'),

    markWatched: (media_id: string, watched: boolean, episode_id?: string | null, duration?: number) =>
        request<{ ok: boolean }>('/api/watch/mark', {
            method: 'POST',
            body: JSON.stringify({ media_id, episode_id, watched, duration })
        }),

    dismissContinue: (media_id: string) =>
        request<{ ok: boolean }>(`/api/watch/dismiss/${media_id}`, { method: 'POST' }),

    uploadSubtitle: async (owner_id: string, file: File, language: string, label: string): Promise<MediaSubtitle> => {
        const fd = new FormData();
        fd.append('owner_id', owner_id);
        fd.append('language', language);
        fd.append('label', label);
        fd.append('file', file);
        const res = await fetch('/api/subtitle/upload', {
            method: 'POST',
            credentials: 'include',
            body: fd
        });
        if (!res.ok) throw new Error(`upload failed (${res.status})`);
        return res.json();
    },

    deleteSubtitle: async (_owner_id: string, sub_id: string): Promise<void> => {
        await request<void>(`/api/subtitle/${sub_id}`, { method: 'DELETE' });
    },

    syncSubtitle: async (sub_id: string): Promise<void> => {
        await request<void>(`/api/subtitle/${sub_id}/sync`, { method: 'POST' });
    },

    syncSubtitleAlass: async (sub_id: string): Promise<void> => {
        await request<void>(`/api/subtitle/${sub_id}/sync_alass`, { method: 'POST' });
    },

    syncSubtitleWhisper: async (sub_id: string): Promise<void> => {
        await request<void>(`/api/subtitle/${sub_id}/sync_whisper`, { method: 'POST' });
    },
    search: (q: string) => request<TmdbSearchItem[]>(`/api/search?q=${encodeURIComponent(q)}`),

    tmdbDetail: (kind: string, tmdb_id: number) =>
        request<TmdbDetail>(`/api/search/${kind}/${tmdb_id}?lang=${tmdbLang()}`),

    tmdbSeason: (tmdb_id: number, season: number) =>
        request<TmdbEpisode[]>(`/api/search/tv/${tmdb_id}/season/${season}?lang=${tmdbLang()}`),

    tmdbVideos: (kind: string, tmdb_id: number) =>
        request<TmdbVideo[]>(`/api/search/${kind}/${tmdb_id}/videos?lang=${tmdbLang()}`),

    tmdbPerson: (person_id: number) => request<TmdbPersonDetail>(`/api/tmdb/person/${person_id}?lang=${tmdbLang()}`),

    tmdbCollection: (id: number) => request<TmdbCollection>(`/api/tmdb/collection/${id}?lang=${tmdbLang()}`),

    discover: () => request<DiscoverResponse>(`/api/discover?lang=${tmdbLang()}&v=2`),

    discoverGenres: () => request<DiscoverGenresResponse>(`/api/discover/genres?lang=${tmdbLang()}`),

    discoverBrowse: (kind: 'movie' | 'tv', genres: number[], page = 1) => {
        const qs = new URLSearchParams({ kind, page: String(page), lang: tmdbLang() });
        if (genres.length) qs.set('genres', genres.join(','));
        return request<TmdbSearchItem[]>(`/api/discover/browse?${qs.toString()}`);
    },

    tmdbSimilar: (kind: string, tmdb_id: number) =>
        request<TmdbSearchItem[]>(`/api/search/${kind}/${tmdb_id}/similar?lang=${tmdbLang()}`),

    mangaSearch: (q: string) => request<MangaHit[]>(`/api/manga/search?q=${encodeURIComponent(q)}`),

    mangaPopular: () => request<MangaHit[]>('/api/manga/popular'),

    mangaDiscover: (kind: 'latest' | 'popular' | 'toprated' | 'new') =>
        request<MangaHit[]>(`/api/manga/discover?kind=${kind}`),

    mangaRelated: (md_id: string) => request<MangaRelated[]>(`/api/manga/${md_id}/related`),

    mangaRecommendations: (md_id: string) => request<MangaRecommended[]>(`/api/manga/${md_id}/recommendations`),

    mangaAnime: (md_id: string) => request<MangaAnimeLink>(`/api/manga/${md_id}/anime`),

    mangaList: (opts?: { mine?: boolean }) => request<Manga[]>(opts?.mine ? '/api/manga?mine=true' : '/api/manga'),

    mangaAdd: (md_id: string) => request<Manga>('/api/manga', { method: 'POST', body: JSON.stringify({ md_id }) }),

    mangaDetail: (md_id: string) => request<MangaDetail>(`/api/manga/${md_id}`),

    mangaDelete: (md_id: string) => request<{ ok: boolean }>(`/api/manga/${md_id}`, { method: 'DELETE' }),

    mangaChapters: (md_id: string) => request<MangaChapter[]>(`/api/manga/${md_id}/chapters`),

    mangaChapterPages: (chapter_id: string) => request<{ pages: string[] }>(`/api/manga/chapter/${chapter_id}/pages`),

    mangaProgress: (body: { md_id: string; chapter_id: string; chapter: string | null; page: number; pages: number }) =>
        request<{ ok: boolean }>('/api/manga/progress', { method: 'POST', body: JSON.stringify(body) }),

    mangaContinue: () => request<MangaContinueItem[]>('/api/manga/continue'),

    bookSearch: (q: string) => request<BookHit[]>(`/api/books/search?q=${encodeURIComponent(q)}`),

    bookPopular: () => request<BookHit[]>('/api/books/popular'),

    bookSeries: (name: string) => request<BookHit[]>(`/api/books/series?name=${encodeURIComponent(name)}`),

    bookSeriesDetail: (name: string) =>
        request<SeriesDetail>(`/api/books/series/detail?name=${encodeURIComponent(name)}`),

    bookByAuthor: (name: string, exclude?: string) => {
        const p = new URLSearchParams({ name });
        if (exclude) p.set('exclude', exclude);
        return request<BookHit[]>(`/api/books/by-author?${p}`);
    },

    bookAuthor: (olid: string) => request<AuthorDetail>(`/api/books/author/${olid}`),

    bookTorrents: (ol_key: string) => request<TorrentOption[]>(`/api/books/${ol_key}/torrents`),

    bookTorrentAdd: (ol_key: string, body: { magnet: string; title?: string }) =>
        request<{ ok: boolean; hash: string }>(`/api/books/${ol_key}/torrent-add`, {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    bookList: (opts?: { mine?: boolean }) => request<Book[]>(opts?.mine ? '/api/books?mine=true' : '/api/books'),

    bookAdd: (ol_key: string) => request<Book>('/api/books', { method: 'POST', body: JSON.stringify({ ol_key }) }),

    bookDetail: (ol_key: string) => request<BookDetail>(`/api/books/${ol_key}`),

    bookDelete: (ol_key: string) => request<{ ok: boolean }>(`/api/books/${ol_key}`, { method: 'DELETE' }),

    bookSources: (ol_key: string) => request<BookSource[]>(`/api/books/${ol_key}/sources`),

    bookFetch: (ol_key: string, md5: string, ext: string) =>
        request<{ ok: boolean; size: number; ext: string }>(`/api/books/${ol_key}/fetch`, {
            method: 'POST',
            body: JSON.stringify({ md5, ext })
        }),

    bookUpload: async (ol_key: string, file: File): Promise<{ ok: boolean; size: number; ext: string }> => {
        const fd = new FormData();
        fd.append('file', file);
        const res = await fetch(`/api/books/${ol_key}/upload`, {
            method: 'POST',
            credentials: 'include',
            body: fd
        });
        if (!res.ok) throw new Error(`upload failed (${res.status})`);
        return res.json();
    },

    bookFileUrl: (ol_key: string) => `/api/books/${ol_key}/file`,

    bookDownloadUrl: (ol_key: string) => `/api/books/${ol_key}/file?dl=1`,

    bookProgress: (body: { ol_key: string; cfi: string | null; percent: number }) =>
        request<{ ok: boolean }>('/api/books/progress', {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    bookContinue: () => request<BookContinueItem[]>('/api/books/continue'),

    bookShelf: () =>
        request<{ items: BookShelfItem[]; read_total: number; read_year: number; goal: number | null }>(
            '/api/books/shelf'
        ),

    bookDaily: () => request<DailyQuote | null>('/api/books/daily'),

    bookGoalSet: (goal: number) =>
        request<{ ok: boolean }>('/api/books/goal', {
            method: 'POST',
            body: JSON.stringify({ goal })
        }),

    bookShelfSet: (ol_key: string, status: string) =>
        request<{ ok: boolean }>(`/api/books/${ol_key}/shelf`, {
            method: 'POST',
            body: JSON.stringify({ status })
        }),

    bookShelfShowcase: (ol_key: string, showcased: boolean) =>
        request<{ ok: boolean }>(`/api/books/${ol_key}/shelf`, {
            method: 'PATCH',
            body: JSON.stringify({ showcased })
        }),

    bookShelfRemove: (ol_key: string) => request<{ ok: boolean }>(`/api/books/${ol_key}/shelf`, { method: 'DELETE' }),

    bookMarks: (ol_key: string) => request<BookMark[]>(`/api/books/${ol_key}/marks`),

    bookMarkCreate: (
        ol_key: string,
        body: { kind: string; cfi: string; color?: string; note?: string; snippet?: string; chapter?: string }
    ) =>
        request<BookMark>(`/api/books/${ol_key}/marks`, {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    bookMarkUpdate: (id: string, body: { color?: string; note?: string }) =>
        request<{ ok: boolean }>(`/api/books/marks/${id}`, {
            method: 'PATCH',
            body: JSON.stringify(body)
        }),

    bookMarkDelete: (id: string) => request<{ ok: boolean }>(`/api/books/marks/${id}`, { method: 'DELETE' }),

    createClip: (body: {
        media_id: string;
        episode_id?: string | null;
        start: number;
        end: number;
        subtitle_id?: string | null;
    }) =>
        request<{
            id: string;
            url: string;
            share_url: string;
            start: number;
            end: number;
            duration: number;
            file_size: number;
        }>('/api/clips', { method: 'POST', body: JSON.stringify(body) }),
    listClips: () => request<ClipInfo[]>(`/api/clips`),
    deleteClip: (id: string) => request<void>(`/api/clips/${id}`, { method: 'DELETE' }),

    createParty: (media_id: string, episode_id?: string | null) =>
        request<{ code: string }>('/api/party', {
            method: 'POST',
            body: JSON.stringify({ media_id, episode_id })
        }),
    partyInfo: (code: string) => request<PartyInfo>(`/api/party/${code}`),
    setPartyEpisode: (code: string, episode_id: string) =>
        request<{ ok: boolean }>(`/api/party/${code}/episode`, {
            method: 'POST',
            body: JSON.stringify({ episode_id })
        }),

    torrentSearch: (q: string, opts: { kind?: string; imdb?: string } = {}) => {
        const params = new URLSearchParams({ q });
        if (opts.kind) params.set('kind', opts.kind);
        if (opts.imdb) params.set('imdb', opts.imdb);
        return request<TorrentOption[]>(`/api/torrents/search?${params}`);
    },

    torrentIndexers: () => request<string[]>('/api/torrents/indexers'),

    listDownloads: () => request<DownloadStatus[]>('/api/downloads'),

    createDownload: (body: {
        magnet: string;
        media_id?: string;
        tmdb_id?: number;
        media_type?: string;
        episode_id?: string | null;
        season?: number | null;
        episode?: number | null;
        title?: string | null;
        torrent?: TorrentOption | null;
    }) =>
        request<Download>('/api/downloads', {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    cancelDownload: (id: string) => request<void>(`/api/downloads/${id}`, { method: 'DELETE' }),

    getSettings: () => request<AdminSettings>('/api/admin/settings'),

    updateSettings: (body: Partial<AdminSettingsUpdate>) =>
        request<AdminSettings>('/api/admin/settings', {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    listUsers: () => request<User[]>('/api/admin/users'),

    approveUser: (id: string) => request<void>(`/api/admin/users/${id}/approve`, { method: 'POST' }),

    setUserRole: (id: string, role: string) =>
        request<void>(`/api/admin/users/${id}/role`, {
            method: 'POST',
            body: JSON.stringify({ role })
        }),

    deleteUser: (id: string) => request<void>(`/api/admin/users/${id}`, { method: 'DELETE' }),

    adminStats: () => request<AdminStats>('/api/admin/stats'),

    adminHealth: () => request<HealthCheck[]>('/api/admin/health'),

    adminStorage: () => request<StorageView>('/api/admin/storage'),

    adminSystem: () => request<SystemMetrics>('/api/admin/system'),

    adminMetrics: () => request<AppMetrics>('/api/admin/metrics'),

    adminInsights: () => request<{ insights: Insight[] }>('/api/admin/insights'),

    processingJobs: () => request<ProcessingJob[]>('/api/admin/processing'),

    watchStats: () => request<WatchStats>('/api/admin/watch-stats'),

    diskUsage: () => request<DiskUsage>('/api/admin/disk-usage'),

    cleanupErrored: () => request<{ removed: number }>('/api/admin/cleanup', { method: 'POST' }),

    cleanDownloads: () =>
        request<{ cleaned_bytes: number; cleaned_downloads: number }>('/api/admin/clean-downloads', { method: 'POST' }),

    vpnStatus: () => request<VpnStatus>('/api/admin/vpn'),

    vpnSave: (body: {
        provider: string;
        wireguard_private_key: string;
        wireguard_addresses: string;
        countries?: string;
    }) =>
        request<{ ok: boolean }>('/api/admin/vpn', {
            method: 'POST',
            body: JSON.stringify(body)
        }),

    vpnDisable: () => request<{ ok: boolean }>('/api/admin/vpn/disable', { method: 'POST' })
};

export type VpnStatus = {
    enabled: boolean;
    provider: string;
    countries: string;
    has_key: boolean;
    addresses: string;
    container_state: string | null;
    public_ip: string | null;
};

export type AppMetrics = {
    media: { total: number; ready: number; error: number };
    episodes: { total: number; ready: number };
    downloads: { active: number; errored: number };
    subtitles: {
        total: number;
        by_language: { language: string; count: number }[];
        by_source: { source: string; count: number }[];
    };
    watch: { total_records: number; completed_records: number; active_last_24h: number };
    users: { total: number; admin: number; pending: number };
};

export type Insight = {
    severity: 'info' | 'warning' | 'critical';
    title: string;
    detail: string;
};

export type ProcessingJob = {
    id: string;
    operation: string;
    source: string;
    started_at: string;
    state: string;
};

export type WatchStats = {
    total_watch_seconds: number;
    total_completed_episodes: number;
    leaderboard: {
        user_id: string;
        username: string;
        watch_seconds: number;
        completed_episodes: number;
    }[];
};

export type DiskUsage = {
    total_disk: number;
    used_disk: number;
    free_disk: number;
    movies_size: number;
    series_size: number;
    anime_size: number;
    media_processed: number;
    video_size: number;
    audio_remux_size: number;
    embedded_subs_size: number;
    downloads_cache: number;
    shared_subs: number;
    clips: number;
    thumbnails: number;
    total_media: number;
};

export type SystemMetrics = {
    cpu_percent: number;
    memory_total_bytes: number;
    memory_used_bytes: number;
    memory_percent: number;
    uptime_seconds: number;
    load_avg: [number, number, number];
    disk_total_bytes: number;
    disk_used_bytes: number;
    disk_free_bytes: number;
};

export type HealthCheck = {
    name: string;
    ok: boolean;
    detail: string | null;
    latency_ms: number | null;
};

export type StorageView = {
    library_bytes: number;
    library_files: number;
    app_bytes: number;
    app_files: number;
    media_root: string;
    free_bytes: number | null;
    total_bytes: number | null;
    used_bytes: number | null;
    directories: StorageBucket[];
    file_types: StorageBucket[];
    inventory: StorageInventory;
    items: StorageMediaItem[];
};

export type StorageBucket = {
    key: string;
    label: string;
    bytes: number;
    files: number;
};

export type StorageInventory = {
    total_items: number;
    movies: number;
    series: number;
    anime: number;
    ready_items: number;
    without_files: number;
    total_episodes: number;
    ready_episodes: number;
    subtitle_tracks: number;
};

export type StorageMediaItem = {
    id: string;
    tmdb_id: number | null;
    media_type: string;
    title: string;
    year: number | null;
    poster_url: string | null;
    status: string;
    added_at: string;
    is_anime: boolean;
    bytes: number;
    files: number;
    video_bytes: number;
    video_files: number;
    audio_bytes: number;
    audio_files: number;
    subtitle_bytes: number;
    subtitle_files: number;
    episode_total: number;
    episode_ready: number;
    has_files: boolean;
    relative_path: string;
};

export type AdminStats = {
    users_total: number;
    users_pending: number;
    media_total: number;
    downloads_active: number;
};

export type AdminSettings = {
    tmdb_api_key_set: boolean;
    tmdb_ready: boolean;
    wyzie_api_key_set: boolean;
    wyzie_ready: boolean;
    wyzie_key_count: number;
    wyzie_keys_masked: string[];
    wyzie_keys_full?: string[];
    omdb_api_key_set: boolean;
    omdb_ready: boolean;
    omdb_key_count: number;
    omdb_keys_masked: string[];
    omdb_keys_full?: string[];
    jackett_url: string;
    jackett_api_key_set: boolean;
    prowlarr_url: string;
    prowlarr_api_key_set: boolean;
    qbit_url: string;
    qbit_user: string;
    qbit_pass_set: boolean;
    jackett_ready: boolean;
    prowlarr_ready: boolean;
    qbit_ready: boolean;
};

export type AdminSettingsUpdate = {
    tmdb_api_key: string;
    wyzie_api_key: string;
    wyzie_key_add: string;
    wyzie_key_remove_mask: string;
    omdb_api_key: string;
    omdb_key_add: string;
    omdb_key_remove_mask: string;
    jackett_url: string;
    jackett_api_key: string;
    prowlarr_url: string;
    prowlarr_api_key: string;
    qbit_url: string;
    qbit_user: string;
    qbit_pass: string;
};

export type TorrentOption = {
    provider: string;
    provider_id: string;
    title: string;
    magnet: string;
    quality: string | null;
    size: number;
    seeds: number;
    peers: number;
    audio: string[];
    video_codec: string | null;
    subtitle_info: string | null;
    release_group: string | null;
    tags: string[];
    pref_score?: number;
    aggregator?: string;
};

export type Download = {
    id: string;
    media_id: string;
    episode_id: string | null;
    magnet: string;
    qbit_hash: string | null;
    status: string;
    save_path: string;
    title: string | null;
    requested_by: string | null;
    created_at: string;
    completed_at: string | null;
};

export type DownloadStatus = Download & {
    progress: number;
    state: string | null;
};
