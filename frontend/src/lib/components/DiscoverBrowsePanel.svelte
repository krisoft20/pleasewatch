<script lang="ts">
    import { api, type DiscoverGenresResponse, type TmdbGenre, type TmdbSearchItem } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import Icon from '$lib/components/Icon.svelte';

    type Kind = 'all' | 'movie' | 'tv';

    let { onPreview }: { onPreview: (item: TmdbSearchItem) => void } = $props();

    const kinds: Array<{ key: Kind; label: string }> = [
        { key: 'all', label: 'all' },
        { key: 'movie', label: 'movies' },
        { key: 'tv', label: 'shows' }
    ];

    const aliases = [
        { terms: ['sci fi', 'scifi', 'science fiction'], names: ['science fiction', 'sci fi'] },
        { terms: ['anime'], names: ['animation'] },
        { terms: ['action'], names: ['action'] },
        { terms: ['rom com', 'romcom'], names: ['romance', 'comedy'] }
    ];

    let kind = $state<Kind>('all');
    let q = $state('');
    let open = $state(false);
    let picked = $state<number[]>([]);
    let genres = $state<DiscoverGenresResponse | null>(null);
    let results = $state<TmdbSearchItem[]>([]);
    let busy = $state(false);
    let error = $state('');
    let page = $state(1);
    let mode = $state<'browse' | 'search'>('browse');
    let seq = 0;

    const visibleGenres = $derived(makeVisibleGenres());
    const selectedCount = $derived(activeIds('movie').length + activeIds('tv').length);

    async function loadGenres() {
        if (genres || busy) return;
        busy = true;
        error = '';
        try {
            genres = await api.discoverGenres();
        } catch (e) {
            console.error('[discover] genres failed', e);
            error = 'genres failed';
        } finally {
            busy = false;
        }
    }

    $effect(() => {
        if (!open) return;
        if (!genres) return;
        const watch = `${kind}|${q}|${picked.join(',')}|${page}`;
        void watch;
        const wait = q.trim().length >= 2 ? 220 : 0;
        const timer = setTimeout(run, wait);
        return () => clearTimeout(timer);
    });

    function clean(s: string) {
        return s
            .toLowerCase()
            .replace(/&/g, ' and ')
            .replace(/[^a-z0-9]+/g, ' ')
            .trim();
    }

    function listFor(k: 'movie' | 'tv') {
        return genres?.[k] ?? [];
    }

    function makeVisibleGenres() {
        if (!genres) return [];
        if (kind === 'movie') return genres.movie;
        if (kind === 'tv') return genres.tv;

        const m = new Map<number, TmdbGenre>();
        for (const g of genres.movie) m.set(g.id, g);
        for (const g of genres.tv) {
            if (!m.has(g.id)) m.set(g.id, g);
        }
        return [...m.values()];
    }

    function idsFromText(k: 'movie' | 'tv') {
        const text = clean(q);
        if (text.length < 2) return [];

        const out = new Set<number>();
        const list = listFor(k);
        for (const g of list) {
            const name = clean(g.name);
            if (name && text.includes(name)) out.add(g.id);
        }

        for (const a of aliases) {
            if (!a.terms.some((term) => text.includes(term))) continue;
            for (const g of list) {
                const name = clean(g.name);
                if (a.names.some((n) => name === n || name.includes(n))) out.add(g.id);
            }
        }
        return [...out];
    }

    function activeIds(k: 'movie' | 'tv') {
        const valid = new Set(listFor(k).map((g) => g.id));
        return [...new Set([...picked.filter((id) => valid.has(id)), ...idsFromText(k)])];
    }

    function interleave(a: TmdbSearchItem[], b: TmdbSearchItem[]) {
        const out: TmdbSearchItem[] = [];
        const n = Math.max(a.length, b.length);
        for (let i = 0; i < n; i++) {
            if (a[i]) out.push(a[i]);
            if (b[i]) out.push(b[i]);
        }
        return out;
    }

    async function run() {
        const mine = ++seq;
        const text = q.trim();
        const movie_ids = activeIds('movie');
        const tv_ids = activeIds('tv');
        const has_genre = movie_ids.length > 0 || tv_ids.length > 0;

        busy = true;
        error = '';
        try {
            let items: TmdbSearchItem[];
            if (text.length >= 2 && !has_genre) {
                mode = 'search';
                const hits = await api.search(text);
                items = kind === 'all' ? hits : hits.filter((x) => x.media_type === kind);
            } else if (kind === 'movie') {
                mode = 'browse';
                items = await api.discoverBrowse('movie', movie_ids, page);
            } else if (kind === 'tv') {
                mode = 'browse';
                items = await api.discoverBrowse('tv', tv_ids, page);
            } else {
                mode = 'browse';
                const [movies, shows] = await Promise.all([
                    api.discoverBrowse('movie', movie_ids, page),
                    api.discoverBrowse('tv', tv_ids, page)
                ]);
                items = interleave(movies, shows);
            }
            if (mine !== seq) return;
            results = page > 1 && mode === 'browse' ? [...results, ...items] : items;
        } catch (e) {
            console.error('[discover] browse failed', e);
            if (mine === seq) {
                results = [];
                error = 'discover failed';
            }
        } finally {
            if (mine === seq) busy = false;
        }
    }

    function setKind(next: Kind) {
        kind = next;
        page = 1;
    }

    function onText() {
        page = 1;
    }

    function toggleGenre(id: number) {
        picked = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id];
        page = 1;
    }

    function clearAll() {
        q = '';
        picked = [];
        page = 1;
    }

    function more() {
        if (mode !== 'browse' || busy) return;
        page += 1;
    }

    function toggleOpen() {
        open = !open;
        if (open) loadGenres();
    }

    function poster(u: string | null | undefined) {
        return u ? u.replace('/w500/', '/w342/') : '';
    }
