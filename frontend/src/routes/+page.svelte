<script lang="ts">
    import { onMount } from 'svelte';
    import { fade } from 'svelte/transition';
    import { goto, afterNavigate } from '$app/navigation';
    import { page } from '$app/state';
    import {
        api,
        type Book,
        type BookHit,
        type BookShelfItem,
        type ContinueItem,
        type DiscoverResponse,
        type Manga,
        type MangaContinueItem,
        type MangaHit,
        type Media,
        type TmdbSearchItem,
        type User
    } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import { category } from '$lib/category.svelte';
    import TopBar from '$lib/components/TopBar.svelte';
    import ShelfRow from '$lib/components/ShelfRow.svelte';
    import ContinueShelf from '$lib/components/ContinueShelf.svelte';
    import DiscoverHero from '$lib/components/DiscoverHero.svelte';
    import DiscoverBrowsePanel from '$lib/components/DiscoverBrowsePanel.svelte';
    import DiscoverPreviewDrawer from '$lib/components/DiscoverPreviewDrawer.svelte';
    import TmdbShelf from '$lib/components/TmdbShelf.svelte';
    import MangaShelf from '$lib/components/MangaShelf.svelte';
    import BookShelf from '$lib/components/BookShelf.svelte';
    import BookGreeting from '$lib/components/BookGreeting.svelte';
    import DailyQuote from '$lib/components/DailyQuote.svelte';
    import MyBooks from '$lib/components/MyBooks.svelte';
    import MediaCollection from '$lib/components/MediaCollection.svelte';
    import ReadingNow from '$lib/components/ReadingNow.svelte';
    import YearInReview from '$lib/components/YearInReview.svelte';

    let user = $state<User | null>(null);
    let media = $state<Media[]>([]);
    let continueItems = $state<ContinueItem[]>([]);
    let progress = $state<Record<string, number>>({});
    let discover = $state<DiscoverResponse | null>(null);
    let discoverLoading = $state(false);
    let discoverPreview = $state<TmdbSearchItem | null>(null);
    let similarShelves = $state<{ title: string; items: TmdbSearchItem[] }[]>([]);
    let loading = $state(true);

    const tab = $derived(page.url.searchParams.get('tab') ?? 'library');

    function setTab(k: string) {
        if (k === tab) return;
        const url = new URL(window.location.href);
        if (k === 'library') url.searchParams.delete('tab');
        else url.searchParams.set('tab', k);
        goto(url.pathname + url.search, { replaceState: false, noScroll: true, keepFocus: true });
    }

    let mangaLib = $state<Manga[]>([]);
    let mangaCont = $state<MangaContinueItem[]>([]);
    let mangaLibLoading = $state(false);
    let mangaLibLoaded = $state(false);
    let mangaMine = $state<Manga[]>([]);
    let mangaMineLoading = $state(false);
    let mangaMineLoaded = $state(false);
    let mangaPop = $state<MangaHit[]>([]);
    let mangaPopLoading = $state(false);
    let mangaPopLoaded = $state(false);

    $effect(() => {
        if (category.current !== 'manga') return;
        if (tab === 'library' && !mangaLibLoaded && !mangaLibLoading) {
            mangaLibLoading = true;
            Promise.all([
                api.mangaList().catch((e) => {
                    console.error('[manga] list failed', e);
                    return [] as Manga[];
                }),
                api.mangaContinue().catch(() => [] as MangaContinueItem[])
            ])
                .then(([m, c]) => {
                    mangaLib = m;
                    mangaCont = c;
                    mangaLibLoaded = true;
                })
                .finally(() => (mangaLibLoading = false));
        }
        if (tab === 'personal' && !mangaMineLoaded && !mangaMineLoading) {
            mangaMineLoading = true;
            api.mangaList({ mine: true })
                .then((m) => {
                    mangaMine = m;
                    mangaMineLoaded = true;
                })
                .catch((e) => {
                    console.error('[manga] collection failed', e);
                })
                .finally(() => (mangaMineLoading = false));
        }
        if (tab === 'discover' && !mangaPopLoaded && !mangaPopLoading) {
            mangaPopLoading = true;
            api.mangaPopular()
                .then((m) => {
                    mangaPop = m;
                    mangaPopLoaded = true;
                })
                .catch((e) => {
                    console.error('[manga] popular failed', e);
                })
                .finally(() => (mangaPopLoading = false));
        }
    });

    async function removeManga(m: { md_id: string; title: string }) {
        if (!confirm(`remove "${m.title}" from library?`)) return;
        try {
            await api.mangaDelete(m.md_id);
            mangaLib = mangaLib.filter((x) => x.md_id !== m.md_id);
            mangaMine = mangaMine.filter((x) => x.md_id !== m.md_id);
        } catch (e) {
            console.error('[manga] delete failed', e);
        }
    }

    const mangaContSub = $derived(
        Object.fromEntries(mangaCont.map((c) => [c.md_id, `ch. ${c.chapter ?? '?'} · p. ${c.page + 1}`]))
    );

    let bookLib = $state<Book[]>([]);
    let bookShelfItems = $state<BookShelfItem[]>([]);
    let bookReadYear = $state(0);
    let bookLibLoading = $state(false);
    let bookLibLoaded = $state(false);
    let yirOpen = $state(false);
    let bookPop = $state<BookHit[]>([]);
    let bookPopLoading = $state(false);
    let bookPopLoaded = $state(false);

    $effect(() => {
        if (category.current !== 'books') return;
        if (tab === 'library' && !bookLibLoaded && !bookLibLoading) {
            bookLibLoading = true;
            Promise.all([
                api.bookList().catch((e) => {
                    console.error('[books] list failed', e);
                    return [] as Book[];
                }),
                api.bookShelf().catch(() => null)
            ])
                .then(([b, s]) => {
                    bookLib = b;
                    if (s) {
                        bookShelfItems = s.items;
                        bookReadYear = s.read_year;
                        if (s.items.some((book) => !book.cover_url)) {
                            setTimeout(() => {
                                Promise.all([api.bookList(), api.bookShelf()])
                                    .then(([freshBooks, freshShelf]) => {
                                        bookLib = freshBooks;
                                        bookShelfItems = freshShelf.items;
                                        bookReadYear = freshShelf.read_year;
                                    })
                                    .catch(() => {});
                            }, 1800);
                        }
                    }
                    bookLibLoaded = true;
                })
                .finally(() => (bookLibLoading = false));
        }
        if (tab === 'discover' && !bookPopLoaded && !bookPopLoading) {
            bookPopLoading = true;
            api.bookPopular()
                .then((b) => {
                    bookPop = b;
                    bookPopLoaded = true;
                })
                .catch((e) => console.error('[books] popular failed', e))
                .finally(() => (bookPopLoading = false));
        }
    });

    async function removeBook(b: { ol_key: string; title: string }) {
        if (!confirm(`remove "${b.title}" from library?`)) return;
        try {
            await api.bookDelete(b.ol_key);
            bookLib = bookLib.filter((x) => x.ol_key !== b.ol_key);
        } catch (e) {
            console.error('[books] delete failed', e);
        }
    }

    $effect(() => {
        if (category.current !== 'video') return;
        if (tab === 'discover' && !discover && !discoverLoading) {
            discoverLoading = true;
            api.discover()
                .then((d) => {
                    discover = d;
                })
                .catch((e) => {
                    console.error('[discover] failed', e);
                })
                .finally(() => {
                    discoverLoading = false;
                });

            loadSimilarShelves();
        }
    });

    async function loadSimilarShelves() {
        try {
            const cw = continueItems.length > 0 ? continueItems : await api.continueWatching().catch(() => []);
            const candidates = cw.filter((c) => c.tmdb_id != null).slice(0, 2);
            const prefix = t('discover.because_you_watched');
            const shelves: { title: string; items: TmdbSearchItem[] }[] = [];
            for (const it of candidates) {
                const kind = it.media_type === 'tv' ? 'tv' : 'movie';
                try {
                    const sim = await api.tmdbSimilar(kind, it.tmdb_id!);
                    if (sim.length > 0) {
                        shelves.push({ title: `${prefix} ${it.media_title}`, items: sim });
                    }
                } catch {}
            }
            similarShelves = shelves;
        } catch {}
    }

    function openDiscoverPreview(item: TmdbSearchItem) {
        discoverPreview = item;
    }

    onMount(async () => {
        try {
            user = await api.me();
        } catch {
            goto('/login', { replaceState: true });
            return;
        }
        if (user.role === 'admin') {
            try {
                const s = await api.getSettings();
                if (!s.tmdb_ready) {
                    goto('/onboarding', { replaceState: true });
                    return;
                }
            } catch {}
        }
        const cwP = api.continueWatching().catch(() => [] as ContinueItem[]);
        const psP = api.watchSummary().catch(() => []);
        media = await api.listMedia().catch((e) => {
            console.error('[lib] failed to load media', e);
            return [] as Media[];
        });
        lastMediaSync = Date.now();
        loading = false;

        cwP.then((cw) => {
            continueItems = cw;
            lastContinueSync = Date.now();
        });
        psP.then((ps) => {
            for (const p of ps) {
                if (p.duration > 0) progress[p.media_id] = Math.round((p.position / p.duration) * 100);
            }
        });
    });

    let lastContinueSync = 0;
    let continueRefreshing = false;
    let lastMediaSync = 0;
    let mediaRefreshing = false;

    async function refreshMedia(force = false) {
        if (mediaRefreshing) return;
        if (!force && Date.now() - lastMediaSync < 30000) return;
        mediaRefreshing = true;
        try {
            media = await api.listMedia();
            lastMediaSync = Date.now();
        } catch (e) {
            console.error('[lib] media refresh failed', e);
        } finally {
            mediaRefreshing = false;
        }
    }

    async function refreshContinue(force = false) {
        if (continueRefreshing) return;
        if (!force && Date.now() - lastContinueSync < 15000) return;
        continueRefreshing = true;
        try {
            continueItems = await api.continueWatching();
            lastContinueSync = Date.now();
        } catch {
        } finally {
            continueRefreshing = false;
        }
    }

    onMount(() => {
        const onVis = () => {
            if (document.visibilityState !== 'visible') return;
            refreshContinue();
            if (category.current === 'video') refreshMedia();
        };
        document.addEventListener('visibilitychange', onVis);
        window.addEventListener('focus', onVis);
        return () => {
            document.removeEventListener('visibilitychange', onVis);
            window.removeEventListener('focus', onVis);
        };
    });

    afterNavigate(({ from, to }) => {
        const toPath = to?.url?.pathname;
        const fromPath = from?.url?.pathname;
        if (toPath !== '/' || !fromPath || fromPath === toPath) return;
        refreshContinue(true);
        if (category.current === 'video') refreshMedia(true);
    });

    async function removeFromLibrary(m: Media) {
        if (!confirm(`remove "${m.title}" from library?`)) return;
        try {
            await api.deleteMedia(m.id);
            media = media.filter((x) => x.id !== m.id);
        } catch (e) {
            console.error('[lib] delete failed', e);
        }
    }

    const isAdmin = $derived(user?.role === 'admin');
    const onRemove = $derived(isAdmin ? removeFromLibrary : undefined);

    const recentlyAdded = $derived(media.slice(0, 16));
    const movies = $derived(media.filter((m) => m.media_type === 'movie'));
    const series = $derived(media.filter((m) => m.media_type === 'tv' && !m.is_anime));
    const animes = $derived(media.filter((m) => m.is_anime));

    const bookStatus = $derived(new Map(bookShelfItems.map((item) => [item.ol_key, item.status])));
    const bookReading = $derived(bookShelfItems.filter((item) => item.status === 'reading'));
    const bookToRead = $derived(bookLib.filter((book) => (bookStatus.get(book.ol_key) ?? 'want') === 'want'));
    const bookCompleted = $derived(bookLib.filter((book) => bookStatus.get(book.ol_key) === 'read'));
    const bookCompletedKeys = $derived(new Set(bookCompleted.map((book) => book.ol_key)));

    const cat = $derived(category.current);

    const tabs = $derived([
        { key: 'library', label: t('tab.library') },
        { key: 'personal', label: t('tab.personal') },
        { key: 'discover', label: t('tab.discover') }
    ]);
