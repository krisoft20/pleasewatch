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
        olKey?: string;
        onClose: () => void;
        onStarted?: (t: TorrentOption) => void;
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
        olKey,
        onClose,
        onStarted
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
    let aggFilter = $state<'all' | 'jackett' | 'prowlarr'>('all');
    type TorrentSource = 'jackett' | 'prowlarr';
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
            if (!old || (item.pref_score ?? 0) > (old.pref_score ?? 0) || item.seeds > old.seeds) {
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
        aggFilter = 'all';
        const sources: TorrentSource[] = ['jackett', 'prowlarr'];
        searching = [...sources];
        const finalQ = (q + langSuffix(lang)).trim();
        const errors: string[] = [];

        async function search(source: TorrentSource) {
            try {
                const found = await api.torrentSearch(finalQ, {
                    kind,
                    imdb: imdbId,
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
        starting = t.provider_id || t.magnet;
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
            onClose();
        } catch (caught) {
            const msg = caught instanceof Error ? caught.message : 'failed to start';
            alert(msg);
        } finally {
            starting = null;
        }
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

    const qualityClasses: Record<string, string> = {
        '2160p': 'pw-q-amber',
        '1080p': 'pw-q-green',
        '720p': 'pw-q-blue',
        '480p': 'pw-q-gray'
    };
</script>

<div
    class="pw-tp-bg"
    onclick={onBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
>
    <div class="pw-tp-card">
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
                <option value="any">Any</option>
                <option value="en">English</option>
                <option value="pl">Polish</option>
                <option value="de">German</option>
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
                {@const filtered = aggFilter === 'all' ? results : results.filter((r) => r.aggregator === aggFilter)}
                {@const matched = filtered.filter((r) => (r.pref_score ?? 0) > 0)}
                {@const rest = filtered.filter((r) => !((r.pref_score ?? 0) > 0))}
                {#snippet torrentRow(t: TorrentOption, pref: boolean)}
                    {@const busy = starting === (t.provider_id || t.magnet)}
                    <div class="pw-tp-row" class:pw-tp-row-pref={pref}>
                        <div class="pw-tp-info">
                            <p class="pw-tp-title">{t.title}</p>
                            <div class="pw-tp-tags">
                                <span class="pw-tp-pill pw-tp-prov">{t.provider}</span>
                                {#if t.aggregator}<span class="pw-tp-pill pw-tp-agg-{t.aggregator}">{t.aggregator}</span
                                    >{/if}
                                {#if t.quality}<span class="pw-tp-pill {qualityClasses[t.quality] ?? 'pw-q-gray'}"
                                        >{t.quality}</span
                                    >{/if}
                                {#if t.video_codec}<span class="pw-tp-pill pw-tp-codec">{t.video_codec}</span>{/if}
                                {#each t.audio as a}<span class="pw-tp-pill pw-tp-audio">{a}</span>{/each}
                                {#if t.subtitle_info}<span class="pw-tp-pill pw-tp-sub">{t.subtitle_info}</span>{/if}
                                {#if t.release_group}<span class="pw-tp-pill pw-tp-grp">{t.release_group}</span>{/if}
                                {#each t.tags.slice(0, 6) as tag}<span class="pw-tp-pill">{tag}</span>{/each}
                                <span class="pw-tp-meta">{fmtSize(t.size)}</span>
                                <span class="pw-tp-seeds">S:{t.seeds}</span>
                                <span class="pw-tp-peers">P:{t.peers}</span>
                            </div>
                        </div>
                        <button class="pw-tp-dl" disabled={starting !== null} onclick={() => start(t)}>
                            {busy ? 'adding...' : 'download'}
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
                    {@render torrentRow(t, false)}
                {/each}
            {/if}
        </div>

        {#if !envErr && !loading && results.length > 0}
            {@const jCount = results.filter((r) => r.aggregator === 'jackett').length}
            {@const pCount = results.filter((r) => r.aggregator === 'prowlarr').length}
            <footer class="pw-tp-foot">
                <span>
                    {results.length} results
                    {#if searching.length > 0}<span class="pw-tp-more">checking {searching.join(' + ')}...</span>{/if}
                </span>
                {#if jCount > 0 && pCount > 0}
                    <div class="pw-tp-agg-tabs">
                        <button class:on={aggFilter === 'all'} onclick={() => (aggFilter = 'all')}>all</button>
                        <button class:on={aggFilter === 'jackett'} onclick={() => (aggFilter = 'jackett')}
                            >jackett ({jCount})</button
                        >
                        <button class:on={aggFilter === 'prowlarr'} onclick={() => (aggFilter = 'prowlarr')}
                            >prowlarr ({pCount})</button
                        >
                    </div>
                {/if}
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
        background: rgba(255, 255, 255, 0.04);
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
        background: rgba(255, 255, 255, 0.07);
        border-color: rgba(255, 255, 255, 0.14);
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
        align-items: flex-start;
        gap: 12px;
        padding: 10px 12px;
        border-radius: 8px;
        transition: background 0.12s ease;
    }
    .pw-tp-row:hover {
        background: rgba(255, 255, 255, 0.045);
    }
    .pw-tp-row-pref {
        background: rgba(192, 132, 252, 0.04);
    }
    .pw-tp-row-pref:hover {
        background: rgba(192, 132, 252, 0.08);
    }
    .pw-tp-section-head {
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.1em;
        color: rgba(192, 132, 252, 0.85);
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
    .pw-tp-prov {
        color: rgba(220, 220, 225, 0.55);
        background: rgba(255, 255, 255, 0.04);
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
    .pw-tp-audio {
        color: oklch(0.8 0.13 290);
        background: color-mix(in oklch, oklch(0.65 0.16 290) 22%, transparent);
    }
    .pw-tp-sub {
        color: oklch(0.82 0.13 200);
        background: color-mix(in oklch, oklch(0.65 0.13 200) 22%, transparent);
    }
    .pw-tp-grp {
        color: oklch(0.82 0.13 50);
        background: color-mix(in oklch, oklch(0.65 0.16 50) 22%, transparent);
    }
    .pw-tp-agg-jackett {
        color: oklch(0.82 0.14 25);
        background: color-mix(in oklch, oklch(0.65 0.16 25) 22%, transparent);
    }
    .pw-tp-agg-prowlarr {
        color: oklch(0.82 0.14 175);
        background: color-mix(in oklch, oklch(0.65 0.16 175) 22%, transparent);
    }

    .pw-tp-agg-tabs {
        display: flex;
        gap: 4px;
    }
    .pw-tp-agg-tabs button {
        background: transparent;
        border: none;
        color: rgba(220, 220, 225, 0.5);
        font-size: 11px;
        padding: 3px 9px;
        border-radius: 4px;
        cursor: pointer;
        font-variant-numeric: tabular-nums;
    }
    .pw-tp-agg-tabs button:hover {
        color: #ececef;
        background: rgba(255, 255, 255, 0.04);
    }
    .pw-tp-agg-tabs button.on {
        color: #ececef;
        background: rgba(255, 255, 255, 0.08);
    }

    .pw-tp-meta {
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.5);
        font-variant-numeric: tabular-nums;
        margin-left: 4px;
    }
    .pw-tp-seeds {
        font-size: 10.5px;
        color: oklch(0.78 0.16 145);
        font-variant-numeric: tabular-nums;
    }
    .pw-tp-peers {
        font-size: 10.5px;
        color: oklch(0.78 0.13 80);
        font-variant-numeric: tabular-nums;
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
