<script lang="ts">
    import { onMount, onDestroy, tick } from 'svelte';
    import { goto, replaceState } from '$app/navigation';
    import { page } from '$app/state';
    import { api, type BookDetail, type BookMark } from '$lib/api';
    import { t, plural } from '$lib/i18n.svelte';

    const olKey = $derived(page.params.ol_key ?? '');
    const PREFS_KEY = 'pw-book-prefs';

    let host: HTMLDivElement | undefined = $state();
    let detail = $state<BookDetail | null>(null);
    let loading = $state(true);
    let loadPct = $state(0);
    let err = $state('');
    let ui = $state(true);
    let rsOpen = $state(false);
    let tocOpen = $state(false);
    let isFs = $state(false);
    let fsOk = $state(false);
    let mode = $state<'epub' | 'pdf' | 'other'>('epub');
    let chapter = $state('');
    let curHref = $state('');
    let tocList = $state<{ href: string; label: string; depth: number }[]>([]);
    let tocTab = $state<'chapters' | 'marks'>('chapters');
    let marks = $state<BookMark[]>([]);
    let pendingSel = $state<{ cfi: string; text: string } | null>(null);
    let editMark = $state<BookMark | null>(null);
    let noteDraft = $state('');
    let resume = $state<{ cfi: string; pct: number } | null>(null);
    let EpubCFI = $state<any>(null);
    let pageInCh = $state(0);
    let pagesInCh = $state(0);
    let percent = $state(0);
    let fontSize = $state(110);
    let theme = $state<'dark' | 'light' | 'sepia'>('dark');
    let family = $state<'default' | 'serif' | 'sans'>('default');
    let lineHeight = $state(1.55);
    let margin = $state<'narrow' | 'normal' | 'wide'>('normal');
    let flow = $state<'paginated' | 'scrolled'>('paginated');
    let progMode = $state<'pages' | 'time'>('pages');
    let spp = $state(45);

    let book: any = null;
    let rendition: any = null;
    let lastCfi = $state<string | null>(null);
    let lastTurn = 0;
    let hadFirstRelocate = false;
    let toc: Record<string, string> = {};
    let saveTimer: ReturnType<typeof setTimeout> | undefined;
    let selContents: any = null;
    let hlTapped = false;
    let lastSync = 0;
    let resumeTimer: ReturnType<typeof setTimeout> | undefined;

    const PALETTE = {
        dark: { bg: '#000', fg: '#d6d6d8' },
        light: { bg: '#fafafa', fg: '#202024' },
        sepia: { bg: '#f4ecd8', fg: '#4a3b2a' }
    };
    const FAMILIES: Record<string, string> = {
        serif: "Georgia, 'Times New Roman', serif",
        sans: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
    };
    const MARGINS = {
        narrow: { width: 1080, pad: 10 },
        normal: { width: 900, pad: 18 },
        wide: { width: 700, pad: 34 }
    };
    const COLORS: Record<string, string> = {
        yellow: '#ffd600',
        green: '#3ddc84',
        blue: '#58a6ff',
        pink: '#ff6fae'
    };

    const FONT_OPTS = [
        { v: 'default', k: 'books.reader.f_book' },
        { v: 'serif', k: 'books.reader.f_serif' },
        { v: 'sans', k: 'books.reader.f_sans' }
    ];
    const THEME_OPTS = [
        { v: 'dark', k: 'books.reader.t_dark' },
        { v: 'light', k: 'books.reader.t_light' },
        { v: 'sepia', k: 'books.reader.t_sepia' }
    ];
    const LH_OPTS = [
        { v: '1.4', k: 'books.reader.lh_tight' },
        { v: '1.55', k: 'books.reader.lh_normal' },
        { v: '1.8', k: 'books.reader.lh_loose' }
    ];
    const MARGIN_OPTS = [
        { v: 'narrow', k: 'books.reader.m_narrow' },
        { v: 'normal', k: 'books.reader.m_normal' },
        { v: 'wide', k: 'books.reader.m_wide' }
    ];
    const FLOW_OPTS = [
        { v: 'paginated', k: 'books.reader.flow_pages' },
        { v: 'scrolled', k: 'books.reader.flow_scroll' }
    ];
    const PROG_OPTS = [
        { v: 'pages', k: 'books.reader.p_pages' },
        { v: 'time', k: 'books.reader.p_time' }
    ];

    function readPrefs() {
        try {
            const p = JSON.parse(localStorage.getItem(PREFS_KEY) ?? '{}');
            if (typeof p.fontSize === 'number') fontSize = p.fontSize;
            if (p.theme in PALETTE) theme = p.theme;
            if (p.flow === 'scrolled' || p.flow === 'paginated') flow = p.flow;
            if (p.family === 'default' || p.family === 'serif' || p.family === 'sans') family = p.family;
            if (typeof p.lineHeight === 'number') lineHeight = p.lineHeight;
            if (p.margin in MARGINS) margin = p.margin;
            if (p.prog === 'pages' || p.prog === 'time') progMode = p.prog;
            if (typeof p.spp === 'number' && p.spp > 0) spp = p.spp;
        } catch {}
    }

    function savePrefs() {
        try {
            localStorage.setItem(
                PREFS_KEY,
                JSON.stringify({
                    fontSize,
                    theme,
                    flow,
                    family,
                    lineHeight,
                    margin,
                    prog: progMode,
                    spp: Math.round(spp)
                })
            );
        } catch {}
    }

    async function readBody(res: Response): Promise<ArrayBuffer> {
        const total = Number(res.headers.get('content-length') ?? 0);
        if (!res.body || !total) return res.arrayBuffer();
        const reader = res.body.getReader();
        const chunks: Uint8Array[] = [];
        let got = 0;
        for (;;) {
            const { done, value } = await reader.read();
            if (done) break;
            chunks.push(value);
            got += value.length;
            loadPct = Math.min(99, Math.round((got / total) * 100));
        }
        const out = new Uint8Array(got);
        let off = 0;
        for (const c of chunks) {
            out.set(c, off);
            off += c.length;
        }
        return out.buffer;
    }

    function themeRules(name: keyof typeof PALETTE) {
        const rules: Record<string, Record<string, string>> = {
            body: { background: PALETTE[name].bg, color: PALETTE[name].fg },
            a: { color: 'inherit' },
            'p, li, blockquote': { 'line-height': `${lineHeight} !important` }
        };
        if (family !== 'default') {
            rules['body, p, div, span, li, blockquote, h1, h2, h3, h4, h5, h6'] = {
                'font-family': `${FAMILIES[family]} !important`
            };
        }
        return rules;
    }

    function applyThemes() {
        if (!rendition) return;
        for (const name of Object.keys(PALETTE)) {
            rendition.themes.register(name, themeRules(name as keyof typeof PALETTE));
        }
        rendition.themes.select(theme);
    }

    function tocLabel(href: string): string {
        const clean = href.split('#')[0];
        if (toc[clean]) return toc[clean];
        const hit = Object.keys(toc).find((k) => k.endsWith(clean) || clean.endsWith(k));
        return hit ? toc[hit] : '';
    }

    function paintMarks() {
        if (!rendition) return;
        for (const m of marks) {
            if (m.kind !== 'highlight') continue;
            rendition.annotations.highlight(m.cfi, { id: m.id }, () => onHlTap(m.id), 'pw-hl', {
                fill: COLORS[m.color ?? 'yellow']
            });
        }
    }

    function onHlTap(id: string) {
        hlTapped = true;
        const m = marks.find((x) => x.id === id);
        if (!m) return;
        editMark = m;
        noteDraft = m.note ?? '';
    }

    function clearSel() {
        try {
            selContents?.window?.getSelection?.()?.removeAllRanges();
        } catch {}
        pendingSel = null;
    }

    async function addHighlight(color: string) {
        if (!pendingSel) return;
        const sel = pendingSel;
        clearSel();
        const dup = marks.find((m) => m.kind === 'highlight' && m.cfi === sel.cfi);
        try {
            if (dup) {
                await recolor(dup, color);
                return;
            }
            const mark = await api.bookMarkCreate(olKey, {
                kind: 'highlight',
                cfi: sel.cfi,
                color,
                snippet: sel.text,
                chapter: chapter || undefined
            });
            marks = [...marks, mark];
            rendition?.annotations.highlight(mark.cfi, { id: mark.id }, () => onHlTap(mark.id), 'pw-hl', {
                fill: COLORS[color]
            });
        } catch (e) {
            console.error('[reader] highlight failed', e);
        }
    }

    async function recolor(m: BookMark, color: string) {
        await api.bookMarkUpdate(m.id, { color });
        m.color = color;
        marks = [...marks];
        try {
            rendition?.annotations.remove(m.cfi, 'highlight');
        } catch {}
        rendition?.annotations.highlight(m.cfi, { id: m.id }, () => onHlTap(m.id), 'pw-hl', {
            fill: COLORS[color]
        });
    }

    async function saveNote() {
        if (!editMark) return;
        try {
            await api.bookMarkUpdate(editMark.id, { note: noteDraft });
            editMark.note = noteDraft;
            marks = [...marks];
            editMark = null;
        } catch (e) {
            console.error('[reader] note save failed', e);
        }
    }

    async function deleteMark(m: BookMark) {
        try {
            await api.bookMarkDelete(m.id);
            marks = marks.filter((x) => x.id !== m.id);
            if (m.kind === 'highlight') {
                try {
                    rendition?.annotations.remove(m.cfi, 'highlight');
                } catch {}
            }
            if (editMark?.id === m.id) editMark = null;
        } catch (e) {
            console.error('[reader] mark delete failed', e);
        }
    }

    async function toggleBookmark() {
        if (!lastCfi) return;
        const existing = marks.find((m) => m.kind === 'bookmark' && m.cfi === lastCfi);
        if (existing) {
            deleteMark(existing);
            return;
        }
        try {
            const mark = await api.bookMarkCreate(olKey, {
                kind: 'bookmark',
                cfi: lastCfi,
                snippet: `${chapter ? chapter + ' · ' : ''}${Math.round(percent * 100)}%`,
                chapter: chapter || undefined
            });
            marks = [...marks, mark];
        } catch (e) {
            console.error('[reader] bookmark failed', e);
        }
    }

    async function loadLocations() {
        const key = `pw-book-locs-${olKey}`;
        try {
            const cached = localStorage.getItem(key);
            if (cached) {
                book.locations.load(cached);
            } else {
                await book.locations.generate(1000);
                try {
                    localStorage.setItem(key, book.locations.save());
                } catch {}
            }
            if (lastCfi) percent = book.locations.percentageFromCfi(lastCfi) || percent;
        } catch (e) {
            console.error('[reader] locations failed', e);
        }
    }

    async function mountRendition(target: string | undefined) {
        if (!host || !book) return;
        rendition = book.renderTo(host, {
            width: '100%',
            height: '100%',
            ...(flow === 'scrolled'
                ? { flow: 'scrolled-doc' }
                : { spread: 'auto', manager: 'default', flow: 'paginated' })
        });

        applyThemes();
        rendition.themes.fontSize(`${fontSize}%`);

        rendition.on('relocated', (loc: any) => {
            const cfi = loc?.start?.cfi ?? null;
            lastCfi = cfi;
            if (book.locations?.length()) percent = loc?.start?.percentage ?? 0;
            pageInCh = loc?.start?.displayed?.page ?? 0;
            pagesInCh = loc?.start?.displayed?.total ?? 0;
            if (loc?.atEnd && hadFirstRelocate && book.locations?.length()) percent = 1;
            hadFirstRelocate = true;
            curHref = (loc?.start?.href ?? '').split('#')[0];
            chapter = tocLabel(loc?.start?.href ?? '');

            const now = Date.now();
            if (lastTurn) {
                const dt = (now - lastTurn) / 1000;
                if (dt > 2 && dt < 120) {
                    spp = spp * 0.8 + dt * 0.2;
                    savePrefs();
                }
            }
            lastTurn = now;
            resume = null;

            queueSave(cfi, percent);
        });

        rendition.on('keyup', (e: KeyboardEvent) => onKey(e));

        rendition.on('selected', (cfiRange: string, contents: any) => {
            const text = contents?.window?.getSelection?.()?.toString() ?? '';
            if (!text.trim()) return;
            pendingSel = { cfi: cfiRange, text: text.trim().slice(0, 500) };
            selContents = contents;
        });

        rendition.on('click', (e: MouseEvent, contents: any) => {
            if (mode !== 'epub') return;
            const sel = contents?.window?.getSelection?.();
            if (sel && !sel.isCollapsed) return;
            if (pendingSel) {
                clearSel();
                return;
            }
            const left = (e.view as any)?.frameElement?.getBoundingClientRect?.()?.left ?? 0;
            const x = left + e.clientX;
            setTimeout(() => {
                if (hlTapped) {
                    hlTapped = false;
                    return;
                }
                if (rsOpen || tocOpen || editMark) return;
                if (flow === 'scrolled') {
                    ui = !ui;
                    return;
                }
                const w = window.innerWidth;
                if (x < w / 3) prev();
                else if (x > (w * 2) / 3) next();
                else ui = !ui;
            }, 0);
        });

        await rendition.display(target);
        paintMarks();
    }

    async function remount() {
        if (!rendition) return;
        try {
            rendition.destroy();
        } catch {}
        rendition = null;
        await mountRendition(lastCfi ?? undefined);
    }

    onMount(async () => {
        readPrefs();

        api.me().catch(() => goto('/login'));
        const restart = page.url.searchParams.get('restart') === '1';
        const jumpCfi = page.url.searchParams.get('cfi');
        const detailP = api.bookDetail(olKey);
        const marksP = api.bookMarks(olKey).catch(() => [] as BookMark[]);
        const epubP = import('epubjs');
        const fileCtl = new AbortController();
        const fileP = fetch(api.bookFileUrl(olKey), { signal: fileCtl.signal, cache: 'default' }).catch(() => null);

        try {
            detail = await detailP;
        } catch (e) {
            console.error('[reader] detail failed', e);
            fileCtl.abort();
            err = 'could not load book';
            loading = false;
            return;
        }

        if (!detail.in_library || detail.book.status !== 'ready') {
            fileCtl.abort();
            err = 'no file yet, fetch one first';
            loading = false;
            return;
        }

        percent = detail.progress?.percent ?? 0;

        const ext = detail.book.ext ?? '';
        mode = ext === 'epub' ? 'epub' : ext === 'pdf' ? 'pdf' : 'other';
        if (mode !== 'epub') {
            fileCtl.abort();
            loading = false;
            return;
        }

        let bail: ReturnType<typeof setTimeout> | undefined;
        try {
            const deadline = new Promise<never>((_, reject) => {
                bail = setTimeout(() => reject(new Error(t('books.reader.timed_out'))), 15000);
            });

            const res = await Promise.race([fileP, deadline]);
            if (!res || !res.ok) {
                err = res ? `file fetch failed (${res.status})` : 'file fetch failed';
                return;
            }
            const buf = await Promise.race([readBody(res), deadline]);
            loadPct = 100;

            const mod = await epubP;
            const ePub = (mod as any).default ?? mod;
            EpubCFI = (mod as any).EpubCFI ?? null;
            book = ePub(buf);

            marks = await marksP;

            await Promise.race([book.ready, deadline]);

            book.loaded.navigation
                .then((nav: any) => {
                    const list: { href: string; label: string; depth: number }[] = [];
                    const walk = (items: any[], depth: number) => {
                        for (const it of items) {
                            const href = (it.href ?? '').split('#')[0];
                            const label = (it.label ?? '').trim();
                            if (href && label) list.push({ href: it.href, label, depth });
                            if (href && !toc[href]) toc[href] = label;
                            if (it.subitems?.length) walk(it.subitems, depth + 1);
                        }
                    };
                    walk(nav?.toc ?? [], 0);
                    tocList = list;
                    chapter = tocLabel(rendition?.location?.start?.href ?? '');
                })
                .catch(() => {});

            if (!host) {
                err = 'mount failed';
                return;
            }
            loading = false;
            await tick();
            const initialCfi = restart ? undefined : (jumpCfi ?? detail.progress?.cfi ?? undefined);
            await Promise.race([mountRendition(initialCfi), deadline]);
            if (restart || jumpCfi) replaceState(`/read-book/${olKey}`, {});
            loadLocations();
        } catch (e) {
            console.error('[reader] epub failed', e);
            fileCtl.abort();
            err = (e as Error)?.message || 'reader failed to start';
        } finally {
            clearTimeout(bail);
            loading = false;
        }
    });

    function onKey(e: KeyboardEvent) {
        if (e.key === 'Escape') {
            if (rsOpen || tocOpen || editMark || pendingSel) {
                rsOpen = false;
                tocOpen = false;
                editMark = null;
                clearSel();
                return;
            }
            if (document.fullscreenElement) return;
            exit();
            return;
        }
        if (mode !== 'epub') return;
        if (e.key === 'ArrowRight' || e.key === ' ') {
            e.preventDefault();
            next();
        } else if (e.key === 'ArrowLeft') prev();
    }

    onMount(() => {
        const handler = (e: KeyboardEvent) => onKey(e);
        window.addEventListener('keydown', handler);
        fsOk = !!document.documentElement.requestFullscreen;
        const onFs = () => (isFs = !!document.fullscreenElement);
        document.addEventListener('fullscreenchange', onFs);
        const onVis = () => {
            if (document.visibilityState !== 'visible') return;
            const now = Date.now();
            if (now - lastSync < 15000) return;
            lastSync = now;
            checkRemote();
        };
        document.addEventListener('visibilitychange', onVis);
        return () => {
            window.removeEventListener('keydown', handler);
            document.removeEventListener('fullscreenchange', onFs);
            document.removeEventListener('visibilitychange', onVis);
        };
    });

    async function checkRemote() {
        if (mode !== 'epub' || loading || err || !rendition || !book?.locations?.length()) return;
        try {
            const d = await api.bookDetail(olKey);
            const srv = d.progress;
            if (!srv?.cfi || srv.cfi === lastCfi) return;
            if (srv.percent - percent <= 0.01) return;
            resume = { cfi: srv.cfi, pct: Math.round(srv.percent * 100) };
            clearTimeout(resumeTimer);
            resumeTimer = setTimeout(() => (resume = null), 10000);
        } catch {}
    }

    function acceptResume() {
        if (!resume) return;
        rendition?.display(resume.cfi);
        resume = null;
    }

    function toggleFs() {
        if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
        else document.documentElement.requestFullscreen().catch(() => {});
    }

    $effect(() => {
        if (!ui) {
            rsOpen = false;
            tocOpen = false;
            editMark = null;
        }
    });

    $effect(() => {
        if (!tocOpen) return;
        tick().then(() => {
            document.querySelector('.pw-toc-row.on')?.scrollIntoView({ block: 'center' });
        });
    });

    function openChapter(href: string) {
        tocOpen = false;
        rendition?.display(href);
    }

    function queueSave(cfi: string | null, pct: number) {
        clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            api.bookProgress({ ol_key: olKey, cfi, percent: pct }).catch(() => {});
        }, 700);
    }

    function next() {
        rendition?.next();
    }
    function prev() {
        rendition?.prev();
    }

    function bumpFont(delta: number) {
        fontSize = Math.min(180, Math.max(70, fontSize + delta));
        rendition?.themes.fontSize(`${fontSize}%`);
        savePrefs();
    }

    function setTheme(v: string) {
        theme = v as typeof theme;
        rendition?.themes.select(theme);
        savePrefs();
    }

    function setFamily(v: string) {
        family = v as typeof family;
        applyThemes();
        savePrefs();
    }

    function setLineHeight(v: string) {
        lineHeight = Number(v);
        applyThemes();
        savePrefs();
    }

    async function setMargin(v: string) {
        margin = v as typeof margin;
        savePrefs();
        await tick();
        await remount();
    }

    async function setFlow(v: string) {
        if (v === flow) return;
        flow = v as typeof flow;
        savePrefs();
        await remount();
    }

    function setProg(v: string) {
        progMode = v as typeof progMode;
        savePrefs();
    }

    function exit() {
        goto(`/book/${olKey}`);
    }

    function download() {
        window.location.href = api.bookDownloadUrl(olKey);
    }

    function zone(e: MouseEvent) {
        if (mode !== 'epub') return;
        if (pendingSel) {
            clearSel();
            return;
        }
        if (flow === 'scrolled') {
            ui = !ui;
            return;
        }
        const w = window.innerWidth;
        const x = e.clientX;
        if (x < w / 3) prev();
        else if (x > (w * 2) / 3) next();
        else ui = !ui;
    }

    onDestroy(() => {
        if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
        try {
            rendition?.destroy();
        } catch {}
        try {
            book?.destroy();
        } catch {}
    });

    const title = $derived(detail?.book.title ?? 'reading');
    const showBars = $derived((ui || mode === 'pdf') && !loading && !err);
    const pagesLeft = $derived(Math.max(0, pagesInCh - pageInCh));
    const minsLeft = $derived(Math.max(1, Math.round((pagesLeft * spp) / 60)));
    const bmHere = $derived(marks.some((m) => m.kind === 'bookmark' && m.cfi === lastCfi));
    const sortedMarks = $derived.by(() => {
        if (!EpubCFI) return marks;
        const cmp = new EpubCFI();
        return [...marks].sort((a, b) => {
            try {
                return cmp.compare(a.cfi, b.cfi);
            } catch {
                return 0;
            }
        });
    });