</script>

<svelte:head>
    <title
        >{tab === 'personal' ? t('tab.personal') : cat === 'video' ? t(`tab.${tab}`) : t(`cat.${cat}`)} - pleasewatch</title
    >
</svelte:head>

{#if user}
    <div class="pw-page">
        <TopBar {user} {tabs} activeTab={tab} onTab={setTab} />

        {#key `${cat}:${tab}`}
            <div in:fade={{ duration: 180, delay: 60 }} out:fade={{ duration: 100 }}>
                {#if cat === 'books'}
                    {#if tab === 'library'}
                        {#if bookLibLoading && !bookLibLoaded}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('lib.loading')}</div>
                                </div>
                            </section>
                        {:else if bookLib.length === 0}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('books.empty.tag')}</div>
                                    <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('books.empty.title')}</h2>
                                    <p class="pw-empty-text">{t('books.empty.body')}</p>
                                </div>
                            </section>
                        {:else}
                            <div class="pw-section pw-section-tight">
                                <BookGreeting
                                    {user}
                                    reading={bookReading.map((book) => ({
                                        ol_key: book.ol_key,
                                        title: book.title,
                                        percent: book.percent
                                    }))}
                                    waiting={bookToRead.length}
                                    thisYearRead={bookReadYear}
                                    year={new Date().getFullYear()}
                                    onYearReview={() => (yirOpen = true)}
                                />
                                <DailyQuote />
                                {#if bookReading.length > 0}
                                    <ReadingNow title={t('books.my.reading')} items={bookReading} />
                                {/if}
                                {#if bookToRead.length > 0}
                                    <BookShelf
                                        title={t('books.my.toread')}
                                        items={bookToRead}
                                        onRemove={isAdmin ? removeBook : undefined}
                                    />
                                {/if}
                                {#if bookCompleted.length > 0}
                                    <BookShelf
                                        title={t('books.my.completed')}
                                        items={bookCompleted}
                                        onRemove={isAdmin ? removeBook : undefined}
                                        done={bookCompletedKeys}
                                    />
                                {/if}
                            </div>
                        {/if}
                    {:else if tab === 'personal'}
                        <MyBooks />
                    {:else}
                        {#if bookPopLoading && !bookPopLoaded}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('lib.loading')}</div>
                                </div>
                            </section>
                        {:else}
                            <div class="pw-section pw-section-tight">
                                <BookShelf title={t('books.popular')} items={bookPop} />
                            </div>
                        {/if}
                    {/if}
                {:else if cat === 'manga'}
                    {#if tab === 'library'}
                        {#if mangaLibLoading && !mangaLibLoaded}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('lib.loading')}</div>
                                </div>
                            </section>
                        {:else if mangaLib.length === 0}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('manga.empty.tag')}</div>
                                    <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('manga.empty.title')}</h2>
                                    <p class="pw-empty-text">{t('manga.empty.body')}</p>
                                </div>
                            </section>
                        {:else}
                            <div class="pw-section pw-section-tight">
                                {#if mangaCont.length > 0}
                                    <MangaShelf
                                        title={t('manga.continue_shelf')}
                                        items={mangaCont}
                                        sub={mangaContSub}
                                        width={150}
                                    />
                                {/if}
                                <MangaShelf
                                    title={t('cat.manga')}
                                    items={mangaLib}
                                    onRemove={isAdmin ? removeManga : undefined}
                                />
                            </div>
                        {/if}
                    {:else if tab === 'personal'}
                        {#if mangaMineLoading && !mangaMineLoaded}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('lib.loading')}</div>
                                </div>
                            </section>
                        {:else if mangaMine.length === 0}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('personal.tag')}</div>
                                    <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('personal.title')}</h2>
                                    <p class="pw-empty-text">{t('manga.empty.body')}</p>
                                </div>
                            </section>
                        {:else}
                            <div class="pw-section pw-section-tight">
                                <MangaShelf title={t('tab.personal')} items={mangaMine} onRemove={removeManga} />
                            </div>
                        {/if}
                    {:else}
                        {#if mangaPopLoading && !mangaPopLoaded}
                            <section class="pw-section pw-empty">
                                <div class="pw-empty-card">
                                    <div class="pw-empty-tag">{t('lib.loading')}</div>
                                </div>
                            </section>
                        {:else}
                            <div class="pw-section pw-section-tight">
                                <MangaShelf title={t('manga.popular')} items={mangaPop} />
                            </div>
                        {/if}
                    {/if}
                {:else if tab === 'library'}
                    {#if loading}
                        <section class="pw-section pw-empty">
                            <div class="pw-empty-card">
                                <div class="pw-empty-tag">{t('lib.loading')}</div>
                            </div>
                        </section>
                    {:else if media.length === 0}
                        <section class="pw-section pw-empty">
                            <div class="pw-empty-card">
                                <div class="pw-empty-tag">{t('lib.empty.tag')}</div>
                                <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('lib.empty.title')}</h2>
                                <p class="pw-empty-text">
                                    {t('lib.empty.body')}
                                </p>
                            </div>
                        </section>
                    {:else}
                        <div class="pw-section pw-section-tight">
                            <ContinueShelf bind:items={continueItems} />
                            <ShelfRow
                                title={t('shelves.recently_added')}
                                items={recentlyAdded}
                                {onRemove}
                                {progress}
                                limit={8}
                                showActivity
                            />
                            {#if movies.length > 0}<ShelfRow
                                    title={t('shelves.movies')}
                                    items={movies}
                                    {onRemove}
                                    {progress}
                                    layout="grid"
                                />{/if}
                            {#if series.length > 0}<ShelfRow
                                    title={t('shelves.tv_series')}
                                    items={series}
                                    {onRemove}
                                    {progress}
                                    layout="grid"
                                />{/if}
                            {#if animes.length > 0}<ShelfRow
                                    title={t('shelves.animations')}
                                    items={animes}
                                    {onRemove}
                                    {progress}
                                    layout="grid"
                                />{/if}
                        </div>
                    {/if}
                {:else if tab === 'personal'}
                    <MediaCollection />
                {:else if tab === 'discover'}
                    {#if discoverLoading && !discover}
                        <section class="pw-section pw-empty">
                            <div class="pw-empty-card">
                                <div class="pw-empty-tag">{t('lib.loading')}</div>
                            </div>
                        </section>
                    {:else if discover}
                        <DiscoverHero items={discover.trending} />
                        <DiscoverBrowsePanel onPreview={openDiscoverPreview} />
                        <div class="pw-section pw-section-tight">
                            <TmdbShelf
                                title={t('discover.trending')}
                                items={discover.trending}
                                onPreview={openDiscoverPreview}
                            />
                            <TmdbShelf
                                title={t('discover.popular_movies')}
                                items={discover.popular_movies}
                                onPreview={openDiscoverPreview}
                            />
                            <TmdbShelf
                                title={t('discover.popular_tv')}
                                items={discover.popular_tv}
                                onPreview={openDiscoverPreview}
                            />
                            <TmdbShelf
                                title={t('discover.top_rated_movies')}
                                items={discover.top_rated_movies}
                                onPreview={openDiscoverPreview}
                            />
                            <TmdbShelf
                                title={t('discover.top_rated_tv')}
                                items={discover.top_rated_tv}
                                onPreview={openDiscoverPreview}
                            />
                            {#each similarShelves as shelf (shelf.title)}
                                <TmdbShelf title={shelf.title} items={shelf.items} onPreview={openDiscoverPreview} />
                            {/each}
                        </div>
                    {:else}
                        <section class="pw-section pw-empty">
                            <div class="pw-empty-card">
                                <div class="pw-empty-tag">{t('discover.tag')}</div>
                                <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('discover.title')}</h2>
                                <p class="pw-empty-text">{t('discover.body')}</p>
                            </div>
                        </section>
                    {/if}
                {/if}
            </div>
        {/key}

        <div style="height: 60px;"></div>
    </div>

    {#if yirOpen}
        <YearInReview items={bookShelfItems} year={new Date().getFullYear()} onClose={() => (yirOpen = false)} />
    {/if}
    {#if discoverPreview}
        <DiscoverPreviewDrawer item={discoverPreview} onClose={() => (discoverPreview = null)} />
    {/if}
{/if}
