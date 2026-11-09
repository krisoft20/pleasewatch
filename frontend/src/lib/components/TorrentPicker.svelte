<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type TorrentOption, type AdminSettings } from '$lib/api';
    import CopyCode from './CopyCode.svelte';

    type Props = {
        query: string;
        imdbId?: string;
        mediaId?: string;
        tmdbId?: number;
        mediaType?: 'movie' | 'tv';
        episodeId?: string;
        season?: number;
        episode?: number;
        kind?: 'movie' | 'tv' | 'anime' | 'book';
        absOffset?: number;
        haveEpisodes?: number[];
        maxEpisode?: number;
        olKey?: string;
        onClose: () => void;
        onStarted?: (t: TorrentOption) => void;
        onMoreStarted?: (episodes: number[]) => void;
    };

    let {
        query,
        imdbId,
        mediaId,
        tmdbId,
        mediaType,
        episodeId,
        season,
        episode,
        kind,
        absOffset,
        haveEpisodes = [],
        maxEpisode,
        olKey,
        onClose,
        onStarted,
        onMoreStarted
    }: Props = $props();

    let settings = $state<AdminSettings | null>(null);
    let savingSettings = $state(false);
    let formJackettUrl = $state('');
    let formJackettKey = $state('');

    type Shell = 'unix' | 'pwsh' | 'cmd';
    let platform = $state<Shell>(typeof navigator !== 'undefined' && /win/i.test(navigator.platform) ? 'pwsh' : 'unix');

    const jackettCmd =
        'docker run -d --name jackett -p 9117:9117 -v jackett-config:/config --restart unless-stopped lscr.io/linuxserver/jackett';

    const qbitCmd = $derived.by(() => {
        const base =
            'docker run -d --name qbittorrent -p 8080:8080 -p 6881:6881 -p 6881:6881/udp -e WEBUI_PORT=8080 -v qbit-config:/config';
        const tail = '--restart unless-stopped lscr.io/linuxserver/qbittorrent';
        if (platform === 'pwsh') return `${base} -v "\${PWD}\\media:/downloads" ${tail}`;
        if (platform === 'cmd') return `${base} -v "%cd%\\media:/downloads" ${tail}`;
        return `${base} -v "$(pwd)/media:/downloads" ${tail}`;
    });

    const qbitPwCmd = $derived.by(() => {
        if (platform === 'pwsh') return 'docker logs qbittorrent 2>&1 | Select-String password';
        if (platform === 'cmd') return 'docker logs qbittorrent 2>&1 | findstr /i password';
        return 'docker logs qbittorrent 2>&1 | grep -i password';
    });

    function defaultJackettUrl() {
        if (typeof window === 'undefined') return 'http://localhost:9117';
        const h = window.location.hostname || 'localhost';
        return `http://${h}:9117`;
    }

    async function loadSettings() {
        try {
            settings = await api.getSettings();
            if (!formJackettUrl) formJackettUrl = settings.jackett_url || defaultJackettUrl();
            if (!formQbitUrl) formQbitUrl = settings.qbit_url || defaultQbitUrl();
            if (settings.qbit_user) formQbitUser = settings.qbit_user;
        } catch {}
    }

    async function saveSettings() {
        if (!formJackettUrl.trim() || !formJackettKey.trim()) return;
        savingSettings = true;
        try {
            settings = await api.updateSettings({
                jackett_url: formJackettUrl.trim(),
                jackett_api_key: formJackettKey.trim()
            });
            envErr = false;
            formJackettKey = '';
            await run();
        } catch (caught) {
            err = caught instanceof Error ? caught.message : 'save failed';
        } finally {
            savingSettings = false;
        }
    }

    let q = $state('');
    let seededQuery = $state(false);
    let results = $state<TorrentOption[]>([]);
    let loading = $state(true);
    let err = $state('');
    let envErr = $state(false);
    let qbitErr = $state(false);
    let formQbitUrl = $state('');
    let formQbitUser = $state('admin');
    let formQbitPass = $state('');
    let savingQbit = $state(false);
    let starting = $state<string | null>(null);
    let rowErr = $state<Record<string, string>>({});
    let nextErr = $state('');
    let card = $state<HTMLDivElement>();
    type TorrentSource = 'jackett' | 'prowlarr' | 'ext';
    const allSources: TorrentSource[] = ['jackett', 'prowlarr', 'ext'];
    let aggFilter = $state<'all' | TorrentSource>('all');
    let searching = $state<TorrentSource[]>([]);
    let timer: ReturnType<typeof setTimeout> | undefined;
    let searchAbort: AbortController | undefined;
    let searchRun = 0;
    let searchInput = $state<HTMLInputElement>();
    let activeIndexers = $derived.by(() => {
        const names: string[] = [];
        if (settings?.jackett_ready) names.push('jackett');
        if (settings?.prowlarr_ready) names.push('prowlarr');
        return names;
    });
    let emptySource = $derived.by(() => {
        if (activeIndexers.length === 0) return 'indexers returned no matches';
        if (activeIndexers.length === 1) return `${activeIndexers[0]} returned no matches`;
        return `${activeIndexers.join(' + ')} returned no matches`;
    });
    let emptyTip = $derived.by(() => {
        if (kind === 'tv' || season) return 'try the show name, s01e01, or switch language back to any';
        return 'try a shorter query, another title, or switch language back to any';
    });

    $effect.pre(() => {
        if (seededQuery) return;
        q = query;
        seededQuery = true;
    });

    function defaultQbitUrl() {
        if (typeof window === 'undefined') return 'http://localhost:8080';
        const h = window.location.hostname || 'localhost';
        return `http://${h}:8080`;
    }

    async function saveQbit() {
        if (!formQbitUrl.trim()) return;
        savingQbit = true;
        try {
            settings = await api.updateSettings({
                qbit_url: formQbitUrl.trim(),
                qbit_user: formQbitUser.trim() || 'admin',
                qbit_pass: formQbitPass
            });
            qbitErr = false;
            formQbitPass = '';
            err = '';
        } catch (caught) {
            err = caught instanceof Error ? caught.message : 'save failed';
        } finally {
            savingQbit = false;
        }
    }

    $effect(() => {
        const onKey = (ev: KeyboardEvent) => {
            if (ev.key === 'Escape') onClose();
        };
        document.addEventListener('keydown', onKey);
        return () => document.removeEventListener('keydown', onKey);
    });

    function trap(ev: KeyboardEvent) {
        if (ev.key !== 'Tab' || !card) return;
        const els = [...card.querySelectorAll<HTMLElement>('input, select, button:not(:disabled), a[href]')];
        if (els.length === 0) return;
        const first = els[0];
        const last = els[els.length - 1];
        if (ev.shiftKey && document.activeElement === first) {
            ev.preventDefault();
            last.focus();
        } else if (!ev.shiftKey && document.activeElement === last) {
            ev.preventDefault();
            first.focus();
        }
    }

    function onBackdrop(ev: MouseEvent) {
        if (ev.target === ev.currentTarget) onClose();
    }

    let lang = $state<'any' | 'pl' | 'de' | 'en'>('any');
    function langSuffix(l: typeof lang): string {
        if (l === 'pl') return ' PL';
        if (l === 'de') return ' GER';
        if (l === 'en') return ' ENG';
        return '';
    }

    function mergeResults(found: TorrentOption[]) {
        const merged = new Map(results.map((item) => [item.magnet, item]));
        for (const item of found) {
            const old = merged.get(item.magnet);
            const oldScore = old?.pref_score ?? 0;
            const nextScore = item.pref_score ?? 0;
            if (!old || nextScore > oldScore || (nextScore === oldScore && item.seeds > old.seeds)) {
                merged.set(item.magnet, item);
            }
        }
        results = [...merged.values()]
            .sort((a, b) => {
                const pref = (b.pref_score ?? 0) - (a.pref_score ?? 0);
                return pref || b.seeds - a.seeds;
            })
            .slice(0, 120);
    }

    async function run() {
        const runId = ++searchRun;
        searchAbort?.abort();
        const abort = new AbortController();
        searchAbort = abort;
        loading = true;
        err = '';
        results = [];
        const sources = allSources.filter((s) => s !== 'ext' || kind !== 'book');
        searching = [...sources];
        const finalQ = (q + langSuffix(lang)).trim();
        const errors: string[] = [];

        async function search(source: TorrentSource) {
            try {
                const found = await api.torrentSearch(finalQ, {
                    kind,
                    imdb: imdbId,
                    abs: absOffset,
                    source,
                    signal: abort.signal
                });
                if (runId !== searchRun) return;
                mergeResults(found);
            } catch (caught) {
                if (runId !== searchRun || abort.signal.aborted) return;
                errors.push(caught instanceof Error ? caught.message : 'search failed');
            } finally {
                if (runId !== searchRun) return;
                searching = searching.filter((name) => name !== source);
                loading = results.length === 0 && searching.length > 0;
            }
        }

        await Promise.all(sources.map(search));
        if (runId !== searchRun || abort.signal.aborted) return;

        loading = false;
        if (results.length > 0 || errors.length === 0) return;
        if (errors.every((msg) => msg.includes('not configured'))) envErr = true;
        else err = errors[0];
    }

    onMount(() => {
        loadSettings();
        searchInput?.focus();
        return () => {
            clearTimeout(timer);
            searchRun++;
            searchAbort?.abort();
        };
    });

    $effect(() => {
        if (!envErr) run();
    });

    function onInput() {
        clearTimeout(timer);
        timer = setTimeout(run, 350);
    }

    async function start(t: TorrentOption) {
        const key = t.provider_id || t.magnet;
        starting = key;
        delete rowErr[key];
        const addBook = kind === 'book' && olKey;
        const req = {
            magnet: t.magnet,
            media_id: mediaId,
            tmdb_id: tmdbId,
            media_type: mediaType,
            episode_id: episodeId ?? null,
            season: season ?? null,
            episode: episode ?? null,
            title: t.title,
            torrent: t
        };
        try {
            if (addBook) {
                await api.bookTorrentAdd(olKey, { magnet: t.magnet, title: t.title });
            } else {
                onStarted?.(t);
                await api.createDownload(req);
            }
            if (addBook) onStarted?.(t);
            if (!addBook && episode != null) {
                lookingAhead = true;
                const hits = await lookAhead(t);
                lookingAhead = false;
                if (hits.length > 0) {
                    nextUp = { label: [t.release_group, t.quality].filter(Boolean).join(' · '), hits };
                    return;
                }
            }
            onClose();
        } catch (caught) {
            rowErr[key] = caught instanceof Error ? caught.message : 'failed to start';
        } finally {
            starting = null;
            lookingAhead = false;
        }
    }

    type NextHit = { ep: number; t: TorrentOption; on: boolean };
    let nextUp = $state<{ label: string; hits: NextHit[] } | null>(null);
    let lookingAhead = $state(false);
    let addingNext = $state(false);

    function sameRelease(a: TorrentOption, b: TorrentOption) {
        if (a.quality !== b.quality) return false;
        if (a.video_codec && b.video_codec && a.video_codec !== b.video_codec) return false;
        if (a.release_group && b.release_group) return a.release_group.toLowerCase() === b.release_group.toLowerCase();
        const shape = (s: string) =>
            s
                .toLowerCase()
                .replace(/s\d{1,2}e\d{1,4}/g, '#')
                .replace(/ - \d{1,4}(v\d)?/g, ' - #');
        return shape(a.title) === shape(b.title);
    }

    function isEpisode(title: string, ep: number) {
        const t = title.toLowerCase();
        if (/\b(batch|complete)\b/.test(t)) return false;
        const p = String(ep).padStart(2, '0');
        return t.includes(`e${p}`) || new RegExp(' - 0*' + ep + '(v[0-9])?([ .[(]|$)').test(t);
    }

    async function lookAhead(t0: TorrentOption): Promise<NextHit[]> {
        if (episode == null) return [];
        const base = (q + langSuffix(lang)).trim();
        if (!/S\d{1,2}E\d{1,4}/i.test(base)) return [];
        const eps: number[] = [];
        for (let e = episode + 1; eps.length < 6 && e <= episode + 12; e++) {
            if (maxEpisode && e > maxEpisode) break;
            if (!haveEpisodes.includes(e)) eps.push(e);
        }
        const found = await Promise.all(
            eps.map(async (e) => {
                const qe = base.replace(/S(\d{1,2})E\d{1,4}/i, (_m, s) => `S${s}E${String(e).padStart(2, '0')}`);
                const lists = await Promise.all(
                    (['jackett', 'prowlarr'] as const).map((source) =>
                        api.torrentSearch(qe, { kind, imdb: imdbId, source, abs: absOffset }).catch(() => [])
                    )
                );
                const best = lists
                    .flat()
                    .filter((r) => sameRelease(t0, r) && isEpisode(r.title, e))
                    .sort((a, b) => b.seeds - a.seeds)[0];
                return best ? { ep: e, t: best, on: true } : null;
            })
        );
        return found.filter((h): h is NextHit => h !== null);
    }

    async function downloadNext() {
        if (!nextUp) return;
        addingNext = true;
        nextErr = '';
        const picked = nextUp.hits.filter((h) => h.on);
        const started: number[] = [];
        for (const h of picked) {
            try {
                await api.createDownload({
                    magnet: h.t.magnet,
                    media_id: mediaId,
                    tmdb_id: tmdbId,
                    media_type: mediaType,
                    season: season ?? null,
                    episode: h.ep,
                    title: h.t.title,
                    torrent: h.t
                });
                started.push(h.ep);
                h.on = false;
            } catch (caught) {
                nextErr = `E${String(h.ep).padStart(2, '0')}: ${caught instanceof Error ? caught.message : 'failed to start'}`;
                break;
            }
        }
        if (started.length > 0) onMoreStarted?.(started);
        addingNext = false;
        if (!nextErr) onClose();
    }

    function fmtSize(bytes: number): string {
        if (bytes <= 0) return '?';
        const u = ['B', 'KB', 'MB', 'GB', 'TB'];
        let i = 0;
        let n = bytes;
        while (n >= 1024 && i < u.length - 1) {
            n /= 1024;
            i++;
        }
        return `${n.toFixed(n >= 10 || i === 0 ? 0 : 1)} ${u[i]}`;
    }

    type SortKey = 'best' | 'seeds' | 'big' | 'small';
    const SORTS: [SortKey, string][] = [
        ['best', 'best match'],
        ['seeds', 'most seeds'],
        ['big', 'largest'],
        ['small', 'smallest']
    ];
    const QUALITY_ORDER = ['2160p', '1080p', '720p', '480p', 'unknown'];
    const GB = 1024 ** 3;

    let sortBy = $state<SortKey>('best');
    let qualities = $state<Set<string>>(new Set());
    let codecs = $state<Set<string>>(new Set());
    let seededOnly = $state(false);

    const qualityOf = (t: TorrentOption) => t.quality ?? 'unknown';
    const huge = (t: TorrentOption) => t.size > (episode != null ? 20 : 80) * GB;

    function toggle(set: Set<string>, v: string) {
        const next = new Set(set);
        if (!next.delete(v)) next.add(v);
        return next;
    }

    function countBy(list: TorrentOption[], key: (t: TorrentOption) => string | null) {
        const m = new Map<string, number>();
        for (const t of list) {
            const k = key(t);
            if (k) m.set(k, (m.get(k) ?? 0) + 1);
        }
        return m;
    }

    const qualityCounts = $derived(
        [...countBy(results, qualityOf)].sort((a, b) => QUALITY_ORDER.indexOf(a[0]) - QUALITY_ORDER.indexOf(b[0]))
    );
    const codecCounts = $derived([...countBy(results, (t) => t.video_codec)].sort((a, b) => b[1] - a[1]));
    const sourceCounts = $derived([...countBy(results, (t) => t.aggregator ?? null)]);

    const filtersOn = $derived(qualities.size > 0 || codecs.size > 0 || seededOnly || aggFilter !== 'all');

    const shown = $derived.by(() => {
        const list = results.filter(
            (t) =>
                (aggFilter === 'all' || t.aggregator === aggFilter) &&
                (qualities.size === 0 || qualities.has(qualityOf(t))) &&
                (codecs.size === 0 || (t.video_codec !== null && codecs.has(t.video_codec))) &&
                (!seededOnly || t.seeds > 0)
        );
        if (sortBy === 'seeds') return list.sort((a, b) => b.seeds - a.seeds);
        if (sortBy === 'big') return list.sort((a, b) => b.size - a.size);
        if (sortBy === 'small') return list.sort((a, b) => (a.size || Infinity) - (b.size || Infinity));
        return list;
    });

    function clearFilters() {
        qualities = new Set();
        codecs = new Set();
        seededOnly = false;
        aggFilter = 'all';
    }

    const qualityClasses: Record<string, string> = {
        '2160p': 'pw-q-amber',
        '1080p': 'pw-q-green',
        '720p': 'pw-q-blue',
        '480p': 'pw-q-gray'
    };
