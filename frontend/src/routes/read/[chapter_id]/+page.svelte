<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { goto } from '$app/navigation';
    import { page } from '$app/state';
    import { api, type MangaChapter, type MangaDetail } from '$lib/api';
    import { t } from '$lib/i18n.svelte';

    type Mode = 'paged' | 'double' | 'webtoon';
    type Fit = 'screen' | 'width' | 'height' | 'original';
    type Theme = 'dark' | 'light' | 'sepia';

    type Prefs = {
        mode: Mode;
        dir: 'rtl' | 'ltr';
        fit: { paged: Fit; double: Fit; webtoon: Fit };
        theme: Theme;
        preload: number;
    };

    const DEFAULT_PREFS: Prefs = {
        mode: 'paged',
        dir: 'rtl',
        fit: { paged: 'screen', double: 'screen', webtoon: 'width' },
        theme: 'dark',
        preload: 3
    };

    const PREFS_KEY = 'pw-manga-prefs';
    const TAP_HINT_KEY = 'pw-manga-tap-hint';
    const WEBTOON_TAGS = new Set(['Long Strip', 'Web Comic', 'Webtoon', 'Full Color']);

    const chapterId = $derived(page.params.chapter_id ?? '');
    const mdId = $derived(page.url.searchParams.get('md') ?? '');

    let prefs = $state<Prefs>({ ...DEFAULT_PREFS, fit: { ...DEFAULT_PREFS.fit } });
    let mode = $derived(prefs.mode);
    let theme = $derived(prefs.theme);
    let fitNow = $derived(prefs.fit[mode] ?? 'screen');

    let pages = $state<string[]>([]);
    let idx = $state(0);
    let imgDims = $state<Record<number, { w: number; h: number }>>({});
    let loading = $state(true);
    let err = $state('');
    let ui = $state(true);
    let aaOpen = $state(false);
    let endOverlay = $state(false);
    let tapHint = $state(false);
    let isFs = $state(false);
    let fsOk = $state(false);
    let chapters = $state<MangaChapter[]>([]);
    let detail = $state<MangaDetail | null>(null);
    let loadedFor = $state('');

    const current = $derived(chapters.find((c) => c.id === chapterId));
    const sameLang = $derived(current ? chapters.filter((c) => c.lang === current.lang) : []);
    const chPos = $derived(sameLang.findIndex((c) => c.id === chapterId));
    const nextCh = $derived(chPos >= 0 ? sameLang[chPos + 1] : undefined);
    const prevCh = $derived(chPos > 0 ? sameLang[chPos - 1] : undefined);

    const isWebtoonTagged = $derived((detail?.tags ?? []).some((tg) => WEBTOON_TAGS.has(tg)));

    const chTitle = $derived(current?.chapter ? `Ch. ${current.chapter}` : 'reading');

    const pagedRight = $derived(mode === 'paged' || mode === 'double');
    const isRtl = $derived(pagedRight && prefs.dir === 'rtl');

    onMount(() => {
        loadPrefs();
        if (mdId && !sessionStorage.getItem(TAP_HINT_KEY)) {
            tapHint = true;
            setTimeout(() => (tapHint = false), 1700);
            sessionStorage.setItem(TAP_HINT_KEY, '1');
        }
        fsOk = !!document.documentElement.requestFullscreen;

        const onFs = () => (isFs = !!document.fullscreenElement);
        const onKey = (e: KeyboardEvent) => {
            if (aaOpen) {
                if (e.key === 'Escape') aaOpen = false;
                return;
            }
            if (e.key === 'ArrowRight') (isRtl ? prev : next)();
            else if (e.key === 'ArrowLeft') (isRtl ? next : prev)();
            else if (e.key === ' ') {
                e.preventDefault();
                next();
            } else if (e.key === 'Escape') {
                if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
                else exit();
            }
        };
        window.addEventListener('keydown', onKey);
        document.addEventListener('fullscreenchange', onFs);

        if (mdId) {
            api.mangaChapters(mdId)
                .then((c) => (chapters = c))
                .catch((e) => console.error('[reader] chapters failed', e));
            api.mangaDetail(mdId)
                .then((d) => {
                    detail = d;
                    applyAutoMode();
                })
                .catch(() => {});
        }

        return () => {
            window.removeEventListener('keydown', onKey);
            document.removeEventListener('fullscreenchange', onFs);
            if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
        };
    });

    function loadPrefs() {
        try {
            const g = localStorage.getItem(PREFS_KEY);
            if (g)
                prefs = {
                    ...DEFAULT_PREFS,
                    ...JSON.parse(g),
                    fit: { ...DEFAULT_PREFS.fit, ...(JSON.parse(g).fit ?? {}) }
                };
        } catch {}
        if (!mdId) return;
        try {
            const s = localStorage.getItem(`${PREFS_KEY}-${mdId}`);
            if (s) prefs = { ...prefs, ...JSON.parse(s), fit: { ...prefs.fit, ...(JSON.parse(s).fit ?? {}) } };
        } catch {}
        const ltr = localStorage.getItem('pw-manga-ltr');
        if (ltr === '1' && prefs.dir === 'rtl') prefs.dir = 'ltr';
    }

    function savePrefs() {
        try {
            localStorage.setItem(
                PREFS_KEY,
                JSON.stringify({
                    mode: prefs.mode,
                    dir: prefs.dir,
                    fit: prefs.fit,
                    theme: prefs.theme,
                    preload: prefs.preload
                })
            );
            if (mdId)
                localStorage.setItem(
                    `${PREFS_KEY}-${mdId}`,
                    JSON.stringify({
                        mode: prefs.mode,
                        dir: prefs.dir,
                        fit: prefs.fit
                    })
                );
        } catch {}
    }

    function applyAutoMode() {
        const explicit = mdId && localStorage.getItem(`${PREFS_KEY}-${mdId}`);
        if (explicit) return;
        if (isWebtoonTagged && prefs.mode !== 'webtoon') {
            prefs.mode = 'webtoon';
        }
    }

    $effect(() => {
        const id = chapterId;
        if (!id || loadedFor === id) return;
        loadedFor = id;
        loading = true;
        err = '';
        endOverlay = false;
        imgDims = {};
        api.mangaChapterPages(id)
            .then((r) => {
                pages = r.pages;
                const wanted = parseInt(page.url.searchParams.get('page') ?? '0', 10);
                idx = wanted > 0 && wanted < r.pages.length ? wanted : 0;
            })
            .catch((e) => {
                console.error('[reader] pages failed', e);
                err = 'chapter unavailable, mangadex might be down';
            })
            .finally(() => (loading = false));
    });

    $effect(() => {
        if (mode === 'webtoon') return;
        const n = prefs.preload;
        for (let i = 1; i <= n; i++) {
            const u = pages[idx + i];
            if (u) new Image().src = u;
        }
        const back = pages[idx - 1];
        if (back) new Image().src = back;
    });

    let saveTimer: ReturnType<typeof setTimeout> | undefined;
    $effect(() => {
        const p = idx;
        if (!mdId || pages.length === 0) return;
        clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            api.mangaProgress({
                md_id: mdId,
                chapter_id: chapterId,
                chapter: current?.chapter ?? null,
                page: p,
                pages: pages.length
            }).catch(() => {});
        }, 600);
    });

    function recordDims(i: number, ev: Event) {
        const img = ev.currentTarget as HTMLImageElement;
        if (!img.naturalWidth || !img.naturalHeight) return;
        imgDims = { ...imgDims, [i]: { w: img.naturalWidth, h: img.naturalHeight } };
    }

    function isLandscape(i: number): boolean {
        const d = imgDims[i];
        return !!d && d.w > d.h;
    }

    function nextIdx(): number {
        if (mode !== 'double') return idx + 1;
        if (idx === 0) return 1;
        if (idx >= pages.length - 1) return idx + 1;
        if (isLandscape(idx) || isLandscape(idx + 1)) return idx + 1;
        return idx + 2;
    }

    function prevIdx(): number {
        if (mode !== 'double') return idx - 1;
        if (idx <= 1) return 0;
        if (isLandscape(idx - 1)) return idx - 1;
        if (isLandscape(idx - 2)) return idx - 1;
        return idx - 2;
    }

    function next() {
        if (mode === 'webtoon') return;
        const ni = nextIdx();
        if (ni < pages.length) {
            idx = ni;
            endOverlay = false;
        } else if (ni >= pages.length) {
            endOverlay = true;
        }
    }

    function prev() {
        if (mode === 'webtoon') return;
        if (endOverlay) {
            endOverlay = false;
            return;
        }
        const pi = prevIdx();
        if (pi >= 0) idx = pi;
        else if (prevCh) goto(`/read/${prevCh.id}?md=${mdId}`);
    }

    function toNextChapter() {
        if (nextCh) goto(`/read/${nextCh.id}?md=${mdId}`);
        else exit();
    }

    function toPrevChapter() {
        if (prevCh) goto(`/read/${prevCh.id}?md=${mdId}`);
        else exit();
    }

    function exit() {
        if (mdId) goto(`/manga/${mdId}`);
        else history.back();
    }

    function toggleFs() {
        if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
        else document.documentElement.requestFullscreen().catch(() => {});
    }

    function zone(e: MouseEvent) {
        if (aaOpen) return;
        if (mode === 'webtoon') {
            ui = !ui;
            return;
        }
        const w = window.innerWidth;
        const x = e.clientX;
        if (x < w / 3) (isRtl ? next : prev)();
        else if (x > (w * 2) / 3) (isRtl ? prev : next)();
        else ui = !ui;
    }

    function setMode(m: string) {
        prefs.mode = m as Mode;
        if (m === 'webtoon') endOverlay = false;
        savePrefs();
    }
    function setDir(d: string) {
        prefs.dir = d as 'rtl' | 'ltr';
        savePrefs();
    }
    function setFit(f: string) {
        prefs.fit = { ...prefs.fit, [mode]: f as Fit };
        savePrefs();
    }
    function setTheme(t: string) {
        prefs.theme = t as Theme;
        savePrefs();
    }
    function bumpPreload(d: number) {
        const v = Math.max(1, Math.min(10, prefs.preload + d));
        prefs.preload = v;
        savePrefs();
    }

    let webRoot = $state<HTMLDivElement | undefined>(undefined);
    let webImgs = $state<Record<number, HTMLImageElement>>({});

    $effect(() => {
        if (mode !== 'webtoon' || !webRoot || pages.length === 0) return;
        const obs = new IntersectionObserver(
            (entries) => {
                for (const e of entries) {
                    const el = e.target as HTMLImageElement;
                    const i = parseInt(el.dataset.i ?? '0', 10);
                    if (e.isIntersecting && !el.src && el.dataset.src) {
                        el.src = el.dataset.src;
                    }
                    if (e.isIntersecting && e.intersectionRatio > 0.4) {
                        idx = i;
                        if (i === pages.length - 1) endOverlay = true;
                    }
                }
            },
            { root: null, rootMargin: '200% 0px', threshold: [0, 0.4, 1] }
        );
        for (const el of Object.values(webImgs)) {
            if (el) obs.observe(el);
        }
        const wanted = parseInt(page.url.searchParams.get('page') ?? '0', 10);
        if (wanted > 0 && webImgs[wanted]) {
            requestAnimationFrame(() => webImgs[wanted]?.scrollIntoView({ block: 'start' }));
        }
        return () => obs.disconnect();
    });

    $effect(() => {
        void mode;
        tick().then(() => {
            ui = true;
            aaOpen = false;
        });
    });

    const fitClass = $derived(`fit-${fitNow}`);
    const pagedShown = $derived(
        mode === 'double' && idx > 0 && idx < pages.length - 1 && !isLandscape(idx) && !isLandscape(idx + 1)
            ? [pages[idx], pages[idx + 1]]
            : [pages[idx]].filter(Boolean)
    );
    const pairOrder = $derived(isRtl ? [...pagedShown].reverse() : pagedShown);
    const progress = $derived(
        pages.length > 0 ? Math.round((Math.min(idx + 1, pages.length) / pages.length) * 100) : 0
    );
