<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type StorageMediaItem, type StorageView } from '$lib/api';
    import Icon from '$lib/components/Icon.svelte';
    import { fmtBytes, fmtSize, storageColor, type CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    let { onCount }: Props = $props();

    let storage = $state<StorageView | null>(null);
    let storageItems = $state<StorageMediaItem[]>([]);
    let cleaning = $state(false);
    let storageRefreshing = $state(false);
    let sortBy = $state<'date' | 'size' | 'title'>('size');
    let filterType = $state<'all' | 'movie' | 'tv' | 'anime' | 'empty'>('all');
    let storageQuery = $state('');
    let expandedMediaId = $state<string | null>(null);

    let filteredStorage = $derived.by(() => {
        const query = storageQuery.trim().toLowerCase();
        const xs = storageItems.filter((item) => {
            if (filterType === 'movie' && (item.media_type !== 'movie' || item.is_anime)) return false;
            if (filterType === 'tv' && (item.media_type !== 'tv' || item.is_anime)) return false;
            if (filterType === 'anime' && !item.is_anime) return false;
            if (filterType === 'empty' && item.has_files) return false;
            if (query && !item.title.toLowerCase().includes(query)) return false;
            return true;
        });
        if (sortBy === 'title') xs.sort((a, b) => a.title.localeCompare(b.title));
        else if (sortBy === 'size') xs.sort((a, b) => b.bytes - a.bytes);
        else xs.sort((a, b) => (a.added_at < b.added_at ? 1 : -1));
        return xs;
    });
    let visibleDirectories = $derived(storage?.directories.filter((bucket) => bucket.bytes > 0) ?? []);
    let storageDiskPct = $derived(
        storage?.total_bytes && storage.used_bytes !== null ? (storage.used_bytes / storage.total_bytes) * 100 : 0
    );
    let storageAppPct = $derived(
        storage?.total_bytes ? Math.min(storageDiskPct, (storage.app_bytes / storage.total_bytes) * 100) : 0
    );
    let storageOtherPct = $derived(Math.max(0, storageDiskPct - storageAppPct));
    let storageFreePct = $derived(Math.max(0, 100 - storageDiskPct));
    let storageHealth = $derived(storageFreePct < 10 ? 'critical' : storageFreePct < 20 ? 'warning' : 'healthy');
    let downloadCache = $derived(storage?.directories.find((bucket) => bucket.key === '_dl') ?? null);

    onMount(() => {
        refreshStorage().catch(() => {});
    });

    async function refreshStorage() {
        storageRefreshing = true;
        try {
            const report = await api.adminStorage();
            storage = report;
            storageItems = report.items;
            onCount?.(report.items.length || null);
        } finally {
            storageRefreshing = false;
        }
    }

    async function deleteMedia(id: string, title: string) {
        if (!confirm(`delete "${title}" and all its files?`)) return;
        await api.deleteMedia(id);
        await refreshStorage();
    }

    async function cleanDownloadCache() {
        const ok = confirm(
            'delete the raw torrent/upload sources for completed downloads? finished media files in /library/ are NOT touched.'
        );
        if (!ok) return;
        cleaning = true;
        try {
            const r = await api.cleanDownloads();
            alert(`cleaned ${fmtSize(r.cleaned_bytes)} from ${r.cleaned_downloads} downloads.`);
            await refreshStorage();
        } catch {
            alert('clean failed');
        } finally {
            cleaning = false;
        }
    }

    function toggleDetails(id: string) {
        expandedMediaId = expandedMediaId === id ? null : id;
    }

    function itemMeta(item: StorageMediaItem): string {
        const kind = item.is_anime ? 'anime' : item.media_type === 'tv' ? 'TV series' : 'movie';
        const year = item.year ? `, ${item.year}` : '';
        return `${kind}${year} / ${item.relative_path}`;
    }
</script>

{#if storage}
    <section class="pw-storage-hero">
        <div class="pw-storage-hero-head">
            <div>
                <p class="pw-storage-kicker">storage overview</p>
                <div class="pw-storage-title-line">
                    <h2>{fmtBytes(storage.used_bytes)} used</h2>
                    <span class="pw-storage-health {storageHealth}">{storageHealth}</span>
                </div>
                <p class="pw-storage-subtitle">
                    {fmtBytes(storage.free_bytes)} free of {fmtBytes(storage.total_bytes)} on {storage.media_root}
                </p>
            </div>
            <button
                class="pw-storage-refresh"
                onclick={refreshStorage}
                disabled={storageRefreshing}
                aria-label="Refresh storage report"
            >
                <Icon name="sync" class={`w-4 h-4 ${storageRefreshing ? 'pw-admin-spin' : ''}`} strokeWidth={1.8} />
                refresh
            </button>
        </div>

        <div class="pw-capacity-track" aria-label={`${storageDiskPct.toFixed(1)}% of disk used`}>
            <span class="pw-capacity-app" style={`width:${storageAppPct}%`}></span>
            <span class="pw-capacity-other" style={`width:${storageOtherPct}%`}></span>
        </div>
        <div class="pw-capacity-legend">
            <span><i class="app"></i>pleasewatch <strong>{fmtBytes(storage.app_bytes)}</strong></span>
            <span>
                <i class="other"></i>other disk use
                <strong>{fmtBytes(Math.max(0, (storage.used_bytes ?? 0) - storage.app_bytes))}</strong>
            </span>
            <span><i class="free"></i>free <strong>{storageFreePct.toFixed(1)}%</strong></span>
        </div>

        <div class="pw-storage-summary">
            <div>
                <span>app footprint</span>
                <strong>{fmtBytes(storage.app_bytes)}</strong>
                <small>{storage.app_files.toLocaleString()} files tracked</small>
            </div>
            <div>
                <span>playable library</span>
                <strong>{fmtBytes(storage.library_bytes)}</strong>
                <small>{storage.library_files.toLocaleString()} library files</small>
            </div>
            <div>
                <span>download cache</span>
                <strong>{fmtBytes(downloadCache?.bytes ?? 0)}</strong>
                <small>{downloadCache?.files.toLocaleString() ?? 0} raw source files</small>
            </div>
            <button class="pw-cache-action" onclick={cleanDownloadCache} disabled={cleaning || !downloadCache?.bytes}>
                <span>{cleaning ? 'cleaning cache' : 'clean download cache'}</span>
                <small>finished media stays untouched</small>
            </button>
        </div>
    </section>

    <div class="pw-storage-grid">
        <section class="pw-storage-panel pw-storage-breakdown">
            <div class="pw-storage-panel-head">
                <div>
                    <p class="pw-storage-kicker">directory breakdown</p>
                    <h3>Where space goes</h3>
                </div>
                <span>{visibleDirectories.length} active locations</span>
            </div>
            <div class="pw-storage-bars">
                {#each visibleDirectories as bucket (bucket.key)}
                    <div class="pw-storage-bar-row">
                        <div class="pw-storage-bar-label">
                            <span><i style={`background:${storageColor(bucket.key)}`}></i>{bucket.label}</span>
                            <strong>{fmtBytes(bucket.bytes)}</strong>
                        </div>
                        <small>
                            {bucket.files.toLocaleString()} files,
                            {storage.app_bytes ? ((bucket.bytes / storage.app_bytes) * 100).toFixed(1) : '0.0'}% of app
                            data
                        </small>
                    </div>
                {/each}
            </div>
        </section>

        <section class="pw-storage-panel pw-storage-inventory">
            <div class="pw-storage-panel-head">
                <div>
                    <p class="pw-storage-kicker">inventory health</p>
                    <h3>{storage.inventory.ready_items} of {storage.inventory.total_items} titles have files</h3>
                </div>
            </div>
            <div class="pw-inventory-types">
                <span><strong>{storage.inventory.movies}</strong> movies</span>
                <span><strong>{storage.inventory.series}</strong> series</span>
                <span><strong>{storage.inventory.anime}</strong> anime</span>
            </div>
            <dl class="pw-inventory-list">
                <div>
                    <dt>episodes on disk</dt>
                    <dd>{storage.inventory.ready_episodes} / {storage.inventory.total_episodes}</dd>
                </div>
                <div>
                    <dt>subtitle tracks</dt>
                    <dd>{storage.inventory.subtitle_tracks.toLocaleString()}</dd>
                </div>
                <div class:attention={storage.inventory.without_files > 0}>
                    <dt>titles without files</dt>
                    <dd>{storage.inventory.without_files}</dd>
                </div>
            </dl>
        </section>
    </div>

    <section class="pw-storage-filetypes">
        <div class="pw-storage-panel-head">
            <div>
                <p class="pw-storage-kicker">library composition</p>
                <h3>Files inside playable media</h3>
            </div>
        </div>
        <div class="pw-filetype-grid">
            {#each storage.file_types as bucket (bucket.key)}
                <div class="pw-filetype-card">
                    <i style={`background:${storageColor(bucket.key)}`}></i>
                    <span>{bucket.label}</span>
                    <strong>{fmtBytes(bucket.bytes)}</strong>
                    <small>{bucket.files.toLocaleString()} files</small>
                </div>
            {/each}
        </div>
    </section>

    <section class="pw-storage-library">
        <div class="pw-library-head">
            <div>
                <p class="pw-storage-kicker">title-level usage</p>
                <h3>Library footprint <span>{filteredStorage.length} / {storageItems.length}</span></h3>
            </div>
            <div class="pw-library-controls">
                <label class="pw-storage-search">
                    <Icon name="search" class="pw-storage-search-icon" strokeWidth={2} />
                    <input bind:value={storageQuery} placeholder="search titles" aria-label="Search storage titles" />
                </label>
                <select bind:value={filterType} aria-label="Filter storage titles">
                    <option value="all">all types</option>
                    <option value="movie">movies</option>
                    <option value="tv">TV series</option>
                    <option value="anime">anime</option>
                    <option value="empty">without files</option>
                </select>
                <select bind:value={sortBy} aria-label="Sort storage titles">
                    <option value="size">largest first</option>
                    <option value="date">newest first</option>
                    <option value="title">A-Z</option>
                </select>
            </div>
        </div>

        {#if filteredStorage.length === 0}
            <p class="pw-storage-empty">no titles match these filters</p>
        {:else}
            <div class="pw-storage-items">
                {#each filteredStorage as item (item.id)}
                    <article class="pw-storage-item" class:expanded={expandedMediaId === item.id}>
                        <button
                            class="pw-storage-item-main"
                            onclick={() => toggleDetails(item.id)}
                            aria-expanded={expandedMediaId === item.id}
                        >
                            {#if item.poster_url}
                                <img src={item.poster_url} alt="" />
                            {:else}
                                <div class="pw-storage-poster-placeholder"></div>
                            {/if}
                            <div class="pw-storage-item-title">
                                <div>
                                    <strong>{item.title}</strong>
                                    <span class:missing={!item.has_files}>
                                        {item.has_files ? 'on disk' : 'no files'}
                                    </span>
                                </div>
                                <small>{itemMeta(item)}</small>
                            </div>
                            <div class="pw-storage-item-metric files">
                                <span>size</span><strong>{fmtBytes(item.bytes)}</strong>
                            </div>
                            <div class="pw-storage-item-metric">
                                <span>files</span><strong>{item.files}</strong>
                            </div>
                            <div class="pw-storage-item-metric episodes">
                                <span>episodes</span>
                                <strong
                                    >{item.media_type === 'tv'
                                        ? `${item.episode_ready} / ${item.episode_total}`
                                        : 'movie'}</strong
                                >
                            </div>
                            <div class="pw-storage-item-metric subs">
                                <span>subtitles</span><strong>{item.subtitle_files}</strong>
                            </div>
                            <Icon
                                name="chevron-right"
                                class={`pw-storage-chevron ${expandedMediaId === item.id ? 'open' : ''}`}
                                strokeWidth={2}
                            />
                        </button>
                        {#if expandedMediaId === item.id}
                            <div class="pw-storage-item-details">
                                <div class="pw-item-breakdown">
                                    <span>
                                        <small>video</small>
                                        <strong>{fmtBytes(item.video_bytes)}</strong>
                                        <em>{item.video_files} files</em>
                                    </span>
                                    <span>
                                        <small>audio cache</small>
                                        <strong>{fmtBytes(item.audio_bytes)}</strong>
                                        <em>{item.audio_files} files</em>
                                    </span>
                                    <span>
                                        <small>subtitles</small>
                                        <strong>{fmtBytes(item.subtitle_bytes)}</strong>
                                        <em>{item.subtitle_files} files</em>
                                    </span>
                                    <span>
                                        <small>other</small>
                                        <strong>
                                            {fmtBytes(
                                                Math.max(
                                                    0,
                                                    item.bytes -
                                                        item.video_bytes -
                                                        item.audio_bytes -
                                                        item.subtitle_bytes
                                                )
                                            )}
                                        </strong>
                                        <em>
                                            {Math.max(
                                                0,
                                                item.files - item.video_files - item.audio_files - item.subtitle_files
                                            )}
                                            files
                                        </em>
                                    </span>
                                </div>
                                <div class="pw-item-actions">
                                    <a
                                        href={`/${item.media_type === 'tv' ? 'tv' : 'movie'}/${item.tmdb_id ?? item.id}`}
                                    >
                                        open title
                                    </a>
                                    <button onclick={() => deleteMedia(item.id, item.title)}
                                        >remove title and files</button
                                    >
                                    <code>{item.relative_path}</code>
                                </div>
                            </div>
                        {/if}
                    </article>
                {/each}
            </div>
        {/if}
    </section>
{:else}
    <div class="pw-storage-loading">building storage report...</div>
{/if}

<style>
    .pw-storage-hero,
    .pw-storage-panel,
    .pw-storage-filetypes,
    .pw-storage-library {
        border: 1px solid rgba(255, 255, 255, 0.075);
        background: #0e131b;
        border-radius: 14px;
    }

    .pw-storage-hero {
        position: relative;
        overflow: hidden;
        padding: 22px;
        margin-bottom: 16px;
        background: linear-gradient(135deg, rgba(69, 112, 184, 0.13), transparent 46%), #0e131b;
    }

    .pw-storage-hero::after {
        content: '';
        position: absolute;
        width: 240px;
        height: 240px;
        right: -100px;
        top: -130px;
        border-radius: 50%;
        border: 1px solid rgba(91, 141, 239, 0.1);
        box-shadow: 0 0 80px rgba(91, 141, 239, 0.08);
        pointer-events: none;
    }

    .pw-storage-hero-head,
    .pw-storage-panel-head,
    .pw-library-head,
    .pw-storage-title-line,
    .pw-capacity-legend,
    .pw-item-actions {
        display: flex;
        align-items: center;
    }

    .pw-storage-hero-head,
    .pw-storage-panel-head,
    .pw-library-head {
        justify-content: space-between;
        gap: 18px;
    }

    .pw-storage-kicker {
        margin: 0 0 5px;
        color: #6f7c90;
        font:
            600 10px/1.2 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        letter-spacing: 0.13em;
        text-transform: uppercase;
    }

    .pw-storage-title-line {
        gap: 10px;
    }

    .pw-storage-title-line h2,
    .pw-storage-panel-head h3,
    .pw-library-head h3 {
        margin: 0;
        color: #f1f3f7;
        letter-spacing: -0.025em;
    }

    .pw-storage-title-line h2 {
        font-size: clamp(22px, 3vw, 30px);
        line-height: 1.15;
        font-weight: 650;
    }

    .pw-storage-panel-head h3,
    .pw-library-head h3 {
        font-size: 15px;
        font-weight: 620;
    }

    .pw-library-head h3 span {
        margin-left: 6px;
        color: #657186;
        font:
            500 11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-storage-subtitle {
        margin: 7px 0 0;
        color: #7f8a9c;
        font:
            11px/1.5 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-storage-health {
        border: 1px solid rgba(72, 190, 144, 0.25);
        background: rgba(72, 190, 144, 0.1);
        color: #68caa2;
        border-radius: 999px;
        padding: 3px 8px;
        font:
            600 9px/1 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .pw-storage-health.warning {
        border-color: rgba(245, 158, 11, 0.28);
        background: rgba(245, 158, 11, 0.1);
        color: #f5b842;
    }

    .pw-storage-health.critical {
        border-color: rgba(239, 91, 91, 0.3);
        background: rgba(239, 91, 91, 0.1);
        color: #f07d7d;
    }

    .pw-storage-refresh {
        position: relative;
        z-index: 1;
        display: inline-flex;
        align-items: center;
        gap: 7px;
        border: 1px solid rgba(255, 255, 255, 0.09);
        border-radius: 7px;
        background: rgba(255, 255, 255, 0.025);
        color: #9aa5b6;
        padding: 7px 10px;
        font-size: 11px;
        cursor: pointer;
        transition:
            border-color 0.15s,
            color 0.15s,
            background 0.15s;
    }

    .pw-storage-refresh:hover {
        color: #e8ebf0;
        border-color: rgba(255, 255, 255, 0.16);
        background: rgba(255, 255, 255, 0.045);
    }

    .pw-storage-refresh:disabled {
        opacity: 0.55;
        cursor: wait;
    }

    .pw-capacity-track {
        display: flex;
        width: 100%;
        height: 14px;
        margin: 22px 0 10px;
        overflow: hidden;
        border: 1px solid rgba(255, 255, 255, 0.055);
        border-radius: 4px;
        background: #18202c;
    }

    .pw-capacity-track span {
        display: block;
        min-width: 0;
        height: 100%;
    }

    .pw-capacity-app {
        background: #5b8def;
        box-shadow: 8px 0 20px rgba(91, 141, 239, 0.2);
    }

    .pw-capacity-other {
        background: #354154;
    }

    .pw-capacity-legend {
        flex-wrap: wrap;
        gap: 8px 20px;
        color: #707c8f;
        font-size: 10px;
    }

    .pw-capacity-legend span {
        display: inline-flex;
        align-items: center;
        gap: 6px;
    }

    .pw-capacity-legend i {
        width: 6px;
        height: 6px;
        border-radius: 2px;
    }

    .pw-capacity-legend i.app {
        background: #5b8def;
    }

    .pw-capacity-legend i.other {
        background: #455267;
    }

    .pw-capacity-legend i.free {
        border: 1px solid #455267;
        background: transparent;
    }

    .pw-capacity-legend strong {
        color: #aeb7c5;
        font-weight: 600;
    }

    .pw-storage-summary {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 1px;
        margin-top: 20px;
        overflow: hidden;
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 9px;
        background: rgba(255, 255, 255, 0.06);
    }

    .pw-storage-summary > div,
    .pw-cache-action {
        min-height: 92px;
        background: #121924;
        padding: 14px 15px;
    }

    .pw-storage-summary span,
    .pw-storage-summary strong,
    .pw-storage-summary small,
    .pw-cache-action span,
    .pw-cache-action small {
        display: block;
    }

    .pw-storage-summary span,
    .pw-cache-action span {
        color: #7f8b9d;
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    .pw-storage-summary strong {
        margin-top: 7px;
        color: #f0f2f5;
        font-size: 18px;
        letter-spacing: -0.025em;
    }

    .pw-storage-summary small,
    .pw-cache-action small {
        margin-top: 4px;
        color: #616d80;
        font-size: 10px;
    }

    .pw-cache-action {
        border: 0;
        color: inherit;
        text-align: left;
        cursor: pointer;
        transition: background 0.15s;
    }

    .pw-cache-action:not(:disabled):hover {
        background: rgba(151, 45, 57, 0.22);
    }

    .pw-cache-action:not(:disabled) span {
        color: #e67e87;
    }

    .pw-cache-action:disabled {
        cursor: default;
    }

    .pw-storage-grid {
        display: grid;
        grid-template-columns: minmax(0, 1.7fr) minmax(300px, 0.8fr);
        gap: 16px;
        margin-bottom: 16px;
    }

    .pw-storage-panel,
    .pw-storage-filetypes,
    .pw-storage-library {
        padding: 20px;
    }

    .pw-storage-panel-head > span {
        color: #626e80;
        font:
            10px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-storage-bars {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 18px 24px;
        margin-top: 22px;
    }

    .pw-storage-bar-label {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 12px;
        margin-bottom: 7px;
    }

    .pw-storage-bar-label span {
        display: flex;
        align-items: center;
        min-width: 0;
        gap: 7px;
        color: #aab3c1;
        font-size: 11px;
    }

    .pw-storage-bar-label i {
        width: 7px;
        height: 7px;
        border-radius: 2px;
        flex: none;
    }

    .pw-storage-bar-label strong {
        color: #e7eaf0;
        font:
            600 11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-storage-bar-row > small {
        display: block;
        margin-top: 7px;
        padding-top: 7px;
        border-top: 1px solid #1d2633;
        color: #566174;
        font-size: 9px;
    }

    .pw-inventory-types {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 7px;
        margin: 20px 0 16px;
    }

    .pw-inventory-types span {
        border: 1px solid rgba(255, 255, 255, 0.055);
        border-radius: 6px;
        background: #121924;
        color: #697587;
        padding: 8px;
        font-size: 9px;
        text-align: center;
    }

    .pw-inventory-types strong {
        display: block;
        margin-bottom: 2px;
        color: #e7eaf0;
        font-size: 14px;
    }

    .pw-inventory-list {
        margin: 0;
    }

    .pw-inventory-list > div {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 9px 0;
        border-top: 1px solid rgba(255, 255, 255, 0.055);
    }

    .pw-inventory-list dt {
        color: #707c8f;
        font-size: 10px;
    }

    .pw-inventory-list dd {
        margin: 0;
        color: #cbd1db;
        font:
            600 11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-inventory-list .attention dt,
    .pw-inventory-list .attention dd {
        color: #e6a256;
    }

    .pw-storage-filetypes {
        margin-bottom: 16px;
    }

    .pw-filetype-grid {
        display: grid;
        grid-template-columns: repeat(5, minmax(0, 1fr));
        gap: 10px;
        margin-top: 16px;
    }

    .pw-filetype-card {
        position: relative;
        overflow: hidden;
        min-width: 0;
        border: 1px solid rgba(255, 255, 255, 0.055);
        border-radius: 8px;
        background: #121924;
        padding: 12px 12px 11px;
    }

    .pw-filetype-card > i {
        position: absolute;
        inset: 0 auto 0 0;
        width: 2px;
    }

    .pw-filetype-card span,
    .pw-filetype-card strong,
    .pw-filetype-card small {
        display: block;
    }

    .pw-filetype-card span {
        color: #788497;
        font-size: 10px;
    }

    .pw-filetype-card strong {
        margin-top: 5px;
        overflow: hidden;
        color: #e6e9ee;
        font-size: 14px;
        text-overflow: ellipsis;
    }

    .pw-filetype-card small {
        margin-top: 2px;
        color: #566174;
        font-size: 9px;
    }

    .pw-library-head {
        align-items: flex-end;
    }

    .pw-library-controls {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .pw-library-controls select,
    .pw-storage-search {
        height: 34px;
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 7px;
        background: #121924;
        color: #aab3c1;
        font-size: 10px;
    }

    .pw-library-controls select {
        padding: 0 28px 0 10px;
        outline: none;
    }

    .pw-storage-search {
        display: flex;
        align-items: center;
        width: 185px;
        gap: 7px;
        padding: 0 10px;
    }

    :global(.pw-storage-search-icon) {
        width: 14px;
        height: 14px;
        flex: none;
        color: #596578;
    }

    .pw-storage-search input {
        min-width: 0;
        border: 0;
        background: transparent;
        color: #e4e7ec;
        font-size: 11px;
        outline: none;
    }

    .pw-storage-search input::placeholder {
        color: #566174;
    }

    .pw-storage-items {
        display: grid;
        gap: 7px;
        margin-top: 18px;
    }

    .pw-storage-item {
        overflow: hidden;
        border: 1px solid rgba(255, 255, 255, 0.055);
        border-radius: 9px;
        background: #111822;
        transition:
            border-color 0.15s,
            background 0.15s;
    }

    .pw-storage-item:hover,
    .pw-storage-item.expanded {
        border-color: rgba(116, 145, 194, 0.25);
        background: #131b27;
    }

    .pw-storage-item-main {
        display: grid;
        grid-template-columns: 44px minmax(180px, 1fr) 95px 64px 82px 70px 16px;
        width: 100%;
        align-items: center;
        gap: 13px;
        border: 0;
        background: transparent;
        padding: 10px 12px;
        text-align: left;
        cursor: pointer;
    }

    .pw-storage-item-main > img,
    .pw-storage-poster-placeholder {
        width: 44px;
        height: 60px;
        border-radius: 5px;
        object-fit: cover;
        background: #202a38;
    }

    .pw-storage-item-title {
        min-width: 0;
    }

    .pw-storage-item-title > div {
        display: flex;
        align-items: center;
        min-width: 0;
        gap: 8px;
    }

    .pw-storage-item-title strong {
        min-width: 0;
        overflow: hidden;
        color: #e9ecf1;
        font-size: 13px;
        font-weight: 620;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pw-storage-item-title span {
        flex: none;
        border-radius: 999px;
        background: rgba(58, 170, 127, 0.11);
        color: #65c89e;
        padding: 3px 6px;
        font:
            600 8px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        text-transform: uppercase;
    }

    .pw-storage-item-title span.missing {
        background: rgba(230, 162, 86, 0.1);
        color: #dda05c;
    }

    .pw-storage-item-title small {
        display: block;
        margin-top: 6px;
        overflow: hidden;
        color: #596578;
        font:
            9px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pw-storage-item-metric span,
    .pw-storage-item-metric strong {
        display: block;
    }

    .pw-storage-item-metric span {
        margin-bottom: 4px;
        color: #536074;
        font-size: 8px;
        letter-spacing: 0.07em;
        text-transform: uppercase;
    }

    .pw-storage-item-metric strong {
        color: #bfc6d2;
        font:
            600 10px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    :global(.pw-storage-chevron) {
        width: 14px;
        height: 14px;
        color: #566174;
        transition: transform 0.18s;
    }

    :global(.pw-storage-chevron.open) {
        transform: rotate(90deg);
    }

    .pw-storage-item-details {
        border-top: 1px solid rgba(255, 255, 255, 0.055);
        padding: 13px 14px 14px 69px;
    }

    .pw-item-breakdown {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 8px;
    }

    .pw-item-breakdown > span {
        border-radius: 6px;
        background: rgba(255, 255, 255, 0.025);
        padding: 9px 10px;
    }

    .pw-item-breakdown small,
    .pw-item-breakdown strong,
    .pw-item-breakdown em {
        display: block;
    }

    .pw-item-breakdown small {
        color: #596578;
        font-size: 8px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    .pw-item-breakdown strong {
        margin-top: 3px;
        color: #cdd3dd;
        font-size: 11px;
    }

    .pw-item-breakdown em {
        margin-top: 1px;
        color: #536074;
        font-size: 8px;
        font-style: normal;
    }

    .pw-item-actions {
        gap: 9px;
        margin-top: 11px;
    }

    .pw-item-actions a,
    .pw-item-actions button {
        border: 0;
        border-radius: 5px;
        background: #202a38;
        color: #aeb7c5;
        padding: 6px 9px;
        font-size: 9px;
        text-decoration: none;
        cursor: pointer;
    }

    .pw-item-actions button {
        background: transparent;
        color: #d87882;
    }

    .pw-item-actions code {
        min-width: 0;
        margin-left: auto;
        overflow: hidden;
        color: #536074;
        font-size: 9px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pw-storage-empty,
    .pw-storage-loading {
        padding: 50px 20px;
        color: #626e80;
        font:
            11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        text-align: center;
    }

    .pw-storage-loading {
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 12px;
        background: #0e131b;
    }

    :global(.pw-admin-spin) {
        animation: pw-admin-spin 0.8s linear infinite;
    }

    @keyframes pw-admin-spin {
        to {
            transform: rotate(360deg);
        }
    }

    @media (max-width: 900px) {
        .pw-storage-grid {
            grid-template-columns: 1fr;
        }

        .pw-storage-summary {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .pw-filetype-grid {
            grid-template-columns: repeat(3, minmax(0, 1fr));
        }

        .pw-storage-item-main {
            grid-template-columns: 44px minmax(140px, 1fr) 85px 54px 72px 16px;
        }

        .pw-storage-item-metric.subs {
            display: none;
        }
    }

    @media (max-width: 680px) {
        .pw-storage-hero,
        .pw-storage-panel,
        .pw-storage-filetypes,
        .pw-storage-library {
            padding: 15px;
        }

        .pw-storage-hero-head {
            align-items: flex-start;
        }

        .pw-storage-refresh {
            padding: 7px;
        }

        .pw-storage-refresh:not(:hover) {
            font-size: 0;
            gap: 0;
        }

        .pw-storage-title-line {
            align-items: flex-start;
            flex-direction: column;
            gap: 7px;
        }

        .pw-storage-subtitle {
            max-width: 240px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }

        .pw-capacity-legend {
            gap: 7px 12px;
        }

        .pw-storage-bars {
            grid-template-columns: 1fr;
            gap: 15px;
        }

        .pw-filetype-grid {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .pw-library-head {
            align-items: stretch;
            flex-direction: column;
        }

        .pw-library-controls {
            display: grid;
            grid-template-columns: 1fr 1fr;
        }

        .pw-storage-search {
            grid-column: 1 / -1;
            width: 100%;
        }

        .pw-library-controls select {
            width: 100%;
        }

        .pw-storage-item-main {
            grid-template-columns: 40px minmax(0, 1fr) auto 14px;
            gap: 10px;
            padding: 9px;
        }

        .pw-storage-item-main > img,
        .pw-storage-poster-placeholder {
            width: 40px;
            height: 54px;
        }

        .pw-storage-item-metric.files,
        .pw-storage-item-metric.episodes,
        .pw-storage-item-metric.subs {
            display: none;
        }

        .pw-storage-item-metric {
            text-align: right;
        }

        .pw-storage-item-title span {
            display: none;
        }

        .pw-storage-item-details {
            padding: 11px;
        }

        .pw-item-breakdown {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .pw-item-actions {
            align-items: flex-start;
            flex-wrap: wrap;
        }

        .pw-item-actions code {
            width: 100%;
            margin: 2px 0 0;
        }
    }

    @media (max-width: 420px) {
        .pw-storage-summary > div,
        .pw-cache-action {
            min-height: 78px;
        }

        .pw-inventory-types {
            gap: 4px;
        }

        .pw-storage-item-title small {
            max-width: 170px;
        }
    }
</style>