</script>

<div class="pw-tp-bg" onclick={onBackdrop} role="presentation">
    <div
        class="pw-tp-card"
        role="dialog"
        aria-modal="true"
        aria-label="download picker"
        tabindex="-1"
        bind:this={card}
        onkeydown={trap}
    >
        <header class="pw-tp-head">
            <input
                bind:value={q}
                bind:this={searchInput}
                oninput={onInput}
                class="pw-tp-input"
                placeholder="search torrents..."
                spellcheck="false"
                autocomplete="off"
            />
            <select
                bind:value={lang}
                onchange={() => run()}
                class="pw-tp-lang"
                aria-label="language"
                title="search language hint"
            >
                <option value="any">any</option>
                <option value="en">english</option>
                <option value="pl">polish</option>
                <option value="de">german</option>
            </select>
            <button class="pw-tp-x" onclick={onClose} aria-label="close">
                <svg
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <path d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
        </header>

        {#if !envErr && !qbitErr && !lookingAhead && !nextUp && results.length > 0}
            {#snippet chip(label: string, on: boolean, flip: () => void, n?: number)}
                <button class="pw-tp-chip" class:on aria-pressed={on} onclick={flip}>
                    {label}{#if n !== undefined}<small>{n}</small>{/if}
                </button>
            {/snippet}
            <div class="pw-tp-filters">
                <div class="pw-tp-chips">
                    {#each qualityCounts as [k, n] (k)}
                        {@render chip(k, qualities.has(k), () => (qualities = toggle(qualities, k)), n)}
                    {/each}
                    {#if codecCounts.length > 1}
                        <span class="pw-tp-sep"></span>
                        {#each codecCounts as [k, n] (k)}
                            {@render chip(k, codecs.has(k), () => (codecs = toggle(codecs, k)), n)}
                        {/each}
                    {/if}
                    {#if sourceCounts.length > 1}
                        <span class="pw-tp-sep"></span>
                        {#each sourceCounts as [k, n] (k)}
                            {@render chip(
                                k,
                                aggFilter === k,
                                () => (aggFilter = aggFilter === k ? 'all' : (k as TorrentSource)),
                                n
                            )}
                        {/each}
                    {/if}
                    <span class="pw-tp-sep"></span>
                    {@render chip('seeded only', seededOnly, () => (seededOnly = !seededOnly))}
                </div>
                <select bind:value={sortBy} class="pw-tp-lang pw-tp-sort" aria-label="sort">
                    {#each SORTS as [k, label] (k)}
                        <option value={k}>{label}</option>
                    {/each}
                </select>
            </div>
        {/if}

        {#if err}
            <div class="pw-tp-err">{err}</div>
        {/if}

        <div class="pw-tp-body">
            {#if qbitErr}
                <div class="pw-setup-card">
                    <div class="pw-setup-head">
                        <div class="pw-setup-tag">// qbittorrent not configured</div>
                        <div class="pw-os-toggle">
                            <button class:on={platform === 'unix'} onclick={() => (platform = 'unix')}
                                >linux / mac</button
                            >
                            <button class:on={platform === 'pwsh'} onclick={() => (platform = 'pwsh')}
                                >powershell</button
                            >
                            <button class:on={platform === 'cmd'} onclick={() => (platform = 'cmd')}>cmd</button>
                        </div>
                    </div>
                    <h3 class="pw-setup-h">qbittorrent setup</h3>
                    {#if platform !== 'unix'}
                        <p class="pw-setup-alt">
                            on windows, the simplest route is the
                            <a
                                href="https://www.qbittorrent.org/download"
                                target="_blank"
                                rel="noreferrer"
                                class="pw-setup-link">native installer</a
                            >
                            (skip docker entirely): install, enable web ui under tools, options, web ui, set a password, then
                            paste url + creds below.
                        </p>
                    {/if}
                    <ol class="pw-setup-steps">
                        <li>
                            or via docker (download path mounts your current dir's <code>media/</code> folder):
                            <CopyCode code={qbitCmd} multiline />
                        </li>
                        <li>
                            open <a
                                href={formQbitUrl || defaultQbitUrl()}
                                target="_blank"
                                rel="noreferrer"
                                class="pw-setup-link">{formQbitUrl || defaultQbitUrl()}</a
                            >
                        </li>
                        <li>
                            for docker: grab temp password from logs:
                            <CopyCode code={qbitPwCmd} multiline />
                        </li>
                        <li>log in, set a permanent password under tools, options, web ui</li>
                        <li>paste url + creds below and save</li>
                    </ol>

                    <div class="pw-setup-form">
                        <label class="pw-setup-label">
                            qbit url
                            <input
                                class="pw-setup-input"
                                bind:value={formQbitUrl}
                                placeholder={defaultQbitUrl()}
                                spellcheck="false"
                                autocomplete="off"
                            />
                        </label>
                        <label class="pw-setup-label">
                            user
                            <input
                                class="pw-setup-input"
                                bind:value={formQbitUser}
                                placeholder="admin"
                                spellcheck="false"
                                autocomplete="off"
                            />
                        </label>
                        <label class="pw-setup-label">
                            password
                            <input
                                class="pw-setup-input"
                                type="password"
                                bind:value={formQbitPass}
                                placeholder="paste here"
                                autocomplete="off"
                            />
                        </label>
                        <button class="pw-setup-save" onclick={saveQbit} disabled={savingQbit || !formQbitUrl.trim()}>
                            {savingQbit ? 'saving...' : 'save'}
                        </button>
                    </div>
                </div>
            {:else if envErr}
                <div class="pw-setup-card">
                    <div class="pw-setup-head">
                        <div class="pw-setup-tag">// jackett not configured</div>
                        <div class="pw-os-toggle">
                            <button class:on={platform === 'unix'} onclick={() => (platform = 'unix')}
                                >linux / mac</button
                            >
                            <button class:on={platform === 'pwsh'} onclick={() => (platform = 'pwsh')}
                                >powershell</button
                            >
                            <button class:on={platform === 'cmd'} onclick={() => (platform = 'cmd')}>cmd</button>
                        </div>
                    </div>
                    <h3 class="pw-setup-h">jackett setup</h3>
                    {#if platform !== 'unix'}
                        <p class="pw-setup-alt">
                            on windows, easiest is the
                            <a
                                href="https://github.com/Jackett/Jackett/releases"
                                target="_blank"
                                rel="noreferrer"
                                class="pw-setup-link">native windows zip</a
                            >
                            (Jackett.Binaries.Windows.zip): unzip, run <code>JackettConsole.exe</code>.
                        </p>
                    {/if}
                    <ol class="pw-setup-steps">
                        <li>
                            or via docker:
                            <CopyCode code={jackettCmd} multiline />
                        </li>
                        <li>
                            open <a
                                href={formJackettUrl || defaultJackettUrl()}
                                target="_blank"
                                rel="noreferrer"
                                class="pw-setup-link">{formJackettUrl || defaultJackettUrl()}</a
                            >, then add the indexers you use.
                        </li>
                        <li>copy the api key from the jackett dashboard (top-right)</li>
                        <li>paste it below and save:</li>
                    </ol>

                    <div class="pw-setup-form">
                        <label class="pw-setup-label">
                            jackett url
                            <input
                                class="pw-setup-input"
                                bind:value={formJackettUrl}
                                placeholder={defaultJackettUrl()}
                                spellcheck="false"
                                autocomplete="off"
                            />
                        </label>
                        <label class="pw-setup-label">
                            api key
                            <input
                                class="pw-setup-input"
                                bind:value={formJackettKey}
                                placeholder="paste here"
                                spellcheck="false"
                                autocomplete="off"
                            />
                        </label>
                        <button
                            class="pw-setup-save"
                            onclick={saveSettings}
                            disabled={savingSettings || !formJackettUrl.trim() || !formJackettKey.trim()}
                        >
                            {savingSettings ? 'saving...' : 'save'}
                        </button>
                    </div>

                    <p class="pw-setup-warn">
                        <strong>note:</strong> on a vps, don't leave port 9117 publicly exposed. bind jackett to
                        <code>127.0.0.1:9117</code>, ssh-tunnel for setup, or front it with a reverse proxy (caddy +
                        auth). the docker compose stack in the deploy slice puts jackett on the internal network only.
                    </p>
                </div>
            {:else if lookingAhead}
                <div class="pw-tp-empty">
                    <div class="pw-tp-spin"></div>
                    <p>download started, checking the next episodes...</p>
                </div>
            {:else if nextUp}
                <div class="pw-tp-next">
                    <p class="pw-tp-next-head">
                        {nextUp.label ? `${nextUp.label} also has:` : 'the same release also has:'}
                    </p>
                    <div class="pw-tp-next-grid">
                        {#each nextUp.hits as h (h.ep)}
                            <button
                                class="pw-tp-next-ep"
                                class:is-on={h.on}
                                title="{h.t.title} · {fmtSize(h.t.size)}"
                                onclick={() => (h.on = !h.on)}
                            >
                                <span>E{String(h.ep).padStart(2, '0')}</span>
                                <small>{h.t.seeds === 0 ? 'dead' : `${h.t.seeds} seeds`}</small>
                            </button>
                        {/each}
                    </div>
                    {#if nextErr}<p class="pw-tp-rowerr pw-tp-next-err" role="alert">{nextErr}</p>{/if}
                    <div class="pw-tp-next-actions">
                        <button class="pw-tp-close" onclick={onClose} disabled={addingNext}>no thanks</button>
                        <button
                            class="pw-tp-dl"
                            onclick={downloadNext}
                            disabled={addingNext || !nextUp.hits.some((h) => h.on)}
                        >
                            {addingNext ? 'adding...' : `download ${nextUp.hits.filter((h) => h.on).length}`}
                        </button>
                    </div>
                </div>
            {:else if loading}
                <div class="pw-tp-empty">
                    <div class="pw-tp-spin"></div>
                    <p>searching {searching.length > 0 ? searching.join(' + ') : 'indexers'}...</p>
                </div>
            {:else if results.length === 0}
                <div class="pw-tp-empty">
                    <p>nothing for "{q}"</p>
                    <p class="pw-tp-empty-source">{emptySource}</p>
                    <p class="pw-tp-empty-hint">
                        {emptyTip}
                        <a href="/admin?tab=settings" class="pw-setup-link">download settings</a>
                    </p>
                </div>
            {:else}
                {@const split = sortBy === 'best'}
                {@const matched = split ? shown.filter((r) => (r.pref_score ?? 0) > 0) : []}
                {@const rest = split ? shown.filter((r) => !((r.pref_score ?? 0) > 0)) : shown}
                {#snippet torrentRow(t: TorrentOption, pref: boolean)}
                    {@const key = t.provider_id || t.magnet}
                    {@const busy = starting === key}
                    {@const extras = [...t.audio, t.subtitle_info, ...t.tags.slice(0, 4)].filter(Boolean)}
                    <div class="pw-tp-row" class:pw-tp-row-pref={pref}>
                        <div class="pw-tp-info">
                            <p class="pw-tp-title">{t.title}</p>
                            <div class="pw-tp-tags">
                                {#if t.quality}<span class="pw-tp-pill {qualityClasses[t.quality] ?? 'pw-q-gray'}"
                                        >{t.quality}</span
                                    >{/if}
                                {#if t.video_codec}<span class="pw-tp-pill pw-tp-codec">{t.video_codec}</span>{/if}
                                {#if t.release_group}<span class="pw-tp-pill pw-tp-grp">{t.release_group}</span>{/if}
                                <span class="pw-tp-src">{t.provider}</span>
                                {#if t.aggregator}<span class="pw-tp-eng pw-tp-eng-{t.aggregator}">{t.aggregator}</span>{/if}
                            </div>
                            {#if extras.length > 0}<p class="pw-tp-extra">{extras.join(' · ')}</p>{/if}
                            {#if rowErr[key]}<p class="pw-tp-rowerr" role="alert">couldn't start: {rowErr[key]}</p>{/if}
                        </div>
                        <div class="pw-tp-nums">
                            <span class="pw-tp-size" class:is-huge={huge(t)}>{fmtSize(t.size)}</span>
                            <span class="pw-tp-seeds" class:is-dead={t.seeds === 0} title="{t.seeds} seeds, {t.peers} peers"
                                >{t.seeds === 0 ? 'dead' : `${t.seeds} seeds`}</span
                            >
                        </div>
                        <button class="pw-tp-get" disabled={starting !== null} onclick={() => start(t)}>
                            {busy ? 'adding...' : rowErr[key] ? 'retry' : 'download'}
                        </button>
                    </div>
                {/snippet}
                {#if matched.length > 0}
                    <div class="pw-tp-section-head">based on your downloads</div>
                {/if}
                {#each matched as t (t.provider_id + t.magnet)}
                    {@render torrentRow(t, true)}
                {/each}
                {#if matched.length > 0 && rest.length > 0}
                    <div class="pw-tp-section-head pw-tp-section-rest">all results</div>
                {/if}
                {#each rest as t (t.provider_id + t.magnet)}
                    {@render torrentRow(t, !split && (t.pref_score ?? 0) > 0)}
                {/each}
                {#if shown.length === 0}
                    <div class="pw-tp-empty">
                        <p>nothing matches these filters</p>
                        <button class="pw-tp-close" onclick={clearFilters}>clear filters</button>
                    </div>
                {/if}
            {/if}
        </div>

        {#if !envErr && !loading && !lookingAhead && !nextUp && results.length > 0}
            <footer class="pw-tp-foot">
                <span>
                    {filtersOn ? `${shown.length} of ${results.length}` : results.length} results
                    {#if filtersOn}<button class="pw-tp-reset" onclick={clearFilters}>reset</button>{/if}
                    {#if searching.length > 0}<span class="pw-tp-more">checking {searching.join(' + ')}...</span>{/if}
                </span>
                <button class="pw-tp-close" onclick={onClose}>close</button>
            </footer>
        {/if}
    </div>
</div>

<style>
    .pw-tp-bg {
        position: fixed;
        inset: 0;
        z-index: 200;
        background: rgba(4, 5, 8, 0.65);
        backdrop-filter: blur(8px);
        animation: pw-tp-fade 0.15s ease-out;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 16px;
    }
    @keyframes pw-tp-fade {
        from {
            opacity: 0;
        }
        to {
            opacity: 1;
        }
    }
    .pw-tp-card {
        width: 100%;
        max-width: 720px;
        max-height: 85vh;
        display: flex;
        flex-direction: column;
        background: rgba(18, 20, 24, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 14px;
        box-shadow:
            0 28px 80px -20px rgba(0, 0, 0, 0.7),
            0 0 0 1px rgba(0, 0, 0, 0.4);
        overflow: hidden;
        animation: pw-tp-rise 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    @keyframes pw-tp-rise {
        from {
            transform: translateY(14px);
            opacity: 0;
        }
        to {
            transform: translateY(0);
            opacity: 1;
        }
    }

    .pw-tp-head {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 14px 16px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .pw-tp-input {
        flex: 1;
        background: none;
        border: none;
        outline: none;
        color: #ececef;
        font: inherit;
        font-size: 15px;
        padding: 0;
    }
    .pw-tp-input::placeholder {
        color: rgba(220, 220, 225, 0.32);
    }
    .pw-tp-lang {
        color-scheme: dark;
        background: #1a1c21;
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: rgba(232, 232, 234, 0.85);
        font-size: 12px;
        padding: 5px 8px;
        border-radius: 6px;
        cursor: pointer;
        outline: none;
        transition:
            background 0.14s ease,
            border-color 0.14s ease;
    }
    .pw-tp-lang:hover {
        background: #22252b;
        border-color: rgba(255, 255, 255, 0.14);
    }
    .pw-tp-lang option {
        background: #16181d;
        color: #ececef;
    }
    .pw-tp-lang:focus {
        border-color: color-mix(in oklch, var(--pw-accent) 50%, transparent);
    }
    .pw-tp-x {
        width: 28px;
        height: 28px;
        display: grid;
        place-items: center;
        background: transparent;
        border: none;
        color: rgba(220, 220, 225, 0.5);
        border-radius: 50%;
        cursor: pointer;
        transition:
            background 0.14s ease,
            color 0.14s ease;
    }
    .pw-tp-x:hover {
        background: rgba(255, 255, 255, 0.06);
        color: #ececef;
    }

    .pw-tp-err {
        margin: 10px 16px 0;
        padding: 8px 12px;
        background: color-mix(in oklch, oklch(0.6 0.2 25) 10%, transparent);
        border: 1px solid color-mix(in oklch, oklch(0.6 0.2 25) 24%, transparent);
        border-radius: 6px;
        color: oklch(0.78 0.14 25);
        font-size: 12.5px;
    }

    .pw-tp-body {
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        padding: 8px 8px;
    }

    .pw-tp-empty {
        text-align: center;
        padding: 60px 16px;
        color: rgba(220, 220, 225, 0.55);
        font-size: 13px;
    }
    .pw-tp-empty p {
        margin: 0;
    }
    .pw-tp-empty-hint {
        color: rgba(220, 220, 225, 0.35);
        font-size: 12px;
        margin-top: 4px !important;
    }
    .pw-tp-empty-source {
        color: rgba(220, 220, 225, 0.42);
        font-size: 12px;
        margin-top: 4px !important;
    }
    .pw-tp-spin {
        width: 22px;
        height: 22px;
        border: 2px solid rgba(255, 255, 255, 0.1);
        border-top-color: var(--pw-accent);
        border-radius: 50%;
        margin: 0 auto 10px;
        animation: pw-tp-spinner 0.8s linear infinite;
    }
    @keyframes pw-tp-spinner {
        to {
            transform: rotate(360deg);
        }
    }

    .pw-tp-row {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 10px 12px;
        border-radius: 8px;
        transition: background 0.12s ease;
    }
    .pw-tp-row:hover {
        background: rgba(255, 255, 255, 0.045);
    }
    .pw-tp-next {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .pw-tp-next-head {
        margin: 2px 12px 6px;
        font-size: 13px;
        color: var(--pw-fg-dim);
    }
    .pw-tp-next-grid {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
        margin: 0 12px;
    }
    .pw-tp-next-ep {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 2px;
        width: 64px;
        height: 56px;
        border-radius: 8px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        background: rgba(255, 255, 255, 0.03);
        color: var(--pw-fg-dim);
        cursor: pointer;
        transition:
            background 0.12s,
            border-color 0.12s,
            color 0.12s;
    }
    .pw-tp-next-ep span {
        font-size: 14px;
        font-weight: 700;
    }
    .pw-tp-next-ep small {
        font-size: 11px;
        opacity: 0.8;
    }
    .pw-tp-next-ep.is-on {
        border-color: var(--pw-accent);
        background: color-mix(in oklch, var(--pw-accent) 18%, transparent);
        color: var(--pw-fg);
    }
    .pw-tp-next-actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
        margin: 10px 12px 4px;
    }
    .pw-tp-row-pref {
        background: color-mix(in oklch, var(--pw-accent) 5%, transparent);
    }
    .pw-tp-row-pref:hover {
        background: color-mix(in oklch, var(--pw-accent) 9%, transparent);
    }
    .pw-tp-section-head {
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.1em;
        color: var(--pw-accent);
        padding: 12px 18px 6px;
    }
    .pw-tp-section-rest {
        color: rgba(220, 220, 225, 0.45);
    }
    .pw-tp-info {
        flex: 1;
        min-width: 0;
    }
    .pw-tp-title {
        margin: 0;
        color: #d8d8dc;
        font-size: 13px;
        line-height: 1.4;
        word-break: break-word;
    }
    .pw-tp-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 5px;
        margin-top: 6px;
        align-items: center;
    }
    .pw-tp-pill {
        font-size: 10px;
        font-weight: 500;
        padding: 2px 7px;
        border-radius: 4px;
        background: rgba(255, 255, 255, 0.05);
        color: rgba(220, 220, 225, 0.65);
        letter-spacing: 0.01em;
    }
    .pw-q-green {
        background: color-mix(in oklch, oklch(0.62 0.16 150) 75%, black);
        color: white;
    }
    .pw-q-amber {
        background: color-mix(in oklch, oklch(0.65 0.16 60) 75%, black);
        color: white;
    }
    .pw-q-blue {
        background: color-mix(in oklch, oklch(0.65 0.13 240) 75%, black);
        color: white;
    }
    .pw-q-gray {
        background: rgba(255, 255, 255, 0.07);
        color: rgba(220, 220, 225, 0.7);
    }
    .pw-tp-codec {
        color: oklch(0.8 0.1 220);
        background: color-mix(in oklch, oklch(0.65 0.13 240) 22%, transparent);
    }
    .pw-tp-grp {
        color: oklch(0.82 0.13 50);
        background: color-mix(in oklch, oklch(0.65 0.16 50) 22%, transparent);
    }

    .pw-tp-filters {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 8px 16px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .pw-tp-chips {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 4px;
    }
    .pw-tp-chip {
        display: inline-flex;
        align-items: baseline;
        gap: 5px;
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.07);
        color: rgba(220, 220, 225, 0.6);
        font-size: 11.5px;
        padding: 3px 9px;
        border-radius: 999px;
        cursor: pointer;
        white-space: nowrap;
        transition:
            background 0.12s ease,
            border-color 0.12s ease,
            color 0.12s ease;
    }
    .pw-tp-chip small {
        font-size: 10px;
        color: rgba(220, 220, 225, 0.38);
        font-variant-numeric: tabular-nums;
    }
    .pw-tp-chip:hover {
        color: #ececef;
        border-color: rgba(255, 255, 255, 0.14);
    }
    .pw-tp-chip.on {
        color: #ececef;
        border-color: color-mix(in oklch, var(--pw-accent) 60%, transparent);
        background: color-mix(in oklch, var(--pw-accent) 16%, transparent);
    }
    .pw-tp-chip.on small {
        color: rgba(236, 236, 239, 0.6);
    }
    .pw-tp-chip:focus-visible,
    .pw-tp-reset:focus-visible {
        outline: 2px solid var(--pw-accent);
        outline-offset: 1px;
    }
    .pw-tp-sep {
        width: 1px;
        height: 14px;
        margin: 0 3px;
        background: rgba(255, 255, 255, 0.08);
    }
    .pw-tp-sort {
        flex-shrink: 0;
        align-self: flex-start;
    }
    .pw-tp-reset {
        margin-left: 6px;
        background: none;
        border: 0;
        padding: 0;
        color: var(--pw-accent);
        font-size: inherit;
        cursor: pointer;
    }

    .pw-tp-src {
        margin-left: 2px;
        font-size: 11.5px;
        font-weight: 500;
        color: rgba(232, 232, 234, 0.78);
    }
    .pw-tp-eng {
        font-size: 10.5px;
        font-weight: 600;
        padding: 1px 6px;
        border-radius: 4px;
        border: 1px solid currentColor;
        opacity: 0.9;
    }
    .pw-tp-eng-jackett {
        color: oklch(0.8 0.14 25);
    }
    .pw-tp-eng-prowlarr {
        color: oklch(0.8 0.13 175);
    }
    .pw-tp-eng-ext {
        color: oklch(0.8 0.13 300);
    }
    .pw-tp-extra {
        margin: 5px 0 0;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.42);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-tp-rowerr {
        margin: 6px 0 0;
        font-size: 11.5px;
        color: oklch(0.78 0.14 25);
    }
    .pw-tp-next-err {
        margin: 10px 12px 0;
    }
    .pw-tp-nums {
        flex-shrink: 0;
        width: 76px;
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 3px;
        font-variant-numeric: tabular-nums;
    }
    .pw-tp-size {
        font-size: 13px;
        font-weight: 500;
        color: #d8d8dc;
    }
    .pw-tp-size.is-huge {
        color: oklch(0.8 0.14 70);
    }
    .pw-tp-seeds {
        font-size: 11px;
        color: oklch(0.78 0.16 145);
    }
    .pw-tp-seeds.is-dead {
        color: oklch(0.68 0.13 25);
    }
    .pw-tp-get {
        flex-shrink: 0;
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: rgba(232, 232, 234, 0.8);
        border-radius: 6px;
        padding: 6px 12px;
        font-size: 12px;
        font-weight: 600;
        cursor: pointer;
        transition:
            background 0.12s ease,
            border-color 0.12s ease,
            color 0.12s ease;
    }
    .pw-tp-row:hover .pw-tp-get:not(:disabled),
    .pw-tp-get:focus-visible {
        background: var(--pw-accent);
        border-color: transparent;
        color: #08090b;
        outline: none;
    }
    .pw-tp-get:disabled {
        opacity: 0.5;
        cursor: default;
    }

    .pw-tp-dl {
        flex-shrink: 0;
        align-self: flex-start;
        margin-top: 2px;
        background: var(--pw-accent);
        color: #08090b;
        border: none;
        border-radius: 6px;
        padding: 6px 14px;
        font-size: 12px;
        font-weight: 600;
        cursor: pointer;
        transition:
            filter 0.15s ease,
            opacity 0.15s ease;
    }
    .pw-tp-dl:hover:not(:disabled) {
        filter: brightness(1.08);
    }
    .pw-tp-dl:disabled {
        opacity: 0.5;
        cursor: default;
    }

    .pw-tp-foot {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 10px 16px;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        font-size: 11.5px;
        color: rgba(220, 220, 225, 0.42);
    }
    .pw-tp-more {
        margin-left: 8px;
        color: var(--pw-accent);
    }
    .pw-tp-close {
        background: transparent;
        border: none;
        color: rgba(220, 220, 225, 0.55);
        font-size: 12px;
        cursor: pointer;
        padding: 4px 10px;
        border-radius: 5px;
        transition:
            color 0.14s ease,
            background 0.14s ease;
    }
    .pw-tp-close:hover {
        color: #ececef;
        background: rgba(255, 255, 255, 0.05);
    }

    @media (max-width: 600px) {
        .pw-tp-card {
            max-height: 95vh;
            border-radius: 14px 14px 0 0;
            align-self: flex-end;
        }
        .pw-tp-bg {
            padding: 0;
            align-items: flex-end;
        }
        .pw-tp-filters {
            padding: 8px 12px;
            align-items: flex-start;
        }
        .pw-tp-chips {
            flex-wrap: nowrap;
            overflow-x: auto;
            scrollbar-width: none;
        }
        .pw-tp-chip {
            padding: 5px 10px;
        }
        .pw-tp-row {
            display: grid;
            grid-template-columns: 1fr auto;
            column-gap: 10px;
            row-gap: 8px;
        }
        .pw-tp-info {
            grid-row: span 2;
        }
        .pw-tp-nums {
            width: auto;
        }
    }

    .pw-setup-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 6px;
        gap: 12px;
        flex-wrap: wrap;
    }
    .pw-os-toggle {
        display: flex;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 5px;
        padding: 2px;
    }
    .pw-os-toggle button {
        background: transparent;
        border: none;
        color: rgba(220, 220, 225, 0.5);
        padding: 3px 9px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10.5px;
        letter-spacing: 0.04em;
        cursor: pointer;
        border-radius: 3px;
        transition:
            background 0.14s ease,
            color 0.14s ease;
    }
    .pw-os-toggle button:hover {
        color: #ececef;
    }
    .pw-os-toggle button.on {
        background: rgba(255, 255, 255, 0.08);
        color: var(--pw-accent);
    }

    .pw-setup-alt {
        margin: 0 0 14px;
        padding: 10px 13px;
        background: color-mix(in oklch, var(--pw-accent) 8%, transparent);
        border: 1px solid color-mix(in oklch, var(--pw-accent) 22%, transparent);
        border-radius: 6px;
        font-size: 12.5px;
        line-height: 1.55;
        color: rgba(220, 220, 225, 0.85);
    }
    .pw-setup-alt code {
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 3px;
        padding: 1px 5px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
    }

    .pw-setup-card {
        max-width: 580px;
        margin: 24px auto;
        padding: 26px 30px;
        background: rgba(20, 22, 26, 0.7);
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-radius: 12px;
    }
    .pw-setup-tag {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        color: var(--pw-accent);
        letter-spacing: 0.08em;
        margin-bottom: 6px;
    }
    .pw-setup-h {
        color: #ececef;
        font-size: 19px;
        font-weight: 600;
        margin: 0 0 14px;
    }
    .pw-setup-steps {
        margin: 0;
        padding-left: 22px;
        color: rgba(220, 220, 225, 0.78);
        font-size: 13.5px;
        line-height: 1.65;
    }
    .pw-setup-steps li {
        margin-bottom: 10px;
    }
    .pw-setup-steps li:last-child {
        margin-bottom: 0;
    }
    .pw-setup-p {
        color: rgba(220, 220, 225, 0.78);
        font-size: 13.5px;
        line-height: 1.55;
        margin: 0;
    }
    .pw-setup-link {
        color: var(--pw-accent);
        text-decoration: none;
        border-bottom: 1px dotted color-mix(in oklch, var(--pw-accent) 50%, transparent);
    }
    .pw-setup-link:hover {
        color: #fff;
        border-bottom-color: #fff;
    }

    .pw-setup-form {
        margin-top: 18px;
        display: flex;
        flex-direction: column;
        gap: 10px;
    }
    .pw-setup-label {
        display: flex;
        flex-direction: column;
        gap: 5px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.55);
        letter-spacing: 0.05em;
    }
    .pw-setup-input {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 6px;
        padding: 9px 12px;
        color: #ececef;
        font: inherit;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 13px;
        outline: none;
        transition: border-color 0.15s ease;
    }
    .pw-setup-input:focus {
        border-color: color-mix(in oklch, var(--pw-accent) 50%, transparent);
    }
    .pw-setup-save {
        align-self: flex-start;
        background: var(--pw-accent);
        color: #08090b;
        border: none;
        border-radius: 6px;
        padding: 8px 18px;
        font-size: 13px;
        font-weight: 600;
        cursor: pointer;
        transition:
            filter 0.15s ease,
            opacity 0.15s ease;
    }
    .pw-setup-save:hover {
        filter: brightness(1.08);
    }
    .pw-setup-save:disabled {
        opacity: 0.45;
        cursor: default;
    }

    .pw-setup-warn {
        margin-top: 18px;
        padding: 11px 14px;
        background: color-mix(in oklch, oklch(0.7 0.15 60) 8%, transparent);
        border: 1px solid color-mix(in oklch, oklch(0.7 0.15 60) 22%, transparent);
        border-radius: 6px;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.72);
        line-height: 1.55;
    }
    .pw-setup-warn strong {
        color: oklch(0.85 0.14 60);
        font-weight: 600;
    }
    .pw-setup-warn code {
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 3px;
        padding: 1px 5px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
    }
</style>
