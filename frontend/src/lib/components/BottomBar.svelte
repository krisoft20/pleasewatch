<script lang="ts">
    import { tick } from 'svelte';
    import { goto, preloadData } from '$app/navigation';
    import { page } from '$app/state';
    import { api, type BookHit, type MangaHit, type TmdbSearchItem, type User } from '$lib/api';
    import { bookCoverSrc, retryBookCover, validateBookCover } from '$lib/bookCover';
    import { t } from '$lib/i18n.svelte';
    import { category, CATEGORIES, type Category } from '$lib/category.svelte';
    import { clickOutside } from '$lib/dismiss';
    import UserMenu from '$lib/components/UserMenu.svelte';

    let user = $state<User | null>(null);
    let sheet = $state<'cat' | 'search' | 'me' | null>(null);

    const NO_BAR = ['/login', '/register', '/onboarding', '/watch', '/party', '/read'];
    const hidden = $derived(NO_BAR.some((p) => page.url.pathname.startsWith(p)));

    $effect(() => {
        if (user || hidden) return;
        api.me()
            .then((u) => {
                user = u;
            })
            .catch(() => {});
    });

    let q = $state('');
    let results = $state<TmdbSearchItem[]>([]);
    let mangaHits = $state<MangaHit[]>([]);
    let bookHits = $state<BookHit[]>([]);
    let busy = $state(false);
    let searchInput = $state<HTMLInputElement>();
    let timer: ReturnType<typeof setTimeout> | undefined;

    function close() {
        sheet = null;
        q = '';
        results = [];
        mangaHits = [];
        bookHits = [];
    }

    function toggle(s: 'cat' | 'search' | 'me') {
        sheet = sheet === s ? null : s;
        if (sheet === 'search') tick().then(() => searchInput?.focus());
    }

    function pickCategory(c: Category) {
        category.set(c);
        close();
        if (page.url.pathname !== '/') goto('/');
    }

    function onSearchInput() {
        clearTimeout(timer);
        if (q.trim().length < 2) {
            results = [];
            mangaHits = [];
            bookHits = [];
            return;
        }
        timer = setTimeout(async () => {
            busy = true;
            try {
                if (category.current === 'manga') {
                    mangaHits = await api.mangaSearch(q);
                    results = [];
                    bookHits = [];
                } else if (category.current === 'books') {
                    bookHits = await api.bookSearch(q);
                    results = [];
                    mangaHits = [];
                } else {
                    results = await api.search(q);
                    mangaHits = [];
                    bookHits = [];
                }
            } catch {
                results = [];
                mangaHits = [];
                bookHits = [];
            } finally {
                busy = false;
            }
        }, 250);
    }

    function pickResult(item: TmdbSearchItem) {
        const route = item.media_type === 'tv' ? `/tv/${item.tmdb_id}` : `/movie/${item.tmdb_id}`;
        close();
        goto(route);
    }

    const initial = $derived(user ? user.username.charAt(0).toUpperCase() : '');
</script>

