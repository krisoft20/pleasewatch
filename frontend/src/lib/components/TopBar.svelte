<script lang="ts">
    import { tick } from 'svelte';
    import { goto, preloadData } from '$app/navigation';
    import { api, type BookHit, type MangaHit, type TmdbSearchItem, type User } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import { category, CATEGORIES, type Category } from '$lib/category.svelte';
    import { clickOutside } from '$lib/dismiss';
    import UserMenu from '$lib/components/UserMenu.svelte';

    type Tab = { key: string; label: string };

    let {
        user,
        tabs: tabsProp,
        activeTab = '',
        onTab,
        back = false,
        unread = 0
    }: {
        user: User;
        tabs?: Tab[];
        activeTab?: string;
        onTab?: (k: string) => void;
        back?: boolean;
        unread?: number;
    } = $props();

    const tabs = $derived<Tab[]>(
        tabsProp ?? [
            { key: 'library', label: t('tab.library') },
            { key: 'personal', label: t('tab.personal') },
            { key: 'discover', label: t('tab.discover') }
        ]
    );

    function clickTab(key: string) {
        if (onTab) {
            onTab(key);
        } else {
            goto(key === 'library' ? '/' : `/?tab=${key}`);
        }
    }

    let q = $state('');
    let searchResults = $state<TmdbSearchItem[]>([]);
    let mangaHits = $state<MangaHit[]>([]);
    let bookHits = $state<BookHit[]>([]);
    let searchOpen = $state(false);
    let searchBusy = $state(false);
    let opening = $state<number | null>(null);
    let libByKey = $state<Map<string, string>>(new Map());
    let searchPillRef = $state<HTMLDivElement>();
    let searchInputRef = $state<HTMLInputElement>();
    let timer: ReturnType<typeof setTimeout> | undefined;

    async function refreshLibraryKeys() {
        try {
            const list = await api.listMedia();
            const m = new Map<string, string>();
            for (const item of list) {
                if (item.tmdb_id != null) m.set(`${item.media_type}:${item.tmdb_id}`, item.id);
            }
            libByKey = m;
        } catch {}
    }

    $effect(() => {
        if (searchOpen) refreshLibraryKeys();
    });

    function onSearchInput() {
        clearTimeout(timer);
        const trimmed = q.trim();
        if (trimmed.length < 2) {
            searchResults = [];
            searchOpen = trimmed.length > 0;
            return;
        }
        searchOpen = true;
        timer = setTimeout(runSearch, 250);
    }

    async function runSearch() {
        searchBusy = true;
        try {
            if (category.current === 'manga') {
                mangaHits = await api.mangaSearch(q);
                searchResults = [];
                bookHits = [];
            } else if (category.current === 'books') {
                bookHits = await api.bookSearch(q);
                searchResults = [];
                mangaHits = [];
            } else {
                searchResults = await api.search(q);
                mangaHits = [];
                bookHits = [];
            }
        } catch {
            searchResults = [];
            mangaHits = [];
            bookHits = [];
        } finally {
            searchBusy = false;
        }
    }

    function pickResult(item: TmdbSearchItem) {
        closeSearch();
        const route = item.media_type === 'tv' ? `/tv/${item.tmdb_id}` : `/movie/${item.tmdb_id}`;
        goto(route);
    }

    function pickManga(hit: MangaHit) {
        closeSearch();
        goto(`/manga/${hit.md_id}`);
    }

    function pickBook(hit: BookHit) {
        closeSearch();
        if (hit.kind === 'series') {
            goto(`/series/${encodeURIComponent(hit.ol_key)}`);
        } else {
            goto(`/book/${hit.ol_key}`);
        }
    }

    function bookHref(hit: BookHit): string {
        return hit.kind === 'series' ? `/series/${encodeURIComponent(hit.ol_key)}` : `/book/${hit.ol_key}`;
    }

    function clearQuery() {
        q = '';
        searchResults = [];
        mangaHits = [];
        bookHits = [];
        searchOpen = false;
    }

    function closeSearch() {
        searchOpen = false;
        q = '';
        searchResults = [];
        mangaHits = [];
        bookHits = [];
        searchInputRef?.blur();
    }

    async function expandSearch() {
        if (searchOpen) return;
        searchOpen = true;
        await tick();
        searchInputRef?.focus();
    }

    $effect(() => {
        if (!searchOpen) return;
        const onDoc = (ev: MouseEvent) => {
            if (searchPillRef && !searchPillRef.contains(ev.target as Node)) {
                searchOpen = false;
            }
        };
        const onKey = (ev: KeyboardEvent) => {
            if (ev.key === 'Escape') closeSearch();
        };
        document.addEventListener('mousedown', onDoc);
        document.addEventListener('keydown', onKey);
        return () => {
            document.removeEventListener('mousedown', onDoc);
            document.removeEventListener('keydown', onKey);
        };
    });

    function goBack() {
        if (typeof history !== 'undefined' && history.length > 1) {
            history.back();
        } else {
            goto('/');
        }
    }

    let menuOpen = $state(false);
    let menuRef = $state<HTMLDivElement>();

    $effect(() => {
        if (!menuOpen) return;
        const onDoc = (ev: MouseEvent) => {
            if (menuRef && !menuRef.contains(ev.target as Node)) menuOpen = false;
        };
        const onKey = (ev: KeyboardEvent) => {
            if (ev.key === 'Escape') menuOpen = false;
        };
        document.addEventListener('mousedown', onDoc);
        document.addEventListener('keydown', onKey);
        return () => {
            document.removeEventListener('mousedown', onDoc);
            document.removeEventListener('keydown', onKey);
        };
    });

    let catOpen = $state(false);

    function pickCategory(c: Category) {
        catOpen = false;
        category.set(c);
        if (location.pathname !== '/') goto('/');
    }

    const initial = $derived(user.username.charAt(0).toUpperCase());
    const isAdmin = $derived(user.role === 'admin');
