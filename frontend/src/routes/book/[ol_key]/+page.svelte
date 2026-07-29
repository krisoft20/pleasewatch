<script lang="ts">
    import { onMount } from 'svelte';
    import { goto, afterNavigate } from '$app/navigation';
    import { page } from '$app/state';
    import { api, type BookDetail, type BookHit, type BookSource, type User } from '$lib/api';
    import { t, plural } from '$lib/i18n.svelte';
    import { clickOutside } from '$lib/dismiss';
    import TopBar from '$lib/components/TopBar.svelte';
    import TorrentPicker from '$lib/components/TorrentPicker.svelte';
    import AuthorModal from '$lib/components/AuthorModal.svelte';

    const olKey = $derived(page.params.ol_key ?? '');

    let user = $state<User | null>(null);
    let detail = $state<BookDetail | null>(null);
    let loading = $state(true);
    let err = $state('');
    let busy = $state(false);

    let shelfOpen = $state(false);
    let sources = $state<BookSource[]>([]);
    let sourcesOpen = $state(false);
    let sourcesLoading = $state(false);
    let sourcesLoaded = $state(false);
    let sourcesErr = $state('');

    let fetching = $state<string | null>(null);
    let uploadBusy = $state(false);

    let seriesBooks = $state<BookHit[]>([]);
    let seriesName = $state<string | null>(null);

    let moreByAuthor = $state<BookHit[]>([]);
    let authorName = $state<string | null>(null);

    let descOpen = $state(false);
    let formatFilter = $state<'all' | 'epub' | 'pdf' | 'other'>('all');
    let langFilter = $state<'all' | 'en'>('all');

    let tpOpen = $state(false);
    let authorOpenOlid = $state<string | null>(null);

    let fileInput: HTMLInputElement | undefined = $state();

    let lastKey = '';
    let safetyTimer: ReturnType<typeof setTimeout> | null = null;
    let authorRefreshToken = 0;

    async function bootstrap(k: string) {
        if (!k || k === lastKey) return;
        lastKey = k;
        const refreshToken = ++authorRefreshToken;
        detail = null;
        seriesBooks = [];
        seriesName = null;
        moreByAuthor = [];
        authorName = null;
        descOpen = false;
        formatFilter = 'all';
        langFilter = 'all';
        err = '';
        loading = true;
        sourcesLoaded = false;
        sources = [];
        if (safetyTimer) clearTimeout(safetyTimer);
        safetyTimer = setTimeout(() => {
            if (loading) {
                loading = false;
                if (!detail) err = 'request timed out, try refresh';
            }
        }, 12000);
        try {
            await loadDetail();
            const loadedDetail = detail as BookDetail | null;
            if (loadedDetail && !loadedDetail.author_keys?.length) {
                void refreshAuthorKeys(k, refreshToken);
            }
        } finally {
            loading = false;
            if (safetyTimer) {
                clearTimeout(safetyTimer);
                safetyTimer = null;
            }
        }
    }

    onMount(async () => {
        try {
            user = await api.me();
        } catch {
            goto('/login');
            return;
        }
        await bootstrap(olKey);
    });

    afterNavigate(() => {
        if (user) bootstrap(olKey);
    });

    async function loadDetail() {
        try {
            detail = await api.bookDetail(olKey);
        } catch (e) {
            console.error('[book] detail failed', e);
            if (!detail) err = 'could not load book';
        }
        const s = detail?.book.series ?? null;
        if (s && s !== seriesName) {
            seriesName = s;
            api.bookSeries(s)
                .then((rs) => (seriesBooks = rs))
                .catch((e) => console.error('[book] series fetch failed', e));
        }
        const firstAuthor = detail?.book.authors?.split(/[,;]/)[0]?.trim() ?? null;
        if (firstAuthor && firstAuthor !== authorName) {
            authorName = firstAuthor;
            api.bookByAuthor(firstAuthor, olKey)
                .then((rs) => (moreByAuthor = rs))
                .catch((e) => console.error('[book] author fetch failed', e));
        }
    }

    async function refreshAuthorKeys(k: string, token: number) {
        for (const delay of [1500, 3000, 6000, 9000]) {
            await new Promise((resolve) => setTimeout(resolve, delay));
            if (token !== authorRefreshToken || lastKey !== k) return;
            try {
                const fresh = await api.bookDetail(k);
                if (fresh.author_keys?.length) {
                    if (token === authorRefreshToken && lastKey === k) detail = fresh;
                    return;
                }
            } catch {}
        }
    }

    $effect(() => {
        if (detail?.book.status !== 'processing') return;
        const timer = setInterval(loadDetail, 2000);
        return () => clearInterval(timer);
    });

    async function toggleLibrary() {
        if (!detail || busy) return;
        busy = true;
        try {
            if (detail.in_library) {
                await api.bookDelete(olKey);
                await loadDetail();
            } else {
                await api.bookAdd(olKey);
                await loadDetail();
            }
        } catch (e) {
            console.error('[book] library toggle failed', e);
        } finally {
            busy = false;
        }
    }

    function openSources() {
        sourcesOpen = true;
        loadSources();
    }

    function openTorrentPicker() {
        sourcesOpen = false;
        tpOpen = true;
    }

    function torrentQuery(): string {
        if (!detail) return '';
        const author = (detail.book.authors ?? '').split(/[,;]/)[0]?.trim() ?? '';
        return `${detail.book.title} ${author}`.trim();
    }

    function closeSources() {
        if (fetching) return;
        sourcesOpen = false;
    }

    async function loadSources() {
        if (sourcesLoading || sourcesLoaded) return;
        sourcesLoading = true;
        sourcesErr = '';
        try {
            sources = await api.bookSources(olKey);
            sourcesLoaded = true;
            if (sources.length === 0) sourcesErr = t('books.sources.none');
        } catch (e) {
            console.error('[book] sources failed', e);
            sourcesErr = t('books.sources.fail');
        } finally {
            sourcesLoading = false;
        }
    }

    async function fetchSource(s: BookSource) {
        if (fetching) return;
        fetching = s.md5;
        try {
            await api.bookFetch(olKey, s.md5, s.ext);
            await loadDetail();
            sourcesOpen = false;
        } catch (e) {
            console.error('[book] fetch failed', e);
            sourcesErr = `${t('books.fetch.fail')}: ${(e as Error).message}`;
        } finally {
            fetching = null;
        }
    }

    async function uploadFile(e: Event) {
        const target = e.target as HTMLInputElement;
        const file = target.files?.[0];
        if (!file) return;
        uploadBusy = true;
        try {
            await api.bookUpload(olKey, file);
            await loadDetail();
            sourcesOpen = false;
        } catch (err) {
            console.error('[book] upload failed', err);
        } finally {
            uploadBusy = false;
            target.value = '';
        }
    }

    function read() {
        goto(`/read-book/${olKey}`);
    }

    function readAgain() {
        goto(`/read-book/${olKey}?restart=1`);
    }

    async function setShelf(status: string) {
        if (!detail) return;
        shelfOpen = false;
        try {
            await api.bookShelfSet(olKey, status);
            detail.shelf = status;
            if (!detail.in_library) await loadDetail();
        } catch (e) {
            console.error('[book] shelf set failed', e);
        }
    }

    async function unshelf() {
        if (!detail) return;
        shelfOpen = false;
        try {
            await api.bookShelfRemove(olKey);
            detail.shelf = null;
        } catch (e) {
            console.error('[book] shelf remove failed', e);
        }
    }

    function fmtCount(n: number): string {
        return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`;
    }

    function download() {
        window.location.href = api.bookDownloadUrl(olKey);
    }

    function onEsc(e: KeyboardEvent) {
        if (e.key === 'Escape' && shelfOpen) shelfOpen = false;
        else if (e.key === 'Escape' && sourcesOpen) closeSources();
    }

    function fmtSize(n: number | null): string {
        if (!n) return '';
        if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)}MB`;
        if (n >= 1024) return `${Math.round(n / 1024)}KB`;
        return `${n}B`;
    }

    const title = $derived(detail?.book.title ?? 'book');
    const status = $derived(detail?.book.status ?? '');
    const rawProgress = $derived(detail?.progress?.percent ?? 0);
    const pct = $derived(Math.round(rawProgress * 100));
    const annaUrl = $derived(
        detail
            ? `/api/books/anna?q=${encodeURIComponent(`${detail.book.title} ${detail.book.authors ?? ''}`.trim())}`
            : ''
    );
    const descLong = $derived((detail?.book.description?.length ?? 0) > 600);

    const firstAuthorOlid = $derived(detail?.author_keys?.[0] ?? null);
    const authorAvatarUrl = $derived(firstAuthorOlid ? `/api/books/author-cover/${firstAuthorOlid}.jpg` : null);
    const authorInitial = $derived(
        (detail?.book.authors ?? '')
            .trim()
            .split(/[\s,;]/)
            .filter(Boolean)[0]
            ?.charAt(0)
            .toUpperCase() ?? ''
    );
    let avatarFailed = $state(false);
    $effect(() => {
        if (authorAvatarUrl) avatarFailed = false;
    });

    const filteredSources = $derived.by(() => {
        return sources.filter((s) => {
            const e = s.ext.toLowerCase();
            if (formatFilter === 'epub' && e !== 'epub') return false;
            if (formatFilter === 'pdf' && e !== 'pdf') return false;
            if (formatFilter === 'other' && (e === 'epub' || e === 'pdf')) return false;
            if (langFilter === 'en') {
                const l = (s.language ?? '').toLowerCase();
                if (l && !l.startsWith('en')) return false;
            }
            return true;
        });
    });

    const MIN_SIZE: Record<string, number> = {
        epub: 220_000,
        pdf: 200_000,
        mobi: 150_000,
        azw3: 150_000,
        djvu: 500_000,
        fb2: 80_000,
        txt: 40_000
    };

    function isSample(s: BookSource): boolean {
        const min = MIN_SIZE[s.ext.toLowerCase()] ?? 50_000;
        return typeof s.size === 'number' && s.size > 0 && s.size < min;
    }

    const bestSources = $derived(filteredSources.filter((s) => !isSample(s)));
    const sampleSources = $derived(filteredSources.filter(isSample));

    function extClass(ext: string): string {
        const e = ext.toLowerCase();
        if (e === 'epub') return 'pw-ext-epub';
        if (e === 'pdf') return 'pw-ext-pdf';
        return 'pw-ext-other';
    }

    const finished = $derived(
        !!detail?.in_library && status === 'ready' && (detail.shelf === 'read' || rawProgress >= 0.97)
    );

    const primary = $derived.by(() => {
        if (finished) return { label: t('books.action.read_again'), go: readAgain };
        if (status === 'ready')
            return { label: t(rawProgress > 0 ? 'books.action.continue' : 'books.action.read'), go: read };
        if (status === 'processing') return { label: t('books.status.processing'), go: null };
        if (status === 'error') return { label: t('books.action.retry'), go: openSources };
        return { label: t('books.action.get_file'), go: openSources };
    });

    const badge = $derived.by(() => {
        if (!detail?.in_library) return '';
        const bits: string[] = [];
        if (detail.book.ext) bits.push(detail.book.ext);
        if (detail.file_size) bits.push(fmtSize(detail.file_size));
        if (status === 'processing') bits.push(t('books.status.processing'));
        else if (status === 'error') bits.push(t('books.status.error'));
        if (finished) bits.push(t('books.finished'));
        return bits.join(' · ');
    });

    const subjectTags = $derived.by(() => {
        const raw = detail?.book.subjects;
        if (!raw) return [];
        const out: string[] = [];
        const seen = new Set<string>();
        for (const s of raw.split(', ')) {
            const v = s.trim();
            if (!v) continue;
            const k = v.toLowerCase();
            if (seen.has(k)) continue;
            seen.add(k);
            out.push(v);
            if (out.length >= 5) break;
        }
        return out;
    });

    const seriesUniq = $derived.by(() => {
        const seen = new Set<string>();
        return seriesBooks.filter((b) => {
            if (seen.has(b.ol_key)) return false;
            seen.add(b.ol_key);
            return true;
        });
    });

    const seriesKeys = $derived(new Set(seriesUniq.map((b) => b.ol_key)));
    const seriesDoneKeys = $derived(new Set(seriesUniq.filter((b) => b.in_library).map((b) => b.ol_key)));
    const seriesYears = $derived.by(() => {
        const ys = seriesUniq.map((b) => b.year).filter((y): y is number => typeof y === 'number');
        if (ys.length === 0) return '';
        const lo = Math.min(...ys);
        const hi = Math.max(...ys);
        return lo === hi ? `${lo}` : `${lo}–${hi}`;
    });
    const authorUniq = $derived.by(() => {
        const seen = new Set<string>();
        return moreByAuthor
            .filter((b) => {
                if (b.ol_key === olKey) return false;
                if (seriesKeys.has(b.ol_key)) return false;
                if (seen.has(b.ol_key)) return false;
                seen.add(b.ol_key);
                return true;
            })
            .slice(0, 18);
    });

    const ringR = 40;
    const ringC = 2 * Math.PI * ringR;
    const ringOffset = $derived(ringC * (1 - pct / 100));
    const showRail = $derived(!!detail?.in_library);

    const totalPages = $derived(detail?.book.pages ?? null);
    const pageOf = $derived(
        totalPages !== null ? Math.max(0, Math.min(totalPages, Math.round((pct / 100) * totalPages))) : null
    );
    const pagesLeft = $derived(totalPages !== null && pageOf !== null ? Math.max(0, totalPages - pageOf) : null);

    const synopsis = $derived(detail?.book.description ?? '');
    const synopsisShort = $derived(synopsis.length > 320 ? synopsis.slice(0, 320).trimEnd() + '… ' : synopsis);

    const scoreShown = $derived(detail?.book.rating ? Number(detail.book.rating.toFixed(1)) : null);

    function fmtDate(s: string | null | undefined): string {
        if (!s) return '';
        const d = new Date(s);
        if (Number.isNaN(d.getTime())) return '';
        const diff = (Date.now() - d.getTime()) / 86400000;
        if (diff < 1) return 'today';
        if (diff < 2) return '1d ago';
        if (diff < 14) return `${Math.floor(diff)}d ago`;
        if (diff < 60) return `${Math.floor(diff / 7)}w ago`;
        return d.toISOString().slice(0, 10);
    }