</script>

<svelte:head><title>{chTitle} - pleasewatch</title></svelte:head>

<div
    class="pw-reader"
    class:pw-light={theme === 'light'}
    class:pw-sepia={theme === 'sepia'}
    data-mode={mode}
    onclick={zone}
    role="presentation"
>
    {#if loading}
        <div class="pw-reader-msg">// loading...</div>
    {:else if err}
        <div class="pw-reader-msg">
            {err}
            <button
                class="pw-reader-msg-btn"
                onclick={(e) => {
                    e.stopPropagation();
                    exit();
                }}>back</button
            >
        </div>
    {:else if mode === 'webtoon'}
        <div class="pw-web-root" bind:this={webRoot}>
            {#each pages as p, i (i)}
                <img
                    class="pw-web-img"
                    class:fit-width={fitNow === 'width' || fitNow === 'screen'}
                    bind:this={webImgs[i]}
                    data-i={i}
                    data-src={p}
                    alt="page {i + 1}"
                    draggable="false"
                    loading="lazy"
                    onload={(e) => recordDims(i, e)}
                />
            {/each}
            <div class="pw-web-end">
                <button
                    class="pw-reader-msg-btn"
                    onclick={(e) => {
                        e.stopPropagation();
                        toPrevChapter();
                    }}
                    disabled={!prevCh}
                >
                    {t('reader.chapter_end.prev')}
                </button>
                <button
                    class="pw-reader-msg-btn pw-end-next"
                    onclick={(e) => {
                        e.stopPropagation();
                        toNextChapter();
                    }}
                    disabled={!nextCh}
                >
                    {t('reader.chapter_end.next')}
                </button>
            </div>
        </div>
    {:else if endOverlay}
        <div class="pw-end-card" role="presentation" onclick={(e) => e.stopPropagation()}>
            <div class="pw-end-title">{t('reader.chapter_end.done')}</div>
            <div class="pw-end-row">
                <button
                    class="pw-reader-msg-btn"
                    onclick={() => {
                        endOverlay = false;
                        idx = pages.length - 1;
                    }}
                >
                    {t('reader.chapter_end.stay')}
                </button>
                {#if prevCh}
                    <button class="pw-reader-msg-btn" onclick={toPrevChapter}>
                        {t('reader.chapter_end.prev')}
                    </button>
                {/if}
                {#if nextCh}
                    <button class="pw-reader-msg-btn pw-end-next" onclick={toNextChapter}>
                        {t('reader.chapter_end.next')}
                    </button>
                {:else}
                    <button class="pw-reader-msg-btn pw-end-next" onclick={exit}>
                        {t('reader.chapter_end.back')}
                    </button>
                {/if}
            </div>
        </div>
    {:else if mode === 'double' && pagedShown.length > 1}
        <div class="pw-pair">
            {#each pairOrder as src, j (src + ':' + j)}
                <img
                    class="pw-reader-img pw-pair-half {fitClass}"
                    {src}
                    alt="page"
                    draggable="false"
                    onload={(e) => recordDims(idx + j, e)}
                />
            {/each}
        </div>
    {:else if pages[idx]}
        <img
            class="pw-reader-img {fitClass}"
            src={pages[idx]}
            alt="page {idx + 1}"
            draggable="false"
            onload={(e) => recordDims(idx, e)}
        />
    {/if}

    {#if ui && !endOverlay}
        <div class="pw-reader-top" role="presentation" onclick={(e) => e.stopPropagation()}>
            <button class="pw-reader-btn" onclick={exit} aria-label="back">
                <svg
                    width="18"
                    height="18"
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
            <span class="pw-reader-title">{chTitle}{current?.title ? ` · ${current.title}` : ''}</span>
            {#if mode !== 'webtoon'}
                <span class="pw-reader-count">{pages.length ? `${idx + 1} / ${pages.length}` : ''}</span>
            {:else}
                <span class="pw-reader-count">{progress}%</span>
            {/if}
            {#if fsOk}
                <button class="pw-reader-btn" onclick={toggleFs} aria-label={t('reader.fullscreen')}>
                    {#if isFs}
                        <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><path d="M8 3v3a2 2 0 0 1-2 2H3" /><path d="M21 8h-3a2 2 0 0 1-2-2V3" /><path
                                d="M3 16h3a2 2 0 0 1 2 2v3"
                            /><path d="M16 21v-3a2 2 0 0 1 2-2h3" /></svg
                        >
                    {:else}
                        <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><path d="M8 3H5a2 2 0 0 0-2 2v3" /><path d="M21 8V5a2 2 0 0 0-2-2h-3" /><path
                                d="M3 16v3a2 2 0 0 0 2 2h3"
                            /><path d="M16 21h3a2 2 0 0 0 2-2v-3" /></svg
                        >
                    {/if}
                </button>
            {/if}
            <button
                class="pw-reader-btn pw-reader-chip"
                onclick={() => (aaOpen = !aaOpen)}
                aria-label={t('reader.settings')}>Aa</button
            >
        </div>

        {#if !endOverlay && mode !== 'webtoon'}
            <div class="pw-reader-bottom" role="presentation" onclick={(e) => e.stopPropagation()}>
                {#if prevCh}
                    <button class="pw-reader-ch" onclick={() => goto(`/read/${prevCh.id}?md=${mdId}`)}>
                        <svg
                            width="15"
                            height="15"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"><polyline points="15 18 9 12 15 6" /></svg
                        >
                        ch. {prevCh.chapter ?? '?'}
                    </button>
                {:else}
                    <span></span>
                {/if}
                {#if nextCh}
                    <button class="pw-reader-ch" onclick={() => goto(`/read/${nextCh.id}?md=${mdId}`)}>
                        ch. {nextCh.chapter ?? '?'}
                        <svg
                            width="15"
                            height="15"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"><polyline points="9 18 15 12 9 6" /></svg
                        >
                    </button>
                {/if}
            </div>
        {/if}
    {/if}

    {#if aaOpen}
        <div
            class="pw-rs-scrim"
            role="presentation"
            onclick={(e) => {
                e.stopPropagation();
                aaOpen = false;
            }}
        >
            <div class="pw-rs-pop" role="presentation" onclick={(e) => e.stopPropagation()}>
                <div class="pw-rs-grab"></div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('reader.mode_label')}</span>
                    <div class="pw-rs-seg">
                        <button class:on={prefs.mode === 'paged'} onclick={() => setMode('paged')}
                            >{t('reader.mode.paged')}</button
                        >
                        <button class:on={prefs.mode === 'double'} onclick={() => setMode('double')}
                            >{t('reader.mode.double')}</button
                        >
                        <button class:on={prefs.mode === 'webtoon'} onclick={() => setMode('webtoon')}
                            >{t('reader.mode.webtoon')}</button
                        >
                    </div>
                </div>
                {#if pagedRight}
                    <div class="pw-rs-row">
                        <span class="pw-rs-label">{t('reader.dir_label')}</span>
                        <div class="pw-rs-seg">
                            <button class:on={prefs.dir === 'rtl'} onclick={() => setDir('rtl')}
                                >{t('reader.dir.rtl')}</button
                            >
                            <button class:on={prefs.dir === 'ltr'} onclick={() => setDir('ltr')}
                                >{t('reader.dir.ltr')}</button
                            >
                        </div>
                    </div>
                {/if}
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('reader.fit_label')}</span>
                    <div class="pw-rs-seg">
                        <button class:on={fitNow === 'screen'} onclick={() => setFit('screen')}
                            >{t('reader.fit.screen')}</button
                        >
                        <button class:on={fitNow === 'width'} onclick={() => setFit('width')}
                            >{t('reader.fit.width')}</button
                        >
                        <button class:on={fitNow === 'height'} onclick={() => setFit('height')}
                            >{t('reader.fit.height')}</button
                        >
                        <button class:on={fitNow === 'original'} onclick={() => setFit('original')}
                            >{t('reader.fit.original')}</button
                        >
                    </div>
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('reader.theme_label')}</span>
                    <div class="pw-rs-seg">
                        <button class:on={prefs.theme === 'dark'} onclick={() => setTheme('dark')}
                            >{t('reader.theme.dark')}</button
                        >
                        <button class:on={prefs.theme === 'light'} onclick={() => setTheme('light')}
                            >{t('reader.theme.light')}</button
                        >
                        <button class:on={prefs.theme === 'sepia'} onclick={() => setTheme('sepia')}
                            >{t('reader.theme.sepia')}</button
                        >
                    </div>
                </div>
                {#if mode !== 'webtoon'}
                    <div class="pw-rs-row">
                        <span class="pw-rs-label">{t('reader.preload_label')}</span>
                        <div class="pw-rs-seg">
                            <button onclick={() => bumpPreload(-1)} aria-label="less">−</button>
                            <span class="pw-rs-val">{prefs.preload}</span>
                            <button onclick={() => bumpPreload(1)} aria-label="more">+</button>
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    {/if}

    {#if tapHint && mode !== 'webtoon'}
        <div class="pw-tap-hint" role="presentation">
            <div class="pw-tap-zone pw-tap-prev">{isRtl ? t('reader.zone.next') : t('reader.zone.prev')}</div>
            <div class="pw-tap-zone pw-tap-mid">{t('reader.zone.menu')}</div>
            <div class="pw-tap-zone pw-tap-next">{isRtl ? t('reader.zone.prev') : t('reader.zone.next')}</div>
        </div>
    {/if}

    {#if mode !== 'webtoon' && pages.length > 0}
        <div class="pw-reader-bar">
            <div style="width: {progress}%;"></div>
        </div>
    {/if}
</div>

<style>
    .pw-reader {
        position: fixed;
        inset: 0;
        z-index: 200;
        background: #000;
        display: flex;
        align-items: center;
        justify-content: center;
        user-select: none;
        overflow: hidden;
        touch-action: pan-x pinch-zoom;
    }
    .pw-reader[data-mode='webtoon'] {
        display: block;
        overflow-y: auto;
        overflow-x: hidden;
        touch-action: pan-y pinch-zoom;
    }
    .pw-light {
        background: #f4f4f6;
        color: #222;
    }
    .pw-sepia {
        background: #f4ecd8;
        color: #4a3b2a;
    }

    .pw-reader-img {
        max-width: 100%;
        max-height: 100vh;
    }
    .pw-reader-img.fit-screen {
        object-fit: contain;
        max-width: 100%;
        max-height: 100vh;
    }
    .pw-reader-img.fit-width {
        width: 100%;
        max-height: none;
        object-fit: contain;
    }
    .pw-reader-img.fit-height {
        height: 100vh;
        max-width: none;
        object-fit: contain;
    }
    .pw-reader-img.fit-original {
        max-width: none;
        max-height: none;
    }

    .pw-pair {
        display: flex;
        align-items: center;
        justify-content: center;
        max-height: 100vh;
    }
    .pw-pair-half {
        max-height: 100vh;
        max-width: 50vw;
    }

    .pw-web-root {
        min-height: 100vh;
        padding: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
    }
    .pw-web-img {
        display: block;
        max-width: 920px;
        width: 100%;
        height: auto;
    }
    .pw-web-end {
        display: flex;
        justify-content: center;
        gap: 12px;
        padding: 40px 16px calc(80px + env(safe-area-inset-bottom));
    }

    .pw-reader-msg {
        color: rgba(220, 220, 225, 0.5);
        font-size: 14px;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 14px;
        position: absolute;
        inset: 0;
        justify-content: center;
    }
    .pw-light .pw-reader-msg,
    .pw-sepia .pw-reader-msg {
        color: rgba(70, 62, 48, 0.65);
    }
    .pw-reader-msg-btn {
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #ececef;
        border-radius: 999px;
        padding: 10px 22px;
        font-size: 14px;
        cursor: pointer;
    }
    .pw-reader-msg-btn:disabled {
        opacity: 0.4;
        cursor: default;
    }
    .pw-light .pw-reader-msg-btn,
    .pw-sepia .pw-reader-msg-btn {
        background: rgba(0, 0, 0, 0.06);
        border-color: rgba(0, 0, 0, 0.14);
        color: #2c2a26;
    }
    .pw-end-next {
        background: rgba(255, 255, 255, 0.16);
        color: #fff;
    }
    .pw-light .pw-end-next,
    .pw-sepia .pw-end-next {
        background: rgba(0, 0, 0, 0.78);
        color: #fff;
        border-color: transparent;
    }

    .pw-end-card {
        position: absolute;
        left: 50%;
        top: 50%;
        transform: translate(-50%, -50%);
        background: rgba(20, 22, 28, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        padding: 26px 24px;
        display: flex;
        flex-direction: column;
        gap: 18px;
        align-items: center;
        z-index: 30;
    }
    .pw-light .pw-end-card {
        background: rgba(255, 255, 255, 0.96);
        border-color: rgba(0, 0, 0, 0.08);
        color: #222;
    }
    .pw-sepia .pw-end-card {
        background: rgba(244, 236, 216, 0.97);
        border-color: rgba(74, 59, 42, 0.18);
        color: #4a3b2a;
    }
    .pw-end-title {
        font-size: 14px;
        color: rgba(232, 232, 234, 0.85);
        letter-spacing: 0.02em;
    }
    .pw-light .pw-end-title,
    .pw-sepia .pw-end-title {
        color: rgba(50, 42, 34, 0.85);
    }
    .pw-end-row {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        justify-content: center;
    }

    .pw-reader-top {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        display: flex;
        align-items: center;
        gap: 10px;
        padding: calc(10px + env(safe-area-inset-top)) 12px 10px;
        background: linear-gradient(180deg, rgba(0, 0, 0, 0.85), transparent);
        z-index: 20;
    }
    .pw-light .pw-reader-top {
        background: linear-gradient(180deg, rgba(244, 244, 246, 0.95), transparent);
    }
    .pw-sepia .pw-reader-top {
        background: linear-gradient(180deg, rgba(244, 236, 216, 0.95), transparent);
    }
    .pw-reader-title {
        flex: 1;
        font-size: 13px;
        color: rgba(232, 232, 234, 0.85);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-light .pw-reader-title,
    .pw-sepia .pw-reader-title {
        color: rgba(40, 38, 33, 0.85);
    }
    .pw-reader-count {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.5);
        font-variant-numeric: tabular-nums;
    }
    .pw-light .pw-reader-count,
    .pw-sepia .pw-reader-count {
        color: rgba(60, 50, 40, 0.55);
    }
    .pw-reader-btn {
        width: 40px;
        height: 40px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: rgba(232, 232, 234, 0.85);
        display: grid;
        place-items: center;
        cursor: pointer;
        flex-shrink: 0;
    }
    .pw-light .pw-reader-btn,
    .pw-sepia .pw-reader-btn {
        background: rgba(0, 0, 0, 0.05);
        border-color: rgba(0, 0, 0, 0.1);
        color: rgba(40, 38, 33, 0.85);
    }
    .pw-reader-chip {
        font-size: 13px;
        font-weight: 600;
        letter-spacing: 0.02em;
    }

    .pw-reader-bottom {
        position: fixed;
        left: 0;
        right: 0;
        bottom: 0;
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 12px 12px calc(14px + env(safe-area-inset-bottom));
        background: linear-gradient(0deg, rgba(0, 0, 0, 0.85), transparent);
        z-index: 20;
    }
    .pw-light .pw-reader-bottom {
        background: linear-gradient(0deg, rgba(244, 244, 246, 0.95), transparent);
    }
    .pw-sepia .pw-reader-bottom {
        background: linear-gradient(0deg, rgba(244, 236, 216, 0.95), transparent);
    }
    .pw-reader-ch {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: rgba(232, 232, 234, 0.85);
        border-radius: 999px;
        padding: 10px 18px;
        font-size: 14px;
        cursor: pointer;
    }
    .pw-light .pw-reader-ch,
    .pw-sepia .pw-reader-ch {
        background: rgba(0, 0, 0, 0.05);
        border-color: rgba(0, 0, 0, 0.1);
        color: rgba(40, 38, 33, 0.85);
    }

    .pw-reader-bar {
        position: fixed;
        left: 0;
        right: 0;
        bottom: 0;
        height: 2px;
        background: rgba(255, 255, 255, 0.08);
        z-index: 25;
    }
    .pw-reader-bar > div {
        height: 100%;
        background: var(--pw-accent);
        transition: width 0.15s ease;
    }

    .pw-rs-scrim {
        position: fixed;
        inset: 0;
        z-index: 40;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        flex-direction: column;
        justify-content: flex-end;
    }
    .pw-rs-pop {
        display: flex;
        flex-direction: column;
        gap: 13px;
        background: rgba(12, 13, 17, 0.98);
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px 16px 0 0;
        padding: 8px 14px calc(18px + env(safe-area-inset-bottom));
        animation: pw-rs-up 0.22s cubic-bezier(0.2, 0.7, 0.2, 1);
        cursor: default;
    }
    @keyframes pw-rs-up {
        from {
            transform: translateY(40px);
            opacity: 0;
        }
    }
    .pw-rs-grab {
        width: 36px;
        height: 4px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.18);
        margin: 4px auto 2px;
        flex-shrink: 0;
    }
    @media (min-width: 640px) {
        .pw-rs-scrim {
            background: transparent;
            justify-content: flex-start;
            align-items: flex-end;
            padding: calc(58px + env(safe-area-inset-top)) 12px 0;
        }
        .pw-rs-pop {
            width: 340px;
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 16px;
            padding: 14px;
            animation: pw-rs-in 0.18s cubic-bezier(0.2, 0.7, 0.2, 1);
        }
        .pw-rs-grab {
            display: none;
        }
    }
    @keyframes pw-rs-in {
        from {
            transform: translateY(-8px);
            opacity: 0;
        }
    }
    .pw-rs-row {
        display: flex;
        flex-direction: column;
        gap: 7px;
    }
    .pw-rs-label {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        letter-spacing: 0.04em;
        padding-left: 2px;
    }
    .pw-rs-seg {
        display: flex;
        align-items: stretch;
        gap: 2px;
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 11px;
        padding: 3px;
    }
    .pw-rs-seg button {
        flex: 1;
        background: transparent;
        border: 0;
        color: rgba(232, 232, 234, 0.7);
        border-radius: 8px;
        padding: 8px 6px;
        font-size: 13px;
        cursor: pointer;
        white-space: nowrap;
        transition:
            background 0.12s ease,
            color 0.12s ease;
    }
    .pw-rs-seg button.on {
        background: rgba(255, 255, 255, 0.14);
        color: #fff;
    }
    .pw-rs-val {
        min-width: 64px;
        display: grid;
        place-items: center;
        font-size: 13px;
        color: #ececef;
        font-variant-numeric: tabular-nums;
    }

    .pw-tap-hint {
        position: fixed;
        inset: 0;
        display: flex;
        pointer-events: none;
        z-index: 50;
        animation: pw-hint-fade 1.7s ease forwards;
    }
    @keyframes pw-hint-fade {
        0% {
            opacity: 1;
        }
        70% {
            opacity: 1;
        }
        100% {
            opacity: 0;
        }
    }
    .pw-tap-zone {
        flex: 1;
        display: grid;
        place-items: center;
        color: rgba(255, 255, 255, 0.65);
        font-size: 12px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
    }
    .pw-tap-prev {
        background: rgba(80, 140, 200, 0.12);
        flex: 0 0 30%;
    }
    .pw-tap-next {
        background: rgba(200, 120, 140, 0.12);
        flex: 0 0 30%;
    }
    .pw-tap-mid {
        background: rgba(140, 140, 160, 0.1);
    }
</style>