</script>

<div class="pw-topbar" class:is-search-focused={searchOpen}>
    <div class="pw-topbar-inner">
        <div class="pw-tb-left">
            {#if back}
                <button class="pw-back-btn" onclick={goBack} aria-label="back">
                    <svg
                        width="16"
                        height="16"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <polyline points="15 18 9 12 15 6" />
                    </svg>
                </button>
            {/if}
            <a class="pw-topbar-wordmark" href="/">
                please<b>watch</b><i>.</i>
            </a>
            <div class="pw-cat" use:clickOutside={() => (catOpen = false)}>
                <button
                    class="pw-cat-btn"
                    class:is-open={catOpen}
                    onclick={() => (catOpen = !catOpen)}
                    aria-label={t(`cat.${category.current}`)}
                >
                    {#if category.current === 'video'}
                        <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.8"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><rect x="2" y="2" width="20" height="20" rx="2.5" /><path
                                d="M7 2v20M17 2v20M2 12h20M2 7h5M2 17h5M17 17h5M17 7h5"
                            /></svg
                        >
                    {:else if category.current === 'manga'}
                        <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.8"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><polygon points="12 2 2 7 12 12 22 7 12 2" /><polyline
                                points="2 17 12 22 22 17"
                            /><polyline points="2 12 12 17 22 12" /></svg
                        >
                    {:else}
                        <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.8"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" /><path
                                d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"
                            /></svg
                        >
                    {/if}
                    <span>{t(`cat.${category.current}`)}</span>
                    <svg
                        class="pw-cat-chev"
                        width="11"
                        height="11"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <polyline points="6 9 12 15 18 9" />
                    </svg>
                </button>
                {#if catOpen}
                    <div class="pw-cat-menu">
                        {#each CATEGORIES as c (c)}
                            <button
                                class="pw-cat-row"
                                class:is-active={c === category.current}
                                onclick={() => pickCategory(c)}
                            >
                                {t(`cat.${c}`)}
                            </button>
                        {/each}
                    </div>
                {/if}
            </div>
            {#if tabs.length > 0}
                <div class="pw-topbar-tabs pw-tabs">
                    {#each tabs as t (t.key)}
                        <button
                            class="pw-tab"
                            class:is-active={activeTab === t.key}
                            onclick={() => clickTab(t.key)}
                            aria-label={t.label}
                        >
                            <span class="pw-tab-icon">
                                {#if t.key === 'library'}
                                    <svg
                                        width="16"
                                        height="16"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.8"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" /><path
                                            d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"
                                        /></svg
                                    >
                                {:else if t.key === 'personal'}
                                    <svg
                                        width="16"
                                        height="16"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.8"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><path d="M6 3h12v18l-6-4-6 4V3z" /><path d="m9 9 2 2 4-4" /></svg
                                    >
                                {:else if t.key === 'discover'}
                                    <svg
                                        width="16"
                                        height="16"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.8"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><circle cx="12" cy="12" r="10" /><polygon
                                            points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76"
                                        /></svg
                                    >
                                {/if}
                            </span>
                            <span class="pw-tab-label">{t.label}</span>
                        </button>
                    {/each}
                </div>
            {/if}
        </div>

        <div class="pw-search-wrap" bind:this={searchPillRef}>
            <div class="pw-search-row">
                <div
                    class="pw-search-pill"
                    class:is-open={searchOpen}
                    role="button"
                    aria-label="search"
                    onclick={expandSearch}
                    onkeydown={(e) => {
                        if (e.target !== e.currentTarget) return;
                        if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            expandSearch();
                        }
                    }}
                    tabindex={searchOpen ? -1 : 0}
                >
                    <svg
                        class="pw-search-icon"
                        width="15"
                        height="15"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <circle cx="11" cy="11" r="7" />
                        <path d="m20 20-3.5-3.5" />
                    </svg>
                    <input
                        bind:this={searchInputRef}
                        class="pw-search-input"
                        bind:value={q}
                        oninput={onSearchInput}
                        onfocus={() => (searchOpen = true)}
                        placeholder={t('search.placeholder')}
                        spellcheck="false"
                        autocomplete="off"
                    />
                    {#if q}
                        <button class="pw-search-clear" onclick={clearQuery} aria-label="clear">
                            <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2.2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                            >
                                <path d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    {/if}
                </div>
                <button class="pw-search-cancel" onclick={closeSearch} type="button">Cancel</button>
            </div>

            {#if searchOpen && q.trim().length >= 2}
                <div class="pw-search-results">
                    {#if searchBusy && searchResults.length === 0 && mangaHits.length === 0 && bookHits.length === 0}
                        <div class="pw-search-empty">
                            <div class="pw-search-spin"></div>
                        </div>
                    {:else if searchResults.length === 0 && mangaHits.length === 0 && bookHits.length === 0}
                        <div class="pw-search-empty">no results for "{q}"</div>
                    {:else if mangaHits.length > 0}
                        {#each mangaHits.slice(0, 8) as r (r.md_id)}
                            <button
                                class="pw-search-result"
                                onclick={() => pickManga(r)}
                                onpointerenter={() => preloadData(`/manga/${r.md_id}`)}
                            >
                                {#if r.cover_url}
                                    <img class="pw-search-poster" src={r.cover_url} alt="" loading="lazy" />
                                {:else}
                                    <div class="pw-search-poster pw-search-poster-empty">?</div>
                                {/if}
                                <div class="pw-search-result-info">
                                    <p class="pw-search-result-title">{r.title}</p>
                                    {#if r.description}
                                        <p class="pw-search-result-desc">{r.description}</p>
                                    {/if}
                                    <div class="pw-search-result-tags">
                                        <span class="pw-search-result-kind">manga</span>
                                        {#if r.year}<span class="pw-search-result-year">{r.year}</span>{/if}
                                        {#if r.status}<span class="pw-search-result-year">{r.status}</span>{/if}
                                    </div>
                                </div>
                            </button>
                        {/each}
                    {:else if bookHits.length > 0}
                        {#each bookHits.slice(0, 8) as r (r.ol_key + ':' + (r.kind ?? 'book'))}
                            <button
                                class="pw-search-result"
                                class:pw-search-result-owned={r.in_library}
                                class:pw-search-result-series={r.kind === 'series'}
                                onclick={() => pickBook(r)}
                                onpointerenter={() => preloadData(bookHref(r))}
                            >
                                {#if r.kind === 'series' && r.series_covers && r.series_covers.length > 0}
                                    <div class="pw-search-poster pw-search-stack">
                                        {#each r.series_covers.slice(0, 3) as c, i (i)}
                                            <img src={c} alt="" loading="lazy" style="z-index: {3 - i};" />
                                        {/each}
                                    </div>
                                {:else if r.cover_url}
                                    <img class="pw-search-poster" src={r.cover_url} alt="" loading="lazy" />
                                {:else}
                                    <div class="pw-search-poster pw-search-poster-empty">?</div>
                                {/if}
                                <div class="pw-search-result-info">
                                    <p class="pw-search-result-title">{r.title}</p>
                                    {#if r.authors}
                                        <p class="pw-search-result-desc">{r.authors}</p>
                                    {/if}
                                    <div class="pw-search-result-tags">
                                        {#if r.kind === 'series'}
                                            <span class="pw-search-result-kind pw-search-kind-series">series</span>
                                            {#if r.series_count}<span class="pw-search-result-year"
                                                    >{r.series_count} books</span
                                                >{/if}
                                        {:else}
                                            <span class="pw-search-result-kind">book</span>
                                            {#if r.year}<span class="pw-search-result-year">{r.year}</span>{/if}
                                            {#if r.language}<span class="pw-search-result-year">{r.language}</span>{/if}
                                            {#if r.in_library}
                                                <span class="pw-search-result-owned-pill">
                                                    <svg
                                                        width="10"
                                                        height="10"
                                                        viewBox="0 0 24 24"
                                                        fill="none"
                                                        stroke="currentColor"
                                                        stroke-width="3"
                                                        stroke-linecap="round"
                                                        stroke-linejoin="round"
                                                        ><polyline points="20 6 9 17 4 12" /></svg
                                                    >
                                                    in library
                                                </span>
                                            {/if}
                                        {/if}
                                    </div>
                                </div>
                            </button>
                        {/each}
                    {:else}
                        {#each searchResults.slice(0, 8) as r (r.tmdb_id + r.media_type)}
                            {@const key = `${r.media_type}:${r.tmdb_id}`}
                            <button
                                class="pw-search-result"
                                disabled={opening === r.tmdb_id}
                                onclick={() => pickResult(r)}
                            >
                                {#if r.poster_url}
                                    <img class="pw-search-poster" src={r.poster_url} alt="" loading="lazy" />
                                {:else}
                                    <div class="pw-search-poster pw-search-poster-empty">?</div>
                                {/if}
                                <div class="pw-search-result-info">
                                    <p class="pw-search-result-title">{r.title}</p>
                                    {#if r.overview}
                                        <p class="pw-search-result-desc">{r.overview}</p>
                                    {/if}
                                    <div class="pw-search-result-tags">
                                        <span class="pw-search-result-kind"
                                            >{r.media_type === 'tv' ? 'series' : 'movie'}</span
                                        >
                                        {#if r.year}<span class="pw-search-result-year">{r.year}</span>{/if}
                                        {#if r.vote_average}<span class="pw-search-result-rating"
                                                >★ {r.vote_average.toFixed(1)}</span
                                            >{/if}
                                        {#if libByKey.has(key)}<span class="pw-search-result-inlib">in library</span
                                            >{/if}
                                    </div>
                                </div>
                            </button>
                        {/each}
                    {/if}
                </div>
            {/if}
        </div>

        <div class="pw-tb-right">
            <div class="pw-topbar-user" bind:this={menuRef} style="position: relative;">
                <button
                    class="pw-user-btn"
                    class:is-open={menuOpen}
                    onclick={() => (menuOpen = !menuOpen)}
                    aria-label={unread > 0 ? `account, ${unread} unread` : 'account'}
                >
                    <span class="pw-avatar">
                        {initial}
                        {#if unread > 0}
                            <span class="pw-avatar-dot" aria-hidden="true">{unread > 9 ? '9+' : unread}</span>
                        {/if}
                    </span>
                    <span class="pw-user-name pw-hide-sm">{user.username}</span>
                    {#if isAdmin}
                        <span class="pw-admin-badge pw-hide-sm">Admin</span>
                    {/if}
                    <span
                        style="color: rgba(220,220,225,0.5); display: flex; transform: {menuOpen
                            ? 'rotate(180deg)'
                            : 'none'}; transition: transform .2s ease;"
                    >
                        <svg
                            width="11"
                            height="11"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <polyline points="6 9 12 15 18 9" />
                        </svg>
                    </span>
                </button>

                {#if menuOpen}
                    <div class="pw-menu">
                        <UserMenu {user} onclose={() => (menuOpen = false)} />
                    </div>
                {/if}
            </div>
        </div>
    </div>
</div>