</script>

<svelte:head><title>{title} - pleasewatch</title></svelte:head>
<svelte:window onkeydown={onEsc} />

{#if user}
    <div class="pw-page pw-bk-page">
        <TopBar {user} back={true} />

        {#if loading}
            <section class="pw-section">
                <div class="flex gap-6 sm:gap-8 flex-col sm:flex-row">
                    <div class="flex-shrink-0 w-40 sm:w-52 mx-auto sm:mx-0">
                        <div class="aspect-[2/3] rounded-lg pw-skel"></div>
                    </div>
                    <div class="min-w-0 flex-1 pw-skel-stack">
                        <div class="pw-skel pw-skel-line" style="width: 60%; height: 28px;"></div>
                        <div class="pw-skel pw-skel-line" style="width: 38%; height: 14px;"></div>
                        <div class="pw-skel pw-skel-line" style="width: 70%; height: 11px;"></div>
                        <div class="pw-skel pw-skel-line" style="width: 90%; height: 11px; margin-top: 16px;"></div>
                        <div class="pw-skel pw-skel-line" style="width: 86%; height: 11px;"></div>
                        <div class="pw-skel pw-skel-line" style="width: 78%; height: 11px;"></div>
                    </div>
                </div>
            </section>
        {:else if err || !detail}
            <section class="pw-section">
                <div class="pw-error" style="max-width: 480px;">{err || 'not found'}</div>
            </section>
        {:else}
            <div class="pw-v1-hero-wrap pw-bk-hero">
                <div class="pw-v1-hero-bg">
                    {#if detail.book.cover_url}
                        <img class="pw-v1-hero-img pw-bk-hero-img" src={detail.book.cover_url} alt="" />
                    {/if}
                    <div class="pw-v1-hero-grad-x"></div>
                    <div class="pw-v1-hero-grad-y"></div>

                    <div class="pw-v1-hero-content">
                        <div class="pw-bk-layout" class:pw-bk-layout-rail={showRail}>
                            <div class="pw-bk-cover-col">
                                <div class="pw-bk-cover" class:pw-plat-frame={finished}>
                                    {#if detail.book.cover_url}
                                        <img src={detail.book.cover_url} alt={title} />
                                    {/if}
                                    {#if scoreShown}
                                        <span class="pw-bk-cover-badge pw-bk-cover-score">★ {scoreShown}</span>
                                    {/if}
                                    {#if finished}
                                        <span class="pw-plat-lg" title={t('books.finished')}>
                                            <svg
                                                width="16"
                                                height="16"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="#0b1116"
                                                stroke-width="3.4"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                            >
                                        </span>
                                    {/if}
                                </div>
                            </div>

                            <div class="pw-bk-info">
                                <div class="pw-bk-bc">
                                    <span class="pw-bk-bc-arrow">{'>'}</span> books <span class="pw-bk-bc-sep">/</span>
                                    <b>{title}</b>
                                </div>
                                <h1 class="pw-bk-title">{title}</h1>

                                {#if detail.book.authors}
                                    <div class="pw-bk-by-row">
                                        <span class="pw-bk-by-label">by</span>
                                        <button
                                            type="button"
                                            class="pw-bk-author-chip"
                                            disabled={!firstAuthorOlid}
                                            onclick={() => firstAuthorOlid && (authorOpenOlid = firstAuthorOlid)}
                                        >
                                            <span class="pw-bk-avatar-wrap">
                                                <span class="pw-bk-avatar-initial">{authorInitial || '?'}</span>
                                                {#if authorAvatarUrl && !avatarFailed}
                                                    <img
                                                        src={authorAvatarUrl}
                                                        alt=""
                                                        class="pw-bk-avatar-img"
                                                        onerror={() => (avatarFailed = true)}
                                                    />
                                                {/if}
                                            </span>
                                            <b class="pw-bk-author-name">{detail.book.authors}</b>
                                        </button>
                                    </div>
                                {/if}

                                <div class="pw-bk-meta">
                                    {#if detail.book.year}<span class="pw-bk-meta-text">{detail.book.year}</span>{/if}
                                    {#if detail.book.year && (detail.book.pages || detail.book.language)}<span
                                            class="pw-bk-meta-sep"
                                        ></span>{/if}
                                    {#if detail.book.pages}<span class="pw-bk-tag-mono pw-bk-tag-neutral"
                                            >{plural('books.meta.pages', detail.book.pages)}</span
                                        >{/if}
                                    {#if detail.book.language}<span class="pw-bk-tag-mono pw-bk-tag-neutral"
                                            >{detail.book.language}</span
                                        >{/if}
                                    <span class="pw-bk-tag-mono pw-bk-tag-neutral">book</span>
                                    {#if detail.book.rating}
                                        <span class="pw-bk-meta-sep"></span>
                                        <span class="pw-bk-score-inline">
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"
                                                ><path
                                                    d="M12 2.5l2.9 6.1 6.6.9-4.8 4.6 1.2 6.6L12 18.6 6.1 21.3l1.2-6.6L2.5 9.5l6.6-.9z"
                                                /></svg
                                            >
                                            {detail.book.rating.toFixed(1)}
                                        </span>
                                        {#if detail.book.rating_count}<span class="pw-bk-meta-mono"
                                                >{fmtCount(detail.book.rating_count)} votes</span
                                            >{/if}
                                    {/if}
                                </div>

                                {#if synopsis}
                                    <p class="pw-bk-synopsis">
                                        {descOpen ? synopsis : synopsisShort}
                                        {#if synopsis.length > 320}
                                            <button class="pw-bk-readmore" onclick={() => (descOpen = !descOpen)}>
                                                {descOpen ? 'show less' : 'read more'}
                                            </button>
                                        {/if}
                                    </p>
                                {/if}

                                {#if subjectTags.length > 0}
                                    <div class="pw-bk-tags">
                                        {#each subjectTags as s (s)}
                                            <span class="pw-bk-tag">{s}</span>
                                        {/each}
                                    </div>
                                {/if}

                                <div class="pw-bk-actions">
                                    <button class="pw-v1-btn-watch" disabled={!primary.go} onclick={primary.go}>
                                        {#if status === 'ready'}
                                            <svg
                                                width="14"
                                                height="14"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" /><path
                                                    d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"
                                                /></svg
                                            >
                                        {:else if status !== 'processing'}
                                            <svg
                                                width="14"
                                                height="14"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline
                                                    points="7 10 12 15 17 10"
                                                /><line x1="12" y1="15" x2="12" y2="3" /></svg
                                            >
                                        {/if}
                                        {primary.label}
                                    </button>

                                    <button
                                        class="pw-v1-btn-lib pw-bk-btn-lib"
                                        class:on={!!detail.shelf}
                                        onclick={() => (shelfOpen = true)}
                                    >
                                        <svg
                                            width="13"
                                            height="13"
                                            viewBox="0 0 24 24"
                                            fill={detail.shelf ? 'currentColor' : 'none'}
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" /></svg
                                        >
                                        {detail.shelf ? t('books.shelf.' + detail.shelf) : t('books.shelf.want')}
                                    </button>

                                    {#if !detail.book.file_path}
                                        <button
                                            class="pw-v1-btn-lib"
                                            onclick={() => fileInput?.click()}
                                            disabled={uploadBusy}
                                        >
                                            <svg
                                                width="13"
                                                height="13"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline
                                                    points="17 8 12 3 7 8"
                                                /><line x1="12" y1="3" x2="12" y2="15" /></svg
                                            >
                                            {uploadBusy ? t('common.loading') : t('books.upload')}
                                        </button>
                                    {/if}
                                    {#if detail.in_library}
                                        {#if detail.book.file_path}
                                            <button class="pw-v1-btn-lib" onclick={download}>
                                                <svg
                                                    width="13"
                                                    height="13"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"
                                                    ><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline
                                                        points="7 10 12 15 17 10"
                                                    /><line x1="12" y1="15" x2="12" y2="3" /></svg
                                                >
                                                {t('books.action.download')}
                                            </button>
                                        {/if}
                                        <button class="pw-v1-btn-lib" onclick={openSources}>
                                            <svg
                                                width="13"
                                                height="13"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path
                                                    d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                                                /></svg
                                            >
                                            {t('books.sources.toggle_show')}
                                        </button>
                                        <button class="pw-v1-btn-lib" onclick={toggleLibrary} disabled={busy}>
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" /></svg
                                            >
                                            {t('books.lib.remove')}
                                        </button>
                                    {:else}
                                        <button class="pw-v1-btn-lib" onclick={toggleLibrary} disabled={busy}>
                                            <svg
                                                width="13"
                                                height="13"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><line x1="12" y1="5" x2="12" y2="19" /><line
                                                    x1="5"
                                                    y1="12"
                                                    x2="19"
                                                    y2="12"
                                                /></svg
                                            >
                                            {t('books.lib.add')}
                                        </button>
                                    {/if}
                                    <a class="pw-bk-link-chip" href={annaUrl} target="_blank" rel="noreferrer noopener">
                                        anna's archive
                                        <svg
                                            width="10"
                                            height="10"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2.4"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"><path d="M7 17 17 7" /><path d="M7 7h10v10" /></svg
                                        >
                                    </a>
                                </div>

                                {#if status === 'processing' || status === 'error'}
                                    <p class="pw-bk-srcnote" class:err={status === 'error'}>
                                        {#if status === 'processing'}
                                            <span class="pw-bk-spin"></span>
                                            <span>{t('books.status.processing')}</span>
                                        {:else}
                                            <span>{t('books.status.error')}</span>
                                        {/if}
                                    </p>
                                {/if}
                            </div>

                            {#if showRail}
                                <aside class="pw-bk-rail">
                                    <div class="pw-bk-panel">
                                        <div class="pw-bk-panel-head">
                                            <span class="pw-bk-panel-arrow">▸</span> reading progress
                                        </div>
                                        <div class="pw-bk-ring-wrap">
                                            {#if pct > 0 || finished}
                                                <div class="pw-bk-ring">
                                                    <svg class="pw-bk-ring-svg" viewBox="0 0 108 108">
                                                        <circle
                                                            cx="54"
                                                            cy="54"
                                                            r="46"
                                                            fill="none"
                                                            stroke="rgba(255,255,255,0.08)"
                                                            stroke-width="7"
                                                        />
                                                        <circle
                                                            cx="54"
                                                            cy="54"
                                                            r="46"
                                                            fill="none"
                                                            stroke="var(--pw-accent)"
                                                            stroke-width="7"
                                                            stroke-linecap="round"
                                                            stroke-dasharray={2 * Math.PI * 46}
                                                            stroke-dashoffset={2 * Math.PI * 46 * (1 - pct / 100)}
                                                            transform="rotate(-90 54 54)"
                                                        />
                                                        {#if pageOf !== null && totalPages !== null}
                                                            <text
                                                                x="54"
                                                                y="50"
                                                                text-anchor="middle"
                                                                dominant-baseline="middle"
                                                                class="pw-bk-ring-cur-svg">{pageOf}</text
                                                            >
                                                            <text
                                                                x="54"
                                                                y="72"
                                                                text-anchor="middle"
                                                                dominant-baseline="middle"
                                                                class="pw-bk-ring-tot-svg">/ {totalPages}</text
                                                            >
                                                        {:else}
                                                            <text
                                                                x="54"
                                                                y="60"
                                                                text-anchor="middle"
                                                                dominant-baseline="middle"
                                                                class="pw-bk-ring-cur-svg"
                                                                >{pct}<tspan
                                                                    class="pw-bk-ring-pctsfx-svg"
                                                                    dx="2"
                                                                    dy="-3">%</tspan
                                                                ></text
                                                            >
                                                        {/if}
                                                    </svg>
                                                </div>
                                            {:else}
                                                <div class="pw-bk-status-icon">
                                                    {#if status === 'ready'}
                                                        <svg
                                                            width="32"
                                                            height="32"
                                                            viewBox="0 0 24 24"
                                                            fill="currentColor"><path d="M8 5l11 7-11 7z" /></svg
                                                        >
                                                    {:else if status === 'processing'}
                                                        <span class="pw-bk-spin pw-bk-spin-lg"></span>
                                                    {:else}
                                                        <svg
                                                            width="30"
                                                            height="30"
                                                            viewBox="0 0 24 24"
                                                            fill="none"
                                                            stroke="currentColor"
                                                            stroke-width="1.6"
                                                            stroke-linecap="round"
                                                            stroke-linejoin="round"
                                                            ><path
                                                                d="M4 4h12a2 2 0 0 1 2 2v14H6a2 2 0 0 1-2-2zM4 4v16"
                                                            /></svg
                                                        >
                                                    {/if}
                                                </div>
                                            {/if}
                                            <div class="pw-bk-rail-side">
                                                {#if finished}
                                                    <div class="pw-bk-rail-pill pw-bk-rail-pill-done">
                                                        <svg
                                                            width="11"
                                                            height="11"
                                                            viewBox="0 0 24 24"
                                                            fill="none"
                                                            stroke="currentColor"
                                                            stroke-width="3"
                                                            stroke-linecap="round"
                                                            stroke-linejoin="round"
                                                            ><polyline points="20 6 9 17 4 12" /></svg
                                                        >
                                                        completed
                                                    </div>
                                                {:else if pct > 0 && pagesLeft !== null && pagesLeft > 0}
                                                    <div class="pw-bk-rail-pill">
                                                        <span class="pw-bk-rail-pill-dot"></span>
                                                        {pagesLeft} pages left
                                                    </div>
                                                {:else if pct > 0}
                                                    <div class="pw-bk-rail-pill">
                                                        <span class="pw-bk-rail-pill-dot"></span>
                                                        in progress
                                                    </div>
                                                {:else if status === 'ready'}
                                                    <div class="pw-bk-rail-state">ready to read</div>
                                                {:else if status === 'processing'}
                                                    <div class="pw-bk-rail-state">{t('books.status.processing')}</div>
                                                {:else}
                                                    <div class="pw-bk-rail-state">no file yet</div>
                                                {/if}
                                                {#if pageOf !== null && totalPages !== null && pct > 0 && !finished}
                                                    <div class="pw-bk-rail-on">
                                                        page <b>{pageOf}</b> of {totalPages}
                                                    </div>
                                                {:else if pct > 0 && !finished}
                                                    <div class="pw-bk-rail-on">at <b>{pct}%</b></div>
                                                {/if}
                                            </div>
                                        </div>
                                        {#if pct > 0 || finished}
                                            <div class="pw-bk-prog-bar">
                                                <span style="width: {finished ? 100 : pct}%;"></span>
                                            </div>
                                        {/if}
                                        <div class="pw-bk-rail-rows">
                                            {#if detail.progress?.updated_at}
                                                <div class="pw-bk-rail-row">
                                                    <span class="pw-bk-rail-k">last read</span>
                                                    <span class="pw-bk-rail-v"
                                                        >{fmtDate(detail.progress.updated_at)}</span
                                                    >
                                                </div>
                                            {/if}
                                            <div class="pw-bk-rail-row">
                                                <span class="pw-bk-rail-k">status</span>
                                                {#if finished}
                                                    <span class="pw-bk-rail-v pw-bk-rail-status-done">Completed</span>
                                                {:else if pct > 0}
                                                    <span class="pw-bk-rail-v pw-bk-rail-status-on">Reading</span>
                                                {:else if status === 'ready'}
                                                    <span class="pw-bk-rail-v">Not started</span>
                                                {:else if status === 'processing'}
                                                    <span class="pw-bk-rail-v">Converting</span>
                                                {:else}
                                                    <span class="pw-bk-rail-v">Waiting for file</span>
                                                {/if}
                                            </div>
                                            {#if detail.book.ext}
                                                <div class="pw-bk-rail-row">
                                                    <span class="pw-bk-rail-k">format</span>
                                                    <span class="pw-bk-rail-v"
                                                        >{detail.book.ext.toUpperCase()}{#if detail.file_size}<span
                                                                class="pw-bk-rail-v-soft"
                                                            >
                                                                · {fmtSize(detail.file_size)}</span
                                                            >{/if}</span
                                                    >
                                                </div>
                                            {/if}
                                            {#if detail.book.publisher}
                                                <div class="pw-bk-rail-row">
                                                    <span class="pw-bk-rail-k">publisher</span>
                                                    <span
                                                        class="pw-bk-rail-v pw-bk-rail-v-trunc"
                                                        title={detail.book.publisher}>{detail.book.publisher}</span
                                                    >
                                                </div>
                                            {/if}
                                        </div>
                                    </div>
                                </aside>
                            {/if}
                        </div>
                    </div>
                </div>
            </div>

            {#if seriesUniq.length > 0 && seriesName}
                <section class="pw-section pw-bk-shelf-sec">
                    <header class="pw-bk-secthead">
                        <div class="pw-bk-secthead-eyebrow">SERIES</div>
                        <h2 class="pw-bk-secthead-h2">
                            <a
                                class="pw-bk-secthead-link"
                                href={`/series/${encodeURIComponent(seriesName)}`}
                                data-sveltekit-preload-data="hover"
                            >
                                {seriesName}
                                <svg
                                    width="13"
                                    height="13"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.4"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"><polyline points="9 6 15 12 9 18" /></svg
                                >
                            </a>
                            {#if seriesYears}<span class="pw-bk-secthead-soft">{seriesYears}</span>{/if}
                        </h2>
                    </header>
                    <div class="pw-bk-grid">
                        {#each seriesUniq as b, i (b.ol_key)}
                            {@const isCurrent = b.ol_key === olKey}
                            <a
                                class="pw-bk-cardx"
                                class:on={isCurrent}
                                href={`/book/${b.ol_key}`}
                                aria-current={isCurrent ? 'page' : undefined}
                                data-sveltekit-preload-data="hover"
                            >
                                <div class="pw-bk-cardx-cover">
                                    <span class="pw-bk-cardx-num">#{i + 1}</span>
                                    {#if b.cover_url}
                                        <img src={b.cover_url} alt={b.title} loading="lazy" decoding="async" />
                                    {:else}
                                        <div class="pw-bk-cardx-empty">
                                            <svg
                                                width="30"
                                                height="30"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="1.2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path d="M4 4h12a2 2 0 0 1 2 2v14H6a2 2 0 0 1-2-2zM4 4v16" /></svg
                                            >
                                            <span class="pw-bk-cardx-empty-t">{b.title}</span>
                                        </div>
                                    {/if}
                                    {#if isCurrent}
                                        <span class="pw-bk-cardx-now">READING</span>
                                    {:else if b.in_library}
                                        <span class="pw-bk-cardx-owned" aria-label="in library">
                                            <svg
                                                width="11"
                                                height="11"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="3"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                            >
                                        </span>
                                    {/if}
                                </div>
                                <div class="pw-bk-cardx-title">{b.title}</div>
                                {#if b.year}<div class="pw-bk-cardx-year">{b.year}</div>{/if}
                            </a>
                        {/each}
                    </div>
                </section>
            {/if}

            {#if authorUniq.length > 0 && authorName}
                <section class="pw-section pw-bk-shelf-sec">
                    <header class="pw-bk-secthead">
                        <div class="pw-bk-secthead-eyebrow">MORE BY</div>
                        <h2 class="pw-bk-secthead-h2">
                            <button
                                type="button"
                                class="pw-bk-secthead-author"
                                disabled={!firstAuthorOlid}
                                onclick={() => firstAuthorOlid && (authorOpenOlid = firstAuthorOlid)}
                            >
                                {authorName}
                                {#if firstAuthorOlid}
                                    <svg
                                        width="11"
                                        height="11"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="2.4"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"><polyline points="9 6 15 12 9 18" /></svg
                                    >
                                {/if}
                            </button>
                        </h2>
                    </header>
                    <div class="pw-bk-grid">
                        {#each authorUniq as b (b.ol_key)}
                            <a class="pw-bk-cardx" href={`/book/${b.ol_key}`} data-sveltekit-preload-data="hover">
                                <div class="pw-bk-cardx-cover">
                                    {#if b.cover_url}
                                        <img src={b.cover_url} alt={b.title} loading="lazy" decoding="async" />
                                    {:else}
                                        <div class="pw-bk-cardx-empty">
                                            <svg
                                                width="30"
                                                height="30"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="1.2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path d="M4 4h12a2 2 0 0 1 2 2v14H6a2 2 0 0 1-2-2zM4 4v16" /></svg
                                            >
                                            <span class="pw-bk-cardx-empty-t">{b.title}</span>
                                        </div>
                                    {/if}
                                    {#if b.in_library}
                                        <span class="pw-bk-cardx-owned" aria-label="in library">
                                            <svg
                                                width="11"
                                                height="11"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="3"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                            >
                                        </span>
                                    {/if}
                                </div>
                                <div class="pw-bk-cardx-title">{b.title}</div>
                                {#if b.year}<div class="pw-bk-cardx-year">{b.year}</div>{/if}
                            </a>
                        {/each}
                    </div>
                </section>
            {/if}
        {/if}

        <div style="height: 60px;"></div>
    </div>

    {#if sourcesOpen}
        <div class="pw-files-scrim">
            <div class="pw-files-pop" use:clickOutside={closeSources}>
                <div class="pw-files-head">
                    <span>{t('books.sources.title')}</span>
                    {#if sourcesLoading}<span class="pw-files-dim">{t('common.loading')}</span>{/if}
                </div>

                {#if sourcesLoaded && sources.length > 0}
                    <div class="pw-files-filters">
                        <div class="pw-files-fg">
                            {#each [['all', 'filter.all'], ['epub', 'filter.epub'], ['pdf', 'filter.pdf'], ['other', 'filter.other']] as [v, k] (v)}
                                <button
                                    class="pw-files-fchip"
                                    class:on={formatFilter === v}
                                    onclick={() => (formatFilter = v as typeof formatFilter)}
                                    >{t('books.sources.' + k)}</button
                                >
                            {/each}
                        </div>
                        <div class="pw-files-fg pw-files-fg-end">
                            {#each [['all', 'lang.all'], ['en', 'lang.en']] as [v, k] (v)}
                                <button
                                    class="pw-files-fchip"
                                    class:on={langFilter === v}
                                    onclick={() => (langFilter = v as typeof langFilter)}
                                    >{t('books.sources.' + k)}</button
                                >
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if sourcesErr}
                    <div class="pw-files-err">{sourcesErr}</div>
                {/if}

                <div class="pw-files-list">
                    {#if sourcesLoaded && filteredSources.length === 0 && sources.length > 0}
                        <div class="pw-files-empty">{t('books.sources.no_match')}</div>
                    {/if}
                    {#each bestSources as s, i (s.md5)}
                        <button class="pw-files-row" disabled={fetching !== null} onclick={() => fetchSource(s)}>
                            <span class="pw-ext-badge {extClass(s.ext)}">{s.ext}</span>
                            <div class="pw-files-row-body">
                                <div class="pw-files-row-line">
                                    <span class="pw-files-row-name">{s.title}</span>
                                    {#if i === 0 && formatFilter === 'all' && langFilter === 'all'}
                                        <span
                                            class="pw-files-row-best"
                                            title={t('books.sources.best_why', { ext: s.ext })}
                                        >
                                            {t('books.sources.best')}
                                        </span>
                                    {/if}
                                    {#if fetching === s.md5}
                                        <span class="pw-files-row-busy">{t('books.fetch.busy')}</span>
                                    {/if}
                                </div>
                                <div class="pw-files-row-meta">
                                    {#if s.publisher}<span class="pw-files-row-pub">{s.publisher}</span>{/if}
                                    {#if s.year}<span>{s.year}</span>{/if}
                                    {#if s.pages}<span>{s.pages}p</span>{/if}
                                </div>
                            </div>
                            <div class="pw-files-row-side">
                                {#if s.size}<span class="pw-files-row-size">{fmtSize(s.size)}</span>{/if}
                                {#if s.language}<span class="pw-files-row-lang"
                                        >{s.language.slice(0, 2).toUpperCase()}</span
                                    >{/if}
                            </div>
                        </button>
                    {/each}

                    {#if sampleSources.length > 0}
                        <div class="pw-files-section">
                            <span class="pw-files-section-label">{t('books.sources.samples')}</span>
                            <span class="pw-files-section-sub">{t('books.sources.samples_hint')}</span>
                        </div>
                        {#each sampleSources as s (s.md5)}
                            <button
                                class="pw-files-row pw-files-row-sample"
                                disabled={fetching !== null}
                                onclick={() => fetchSource(s)}
                            >
                                <span class="pw-ext-badge {extClass(s.ext)}">{s.ext}</span>
                                <div class="pw-files-row-body">
                                    <div class="pw-files-row-line">
                                        <span class="pw-files-row-name">{s.title}</span>
                                        {#if fetching === s.md5}
                                            <span class="pw-files-row-busy">{t('books.fetch.busy')}</span>
                                        {/if}
                                    </div>
                                    <div class="pw-files-row-meta">
                                        {#if s.publisher}<span class="pw-files-row-pub">{s.publisher}</span>{/if}
                                        {#if s.year}<span>{s.year}</span>{/if}
                                        {#if s.pages}<span>{s.pages}p</span>{/if}
                                    </div>
                                </div>
                                <div class="pw-files-row-side">
                                    {#if s.size}<span class="pw-files-row-size">{fmtSize(s.size)}</span>{/if}
                                    {#if s.language}<span class="pw-files-row-lang"
                                            >{s.language.slice(0, 2).toUpperCase()}</span
                                        >{/if}
                                </div>
                            </button>
                        {/each}
                    {/if}
                </div>

                <div class="pw-files-foot">
                    <button
                        class="pw-files-foot-btn pw-files-foot-primary"
                        onclick={() => fileInput?.click()}
                        disabled={uploadBusy}
                    >
                        {uploadBusy ? t('common.loading') : t('books.upload')}
                    </button>
                    <button class="pw-files-foot-btn" onclick={openTorrentPicker}>
                        {t('books.search.torrent')}
                    </button>
                    <a href={annaUrl} target="_blank" rel="noreferrer noopener" class="pw-files-foot-link">
                        {t('books.search.anna')}
                    </a>
                </div>
            </div>
        </div>
    {/if}

    <input
        bind:this={fileInput}
        type="file"
        accept=".epub,.pdf,.mobi,.azw3,.fb2"
        class="hidden"
        onchange={uploadFile}
    />

    {#if tpOpen && detail}
        <TorrentPicker
            query={torrentQuery()}
            kind="book"
            {olKey}
            onClose={() => (tpOpen = false)}
            onStarted={() => {
                tpOpen = false;
                loadDetail();
            }}
        />
    {/if}

    {#if authorOpenOlid}
        <AuthorModal olid={authorOpenOlid} onClose={() => (authorOpenOlid = null)} />
    {/if}

    {#if shelfOpen && detail}
        <div class="pw-files-scrim">
            <div class="pw-files-pop pw-shelf-pop" use:clickOutside={() => (shelfOpen = false)}>
                <div class="pw-files-head"><span>{t('books.shelf.title')}</span></div>
                <div class="pw-files-list">
                    {#each ['want', 'reading', 'read'] as s (s)}
                        <button class="pw-shelf-row" onclick={() => setShelf(s)}>
                            <span>{t('books.shelf.' + s)}</span>
                            {#if detail.shelf === s}
                                <svg
                                    width="15"
                                    height="15"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    style="color: var(--pw-accent);"><polyline points="20 6 9 17 4 12" /></svg
                                >
                            {/if}
                        </button>
                    {/each}
                    {#if detail.shelf}
                        <button class="pw-shelf-row pw-shelf-del" onclick={unshelf}>{t('books.shelf.remove')}</button>
                    {/if}
                </div>
            </div>
        </div>
    {/if}
{/if}

<style>
    .pw-shelf-pop {
        max-width: 320px;
    }
    .pw-shelf-row {
        width: 100%;
        display: flex;
        align-items: center;
        justify-content: space-between;
        background: none;
        border: 0;
        padding: 12px 12px;
        border-radius: 10px;
        font-size: 14px;
        color: rgba(220, 220, 225, 0.78);
        cursor: pointer;
        text-align: left;
    }
    .pw-shelf-row:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .pw-shelf-del {
        color: #f87171;
    }

    .pw-files-scrim {
        position: fixed;
        inset: 0;
        z-index: 120;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 16px;
    }
    .pw-files-pop {
        width: 100%;
        max-width: 560px;
        max-height: min(72vh, 580px);
        display: flex;
        flex-direction: column;
        background: rgba(12, 13, 17, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        overflow: hidden;
        animation: pw-files-in 0.18s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    @keyframes pw-files-in {
        from {
            transform: translateY(14px);
            opacity: 0;
        }
    }
    .pw-files-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 14px 16px 10px;
        font-size: 14px;
        font-weight: 600;
        color: #ececef;
    }
    .pw-files-dim {
        font-size: 12px;
        font-weight: 400;
        color: rgba(220, 220, 225, 0.5);
    }
    .pw-files-err {
        padding: 0 16px 10px;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.5);
    }
    .pw-files-list {
        overflow-y: auto;
        padding: 0 8px 8px;
        min-height: 60px;
    }
    .pw-files-row {
        display: flex;
        align-items: center;
        gap: 10px;
        width: 100%;
        padding: 7px 10px;
        border-radius: 8px;
        background: transparent;
        border: 0;
        text-align: left;
        cursor: pointer;
        transition: background 0.12s ease;
    }
    .pw-files-row:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .pw-files-row:disabled {
        opacity: 0.5;
        cursor: default;
    }
    .pw-files-row-sample {
        opacity: 0.62;
    }
    .pw-files-row-sample:hover {
        opacity: 0.85;
    }
    .pw-files-row-body {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .pw-files-row-side {
        display: flex;
        align-items: center;
        gap: 8px;
        flex-shrink: 0;
        font-size: 11px;
        font-variant-numeric: tabular-nums;
    }
    .pw-files-row-lang {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        color: rgba(220, 220, 225, 0.42);
        letter-spacing: 0.05em;
    }
    .pw-ext-badge {
        flex-shrink: 0;
        display: grid;
        place-items: center;
        width: 34px;
        height: 34px;
        border-radius: 7px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 9.5px;
        font-weight: 600;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }
    .pw-ext-epub {
        color: var(--pw-accent);
        background: color-mix(in oklch, var(--pw-accent) 14%, transparent);
    }
    .pw-ext-pdf {
        color: rgba(220, 220, 225, 0.7);
        background: rgba(255, 255, 255, 0.07);
    }
    .pw-ext-other {
        color: rgba(220, 220, 225, 0.5);
        background: rgba(255, 255, 255, 0.04);
    }
    .pw-files-row-line {
        display: flex;
        align-items: center;
        gap: 8px;
        min-width: 0;
    }
    .pw-files-row-name {
        font-size: 13px;
        color: #ececef;
        line-height: 1.3;
        min-width: 0;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-files-row-meta {
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.4);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-files-row-pub {
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-files-filters {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 4px 12px 10px;
    }
    .pw-files-fg {
        display: flex;
        gap: 2px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 9px;
        padding: 2px;
    }
    .pw-files-fg-end {
        margin-left: auto;
    }
    .pw-files-fchip {
        background: transparent;
        border: 0;
        color: rgba(232, 232, 234, 0.55);
        border-radius: 6px;
        padding: 4px 9px;
        font-size: 11.5px;
        cursor: pointer;
        transition:
            background 0.12s ease,
            color 0.12s ease;
    }
    .pw-files-fchip:hover {
        color: rgba(232, 232, 234, 0.9);
    }
    .pw-files-fchip.on {
        background: rgba(255, 255, 255, 0.12);
        color: #fff;
    }
    .pw-files-empty {
        padding: 18px 12px;
        font-size: 13px;
        color: rgba(220, 220, 225, 0.4);
        text-align: center;
    }
    .pw-files-section {
        display: flex;
        align-items: baseline;
        gap: 8px;
        padding: 14px 10px 6px;
        margin-top: 6px;
        border-top: 1px dashed rgba(255, 255, 255, 0.06);
    }
    .pw-files-section-label {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: rgba(220, 220, 225, 0.45);
    }
    .pw-files-section-sub {
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.35);
    }
    .pw-files-foot-btn {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.06);
        color: rgba(232, 232, 234, 0.85);
        border-radius: 999px;
        padding: 7px 14px;
        font-size: 12.5px;
        cursor: pointer;
    }
    .pw-files-foot-btn:hover {
        background: rgba(255, 255, 255, 0.1);
    }
    .pw-files-foot-btn:disabled {
        opacity: 0.5;
        cursor: default;
    }
    .pw-files-foot-primary {
        background: color-mix(in oklch, var(--pw-accent) 18%, transparent);
        border-color: color-mix(in oklch, var(--pw-accent) 30%, transparent);
        color: #fff;
    }
    .pw-files-foot-primary:hover {
        background: color-mix(in oklch, var(--pw-accent) 26%, transparent);
    }
    .pw-files-foot-link {
        color: rgba(220, 220, 225, 0.55);
        text-decoration: none;
        font-size: 12.5px;
        padding: 7px 0;
    }
    .pw-files-foot-link:hover {
        color: rgba(232, 232, 234, 0.9);
        text-decoration: underline;
        text-underline-offset: 3px;
    }
    .pw-cover-shadow {
        box-shadow:
            0 18px 38px -22px rgba(0, 0, 0, 0.85),
            0 0 0 1px rgba(255, 255, 255, 0.04);
    }
    .pw-author-card {
        display: inline-flex;
        align-items: center;
        gap: 12px;
        margin-top: 14px;
        padding: 6px 14px 6px 6px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.06);
        cursor: pointer;
        color: inherit;
        text-align: left;
        max-width: 100%;
        min-width: 0;
        transition:
            background 0.15s ease,
            border-color 0.15s ease,
            transform 0.15s ease;
    }
    .pw-author-card:not(:disabled):hover {
        background: rgba(255, 255, 255, 0.06);
        border-color: color-mix(in oklch, var(--pw-accent) 28%, rgba(255, 255, 255, 0.1));
        transform: translateY(-1px);
    }
    .pw-author-card:disabled {
        cursor: default;
    }
    .pw-author-avatar-wrap {
        position: relative;
        width: 44px;
        height: 44px;
        border-radius: 999px;
        background: linear-gradient(
            135deg,
            color-mix(in oklch, var(--pw-accent) 38%, transparent),
            color-mix(in oklch, var(--pw-accent) 14%, transparent)
        );
        flex-shrink: 0;
        overflow: hidden;
        display: grid;
        place-items: center;
    }
    .pw-author-initial {
        font-size: 18px;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.88);
        letter-spacing: 0.02em;
    }
    .pw-author-avatar {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .pw-author-stack {
        display: flex;
        flex-direction: column;
        min-width: 0;
        gap: 1px;
    }
    .pw-author-name {
        font-size: 14px;
        color: #ececef;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .pw-author-label {
        font-size: 10.5px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: rgba(220, 220, 225, 0.4);
    }
    .pw-h3 {
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.1em;
        text-transform: uppercase;
        color: rgba(220, 220, 225, 0.5);
        margin: 0 0 10px;
    }
    .pw-detail-block {
        padding-top: 0;
        max-width: 760px;
    }
    .pw-detail-desc {
        font-size: 14.5px;
        color: rgba(220, 220, 225, 0.82);
        line-height: 1.65;
        margin: 0;
    }
    .pw-desc-toggle {
        margin-top: 10px;
        background: none;
        border: 0;
        color: var(--pw-accent);
        font-size: 12.5px;
        cursor: pointer;
        padding: 0;
    }
    .pw-desc-toggle:hover {
        text-decoration: underline;
        text-underline-offset: 3px;
    }
    .pw-subjects-label {
        font-size: 10.5px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: rgba(220, 220, 225, 0.4);
    }
    .pw-subj-chip {
        font-size: 12.5px;
        padding: 4px 11px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.07);
        color: rgba(220, 220, 225, 0.78);
        transition:
            background 0.12s ease,
            color 0.12s ease;
    }
    .pw-subj-chip:hover {
        background: rgba(255, 255, 255, 0.09);
        color: rgba(232, 232, 234, 0.95);
    }
    .pw-skel {
        background: linear-gradient(
            90deg,
            rgba(255, 255, 255, 0.04) 0%,
            rgba(255, 255, 255, 0.08) 50%,
            rgba(255, 255, 255, 0.04) 100%
        );
        background-size: 200% 100%;
        animation: pw-skel-shimmer 1.4s ease-in-out infinite;
    }
    .pw-skel-line {
        border-radius: 4px;
    }
    .pw-skel-stack {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    @keyframes pw-skel-shimmer {
        0% {
            background-position: 200% 0;
        }
        100% {
            background-position: -200% 0;
        }
    }
    .pw-files-row-busy {
        font-size: 11px;
        color: var(--pw-accent);
        flex-shrink: 0;
        white-space: nowrap;
    }
    .pw-files-row-best {
        font-size: 10px;
        padding: 2px 7px;
        border-radius: 999px;
        color: var(--pw-accent);
        background: color-mix(in oklch, var(--pw-accent) 14%, transparent);
        flex-shrink: 0;
        white-space: nowrap;
        letter-spacing: 0.02em;
        font-weight: 500;
    }
    .pw-files-row-meta {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 4px 10px;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.42);
        font-variant-numeric: tabular-nums;
    }
    .pw-files-row-pub {
        color: rgba(220, 220, 225, 0.55);
        max-width: 280px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .pw-files-row-ext {
        text-transform: uppercase;
        color: rgba(220, 220, 225, 0.55);
        font-weight: 500;
        letter-spacing: 0.04em;
    }
    .pw-files-row-size {
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-files-divider {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10.5px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: rgba(220, 220, 225, 0.45);
        padding: 14px 12px 6px;
        border-top: 1px dashed rgba(255, 255, 255, 0.07);
        margin-top: 8px;
        display: flex;
        align-items: baseline;
        gap: 8px;
    }
    .pw-bt-search {
        color: var(--pw-accent);
        text-transform: none;
        letter-spacing: 0;
        font-size: 11px;
    }
    .pw-bt-count {
        color: rgba(220, 220, 225, 0.35);
    }

    .pw-bt-row {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 12px;
        border-radius: 10px;
        background: rgba(255, 255, 255, 0.025);
        margin: 0 0 4px;
        border: 1px solid rgba(255, 255, 255, 0.04);
        transition:
            background 0.12s ease,
            border-color 0.12s ease;
    }
    .pw-bt-row:hover {
        background: rgba(255, 255, 255, 0.05);
        border-color: rgba(255, 255, 255, 0.08);
    }
    .pw-bt-info {
        flex: 1;
        min-width: 0;
    }
    .pw-bt-title {
        font-size: 13px;
        color: #ececef;
        line-height: 1.35;
        overflow: hidden;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
    }
    .pw-bt-tags {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 6px 10px;
        margin-top: 5px;
        font-size: 11px;
        font-variant-numeric: tabular-nums;
    }
    .pw-bt-pill {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        text-transform: uppercase;
        font-size: 9.5px;
        letter-spacing: 0.04em;
        padding: 2px 7px;
        border-radius: 3px;
        background: rgba(255, 255, 255, 0.06);
        color: rgba(220, 220, 225, 0.78);
    }
    .pw-bt-prov {
        color: rgba(170, 200, 220, 0.92);
        background: rgba(80, 130, 170, 0.18);
    }
    .pw-bt-meta {
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-bt-seeds {
        color: rgba(160, 220, 180, 0.92);
        font-weight: 600;
    }
    .pw-bt-peers {
        color: rgba(220, 180, 160, 0.78);
    }
    .pw-bt-mag {
        flex-shrink: 0;
        height: 32px;
        padding: 0 14px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #ececef;
        font-size: 12px;
        cursor: pointer;
        transition: background 0.12s ease;
    }
    .pw-bt-mag:hover {
        background: rgba(255, 255, 255, 0.14);
    }
    .pw-files-foot {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 12px;
        padding: 12px 16px calc(12px + env(safe-area-inset-bottom));
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        font-size: 12px;
        color: rgba(220, 220, 225, 0.5);
    }

    .pw-bk-shelf-sec {
        padding-top: 8px;
        padding-bottom: 28px;
    }
    .pw-bk-secthead {
        max-width: 760px;
        margin-bottom: 22px;
        padding-bottom: 14px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .pw-bk-secthead-eyebrow {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.18em;
        color: var(--pw-accent);
        margin-bottom: 6px;
    }
    .pw-bk-secthead-h2 {
        font-size: clamp(22px, 2.6vw, 30px);
        font-weight: 500;
        color: #f4f4f6;
        letter-spacing: -0.02em;
        margin: 0;
        line-height: 1.1;
        display: flex;
        flex-wrap: wrap;
        align-items: baseline;
        gap: 12px;
    }
    .pw-bk-secthead-soft {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 13px;
        color: rgba(220, 220, 225, 0.4);
        letter-spacing: 0.02em;
        font-weight: 400;
    }
    .pw-bk-secthead-link {
        color: inherit;
        text-decoration: none;
        display: inline-flex;
        align-items: center;
        gap: 8px;
        transition: color 0.12s ease;
    }
    .pw-bk-secthead-link:hover {
        color: var(--pw-accent);
    }
    .pw-bk-secthead-link :global(svg) {
        opacity: 0.55;
        transition:
            transform 0.15s ease,
            opacity 0.15s ease;
    }
    .pw-bk-secthead-link:hover :global(svg) {
        transform: translateX(3px);
        opacity: 1;
    }
    .pw-bk-secthead-author {
        background: none;
        border: 0;
        padding: 0;
        font: inherit;
        color: inherit;
        display: inline-flex;
        align-items: center;
        gap: 6px;
        cursor: pointer;
        transition: color 0.12s ease;
    }
    .pw-bk-secthead-author:hover {
        color: var(--pw-accent);
    }
    .pw-bk-secthead-author:disabled {
        cursor: default;
    }
    .pw-bk-secthead-author :global(svg) {
        opacity: 0.55;
        transition: transform 0.15s ease;
    }
    .pw-bk-secthead-author:hover :global(svg) {
        transform: translateX(2px);
        opacity: 1;
    }
    .pw-bk-secthead-sub {
        margin-top: 8px;
        font-size: 12.5px;
        color: rgba(220, 220, 225, 0.45);
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 7px;
    }
    .pw-bk-secthead-sub :global(b) {
        color: #ececef;
        font-weight: 600;
    }
    .pw-bk-secthead-dot {
        opacity: 0.4;
    }

    .pw-bk-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
        gap: 22px 18px;
    }
    .pw-bk-cardx {
        text-decoration: none;
        color: inherit;
        min-width: 0;
        display: flex;
        flex-direction: column;
    }
    .pw-bk-cardx-cover {
        position: relative;
        width: 100%;
        aspect-ratio: 2/3;
        border-radius: 10px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.03);
        box-shadow: 0 14px 30px -18px rgba(0, 0, 0, 0.9);
        transition:
            transform 0.25s cubic-bezier(0.2, 0.7, 0.2, 1),
            box-shadow 0.25s ease;
    }
    .pw-bk-cardx:hover .pw-bk-cardx-cover {
        transform: translateY(-5px);
        box-shadow:
            0 24px 40px -20px rgba(0, 0, 0, 0.95),
            0 0 0 1px rgba(255, 255, 255, 0.1);
    }
    .pw-bk-cardx.on .pw-bk-cardx-cover {
        box-shadow:
            0 0 0 2px var(--pw-accent),
            0 18px 32px -18px rgba(0, 0, 0, 0.9);
    }
    .pw-bk-cardx.on:hover .pw-bk-cardx-cover {
        box-shadow:
            0 0 0 2px var(--pw-accent),
            0 24px 40px -20px rgba(0, 0, 0, 0.95);
    }
    .pw-bk-cardx.on .pw-bk-cardx-title {
        color: var(--pw-accent);
    }
    .pw-bk-cardx-cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }

    .pw-bk-cardx-num {
        position: absolute;
        top: 6px;
        left: 6px;
        z-index: 2;
        font-family: ui-monospace, monospace;
        font-size: 9.5px;
        font-weight: 600;
        letter-spacing: 0.02em;
        padding: 1px 5px;
        border-radius: 3px;
        color: rgba(255, 255, 255, 0.85);
        background: rgba(8, 9, 12, 0.55);
        backdrop-filter: blur(6px);
        font-variant-numeric: tabular-nums;
    }

    .pw-bk-cardx-now {
        position: absolute;
        top: 8px;
        right: 8px;
        z-index: 2;
        font-family: ui-monospace, monospace;
        font-size: 9.5px;
        font-weight: 700;
        letter-spacing: 0.1em;
        padding: 3px 8px;
        border-radius: 4px;
        background: var(--pw-accent);
        color: #0b1116;
        text-transform: uppercase;
    }

    .pw-bk-cardx-empty {
        width: 100%;
        height: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 10px;
        padding: 14px 10px;
        color: rgba(220, 220, 225, 0.32);
        background:
            linear-gradient(135deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.01) 60%),
            radial-gradient(circle at 50% 30%, rgba(140, 110, 200, 0.12), transparent 60%);
    }
    .pw-bk-cardx-empty-t {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.5);
        text-align: center;
        line-height: 1.3;
        display: -webkit-box;
        -webkit-line-clamp: 3;
        line-clamp: 3;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }

    .pw-bk-cardx-owned {
        position: absolute;
        bottom: 8px;
        right: 8px;
        width: 20px;
        height: 20px;
        border-radius: 999px;
        background: rgba(120, 200, 150, 0.95);
        color: #0b1116;
        display: grid;
        place-items: center;
        box-shadow: 0 4px 12px -4px rgba(0, 0, 0, 0.6);
    }

    .pw-bk-cardx-title {
        margin-top: 10px;
        font-size: 13px;
        font-weight: 500;
        color: #ececef;
        line-height: 1.32;
        overflow: hidden;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
        transition: color 0.12s ease;
    }
    .pw-bk-cardx:hover .pw-bk-cardx-title {
        color: #fff;
    }
    .pw-bk-cardx-year {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        color: rgba(220, 220, 225, 0.38);
        margin-top: 3px;
        letter-spacing: 0.04em;
    }
    @media (max-width: 720px) {
        .pw-bk-grid {
            grid-template-columns: repeat(auto-fill, minmax(108px, 1fr));
            gap: 18px 12px;
        }
        .pw-bk-secthead-h2 {
            font-size: 20px;
        }
        .pw-bk-secthead {
            margin-bottom: 16px;
            padding-bottom: 10px;
        }
        .pw-bk-cardx-num {
            font-size: 9px;
            padding: 1px 4px;
            top: 5px;
            left: 5px;
        }
    }

    .pw-bk-hero {
        isolation: isolate;
        overflow: hidden;
    }
    .pw-bk-hero-img {
        filter: blur(48px) saturate(108%) brightness(0.55);
        transform: scale(1.12);
    }

    .pw-bk-layout {
        display: grid;
        grid-template-columns: clamp(180px, 24vw, 240px) minmax(0, 1fr);
        gap: clamp(20px, 3vw, 44px);
        align-items: start;
    }
    @media (min-width: 1180px) {
        .pw-bk-layout-rail {
            grid-template-columns: clamp(200px, 19vw, 268px) minmax(0, 1fr) clamp(232px, 21vw, 280px);
        }
    }
    @media (max-width: 720px) {
        .pw-bk-layout,
        .pw-bk-layout-rail {
            grid-template-columns: 1fr;
            gap: 20px;
        }
        .pw-bk-cover-col {
            width: clamp(140px, 42vw, 200px);
            margin: 0 auto;
        }
        .pw-bk-title {
            font-size: clamp(26px, 7vw, 38px);
        }
        .pw-bk-meta {
            gap: 6px 10px;
            font-size: 12px;
        }
        .pw-bk-synopsis {
            font-size: 13.5px;
            margin-top: 16px;
        }
        .pw-bk-tags {
            margin-top: 12px;
        }
        .pw-bk-actions {
            gap: 8px;
            margin-top: 18px;
        }

        .pw-bk-panel {
            padding: 14px 14px 12px;
            border-radius: 12px;
        }
        .pw-bk-panel-head {
            margin-bottom: 14px;
            font-size: 9.5px;
        }
        .pw-bk-ring,
        .pw-bk-status-icon {
            width: 96px;
            height: 96px;
        }
        .pw-bk-ring-svg {
            width: 96px;
            height: 96px;
        }
        .pw-bk-ring-cur-svg {
            font-size: 26px;
        }
        .pw-bk-ring-tot-svg {
            font-size: 10.5px;
        }
        .pw-bk-ring-pctsfx-svg {
            font-size: 14px;
        }
        .pw-bk-ring-wrap {
            gap: 12px;
            padding: 0 0 4px;
        }
        .pw-bk-status-icon :global(svg) {
            width: 22px;
            height: 22px;
        }
        .pw-bk-ring-wrap {
            gap: 12px;
        }
        .pw-bk-rail-pill {
            font-size: 11px;
            padding: 3px 9px;
        }
        .pw-bk-rail-on {
            font-size: 11.5px;
        }
        .pw-bk-prog-bar {
            margin-top: 12px;
            height: 3px;
        }
        .pw-bk-rail-rows {
            margin-top: 6px;
        }
        .pw-bk-rail-row {
            padding: 7px 0;
        }
        .pw-bk-rail-k {
            font-size: 9.5px;
        }
        .pw-bk-rail-v {
            font-size: 12px;
        }
    }
    @media (max-width: 480px) {
        :global(.pw-bk-page .pw-v1-hero-content) {
            padding-top: 64px !important;
        }
    }

    .pw-bk-cover {
        position: relative;
        width: 100%;
        aspect-ratio: 2/3;
        border-radius: 12px;
        overflow: hidden;
        background: #14141a;
        box-shadow:
            0 25px 50px -24px rgba(0, 0, 0, 0.9),
            0 0 0 1px rgba(255, 255, 255, 0.05);
    }
    .pw-bk-cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-bk-cover-badge {
        position: absolute;
        top: 10px;
        font-size: 10px;
        padding: 3px 8px;
        border-radius: 4px;
        letter-spacing: 0.05em;
        font-weight: 600;
        backdrop-filter: blur(4px);
    }
    .pw-bk-cover-score {
        right: 10px;
        background: rgba(20, 20, 26, 0.75);
        color: var(--pw-accent);
        border: 1px solid rgba(255, 255, 255, 0.06);
    }

    .pw-bk-info {
        min-width: 0;
    }
    .pw-bk-bc {
        font-size: 11.5px;
        letter-spacing: 0.04em;
        color: rgba(220, 220, 225, 0.5);
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 14px;
    }
    .pw-bk-bc-arrow {
        color: var(--pw-accent);
        font-family: ui-monospace, monospace;
    }
    .pw-bk-bc-sep {
        opacity: 0.4;
    }
    .pw-bk-bc :global(b) {
        color: #ececef;
        font-weight: 500;
    }

    .pw-bk-title {
        font-size: clamp(30px, 4.6vw, 56px);
        font-weight: 300;
        letter-spacing: -0.025em;
        line-height: 1.02;
        color: #f4f4f6;
        margin: 0;
    }

    .pw-bk-by-row {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        margin-top: 12px;
        font-size: 14px;
        color: rgba(220, 220, 225, 0.62);
        flex-wrap: wrap;
        max-width: 100%;
        min-width: 0;
    }
    .pw-bk-by-label {
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-bk-author-chip {
        display: inline-flex;
        align-items: center;
        gap: 7px;
        padding: 2px 10px 2px 2px;
        border-radius: 999px;
        background: transparent;
        border: 1px solid transparent;
        cursor: pointer;
        color: inherit;
        font: inherit;
        min-width: 0;
        max-width: 100%;
        transition:
            background 0.15s ease,
            border-color 0.15s ease;
    }
    .pw-bk-author-chip:not(:disabled):hover {
        background: rgba(255, 255, 255, 0.05);
        border-color: rgba(255, 255, 255, 0.08);
    }
    .pw-bk-author-chip:disabled {
        cursor: default;
        padding-left: 0;
    }
    .pw-bk-avatar-wrap {
        position: relative;
        width: 26px;
        height: 26px;
        border-radius: 999px;
        background: linear-gradient(
            135deg,
            color-mix(in oklch, var(--pw-accent) 42%, transparent),
            color-mix(in oklch, var(--pw-accent) 16%, transparent)
        );
        flex-shrink: 0;
        overflow: hidden;
        display: grid;
        place-items: center;
    }
    .pw-bk-avatar-initial {
        font-size: 11px;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.88);
    }
    .pw-bk-avatar-img {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .pw-bk-author-name {
        font-size: 14px;
        color: #d8d8da;
        font-weight: 500;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pw-bk-meta {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 8px 12px;
        margin-top: 16px;
        font-size: 13px;
    }
    .pw-bk-meta-text {
        color: rgba(220, 220, 225, 0.78);
    }
    .pw-bk-meta-sep {
        width: 3px;
        height: 3px;
        border-radius: 999px;
        background: rgba(220, 220, 225, 0.28);
    }
    .pw-bk-tag-mono {
        font-family: ui-monospace, monospace;
        font-size: 10.5px;
        letter-spacing: 0.08em;
        padding: 3px 8px;
        border-radius: 4px;
        text-transform: uppercase;
    }
    .pw-bk-tag-neutral {
        background: rgba(255, 255, 255, 0.06);
        color: rgba(220, 220, 225, 0.72);
    }
    .pw-bk-score-inline {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        color: var(--pw-accent);
        font-weight: 600;
    }
    .pw-bk-meta-mono {
        font-family: ui-monospace, monospace;
        font-size: 11.5px;
        color: rgba(220, 220, 225, 0.42);
    }

    .pw-bk-synopsis {
        font-size: 14px;
        line-height: 1.66;
        color: rgba(216, 216, 218, 0.78);
        margin-top: 20px;
        max-width: 760px;
    }
    .pw-bk-readmore {
        color: var(--pw-accent);
        background: none;
        border: 0;
        font: inherit;
        font-weight: 500;
        cursor: pointer;
        padding: 0;
        margin-left: 2px;
    }
    .pw-bk-readmore:hover {
        text-decoration: underline;
        text-underline-offset: 3px;
    }

    .pw-bk-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 5px;
        margin-top: 14px;
    }
    .pw-bk-tag {
        font-size: 10.5px;
        padding: 2.5px 9px;
        border-radius: 999px;
        color: rgba(220, 220, 225, 0.62);
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.05);
        transition: all 0.14s ease;
        letter-spacing: 0.01em;
    }
    .pw-bk-tag:hover {
        color: #ececef;
        border-color: rgba(140, 110, 200, 0.45);
        background: rgba(140, 110, 200, 0.1);
    }

    .pw-bk-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-top: 24px;
        align-items: center;
    }
    .pw-bk-btn-lib.on {
        color: rgba(160, 220, 180, 0.95);
        border-color: rgba(120, 200, 150, 0.28);
        background: rgba(120, 200, 150, 0.1);
    }
    .pw-bk-btn-lib.on:hover {
        background: rgba(120, 200, 150, 0.18);
    }
    .pw-bk-link-chip {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        height: 38px;
        padding: 0 14px;
        border-radius: 999px;
        background: transparent;
        color: rgba(220, 220, 225, 0.55);
        font-size: 13px;
        text-decoration: none;
    }
    .pw-bk-link-chip:hover {
        color: #ececef;
        background: rgba(255, 255, 255, 0.05);
    }

    .pw-bk-srcnote {
        margin-top: 16px;
        font-size: 12px;
        line-height: 1.55;
        color: rgba(220, 220, 225, 0.5);
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .pw-bk-srcnote.err {
        color: #f87171;
    }
    .pw-bk-spin {
        display: inline-block;
        width: 12px;
        height: 12px;
        border-radius: 999px;
        border: 2px solid rgba(220, 220, 225, 0.3);
        border-top-color: transparent;
        animation: pw-bk-spin 0.9s linear infinite;
    }
    @keyframes pw-bk-spin {
        to {
            transform: rotate(360deg);
        }
    }

    .pw-bk-rail {
        position: sticky;
        top: 76px;
    }
    @media (max-width: 1180px) {
        .pw-bk-rail {
            position: static;
        }
    }
    .pw-bk-panel {
        background: rgba(15, 16, 20, 0.7);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 14px;
        padding: 18px;
        backdrop-filter: blur(10px);
    }
    .pw-bk-panel-head {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        letter-spacing: 0.12em;
        text-transform: uppercase;
        color: rgba(220, 220, 225, 0.42);
        margin-bottom: 16px;
        display: flex;
        align-items: center;
        gap: 7px;
    }
    .pw-bk-panel-arrow {
        color: var(--pw-accent);
    }
    .pw-bk-ring-wrap {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 14px;
        padding: 4px 0 8px;
    }
    .pw-bk-ring {
        position: relative;
        width: 108px;
        height: 108px;
        flex-shrink: 0;
    }
    .pw-bk-ring-svg {
        width: 108px;
        height: 108px;
    }
    .pw-bk-ring-cur-svg {
        font-size: 30px;
        font-weight: 500;
        fill: #f2f2f4;
        font-variant-numeric: tabular-nums;
        letter-spacing: -0.01em;
    }
    .pw-bk-ring-tot-svg {
        font-family: ui-monospace, monospace;
        font-size: 12px;
        fill: rgba(220, 220, 225, 0.5);
        letter-spacing: 0.04em;
    }
    .pw-bk-ring-pctsfx-svg {
        font-size: 16px;
        fill: rgba(220, 220, 225, 0.5);
        font-family: ui-monospace, monospace;
        font-weight: 400;
    }
    .pw-bk-status-icon {
        width: 108px;
        height: 108px;
        border-radius: 50%;
        background: rgba(255, 255, 255, 0.04);
        border: 1px dashed rgba(255, 255, 255, 0.1);
        display: grid;
        place-items: center;
        color: var(--pw-accent);
        flex-shrink: 0;
    }
    .pw-bk-status-icon.done {
        color: rgba(160, 220, 180, 0.95);
        background: rgba(120, 200, 150, 0.12);
        border: 1px solid rgba(120, 200, 150, 0.25);
    }
    .pw-bk-spin-lg {
        width: 28px;
        height: 28px;
        border-width: 3px;
    }
    .pw-bk-rail-side {
        min-width: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        text-align: center;
        gap: 8px;
    }
    .pw-bk-rail-state {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.6);
    }
    .pw-bk-rail-pill {
        display: inline-flex;
        align-items: center;
        gap: 7px;
        padding: 5px 12px;
        border-radius: 999px;
        background: rgba(140, 110, 200, 0.16);
        color: var(--pw-accent);
        font-size: 12.5px;
        font-weight: 600;
    }
    .pw-bk-rail-pill-dot {
        width: 6px;
        height: 6px;
        border-radius: 999px;
        background: var(--pw-accent);
    }
    .pw-bk-rail-pill-done {
        background: rgba(120, 200, 150, 0.16);
        color: rgba(160, 220, 180, 0.95);
    }
    .pw-bk-rail-on {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.6);
        line-height: 1.5;
    }
    .pw-bk-rail-on :global(b) {
        color: #ececef;
        font-weight: 600;
    }
    .pw-bk-prog-bar {
        display: block;
        height: 4px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.1);
        overflow: hidden;
        margin-top: 14px;
    }
    .pw-bk-prog-bar > span {
        display: block;
        height: 100%;
        background: var(--pw-accent);
        border-radius: 999px;
        transition: width 0.2s ease;
    }
    .pw-bk-rail-rows {
        margin-top: 8px;
    }
    .pw-bk-rail-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        padding: 9px 0;
        border-top: 1px dashed rgba(255, 255, 255, 0.07);
    }
    .pw-bk-rail-k {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        letter-spacing: 0.06em;
        color: rgba(220, 220, 225, 0.4);
        text-transform: uppercase;
    }
    .pw-bk-rail-v {
        font-size: 13px;
        color: #ececef;
        font-weight: 500;
        font-variant-numeric: tabular-nums;
    }
    .pw-bk-rail-v-soft {
        color: rgba(220, 220, 225, 0.42);
        font-weight: 400;
    }
    .pw-bk-rail-v-trunc {
        max-width: 60%;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        text-align: right;
    }
    .pw-bk-rail-status-done {
        color: rgba(160, 220, 180, 0.95);
    }
    .pw-bk-rail-status-on {
        color: var(--pw-accent);
    }
    .pw-bk-rail-v-trunc {
        max-width: 60%;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        text-align: right;
    }
</style>