</script>

<section class="pw-db" class:is-open={open}>
    <button class="pw-db-toggle" type="button" onclick={toggleOpen} aria-expanded={open}>
        <span>{t('discover.browse')}</span>
        <span class="pw-db-caret">
            <Icon name="chevron-down" class="w-full h-full" strokeWidth={2} />
        </span>
    </button>

    {#if open}
        <div class="pw-db-panel">
            <div class="pw-db-kind" aria-label="discover kind">
                {#each kinds as k}
                    <button type="button" class:is-active={kind === k.key} onclick={() => setKind(k.key)}>
                        {k.label}
                    </button>
                {/each}
            </div>

            <div class="pw-db-search">
                <span class="pw-db-search-icon">
                    <Icon name="search" class="w-full h-full" strokeWidth={2} />
                </span>
                <input
                    bind:value={q}
                    oninput={onText}
                    placeholder={t('discover.search_placeholder')}
                    aria-label={t('discover.search_placeholder')}
                />
                {#if q || picked.length}
                    <button type="button" class="pw-db-clear" onclick={clearAll}>{t('discover.clear')}</button>
                {/if}
            </div>

            {#if visibleGenres.length > 0}
                <div class="pw-db-genres">
                    {#each visibleGenres as g (g.id)}
                        <button type="button" class:is-active={picked.includes(g.id)} onclick={() => toggleGenre(g.id)}>
                            {g.name}
                        </button>
                    {/each}
                </div>
            {/if}

            <div class="pw-db-state">
                <span>{busy && page === 1 ? t('common.loading') : `${results.length} ${t('discover.results')}`}</span>
                {#if selectedCount > 0}<span>{selectedCount} {t('discover.filters')}</span>{/if}
                {#if error}<span class="pw-db-error">{error}</span>{/if}
            </div>

            {#if results.length > 0}
                <div class="pw-db-grid">
                    {#each results as item (item.media_type + '-' + item.tmdb_id)}
                        <button type="button" class="pw-db-card" onclick={() => onPreview(item)}>
                            <div class="pw-db-poster">
                                {#if item.poster_url}
                                    <img src={poster(item.poster_url)} alt={item.title} loading="lazy" decoding="async" />
                                {:else}
                                    <div class="pw-db-missing">
                                        <Icon name="screen-text" class="w-8 h-8" strokeWidth={1.8} />
                                    </div>
                                {/if}
                                <span>{item.media_type === 'tv' ? 'series' : 'movie'}</span>
                            </div>
                            <strong>{item.title}</strong>
                            <div class="pw-db-meta">
                                {#if item.year}<span>{item.year}</span>{/if}
                                {#if item.vote_average}<span>{item.vote_average.toFixed(1)}</span>{/if}
                            </div>
                            {#if item.overview}<p>{item.overview}</p>{/if}
                        </button>
                    {/each}
                </div>
                {#if mode === 'browse'}
                    <button type="button" class="pw-db-more" onclick={more} disabled={busy}>
                        {busy && page > 1 ? t('common.loading') : t('discover.more')}
                    </button>
                {/if}
            {:else if !busy}
                <div class="pw-db-empty">{t('discover.nothing')}</div>
            {/if}
        </div>
    {/if}
</section>

<style>
    .pw-db {
        width: min(980px, calc(100% - 72px));
        margin: 0 auto 28px;
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(11, 13, 18, 0.72);
        border-radius: 8px;
        overflow: hidden;
    }
    .pw-db-toggle {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        width: 100%;
        min-height: 46px;
        padding: 0 16px;
        border: 0;
        color: rgba(244, 244, 245, 0.9);
        background: rgba(255, 255, 255, 0.03);
        cursor: pointer;
        font-size: 12px;
        font-weight: 750;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }
    .pw-db-toggle:hover {
        background: rgba(255, 255, 255, 0.055);
    }
    .pw-db-caret {
        width: 16px;
        height: 16px;
        color: rgba(235, 238, 244, 0.58);
        transition: transform 0.15s ease;
    }
    .pw-db.is-open .pw-db-caret {
        transform: rotate(180deg);
    }
    .pw-db-panel {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr);
        gap: 12px;
        padding: 14px 16px 16px;
        border-top: 1px solid rgba(255, 255, 255, 0.07);
    }
    .pw-db-kind {
        display: inline-flex;
        gap: 4px;
        padding: 4px;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
    }
    .pw-db-kind button,
    .pw-db-genres button,
    .pw-db-clear,
    .pw-db-more {
        border: none;
        cursor: pointer;
        color: rgba(235, 238, 244, 0.82);
        background: transparent;
    }
    .pw-db-kind button {
        min-width: 58px;
        height: 30px;
        border-radius: 6px;
        font-size: 12px;
        font-weight: 650;
    }
    .pw-db-kind button.is-active {
        color: #071018;
        background: #f4f4f5;
    }
    .pw-db-search {
        position: relative;
        display: flex;
        align-items: center;
        min-height: 44px;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.08);
        overflow: hidden;
    }
    .pw-db-search-icon {
        width: 17px;
        height: 17px;
        margin-left: 14px;
        color: rgba(235, 238, 244, 0.52);
        flex: 0 0 auto;
    }
    .pw-db-search input {
        width: 100%;
        height: 44px;
        padding: 0 11px;
        border: 0;
        outline: 0;
        color: #f4f4f5;
        background: transparent;
        font-size: 14px;
    }
    .pw-db-search input::placeholder {
        color: rgba(235, 238, 244, 0.38);
    }
    .pw-db-clear {
        height: 30px;
        margin-right: 7px;
        padding: 0 10px;
        border-radius: 6px;
        background: rgba(255, 255, 255, 0.08);
        font-size: 12px;
    }
    .pw-db-genres {
        display: flex;
        flex-wrap: wrap;
        gap: 7px;
        max-height: 104px;
        overflow: hidden;
        grid-column: 1 / -1;
        margin-top: 12px;
    }
    .pw-db-genres button {
        height: 28px;
        padding: 0 10px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.075);
        font-size: 12px;
    }
    .pw-db-genres button.is-active {
        color: #06131c;
        background: #67b7ff;
        border-color: #67b7ff;
    }
    .pw-db-state {
        display: flex;
        gap: 12px;
        align-items: center;
        grid-column: 1 / -1;
        min-height: 38px;
        color: rgba(235, 238, 244, 0.54);
        font-size: 12px;
    }
    .pw-db-error {
        color: #ff8080;
    }
    .pw-db-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(148px, 1fr));
        gap: 15px;
        grid-column: 1 / -1;
    }
    .pw-db-card {
        display: block;
        min-width: 0;
        padding: 0;
        border: 0;
        text-align: left;
        color: #f4f4f5;
        background: transparent;
        cursor: pointer;
    }
    .pw-db-poster {
        position: relative;
        aspect-ratio: 2 / 3;
        overflow: hidden;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.08);
    }
    .pw-db-poster img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        transition: transform 0.18s ease;
    }
    .pw-db-card:hover img {
        transform: scale(1.035);
    }
    .pw-db-poster span {
        position: absolute;
        top: 8px;
        left: 8px;
        padding: 4px 7px;
        border-radius: 5px;
        background: rgba(4, 6, 10, 0.74);
        color: rgba(255, 255, 255, 0.86);
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
    }
    .pw-db-missing {
        display: grid;
        place-items: center;
        width: 100%;
        height: 100%;
        color: rgba(255, 255, 255, 0.25);
    }
    .pw-db-card strong {
        display: block;
        margin-top: 9px;
        color: rgba(244, 244, 245, 0.94);
        font-size: 13.5px;
        line-height: 1.25;
    }
    .pw-db-meta {
        display: flex;
        gap: 8px;
        margin-top: 4px;
        color: rgba(235, 238, 244, 0.48);
        font-size: 12px;
    }
    .pw-db-card p {
        display: -webkit-box;
        margin: 7px 0 0;
        color: rgba(235, 238, 244, 0.58);
        font-size: 12px;
        line-height: 1.45;
        -webkit-line-clamp: 3;
        line-clamp: 3;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .pw-db-more,
    .pw-db-empty {
        grid-column: 1 / -1;
        margin-top: 16px;
    }
    .pw-db-more {
        height: 36px;
        padding: 0 15px;
        border-radius: 7px;
        background: rgba(255, 255, 255, 0.08);
        font-size: 13px;
        font-weight: 650;
    }
    .pw-db-more:disabled {
        cursor: default;
        opacity: 0.55;
    }
    .pw-db-empty {
        color: rgba(235, 238, 244, 0.54);
        font-size: 13px;
    }
    @media (max-width: 720px) {
        .pw-db {
            width: calc(100% - 24px);
        }
        .pw-db-toggle {
            min-height: 44px;
            padding: 0 14px;
        }
        .pw-db-panel {
            display: block;
        }
        .pw-db-kind {
            width: 100%;
            margin-bottom: 12px;
        }
        .pw-db-kind button {
            flex: 1;
        }
        .pw-db-grid {
            grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
            gap: 13px;
        }
    }
</style>