{#if user && !hidden}
    <nav class="pw-bb">
        <button class="pw-bb-item" class:is-on={sheet === 'cat'} onclick={() => toggle('cat')}>
            <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" />
                <rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" />
            </svg>
            <span>{t(`cat.${category.current}`)}</span>
        </button>
        <button class="pw-bb-item" class:is-on={sheet === 'search'} onclick={() => toggle('search')}>
            <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <circle cx="11" cy="11" r="7" />
                <path d="m20 20-3.5-3.5" />
            </svg>
            <span>{t('nav.search')}</span>
        </button>
        <button class="pw-bb-item" class:is-on={sheet === 'me'} onclick={() => toggle('me')}>
            <span class="pw-bb-ava">{initial}</span>
            <span>{t('nav.profile')}</span>
        </button>
    </nav>

    {#if sheet === 'cat'}
        <div class="pw-bb-scrim">
            <div class="pw-bb-sheet" use:clickOutside={close}>
                {#each CATEGORIES as c (c)}
                    <button class="pw-bb-row" class:is-active={c === category.current} onclick={() => pickCategory(c)}>
                        {t(`cat.${c}`)}
                        {#if c === category.current}
                            <svg
                                width="15"
                                height="15"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2.2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                            >
                                <polyline points="20 6 9 17 4 12" />
                            </svg>
                        {/if}
                    </button>
                {/each}
            </div>
        </div>
    {:else if sheet === 'me'}
        <div class="pw-bb-scrim">
            <div class="pw-bb-sheet pw-bb-sheet-menu" use:clickOutside={close}>
                <UserMenu {user} onclose={close} />
            </div>
        </div>
    {:else if sheet === 'search'}
        <div class="pw-bb-find">
            <div class="pw-bb-find-top">
                <input
                    bind:this={searchInput}
                    bind:value={q}
                    oninput={onSearchInput}
                    placeholder={t('search.placeholder')}
                    spellcheck="false"
                    autocomplete="off"
                />
                <button class="pw-bb-find-cancel" onclick={close}>{t('buttons.cancel')}</button>
            </div>
            <div class="pw-bb-find-results">
                {#if busy && results.length === 0 && mangaHits.length === 0 && bookHits.length === 0}
                    <div class="pw-search-empty"><div class="pw-search-spin"></div></div>
                {:else if q.trim().length >= 2 && results.length === 0 && mangaHits.length === 0 && bookHits.length === 0 && !busy}
                    <div class="pw-search-empty">no results for "{q}"</div>
                {:else if mangaHits.length > 0}
                    {#each mangaHits.slice(0, 12) as r (r.md_id)}
                        <button
                            class="pw-search-result"
                            onclick={() => {
                                close();
                                goto(`/manga/${r.md_id}`);
                            }}
                            onpointerdown={() => preloadData(`/manga/${r.md_id}`)}
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
                                </div>
                            </div>
                        </button>
                    {/each}
                {:else if bookHits.length > 0}
                    {#each bookHits.slice(0, 12) as r (r.ol_key + ':' + (r.kind ?? 'book'))}
                        {@const href =
                            r.kind === 'series' ? `/series/${encodeURIComponent(r.ol_key)}` : `/book/${r.ol_key}`}
                        <button
                            class="pw-search-result"
                            class:pw-search-result-owned={r.in_library}
                            class:pw-search-result-series={r.kind === 'series'}
                            onclick={() => {
                                close();
                                goto(href);
                            }}
                            onpointerdown={() => preloadData(href)}
                        >
                            {#if r.kind === 'series' && r.series_covers && r.series_covers.length > 0}
                                <div class="pw-search-poster pw-search-stack">
                                    {#each r.series_covers.slice(0, 3) as c, i (i)}
                                        <img
                                            src={bookCoverSrc(c)}
                                            alt=""
                                            loading="lazy"
                                            onload={(event) => validateBookCover(event, c)}
                                            onerror={(event) => retryBookCover(event, c)}
                                        />
                                    {/each}
                                </div>
                            {:else if r.cover_url}
                                <img
                                    class="pw-search-poster"
                                    src={bookCoverSrc(r.cover_url)}
                                    alt=""
                                    loading="lazy"
                                    onload={(event) => validateBookCover(event, r.cover_url!)}
                                    onerror={(event) => retryBookCover(event, r.cover_url!)}
                                />
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
                                                    stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
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
                    {#each results.slice(0, 12) as r (r.tmdb_id + r.media_type)}
                        <button class="pw-search-result" onclick={() => pickResult(r)}>
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
                                </div>
                            </div>
                        </button>
                    {/each}
                {/if}
            </div>
        </div>
    {/if}
{/if}

<style>
    .pw-bb {
        position: fixed;
        left: 0;
        right: 0;
        bottom: 0;
        z-index: 90;
        display: none;
        background: rgba(8, 9, 12, 0.92);
        backdrop-filter: blur(14px);
        -webkit-backdrop-filter: blur(14px);
        border-top: 1px solid rgba(255, 255, 255, 0.07);
        padding-bottom: env(safe-area-inset-bottom);
    }
    .pw-bb-item {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 3px;
        background: none;
        border: none;
        cursor: pointer;
        padding: 9px 0 7px;
        color: rgba(220, 220, 225, 0.55);
        font-size: 10.5px;
        font-weight: 500;
    }
    .pw-bb-item.is-on {
        color: var(--pw-accent);
    }
    .pw-bb-ava {
        width: 19px;
        height: 19px;
        border-radius: 999px;
        background: color-mix(in oklch, var(--pw-accent) 30%, #1a1a1f);
        display: grid;
        place-items: center;
        font-size: 9.5px;
        font-weight: 600;
        color: var(--pw-accent);
        border: 1px solid color-mix(in oklch, var(--pw-accent) 50%, transparent);
    }
    .pw-bb-scrim {
        position: fixed;
        inset: 0;
        z-index: 95;
        background: rgba(0, 0, 0, 0.55);
        display: none;
        flex-direction: column;
        justify-content: flex-end;
    }
    .pw-bb-sheet {
        background: rgba(12, 13, 17, 0.98);
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px 16px 0 0;
        padding: 10px 10px calc(14px + env(safe-area-inset-bottom));
        animation: pw-bb-up 0.22s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    .pw-bb-sheet-menu {
        padding: 0 0 calc(8px + env(safe-area-inset-bottom));
        overflow: hidden;
    }
    @keyframes pw-bb-up {
        from {
            transform: translateY(40px);
            opacity: 0;
        }
    }
    .pw-bb-row {
        width: 100%;
        display: flex;
        align-items: center;
        justify-content: space-between;
        background: none;
        border: none;
        cursor: pointer;
        padding: 13px 12px;
        border-radius: 10px;
        color: rgba(220, 220, 225, 0.75);
        font-size: 15px;
    }
    .pw-bb-row.is-active {
        color: #ececef;
        background: rgba(255, 255, 255, 0.06);
    }
    .pw-bb-row svg {
        color: var(--pw-accent);
    }
    .pw-bb-find {
        position: fixed;
        inset: 0;
        z-index: 95;
        background: rgba(8, 9, 12, 0.98);
        display: none;
        flex-direction: column;
    }
    .pw-bb-find-top {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: calc(12px + env(safe-area-inset-top)) 12px 10px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .pw-bb-find-top input {
        flex: 1;
        min-width: 0;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 999px;
        padding: 9px 16px;
        color: #ececef;
        font-size: 16px;
        outline: none;
    }
    .pw-bb-find-cancel {
        background: none;
        border: none;
        color: var(--pw-accent);
        font-size: 15px;
        font-weight: 500;
        padding: 0 6px 0 10px;
        cursor: pointer;
        white-space: nowrap;
    }
    .pw-bb-find-results {
        flex: 1;
        overflow-y: auto;
        padding: 6px 8px calc(20px + env(safe-area-inset-bottom));
    }
    @media (max-width: 640px) {
        .pw-bb {
            display: flex;
        }
        .pw-bb-scrim {
            display: flex;
        }
        .pw-bb-find {
            display: flex;
        }
    }
</style>
