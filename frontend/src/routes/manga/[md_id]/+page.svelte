<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { page } from '$app/state';
    import {
        api,
        type MangaAnimeLink,
        type MangaChapter,
        type MangaDetail,
        type MangaRecommended,
        type MangaRelated,
        type User
    } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import TopBar from '$lib/components/TopBar.svelte';

    const mdId = $derived(page.params.md_id ?? '');

    let user = $state<User | null>(null);
    let detail = $state<MangaDetail | null>(null);
    let chapters = $state<MangaChapter[]>([]);
    let anime = $state<MangaAnimeLink>(null);
    let related = $state<MangaRelated[]>([]);
    let recs = $state<MangaRecommended[]>([]);
    let loading = $state(true);
    let chaptersLoading = $state(true);
    let err = $state('');
    let busy = $state(false);
    let lang = $state('all');
    let sort = $state<'new' | 'old'>('new');
    let view = $state<'all' | 'unread'>('all');
    let synopsisOpen = $state(false);

    onMount(async () => {
        const detailP = api.mangaDetail(mdId).catch((e) => {
            console.error('[manga] detail failed', e);
            return null;
        });
        const chaptersP = api.mangaChapters(mdId).catch((e) => {
            console.error('[manga] chapters failed', e);
            return null;
        });
        try {
            user = await api.me();
        } catch {
            goto('/login');
            return;
        }
        const d = await detailP;
        if (d) detail = d;
        else err = 'could not load manga';
        loading = false;

        chapters = (await chaptersP) ?? chapters;
        chaptersLoading = false;

        if (detail?.anilist_id) {
            api.mangaAnime(mdId)
                .then((a) => (anime = a))
                .catch(() => {});
            api.mangaRelated(mdId)
                .then((r) => (related = r))
                .catch(() => {});
            api.mangaRecommendations(mdId)
                .then((r) => (recs = r))
                .catch(() => {});
        }
    });

    function chNum(c: MangaChapter): number {
        const v = parseFloat(c.chapter ?? '');
        return Number.isFinite(v) ? v : -1;
    }

    const langs = $derived([...new Set(chapters.map((c) => c.lang))]);
    const inLang = $derived(lang === 'all' ? chapters : chapters.filter((c) => c.lang === lang));

    const currentChapter = $derived(chapters.find((c) => c.id === detail?.progress?.chapter_id) ?? null);
    const currentNum = $derived(detail?.progress?.chapter ? parseFloat(detail.progress.chapter) : null);
    const latestNum = $derived(chapters.length > 0 ? Math.max(...chapters.map(chNum).filter((n) => n >= 0)) : null);
    const totalChapters = $derived(
        detail?.last_chapter ? parseFloat(detail.last_chapter) : (latestNum ?? inLang.length)
    );
    const newCount = $derived(latestNum !== null && currentNum !== null ? Math.max(0, latestNum - currentNum) : 0);
    const readCount = $derived(
        currentNum !== null ? inLang.filter((c) => chNum(c) > 0 && chNum(c) < currentNum).length : 0
    );

    function stateOf(c: MangaChapter): 'continue' | 'new' | 'read' | 'unread' {
        if (!detail?.progress) return 'unread';
        if (c.id === detail.progress.chapter_id) return 'continue';
        if (currentNum === null) return 'unread';
        const n = chNum(c);
        if (n < 0) return 'unread';
        return n < currentNum ? 'read' : 'new';
    }

    const visibleRows = $derived.by(() => {
        let rows = inLang.map((c) => ({ ch: c, state: stateOf(c) }));
        if (view === 'unread') {
            rows = rows.filter((r) => r.state === 'new' || r.state === 'continue');
        }
        rows.sort((a, b) => (sort === 'new' ? chNum(b.ch) - chNum(a.ch) : chNum(a.ch) - chNum(b.ch)));
        return rows;
    });

    const gridRows = $derived(visibleRows.filter((r) => r.state !== 'continue'));

    const firstChapter = $derived(inLang.slice().sort((a, b) => chNum(a) - chNum(b))[0]);

    function fmtDate(s: string | null): string {
        if (!s) return '';
        const d = new Date(s);
        if (Number.isNaN(d.getTime())) return '';
        const now = Date.now();
        const diff = (now - d.getTime()) / 86400000;
        if (diff < 1) return 'today';
        if (diff < 2) return '1d ago';
        if (diff < 14) return `${Math.floor(diff)}d ago`;
        if (diff < 60) return `${Math.floor(diff / 7)}w ago`;
        return d.toISOString().slice(0, 10);
    }

    const officialUrl = $derived(detail?.links?.engtl ?? detail?.links?.raw ?? null);
    const officialLabel = $derived(brandFor(officialUrl));

    function brandFor(url: string | null): string {
        if (!url) return 'Official Site';
        const u = url.toLowerCase();
        if (u.includes('mangaplus')) return 'Manga Plus';
        if (u.includes('shonenjump')) return 'Shonen Jump';
        if (u.includes('bookwalker')) return 'BookWalker';
        if (u.includes('mangadex')) return 'MangaDex';
        if (u.includes('webtoons.com')) return 'Webtoons';
        if (u.includes('tappytoon')) return 'Tappytoon';
        if (u.includes('lezhin')) return 'Lezhin';
        if (u.includes('amazon')) return 'Amazon';
        if (u.includes('ebookjapan')) return 'eBookJapan';
        if (u.includes('cdjapan')) return 'CDJapan';
        if (u.includes('kakao')) return 'KakaoPage';
        if (u.includes('naver')) return 'Naver';
        return 'Official Site';
    }

    const fallbackInList = $derived(inLang.some((c) => c.id.startsWith('ck:') || c.id.startsWith('mk:')));
    const restrictedNow = $derived(
        fallbackInList || (!!detail?.restricted && (lang === 'all' || (detail?.restricted_langs ?? []).includes(lang)))
    );

    const tagsShown = $derived((detail?.tags ?? []).slice(0, 8));
    const authorsLine = $derived(
        [...(detail?.authors ?? []), ...(detail?.artists ?? [])]
            .filter((s, i, arr) => arr.indexOf(s) === i)
            .slice(0, 3)
            .join(', ')
    );
    const scoreShown = $derived(detail?.score ? Number(detail.score.toFixed(1)) : null);

    function fmtCount(n: number | null | undefined): string {
        if (!n) return '';
        if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
        return `${n}`;
    }

    function animeHref(a: MangaAnimeLink): string | null {
        if (!a || !a.tmdb) return null;
        return a.tmdb.media_type === 'tv' ? `/tv/${a.tmdb.tmdb_id}` : `/movie/${a.tmdb.tmdb_id}`;
    }

    async function toggleLibrary() {
        if (!detail || busy) return;
        busy = true;
        try {
            if (detail.in_library) {
                await api.mangaDelete(mdId);
                detail.in_library = false;
            } else {
                await api.mangaAdd(mdId);
                detail.in_library = true;
            }
        } catch (e) {
            console.error('[manga] library toggle failed', e);
        } finally {
            busy = false;
        }
    }

    function read(ch: MangaChapter, pageNum = 0) {
        goto(`/read/${ch.id}?md=${mdId}${pageNum > 0 ? `&page=${pageNum}` : ''}`);
    }

    function continueReading() {
        const p = detail?.progress;
        if (!p) return;
        goto(`/read/${p.chapter_id}?md=${mdId}&page=${p.page}`);
    }

    const title = $derived(detail?.manga.title ?? 'manga');
    const synopsis = $derived(detail?.manga.description ?? '');
    const synopsisShort = $derived(synopsis.length > 320 ? synopsis.slice(0, 320).trimEnd() + '… ' : synopsis);

    const ringR = 40;
    const ringC = 2 * Math.PI * ringR;
    const ringPct = $derived(
        currentNum !== null && totalChapters && totalChapters > 0
            ? Math.max(0, Math.min(1, currentNum / totalChapters))
            : 0
    );
    const ringOffset = $derived(ringC * (1 - ringPct));

    const showRail = $derived(!!detail?.in_library && !!detail?.progress);
    const continuePct = $derived.by(() => {
        const p = detail?.progress;
        if (!p || !p.pages) return 0;
        return Math.round(((p.page + 1) / p.pages) * 100);
    });

    let chaptersOpen = $state(true);
    let relatedOpen = $state(true);
    let recsOpen = $state(true);