</script>

{#snippet seg(opts: { v: string; k: string }[], cur: string, pick: (v: string) => void)}
    <div class="pw-rs-seg">
        {#each opts as o (o.v)}
            <button class:on={cur === o.v} onclick={() => pick(o.v)}>{t(o.k)}</button>
        {/each}
    </div>
{/snippet}

<svelte:head><title>{title} - pleasewatch</title></svelte:head>

<div
    class="pw-reader"
    class:lightbg={mode === 'epub' && theme !== 'dark'}
    data-rtheme={mode === 'epub' ? theme : 'dark'}
    style:background={mode === 'epub' ? PALETTE[theme].bg : '#000'}
    style:--rbg={mode === 'epub' ? PALETTE[theme].bg : '#000'}
    onclick={zone}
    role="presentation"
>
    {#if loading}
        <div class="pw-reader-msg">{loadPct ? t('books.reader.loading_pct', { pct: loadPct }) : '// loading...'}</div>
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
    {:else if mode === 'pdf'}
        <iframe class="pw-pdf-frame" src={api.bookFileUrl(olKey)} {title}></iframe>
    {:else if mode === 'other'}
        <div class="pw-reader-msg">
            {t('books.reader.unsupported_format')} ({detail?.book.ext})
            <button
                class="pw-reader-msg-btn"
                onclick={(e) => {
                    e.stopPropagation();
                    download();
                }}
            >
                {t('books.action.download')}
            </button>
            <button
                class="pw-reader-msg-btn"
                onclick={(e) => {
                    e.stopPropagation();
                    exit();
                }}>back</button
            >
        </div>
    {/if}

    <div
        bind:this={host}
        class="pw-book-host"
        class:hidden={mode !== 'epub' || loading || !!err}
        style:max-width="{MARGINS[margin].width}px"
        style:padding-left="{MARGINS[margin].pad}px"
        style:padding-right="{MARGINS[margin].pad}px"
    ></div>

    {#if showBars && mode !== 'other'}
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
            <span class="pw-reader-title">{title}</span>
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
            {#if mode === 'epub'}
                <button class="pw-reader-btn" onclick={toggleBookmark} aria-label={t('books.marks.bookmark')}>
                    <svg
                        width="16"
                        height="16"
                        viewBox="0 0 24 24"
                        fill={bmHere ? 'currentColor' : 'none'}
                        stroke="currentColor"
                        stroke-width="2.2"
                        stroke-linecap="round"
                        stroke-linejoin="round"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" /></svg
                    >
                </button>
                <button
                    class="pw-reader-btn pw-reader-chip"
                    onclick={() => (rsOpen = !rsOpen)}
                    aria-label={t('books.reader.settings')}>Aa</button
                >
            {/if}
        </div>

        {#if mode === 'epub' && !rsOpen}
            <div class="pw-reader-bottom" role="presentation" onclick={(e) => e.stopPropagation()}>
                <button class="pw-reader-ch" onclick={prev}>
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
                    prev
                </button>
                <button class="pw-reader-mid" disabled={!tocList.length} onclick={() => (tocOpen = true)}>
                    {#if chapter}
                        <span class="pw-reader-chap">{chapter}</span>
                    {/if}
                    <span class="pw-reader-stats">
                        {#if flow === 'paginated' && pagesInCh > 1 && pagesLeft > 0}
                            <span
                                >{progMode === 'time'
                                    ? t('books.reader.time_left', { n: minsLeft })
                                    : plural('books.reader.left', pagesLeft)}</span
                            >
                            <span>·</span>
                        {/if}
                        <span>{Math.round(percent * 100)}%</span>
                    </span>
                </button>
                <button class="pw-reader-ch" onclick={next}>
                    next
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
            </div>
        {/if}
    {/if}

    {#if pendingSel && mode === 'epub' && !loading && !err}
        <div class="pw-sel-bar" role="presentation" onclick={(e) => e.stopPropagation()}>
            {#each Object.keys(COLORS) as c (c)}
                <button class="pw-sel-dot" style:background={COLORS[c]} aria-label={c} onclick={() => addHighlight(c)}
                ></button>
            {/each}
            <button class="pw-sel-x" onclick={clearSel} aria-label="cancel">
                <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.2"
                    stroke-linecap="round"
                    stroke-linejoin="round"><path d="M18 6L6 18M6 6l12 12" /></svg
                >
            </button>
        </div>
    {/if}

    {#if resume && mode === 'epub' && !loading && !err}
        <button
            class="pw-resume-chip"
            onclick={(e) => {
                e.stopPropagation();
                acceptResume();
            }}
        >
            {t('books.reader.resume_chip', { pct: resume.pct })}
        </button>
    {/if}

    {#if tocOpen && mode === 'epub' && !loading && !err}
        <div
            class="pw-toc-scrim"
            role="presentation"
            onclick={(e) => {
                e.stopPropagation();
                tocOpen = false;
            }}
        >
            <div class="pw-toc-pop" role="presentation" onclick={(e) => e.stopPropagation()}>
                <div class="pw-rs-grab"></div>
                <div class="pw-toc-tabs">
                    <div class="pw-rs-seg">
                        <button class:on={tocTab === 'chapters'} onclick={() => (tocTab = 'chapters')}
                            >{t('books.marks.tab_chapters')}</button
                        >
                        <button class:on={tocTab === 'marks'} onclick={() => (tocTab = 'marks')}
                            >{t('books.marks.tab_marks')}</button
                        >
                    </div>
                </div>
                {#if tocTab === 'chapters'}
                    <div class="pw-toc-list">
                        {#each tocList as item, i (i)}
                            <button
                                class="pw-toc-row"
                                class:on={item.href.split('#')[0] === curHref}
                                style:padding-left="{12 + item.depth * 16}px"
                                onclick={() => openChapter(item.href)}>{item.label}</button
                            >
                        {/each}
                    </div>
                {:else}
                    <div class="pw-toc-list">
                        {#if sortedMarks.length === 0}
                            <div class="pw-marks-empty">{t('books.marks.empty')}</div>
                        {/if}
                        {#each sortedMarks as m (m.id)}
                            <div class="pw-mark-row">
                                <button
                                    class="pw-mark-main"
                                    onclick={() => {
                                        tocOpen = false;
                                        rendition?.display(m.cfi);
                                    }}
                                >
                                    {#if m.kind === 'bookmark'}
                                        <svg
                                            class="pw-mark-bm"
                                            width="14"
                                            height="14"
                                            viewBox="0 0 24 24"
                                            fill="currentColor"
                                            ><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" /></svg
                                        >
                                    {:else}
                                        <span class="pw-mark-dot" style:background={COLORS[m.color ?? 'yellow']}></span>
                                    {/if}
                                    <span class="pw-mark-text">
                                        <span class="pw-mark-snip">{m.snippet || m.chapter || m.kind}</span>
                                        {#if m.note}<span class="pw-mark-note">{m.note}</span>{/if}
                                    </span>
                                </button>
                                <button
                                    class="pw-mark-x"
                                    onclick={() => deleteMark(m)}
                                    aria-label={t('books.marks.delete')}
                                >
                                    <svg
                                        width="13"
                                        height="13"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="2.2"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"><path d="M18 6L6 18M6 6l12 12" /></svg
                                    >
                                </button>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        </div>
    {/if}

    {#if editMark}
        <div
            class="pw-toc-scrim"
            role="presentation"
            onclick={(e) => {
                e.stopPropagation();
                editMark = null;
            }}
        >
            <div class="pw-toc-pop pw-mark-sheet" role="presentation" onclick={(e) => e.stopPropagation()}>
                <div class="pw-rs-grab"></div>
                {#if editMark.snippet}
                    <p class="pw-mark-quote">{editMark.snippet}</p>
                {/if}
                <div class="pw-sel-row">
                    {#each Object.keys(COLORS) as c (c)}
                        <button
                            class="pw-sel-dot"
                            class:on={editMark.color === c}
                            style:background={COLORS[c]}
                            aria-label={c}
                            onclick={() => editMark && recolor(editMark, c)}
                        ></button>
                    {/each}
                </div>
                <textarea class="pw-mark-ta" rows="3" placeholder={t('books.marks.note_ph')} bind:value={noteDraft}
                ></textarea>
                <div class="pw-mark-actions">
                    <button class="pw-reader-msg-btn pw-mark-del" onclick={() => editMark && deleteMark(editMark)}
                        >{t('books.marks.delete')}</button
                    >
                    <button class="pw-reader-msg-btn pw-mark-save" onclick={saveNote}>{t('books.marks.save')}</button>
                </div>
            </div>
        </div>
    {/if}

    {#if rsOpen && mode === 'epub' && !loading && !err}
        <div
            class="pw-rs-scrim"
            role="presentation"
            onclick={(e) => {
                e.stopPropagation();
                rsOpen = false;
            }}
        >
            <div class="pw-rs-pop" role="presentation" onclick={(e) => e.stopPropagation()}>
                <div class="pw-rs-grab"></div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.size')}</span>
                    <div class="pw-rs-seg">
                        <button onclick={() => bumpFont(-10)} aria-label={t('books.reader.font_small')}>A-</button>
                        <span class="pw-rs-val">{fontSize}%</span>
                        <button onclick={() => bumpFont(10)} aria-label={t('books.reader.font_large')}>A+</button>
                    </div>
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.font')}</span>
                    {@render seg(FONT_OPTS, family, setFamily)}
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.theme')}</span>
                    {@render seg(THEME_OPTS, theme, setTheme)}
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.lineheight')}</span>
                    {@render seg(LH_OPTS, String(lineHeight), setLineHeight)}
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.margins')}</span>
                    {@render seg(MARGIN_OPTS, margin, setMargin)}
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.flow')}</span>
                    {@render seg(FLOW_OPTS, flow, setFlow)}
                </div>
                <div class="pw-rs-row">
                    <span class="pw-rs-label">{t('books.reader.progress')}</span>
                    {@render seg(PROG_OPTS, progMode, setProg)}
                </div>
            </div>
        </div>
    {/if}

    {#if mode === 'epub' && !loading && !err}
        <div class="pw-reader-bar">
            <div style="width: {Math.round(percent * 100)}%;"></div>
        </div>
    {/if}
</div>

<style>
    .pw-reader {
        position: fixed;
        inset: 0;
        z-index: 200;
        background: #000;
        user-select: none;
        overflow: hidden;
    }
    .pw-book-host {
        position: absolute;
        inset: 0;
        margin: 0 auto;
        max-width: 900px;
        padding: calc(62px + env(safe-area-inset-top)) 16px calc(72px + env(safe-area-inset-bottom));
    }
    .hidden {
        visibility: hidden;
    }
    .pw-pdf-frame {
        position: absolute;
        top: calc(54px + env(safe-area-inset-top));
        left: 0;
        right: 0;
        bottom: 0;
        width: 100%;
        height: calc(100% - 54px - env(safe-area-inset-top));
        border: 0;
        background: #333;
    }
    .pw-reader-msg {
        position: absolute;
        inset: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 14px;
        color: rgba(220, 220, 225, 0.5);
        font-size: 14px;
        padding: 0 24px;
        text-align: center;
    }
    .lightbg .pw-reader-msg {
        color: rgba(70, 62, 48, 0.65);
    }
    .pw-reader-msg-btn {
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #ececef;
        border-radius: 999px;
        padding: 9px 20px;
        font-size: 14px;
        cursor: pointer;
    }
    .lightbg .pw-reader-msg-btn {
        background: rgba(0, 0, 0, 0.06);
        border-color: rgba(0, 0, 0, 0.14);
        color: #2c2a26;
    }
    .pw-reader-top {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        display: flex;
        align-items: center;
        gap: 10px;
        padding: calc(10px + env(safe-area-inset-top)) 12px 10px;
        background: linear-gradient(180deg, var(--rbg, #000) 58%, transparent);
        z-index: 10;
    }
    .pw-reader-title {
        flex: 1;
        font-size: 13px;
        color: rgba(232, 232, 234, 0.85);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .lightbg .pw-reader-title {
        color: rgba(40, 38, 33, 0.85);
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
    .lightbg .pw-reader-btn {
        background: rgba(0, 0, 0, 0.05);
        border-color: rgba(0, 0, 0, 0.1);
        color: rgba(45, 42, 36, 0.8);
    }
    .pw-reader-chip {
        width: auto;
        padding: 0 13px;
        font-size: 12px;
        letter-spacing: 0.02em;
    }
    .pw-toc-scrim {
        position: fixed;
        inset: 0;
        z-index: 40;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        flex-direction: column;
        justify-content: flex-end;
    }
    .pw-toc-pop {
        display: flex;
        flex-direction: column;
        background: rgba(12, 13, 17, 0.98);
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px 16px 0 0;
        padding: 8px 8px calc(10px + env(safe-area-inset-bottom));
        max-height: 70vh;
        animation: pw-rs-up 0.22s cubic-bezier(0.2, 0.7, 0.2, 1);
        cursor: default;
    }
    @media (min-width: 640px) {
        .pw-toc-scrim {
            justify-content: center;
            align-items: center;
            padding: 24px;
        }
        .pw-toc-pop {
            width: 100%;
            max-width: 440px;
            max-height: min(70vh, 600px);
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 16px;
            animation: pw-rs-in 0.18s cubic-bezier(0.2, 0.7, 0.2, 1);
        }
    }
    .pw-toc-list {
        overflow-y: auto;
        min-height: 60px;
        padding-top: 4px;
    }
    .pw-toc-row {
        width: 100%;
        display: block;
        text-align: left;
        background: none;
        border: 0;
        padding: 11px 12px;
        border-radius: 10px;
        font-size: 14px;
        color: rgba(220, 220, 225, 0.75);
        cursor: pointer;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-toc-row:hover {
        background: rgba(255, 255, 255, 0.04);
    }
    .pw-toc-row.on {
        color: var(--pw-accent);
        background: rgba(255, 255, 255, 0.05);
    }

    .pw-reader[data-rtheme='dark'] :global(g.pw-hl) {
        mix-blend-mode: screen;
        fill-opacity: 0.42;
    }
    .pw-reader[data-rtheme='light'] :global(g.pw-hl) {
        mix-blend-mode: multiply;
        fill-opacity: 0.3;
    }
    .pw-reader[data-rtheme='sepia'] :global(g.pw-hl) {
        mix-blend-mode: multiply;
        fill-opacity: 0.35;
    }

    .pw-sel-bar {
        position: absolute;
        left: 50%;
        transform: translateX(-50%);
        bottom: calc(84px + env(safe-area-inset-bottom));
        display: flex;
        align-items: center;
        gap: 10px;
        background: rgba(12, 13, 17, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 999px;
        padding: 9px 14px;
        z-index: 30;
        animation: pw-rs-in 0.15s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    .pw-sel-dot {
        width: 26px;
        height: 26px;
        border-radius: 999px;
        border: 2px solid transparent;
        cursor: pointer;
        flex-shrink: 0;
    }
    .pw-sel-dot.on {
        border-color: #fff;
    }
    .pw-sel-x {
        width: 26px;
        height: 26px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: rgba(232, 232, 234, 0.8);
        display: grid;
        place-items: center;
        cursor: pointer;
        flex-shrink: 0;
    }
    .pw-sel-row {
        display: flex;
        gap: 10px;
    }
    .pw-resume-chip {
        position: absolute;
        left: 50%;
        transform: translateX(-50%);
        bottom: calc(84px + env(safe-area-inset-bottom));
        background: rgba(12, 13, 17, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: #ececef;
        border-radius: 999px;
        padding: 9px 16px;
        font-size: 13px;
        cursor: pointer;
        z-index: 30;
        animation: pw-rs-in 0.15s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    .pw-toc-tabs {
        padding: 6px 8px 4px;
        flex-shrink: 0;
    }
    .pw-marks-empty {
        padding: 18px 12px;
        font-size: 13px;
        color: rgba(220, 220, 225, 0.4);
    }
    .pw-mark-row {
        display: flex;
        align-items: center;
        gap: 2px;
    }
    .pw-mark-main {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: flex-start;
        gap: 10px;
        text-align: left;
        background: none;
        border: 0;
        padding: 10px 6px 10px 12px;
        border-radius: 10px;
        cursor: pointer;
        color: rgba(220, 220, 225, 0.75);
    }
    .pw-mark-main:hover {
        background: rgba(255, 255, 255, 0.04);
    }
    .pw-mark-dot {
        width: 10px;
        height: 10px;
        border-radius: 999px;
        flex-shrink: 0;
        margin-top: 4px;
    }
    .pw-mark-bm {
        color: var(--pw-accent);
        flex-shrink: 0;
        margin-top: 2px;
    }
    .pw-mark-text {
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .pw-mark-snip {
        font-size: 13px;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .pw-mark-note {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.45);
        display: -webkit-box;
        -webkit-line-clamp: 1;
        line-clamp: 1;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .pw-mark-x {
        width: 34px;
        height: 34px;
        flex-shrink: 0;
        background: none;
        border: 0;
        color: rgba(220, 220, 225, 0.4);
        display: grid;
        place-items: center;
        cursor: pointer;
        border-radius: 999px;
    }
    .pw-mark-x:hover {
        color: #f87171;
        background: rgba(255, 255, 255, 0.04);
    }
    .pw-mark-sheet {
        padding: 8px 14px calc(18px + env(safe-area-inset-bottom));
        gap: 12px;
    }
    .pw-mark-quote {
        margin: 0;
        font-size: 13px;
        line-height: 1.45;
        color: rgba(232, 232, 234, 0.75);
        border-left: 3px solid rgba(255, 255, 255, 0.15);
        padding-left: 10px;
        max-height: 80px;
        overflow: hidden;
    }
    .pw-mark-ta {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 10px;
        color: #ececef;
        font: inherit;
        font-size: 14px;
        padding: 9px 11px;
        resize: none;
        outline: none;
        user-select: text;
    }
    .pw-mark-actions {
        display: flex;
        justify-content: space-between;
        gap: 10px;
    }
    .pw-mark-del {
        color: #f87171;
    }
    .pw-mark-save {
        background: rgba(255, 255, 255, 0.16);
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
            width: 330px;
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
    .pw-reader-bottom {
        position: absolute;
        left: 0;
        right: 0;
        bottom: 0;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        padding: 12px 12px calc(14px + env(safe-area-inset-bottom));
        background: linear-gradient(0deg, var(--rbg, #000) 58%, transparent);
        z-index: 10;
    }
    .pw-reader-mid {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 2px;
        text-align: center;
        background: none;
        border: 0;
        font: inherit;
        cursor: pointer;
        padding: 2px 6px;
    }
    .pw-reader-mid:disabled {
        cursor: default;
    }
    .pw-reader-chap {
        font-size: 11px;
        color: rgba(232, 232, 234, 0.7);
        max-width: 100%;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .lightbg .pw-reader-chap {
        color: rgba(45, 42, 36, 0.75);
    }
    .pw-reader-stats {
        display: flex;
        gap: 6px;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        font-variant-numeric: tabular-nums;
    }
    .lightbg .pw-reader-stats {
        color: rgba(45, 42, 36, 0.5);
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
        flex-shrink: 0;
    }
    .lightbg .pw-reader-ch {
        background: rgba(0, 0, 0, 0.05);
        border-color: rgba(0, 0, 0, 0.1);
        color: rgba(45, 42, 36, 0.8);
    }
    .pw-reader-bar {
        position: absolute;
        left: 0;
        right: 0;
        bottom: 0;
        height: 2px;
        background: rgba(255, 255, 255, 0.08);
        z-index: 11;
    }
    .lightbg .pw-reader-bar {
        background: rgba(0, 0, 0, 0.08);
    }
    .pw-reader-bar > div {
        height: 100%;
        background: var(--pw-accent);
        transition: width 0.15s ease;
    }
</style>