</script>

<svelte:head><title>{title} - pleasewatch</title></svelte:head>

{#if user}
    <div class="pw-page pw-mg-page">
        <TopBar {user} back={true} />

        {#if loading}
            <section class="pw-section pw-empty">
                <div class="pw-empty-card"><div class="pw-empty-tag">// loading...</div></div>
            </section>
        {:else if err || !detail}
            <section class="pw-section">
                <div class="pw-error" style="max-width: 480px;">{err || 'not found'}</div>
            </section>
        {:else}
            <div class="pw-v1-hero-wrap pw-mg-hero">
                <div class="pw-v1-hero-bg">
                    {#if detail.manga.cover_url}
                        <img class="pw-v1-hero-img pw-mg-hero-img" src={detail.manga.cover_url} alt="" />
                    {/if}
                    <div class="pw-v1-hero-grad-x"></div>
                    <div class="pw-v1-hero-grad-y"></div>

                    <div class="pw-v1-hero-content">
                        <div class="pw-mg-layout" class:pw-mg-layout-rail={showRail}>
                            <div class="pw-mg-cover-col">
                                <div class="pw-mg-cover">
                                    {#if detail.manga.cover_url}
                                        <img src={detail.manga.cover_url} alt={title} />
                                    {/if}
                                    {#if scoreShown}
                                        <span class="pw-mg-cover-badge pw-mg-cover-score">★ {scoreShown}</span>
                                    {/if}
                                </div>
                            </div>

                            <div class="pw-mg-info">
                                <div class="pw-mg-bc">
                                    <span class="pw-mg-bc-arrow">{'>'}</span> manga <span class="pw-mg-bc-sep">/</span>
                                    <b>{title}</b>
                                </div>
                                <h1 class="pw-mg-title">{title}</h1>
                                {#if authorsLine}
                                    <div class="pw-mg-authors">by <b>{authorsLine}</b></div>
                                {/if}

                                <div class="pw-mg-meta">
                                    {#if detail.manga.year}<span class="pw-mg-meta-text">{detail.manga.year}</span>{/if}
                                    {#if detail.manga.year && (detail.manga.status || detail.demographic)}<span
                                            class="pw-mg-meta-sep"
                                        ></span>{/if}
                                    {#if detail.manga.status}<span class="pw-mg-tag-mono pw-mg-status-ok"
                                            >{detail.manga.status}</span
                                        >{/if}
                                    {#if detail.demographic}<span class="pw-mg-tag-mono pw-mg-tag-neutral"
                                            >{detail.demographic}</span
                                        >{/if}
                                    {#if scoreShown}
                                        <span class="pw-mg-meta-sep"></span>
                                        <span class="pw-mg-score-inline">
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"
                                                ><path
                                                    d="M12 2.5l2.9 6.1 6.6.9-4.8 4.6 1.2 6.6L12 18.6 6.1 21.3l1.2-6.6L2.5 9.5l6.6-.9z"
                                                /></svg
                                            >
                                            {scoreShown}
                                        </span>
                                        {#if detail.score_count}<span class="pw-mg-meta-mono"
                                                >{fmtCount(detail.score_count)} votes</span
                                            >{/if}
                                    {/if}
                                    {#if detail.follow_count}
                                        <span class="pw-mg-meta-sep"></span>
                                        <span class="pw-mg-meta-hearts">
                                            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
                                                ><path
                                                    d="M12 21s-7.5-4.7-10-9.3C.6 8.9 2 5.5 5.2 5.5c2 0 3.2 1.2 3.8 2.2.6-1 1.8-2.2 3.8-2.2 3.2 0 4.6 3.4 3.2 6.2C19.5 16.3 12 21 12 21z"
                                                /></svg
                                            >
                                            {fmtCount(detail.follow_count)}
                                        </span>
                                    {/if}
                                </div>

                                {#if synopsis}
                                    <p class="pw-mg-synopsis">
                                        {synopsisOpen ? synopsis : synopsisShort}
                                        {#if synopsis.length > 320}
                                            <button
                                                class="pw-mg-readmore"
                                                onclick={() => (synopsisOpen = !synopsisOpen)}
                                            >
                                                {synopsisOpen ? 'show less' : 'read more'}
                                            </button>
                                        {/if}
                                    </p>
                                {/if}

                                {#if tagsShown.length > 0}
                                    <div class="pw-mg-tags">
                                        {#each tagsShown as tg (tg)}
                                            <span class="pw-mg-tag">{tg}</span>
                                        {/each}
                                    </div>
                                {/if}

                                <div class="pw-mg-actions">
                                    {#if detail.progress}
                                        <button class="pw-v1-btn-watch" onclick={continueReading}>
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M8 5l11 7-11 7z" /></svg
                                            >
                                            {t('manga.read.continue')}
                                            {detail.progress.chapter ?? ''}
                                        </button>
                                    {:else if firstChapter}
                                        <button class="pw-v1-btn-watch" onclick={() => read(firstChapter)}>
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M8 5l11 7-11 7z" /></svg
                                            >
                                            {t('manga.read.start')}
                                        </button>
                                    {/if}
                                    <button
                                        class="pw-v1-btn-lib pw-mg-btn-lib"
                                        class:on={detail.in_library}
                                        onclick={toggleLibrary}
                                        disabled={busy}
                                    >
                                        {#if detail.in_library}
                                            <svg
                                                width="13"
                                                height="13"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2.6"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                            >
                                            {t('manga.lib.remove')}
                                        {:else}
                                            {t('manga.lib.add')}
                                        {/if}
                                    </button>
                                    {#if anime && animeHref(anime)}
                                        <a class="pw-link-chip pw-anime-chip" href={animeHref(anime)}>
                                            <svg
                                                width="13"
                                                height="13"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="1.8"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><rect x="3" y="4" width="18" height="16" rx="2" /><path
                                                    d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"
                                                /></svg
                                            >
                                            {t('manga.watch_anime')}
                                        </a>
                                    {/if}
                                    {#if officialUrl}
                                        <a class="pw-link-chip" href={officialUrl} target="_blank" rel="noopener">
                                            {officialLabel}
                                            <svg
                                                width="10"
                                                height="10"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2.4"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                                ><path d="M7 17 17 7" /><path d="M7 7h10v10" /></svg
                                            >
                                        </a>
                                    {/if}
                                </div>

                                {#if restrictedNow}
                                    <p class="pw-mg-srcnote">
                                        {t('manga.restricted.note')}
                                        {#if officialUrl}<a href={officialUrl} target="_blank" rel="noopener"
                                                >{officialLabel}</a
                                            >.{/if}
                                    </p>
                                {/if}
                            </div>

                            {#if showRail}
                                <aside class="pw-mg-rail">
                                    <div class="pw-mg-panel">
                                        <div class="pw-mg-panel-head">
                                            <span class="pw-mg-panel-arrow">▸</span> reading progress
                                        </div>
                                        <div class="pw-mg-ring-wrap">
                                            <div class="pw-mg-ring">
                                                <svg width="92" height="92" viewBox="0 0 92 92">
                                                    <circle
                                                        cx="46"
                                                        cy="46"
                                                        r={ringR}
                                                        fill="none"
                                                        stroke="rgba(255,255,255,0.08)"
                                                        stroke-width="6"
                                                    />
                                                    <circle
                                                        cx="46"
                                                        cy="46"
                                                        r={ringR}
                                                        fill="none"
                                                        stroke="var(--pw-accent)"
                                                        stroke-width="6"
                                                        stroke-linecap="round"
                                                        stroke-dasharray={ringC}
                                                        stroke-dashoffset={ringOffset}
                                                        transform="rotate(-90 46 46)"
                                                    />
                                                </svg>
                                                <div class="pw-mg-ring-num">
                                                    <span class="pw-mg-ring-cur">{currentNum ?? '?'}</span>
                                                    <span class="pw-mg-ring-tot">/ {totalChapters || '?'}</span>
                                                </div>
                                            </div>
                                            <div class="pw-mg-rail-side">
                                                {#if newCount > 0}
                                                    <div class="pw-mg-new-pill">
                                                        <span class="pw-mg-new-dot"></span>
                                                        {newCount} new {newCount === 1 ? 'chapter' : 'chapters'}
                                                    </div>
                                                {:else if currentNum !== null}
                                                    <div class="pw-mg-rail-state">caught up</div>
                                                {/if}
                                                {#if detail.progress}
                                                    <div class="pw-mg-rail-on">
                                                        on <b>Ch. {detail.progress.chapter ?? '?'}</b> · page {detail
                                                            .progress.page + 1} of {detail.progress.pages}
                                                    </div>
                                                {/if}
                                            </div>
                                        </div>
                                        {#if detail.progress}
                                            <div class="pw-mg-prog-bar">
                                                <span style="width: {continuePct}%;"></span>
                                            </div>
                                        {/if}
                                        <div class="pw-mg-rail-rows">
                                            {#if detail.progress?.updated_at}
                                                <div class="pw-mg-rail-row">
                                                    <span class="pw-mg-rail-k">last read</span>
                                                    <span class="pw-mg-rail-v"
                                                        >{fmtDate(detail.progress.updated_at)}</span
                                                    >
                                                </div>
                                            {/if}
                                            <div class="pw-mg-rail-row">
                                                <span class="pw-mg-rail-k">chapters read</span>
                                                <span class="pw-mg-rail-v"
                                                    >{readCount}
                                                    <span class="pw-mg-rail-v-soft">of {totalChapters || '?'}</span
                                                    ></span
                                                >
                                            </div>
                                            {#if detail.manga.status}
                                                <div class="pw-mg-rail-row">
                                                    <span class="pw-mg-rail-k">status</span>
                                                    <span class="pw-mg-rail-v pw-mg-rail-status"
                                                        >{detail.manga.status}</span
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

            <section class="pw-section pw-mg-sec">
                <div class="pw-mg-sechead">
                    <button
                        class="pw-mg-h2btn"
                        onclick={() => (chaptersOpen = !chaptersOpen)}
                        aria-expanded={chaptersOpen}
                    >
                        <svg
                            class="pw-mg-chev"
                            class:on={chaptersOpen}
                            width="14"
                            height="14"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"><polyline points="9 6 15 12 9 18" /></svg
                        >
                        <span class="pw-mg-h2">Chapters</span>
                        <span class="pw-mg-count">{visibleRows.length}</span>
                    </button>
                    {#if chaptersOpen}
                        <div class="pw-mg-controls">
                            {#if langs.length > 1}
                                <div class="pw-mg-langchips">
                                    {#each ['all', ...langs] as l (l)}
                                        <button class="pw-mg-langchip" class:on={lang === l} onclick={() => (lang = l)}
                                            >{l}</button
                                        >
                                    {/each}
                                </div>
                            {/if}
                            <div class="pw-mg-seg">
                                <button class:on={view === 'all'} onclick={() => (view = 'all')}>All</button>
                                <button class:on={view === 'unread'} onclick={() => (view = 'unread')}>
                                    Unread{#if newCount > 0}<span class="pw-mg-seg-n"
                                            >{newCount + (detail.progress ? 1 : 0)}</span
                                        >{/if}
                                </button>
                            </div>
                            <div class="pw-mg-seg">
                                <button class:on={sort === 'new'} onclick={() => (sort = 'new')}>Newest</button>
                                <button class:on={sort === 'old'} onclick={() => (sort = 'old')}>Oldest</button>
                            </div>
                        </div>
                    {/if}
                </div>

                {#if chaptersOpen}
                    {#if chaptersLoading}
                        <div class="pw-empty-tag">// loading chapters...</div>
                    {:else if inLang.length === 0}
                        {#if detail.restricted && officialUrl}
                            <div class="pw-empty-card">
                                <div class="pw-empty-tag">// {t('manga.restricted.empty')}</div>
                                <a
                                    class="pw-v1-btn-watch"
                                    href={officialUrl}
                                    target="_blank"
                                    rel="noopener"
                                    style="margin-top: 14px; display: inline-flex; align-items: center;"
                                >
                                    {officialLabel}
                                    <svg
                                        width="11"
                                        height="11"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="2.4"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        style="margin-left: 6px;"><path d="M7 17 17 7" /><path d="M7 7h10v10" /></svg
                                    >
                                </a>
                            </div>
                        {:else}
                            <div class="pw-empty-tag">// {t('manga.empty.chapters')}</div>
                        {/if}
                    {:else}
                        {#if currentChapter && detail.progress}
                            <button class="pw-mg-continue" onclick={continueReading}>
                                <span class="pw-mg-cont-play">
                                    <svg width="20" height="20" viewBox="0 0 24 24" fill="#08090b"
                                        ><path d="M8 5l11 7-11 7z" /></svg
                                    >
                                </span>
                                <span class="pw-mg-cont-body">
                                    <span class="pw-mg-cont-head">continue reading</span>
                                    <span class="pw-mg-cont-row">
                                        <span class="pw-mg-cont-num">Ch. {detail.progress.chapter ?? '?'}</span>
                                        {#if currentChapter.title}<span class="pw-mg-cont-title"
                                                >{currentChapter.title}</span
                                            >{/if}
                                        <span class="pw-mg-cont-page"
                                            >page {detail.progress.page + 1} / {detail.progress.pages}</span
                                        >
                                    </span>
                                    <span class="pw-mg-prog-bar"><span style="width: {continuePct}%;"></span></span>
                                </span>
                                <svg
                                    class="pw-mg-cont-chev"
                                    width="20"
                                    height="20"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"><polyline points="9 6 15 12 9 18" /></svg
                                >
                            </button>
                        {/if}

                        <div class="pw-mg-chgrid">
                            {#each gridRows as r (r.ch.id)}
                                {@const ch = r.ch}
                                {@const st = r.state}
                                {@const fromCk = ch.id.startsWith('ck:')}
                                {@const fromMk = ch.id.startsWith('mk:')}
                                <button class="pw-mg-ch" data-state={st} onclick={() => read(ch, 0)}>
                                    <span class="pw-mg-ch-dot" data-state={st}></span>
                                    <span class="pw-mg-ch-num">Ch. {ch.chapter ?? '?'}</span>
                                    {#if ch.title && st !== 'read'}
                                        <span class="pw-mg-ch-title">{ch.title}</span>
                                    {:else}
                                        <span class="pw-mg-ch-title"></span>
                                    {/if}
                                    <span class="pw-mg-ch-meta">
                                        {#if st === 'new'}<span class="pw-mg-ch-new">NEW</span>{/if}
                                        {#if fromCk}
                                            <span class="pw-mg-ch-group">comick</span>
                                        {:else if fromMk}
                                            <span class="pw-mg-ch-group">katana</span>
                                        {/if}
                                        {#if ch.published_at}
                                            <span class="pw-mg-ch-date">{fmtDate(ch.published_at)}</span>
                                        {/if}
                                        {#if st === 'read'}
                                            <span class="pw-mg-ch-check">
                                                <svg
                                                    width="13"
                                                    height="13"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2.6"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                                >
                                            </span>
                                        {/if}
                                    </span>
                                </button>
                            {/each}
                        </div>
                    {/if}
                {/if}
            </section>

            {#if related.length > 0}
                <section class="pw-section pw-mg-sec">
                    <div class="pw-mg-sechead">
                        <button
                            class="pw-mg-h2btn"
                            onclick={() => (relatedOpen = !relatedOpen)}
                            aria-expanded={relatedOpen}
                        >
                            <svg
                                class="pw-mg-chev"
                                class:on={relatedOpen}
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"><polyline points="9 6 15 12 9 18" /></svg
                            >
                            <span class="pw-mg-h2">Related</span>
                            <span class="pw-mg-count">{related.length}</span>
                        </button>
                    </div>
                    {#if relatedOpen}
                        <div class="pw-mg-shelf">
                            {#each related as r (r.anilist_id)}
                                <a
                                    class="pw-mg-relcard"
                                    href="https://anilist.co/manga/{r.anilist_id}"
                                    target="_blank"
                                    rel="noopener"
                                    title={r.title}
                                >
                                    <div class="pw-mg-relcard-frame">
                                        {#if r.cover_url}<img
                                                src={r.cover_url}
                                                alt={r.title}
                                                loading="lazy"
                                                decoding="async"
                                            />{/if}
                                    </div>
                                    <div class="pw-mg-relcard-title">{r.title}</div>
                                    <div class="pw-mg-relcard-sub">{r.relation.replace('_', ' ').toLowerCase()}</div>
                                </a>
                            {/each}
                        </div>
                    {/if}
                </section>
            {/if}

            {#if recs.length > 0}
                <section class="pw-section pw-mg-sec">
                    <div class="pw-mg-sechead">
                        <button class="pw-mg-h2btn" onclick={() => (recsOpen = !recsOpen)} aria-expanded={recsOpen}>
                            <svg
                                class="pw-mg-chev"
                                class:on={recsOpen}
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"><polyline points="9 6 15 12 9 18" /></svg
                            >
                            <span class="pw-mg-h2">Recommendations</span>
                            <span class="pw-mg-count">{recs.length}</span>
                        </button>
                    </div>
                    {#if recsOpen}
                        <div class="pw-mg-shelf">
                            {#each recs as r (r.anilist_id)}
                                <a
                                    class="pw-mg-relcard"
                                    href="https://anilist.co/manga/{r.anilist_id}"
                                    target="_blank"
                                    rel="noopener"
                                    title={r.title}
                                >
                                    <div class="pw-mg-relcard-frame">
                                        {#if r.cover_url}<img
                                                src={r.cover_url}
                                                alt={r.title}
                                                loading="lazy"
                                                decoding="async"
                                            />{/if}
                                    </div>
                                    <div class="pw-mg-relcard-title">{r.title}</div>
                                </a>
                            {/each}
                        </div>
                    {/if}
                </section>
            {/if}
        {/if}

        <div style="height: 60px;"></div>
    </div>
{/if}

<style>
    .pw-mg-hero {
        isolation: isolate;
        overflow: hidden;
    }
    .pw-mg-hero-img {
        filter: blur(48px) saturate(108%) brightness(0.55);
        transform: scale(1.12);
    }

    .pw-mg-layout {
        display: grid;
        grid-template-columns: clamp(180px, 24vw, 240px) minmax(0, 1fr);
        gap: clamp(20px, 3vw, 44px);
        align-items: start;
    }
    @media (min-width: 1180px) {
        .pw-mg-layout-rail {
            grid-template-columns: clamp(200px, 19vw, 268px) minmax(0, 1fr) clamp(232px, 21vw, 280px);
        }
    }
    @media (max-width: 720px) {
        .pw-mg-layout,
        .pw-mg-layout-rail {
            grid-template-columns: 1fr;
            gap: 22px;
        }
        .pw-mg-cover-col {
            width: clamp(140px, 38vw, 200px);
            margin: 0 auto;
        }
    }

    .pw-mg-cover {
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
    .pw-mg-cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-mg-cover-badge {
        position: absolute;
        top: 10px;
        font-size: 10px;
        padding: 3px 8px;
        border-radius: 4px;
        letter-spacing: 0.05em;
        font-weight: 600;
        backdrop-filter: blur(4px);
    }
    .pw-mg-cover-score {
        right: 10px;
        background: rgba(20, 20, 26, 0.75);
        color: var(--pw-accent);
        border: 1px solid rgba(255, 255, 255, 0.06);
    }

    .pw-mg-info {
        min-width: 0;
    }
    .pw-mg-bc {
        font-size: 11.5px;
        letter-spacing: 0.04em;
        color: rgba(220, 220, 225, 0.5);
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 14px;
    }
    .pw-mg-bc-arrow {
        color: var(--pw-accent);
        font-family: ui-monospace, monospace;
    }
    .pw-mg-bc-sep {
        opacity: 0.4;
    }
    .pw-mg-bc b {
        color: #ececef;
        font-weight: 500;
    }

    .pw-mg-title {
        font-size: clamp(30px, 4.6vw, 56px);
        font-weight: 300;
        letter-spacing: -0.025em;
        line-height: 1.02;
        color: #f4f4f6;
        margin: 0;
    }
    .pw-mg-authors {
        font-size: 14px;
        color: rgba(220, 220, 225, 0.62);
        margin-top: 10px;
    }
    .pw-mg-authors b {
        color: #d8d8da;
        font-weight: 500;
    }

    .pw-mg-meta {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 8px 12px;
        margin-top: 16px;
        font-size: 13px;
    }
    .pw-mg-meta-text {
        color: rgba(220, 220, 225, 0.78);
    }
    .pw-mg-meta-sep {
        width: 3px;
        height: 3px;
        border-radius: 999px;
        background: rgba(220, 220, 225, 0.28);
    }
    .pw-mg-tag-mono {
        font-family: ui-monospace, monospace;
        font-size: 10.5px;
        letter-spacing: 0.08em;
        padding: 3px 8px;
        border-radius: 4px;
        text-transform: uppercase;
    }
    .pw-mg-status-ok {
        background: rgba(120, 200, 150, 0.16);
        color: rgba(160, 220, 180, 0.95);
    }
    .pw-mg-tag-neutral {
        background: rgba(255, 255, 255, 0.06);
        color: rgba(220, 220, 225, 0.72);
    }
    .pw-mg-score-inline {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        color: var(--pw-accent);
        font-weight: 600;
    }
    .pw-mg-meta-mono {
        font-family: ui-monospace, monospace;
        font-size: 11.5px;
        color: rgba(220, 220, 225, 0.42);
    }
    .pw-mg-meta-hearts {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        color: rgba(220, 220, 225, 0.6);
    }
    .pw-mg-meta-hearts svg {
        color: rgba(220, 100, 120, 0.85);
    }

    .pw-mg-synopsis {
        font-size: 14px;
        line-height: 1.66;
        color: rgba(216, 216, 218, 0.78);
        margin-top: 20px;
        max-width: 760px;
    }
    .pw-mg-readmore {
        color: var(--pw-accent);
        background: none;
        border: 0;
        font: inherit;
        font-weight: 500;
        cursor: pointer;
        padding: 0;
    }

    .pw-mg-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 7px;
        margin-top: 18px;
    }
    .pw-mg-tag {
        font-size: 11.5px;
        padding: 4px 11px;
        border-radius: 999px;
        color: rgba(220, 220, 225, 0.75);
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.07);
        cursor: default;
        transition: all 0.14s ease;
    }
    .pw-mg-tag:hover {
        color: #ececef;
        border-color: rgba(140, 110, 200, 0.55);
        background: rgba(140, 110, 200, 0.12);
    }

    .pw-mg-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-top: 24px;
        align-items: center;
    }
    .pw-mg-btn-lib.on {
        color: rgba(160, 220, 180, 0.95);
        border-color: rgba(120, 200, 150, 0.28);
        background: rgba(120, 200, 150, 0.1);
    }
    .pw-mg-btn-lib.on:hover {
        background: rgba(120, 200, 150, 0.18);
    }

    .pw-mg-srcnote {
        margin-top: 16px;
        font-size: 12px;
        line-height: 1.55;
        color: rgba(220, 220, 225, 0.5);
        max-width: 560px;
    }
    .pw-mg-srcnote a {
        color: var(--pw-accent);
        text-decoration: none;
        border-bottom: 1px dotted rgba(140, 110, 200, 0.5);
    }

    .pw-mg-rail {
        position: sticky;
        top: 76px;
    }
    @media (max-width: 1180px) {
        .pw-mg-rail {
            position: static;
        }
    }
    .pw-mg-panel {
        background: rgba(15, 16, 20, 0.7);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 14px;
        padding: 18px;
        backdrop-filter: blur(10px);
    }
    .pw-mg-panel-head {
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
    .pw-mg-panel-arrow {
        color: var(--pw-accent);
    }

    .pw-mg-ring-wrap {
        display: flex;
        align-items: center;
        gap: 16px;
    }
    .pw-mg-ring {
        position: relative;
        width: 92px;
        height: 92px;
        flex-shrink: 0;
    }
    .pw-mg-ring-num {
        position: absolute;
        inset: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
    }
    .pw-mg-ring-cur {
        font-size: 26px;
        color: #f2f2f4;
        line-height: 1;
        font-weight: 500;
    }
    .pw-mg-ring-tot {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        color: rgba(220, 220, 225, 0.42);
        margin-top: 3px;
    }
    .pw-mg-rail-side {
        min-width: 0;
    }
    .pw-mg-new-pill {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 4px 9px;
        border-radius: 999px;
        background: rgba(140, 110, 200, 0.16);
        color: var(--pw-accent);
        font-size: 11.5px;
        font-weight: 600;
    }
    .pw-mg-new-dot {
        width: 6px;
        height: 6px;
        border-radius: 999px;
        background: var(--pw-accent);
    }
    .pw-mg-rail-state {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.6);
    }
    .pw-mg-rail-on {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.55);
        margin-top: 9px;
        line-height: 1.5;
    }
    .pw-mg-rail-on b {
        color: #ececef;
        font-weight: 600;
    }

    .pw-mg-prog-bar {
        display: block;
        height: 4px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.1);
        overflow: hidden;
        margin-top: 14px;
    }
    .pw-mg-prog-bar > span {
        display: block;
        height: 100%;
        background: var(--pw-accent);
        border-radius: 999px;
        transition: width 0.2s ease;
    }

    .pw-mg-rail-rows {
        margin-top: 8px;
    }
    .pw-mg-rail-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 9px 0;
        border-top: 1px dashed rgba(255, 255, 255, 0.07);
    }
    .pw-mg-rail-k {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        letter-spacing: 0.06em;
        color: rgba(220, 220, 225, 0.4);
        text-transform: uppercase;
    }
    .pw-mg-rail-v {
        font-size: 14px;
        color: #ececef;
        font-weight: 500;
    }
    .pw-mg-rail-v-soft {
        color: rgba(220, 220, 225, 0.4);
        font-weight: 400;
        font-size: 12px;
    }
    .pw-mg-rail-status {
        color: rgba(160, 220, 180, 0.95);
        text-transform: capitalize;
    }

    .pw-mg-sec {
        max-width: 1800px;
        margin: 0 auto;
        padding: 14px clamp(16px, 4vw, 36px) 6px;
    }
    .pw-mg-sec + .pw-mg-sec {
        padding-top: 6px;
    }
    .pw-mg-sechead {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        flex-wrap: wrap;
        margin-bottom: 12px;
    }
    .pw-mg-h2btn {
        display: inline-flex;
        align-items: baseline;
        gap: 8px;
        background: none;
        border: 0;
        padding: 0;
        font: inherit;
        color: inherit;
        cursor: pointer;
        min-width: 0;
    }
    .pw-mg-h2btn:hover .pw-mg-h2 {
        color: #fff;
    }
    .pw-mg-h2 {
        font-size: 22px;
        font-weight: 600;
        color: #f4f4f6;
        letter-spacing: -0.02em;
        line-height: 1.2;
        transition: color 0.12s ease;
    }
    .pw-mg-chev {
        color: rgba(220, 220, 225, 0.5);
        transform: rotate(0deg);
        transform-origin: 50% 50%;
        transition: transform 0.18s ease;
        align-self: center;
    }
    .pw-mg-chev.on {
        transform: rotate(90deg);
    }
    .pw-mg-count {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.42);
        font-weight: 500;
        font-variant-numeric: tabular-nums;
    }
    .pw-mg-controls {
        display: flex;
        gap: 10px;
        align-items: center;
        flex-wrap: wrap;
    }
    .pw-mg-langchips {
        display: flex;
        gap: 2px;
    }
    .pw-mg-langchip {
        background: transparent;
        border: 0;
        padding: 6px 11px;
        border-radius: 999px;
        font-size: 11.5px;
        color: rgba(220, 220, 225, 0.45);
        cursor: pointer;
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }
    .pw-mg-langchip.on {
        background: rgba(255, 255, 255, 0.1);
        color: #ececef;
    }
    .pw-mg-seg {
        display: inline-flex;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-radius: 8px;
        padding: 3px;
        gap: 2px;
    }
    .pw-mg-seg button {
        background: transparent;
        border: 0;
        padding: 6px 12px;
        border-radius: 6px;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.55);
        cursor: pointer;
        display: inline-flex;
        align-items: center;
        gap: 6px;
        transition: all 0.14s ease;
    }
    .pw-mg-seg button:hover {
        color: #ececef;
    }
    .pw-mg-seg button.on {
        background: #ececef;
        color: #08090b;
        font-weight: 600;
    }
    .pw-mg-seg-n {
        font-size: 10.5px;
        opacity: 0.6;
        font-variant-numeric: tabular-nums;
    }

    .pw-mg-continue {
        position: relative;
        display: flex;
        align-items: center;
        gap: 18px;
        padding: 18px 20px;
        margin-bottom: 18px;
        border-radius: 14px;
        border: 1px solid rgba(140, 110, 200, 0.22);
        background: linear-gradient(110deg, rgba(140, 110, 200, 0.1), #0f1014 60%);
        cursor: pointer;
        width: 100%;
        text-align: left;
        font: inherit;
        color: inherit;
        transition:
            border-color 0.15s ease,
            background 0.15s ease;
    }
    .pw-mg-continue:hover {
        border-color: rgba(140, 110, 200, 0.4);
        background: linear-gradient(110deg, rgba(140, 110, 200, 0.14), #0f1014 60%);
    }
    .pw-mg-cont-play {
        width: 48px;
        height: 48px;
        border-radius: 999px;
        background: var(--pw-accent);
        display: grid;
        place-items: center;
        flex-shrink: 0;
    }
    .pw-mg-cont-body {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    .pw-mg-cont-head {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        letter-spacing: 0.1em;
        text-transform: uppercase;
        color: var(--pw-accent);
    }
    .pw-mg-cont-row {
        display: flex;
        align-items: baseline;
        gap: 10px;
        flex-wrap: wrap;
    }
    .pw-mg-cont-num {
        font-size: 17px;
        font-weight: 600;
        color: #f2f2f4;
    }
    .pw-mg-cont-title {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.6);
    }
    .pw-mg-cont-page {
        font-family: ui-monospace, monospace;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        margin-left: auto;
    }
    .pw-mg-cont-chev {
        color: var(--pw-accent);
        flex-shrink: 0;
    }

    .pw-mg-chgrid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
        gap: 6px 14px;
    }
    @media (max-width: 760px) {
        .pw-mg-chgrid {
            grid-template-columns: 1fr;
        }
    }

    .pw-mg-ch {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 10px 12px;
        border-radius: 9px;
        border: 1px solid transparent;
        background: rgba(255, 255, 255, 0.018);
        cursor: pointer;
        font: inherit;
        color: inherit;
        text-align: left;
        transition:
            background 0.12s ease,
            border-color 0.12s ease,
            transform 0.08s ease;
    }
    .pw-mg-ch:hover {
        background: rgba(255, 255, 255, 0.05);
        border-color: rgba(255, 255, 255, 0.09);
    }
    .pw-mg-ch:active {
        transform: scale(0.997);
    }

    .pw-mg-ch-dot {
        width: 8px;
        height: 8px;
        border-radius: 999px;
        flex-shrink: 0;
        background: rgba(220, 220, 225, 0.5);
    }
    .pw-mg-ch-dot[data-state='read'] {
        background: transparent;
        border: 1.5px solid rgba(255, 255, 255, 0.14);
    }
    .pw-mg-ch-dot[data-state='new'],
    .pw-mg-ch-dot[data-state='continue'] {
        background: var(--pw-accent);
    }

    .pw-mg-ch-num {
        font-family: ui-monospace, monospace;
        font-size: 13px;
        color: #e6e6e8;
        min-width: 62px;
        flex-shrink: 0;
    }
    .pw-mg-ch[data-state='read'] .pw-mg-ch-num {
        color: rgba(220, 220, 225, 0.42);
        font-weight: 400;
    }
    .pw-mg-ch[data-state='new'] .pw-mg-ch-num {
        color: #f2f2f4;
    }

    .pw-mg-ch-title {
        font-size: 12.5px;
        color: rgba(220, 220, 225, 0.6);
        flex: 1;
        min-width: 0;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-mg-ch-meta {
        display: flex;
        align-items: center;
        gap: 9px;
        flex-shrink: 0;
    }
    .pw-mg-ch-new {
        font-family: ui-monospace, monospace;
        font-size: 9px;
        font-weight: 700;
        letter-spacing: 0.08em;
        padding: 2px 6px;
        border-radius: 3px;
        background: rgba(140, 110, 200, 0.22);
        color: var(--pw-accent);
    }
    .pw-mg-ch-group {
        font-family: ui-monospace, monospace;
        font-size: 9px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
        padding: 2px 6px;
        border-radius: 3px;
        color: rgba(220, 220, 225, 0.4);
        background: rgba(255, 255, 255, 0.05);
        opacity: 0;
        transition: opacity 0.14s ease;
    }
    .pw-mg-ch:hover .pw-mg-ch-group {
        opacity: 1;
    }
    @media (hover: none) {
        .pw-mg-ch-group {
            opacity: 1;
        }
    }
    .pw-mg-ch-date {
        font-family: ui-monospace, monospace;
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.34);
    }
    .pw-mg-ch-check {
        color: rgba(220, 220, 225, 0.32);
        display: flex;
    }

    .pw-mg-shelf {
        display: flex;
        gap: 14px;
        overflow-x: auto;
        padding-bottom: 8px;
        scroll-snap-type: x proximity;
    }
    .pw-mg-shelf::-webkit-scrollbar {
        height: 6px;
    }
    .pw-mg-shelf::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }
    .pw-mg-relcard {
        flex-shrink: 0;
        width: 144px;
        scroll-snap-align: start;
        text-decoration: none;
        color: inherit;
        cursor: pointer;
    }
    .pw-mg-relcard-frame {
        position: relative;
        width: 100%;
        aspect-ratio: 2/3;
        border-radius: 9px;
        overflow: hidden;
        background: #14141a;
        box-shadow: 0 10px 26px -14px rgba(0, 0, 0, 0.85);
        transition:
            transform 0.25s cubic-bezier(0.2, 0.7, 0.2, 1),
            box-shadow 0.25s ease;
    }
    .pw-mg-relcard:hover .pw-mg-relcard-frame {
        transform: translateY(-3px);
        box-shadow:
            0 16px 32px -16px rgba(0, 0, 0, 0.9),
            0 0 0 1px rgba(255, 255, 255, 0.1);
    }
    .pw-mg-relcard-frame img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-mg-relcard-title {
        font-size: 12px;
        color: #ececef;
        margin-top: 8px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-mg-relcard-sub {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        color: rgba(220, 220, 225, 0.4);
        text-transform: uppercase;
        letter-spacing: 0.04em;
        margin-top: 2px;
    }

    .pw-link-chip {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        height: 30px;
        padding: 0 12px;
        font-size: 12.5px;
        font-weight: 500;
        color: rgba(232, 232, 234, 0.78);
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 999px;
        text-decoration: none;
        cursor: pointer;
        transition:
            background 0.12s ease,
            color 0.12s ease;
    }
    .pw-link-chip:hover {
        background: rgba(255, 255, 255, 0.1);
        color: #fff;
    }
    .pw-anime-chip {
        color: var(--pw-accent);
        border-color: rgba(140, 110, 200, 0.32);
        background: rgba(140, 110, 200, 0.08);
    }
    .pw-anime-chip:hover {
        background: rgba(140, 110, 200, 0.16);
        color: var(--pw-accent);
    }
</style>
