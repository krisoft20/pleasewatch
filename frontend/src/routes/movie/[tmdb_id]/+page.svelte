<script lang="ts">
    import { onMount } from 'svelte';
    import { page } from '$app/state';
    import { goto } from '$app/navigation';
    import {
        api,
        type MediaWithEpisodes,
        type TmdbCollection,
        type TmdbDetail,
        type TmdbSearchItem,
        type TmdbVideo,
        type User
    } from '$lib/api';
    import TopBar from '$lib/components/TopBar.svelte';
    import TrailerModal from '$lib/components/TrailerModal.svelte';
    import TorrentPicker from '$lib/components/TorrentPicker.svelte';
    import CastModal from '$lib/components/CastModal.svelte';
    import CastRow from '$lib/components/CastRow.svelte';
    import CollectionRow from '$lib/components/CollectionRow.svelte';
    import CollectionControl from '$lib/components/CollectionControl.svelte';
    import TmdbShelf from '$lib/components/TmdbShelf.svelte';
    import { t } from '$lib/i18n.svelte';

    type WsUpdate = {
        download_id: string;
        media_id: string;
        status: string;
        progress: number;
        state: string | null;
        title: string | null;
        seeds?: number;
        peers?: number;
        dlspeed?: number;
    };

    let user = $state<User | null>(null);
    let media = $state<MediaWithEpisodes | null>(null);
    let detail = $state<TmdbDetail | null>(null);
    let trailer = $state<TmdbVideo | null>(null);
    let trailerOpen = $state(false);
    let collection = $state<TmdbCollection | null>(null);
    let similar = $state<TmdbSearchItem[]>([]);
    let pickerOpen = $state(false);
    let castOpenId = $state<number | null>(null);
    let synopsisOpen = $state(false);
    let loading = $state(true);
    let err = $state('');
    let adding = $state(false);
    let activeDl = $state<WsUpdate | null>(null);
    let ws: WebSocket | null = null;

    const tmdbId = $derived(Number(page.params.tmdb_id));

    let loadToken = 0;

    onMount(async () => {
        try {
            user = await api.me();
        } catch {
            goto('/login', { replaceState: true });
            return;
        }
        connectWs();
    });

    $effect(() => {
        const id = tmdbId;
        if (!Number.isFinite(id)) {
            err = 'invalid tmdb id';
            loading = false;
            return;
        }
        void loadTmdb(id);
    });

    $effect(() => {
        const busy = (!!activeDl && !media?.file_path) || !!media?.subs_processing;
        if (!busy) return;
        const token = loadToken;
        const id = setInterval(() => {
            api.getMediaByTmdb('movie', tmdbId)
                .then((m) => {
                    if (loadToken === token) media = m;
                })
                .catch(() => {});
        }, 12000);
        return () => clearInterval(id);
    });

    async function loadTmdb(id: number) {
        const token = ++loadToken;
        media = null;
        detail = null;
        trailer = null;
        collection = null;
        similar = [];
        activeDl = null;
        err = '';
        loading = true;

        try {
            const [d, m] = await Promise.all([
                api.tmdbDetail('movie', id),
                api.getMediaByTmdb('movie', id).catch(() => null)
            ]);
            if (loadToken !== token) return;
            detail = d;
            media = m;

            api.tmdbVideos('movie', id)
                .then((vs) => {
                    if (loadToken === token) trailer = vs[0] ?? null;
                })
                .catch(() => {});

            const collId = d?.belongs_to_collection?.id;
            if (collId) {
                api.tmdbCollection(collId)
                    .then((c) => {
                        if (loadToken === token) collection = c;
                    })
                    .catch(() => {});
            }

            api.tmdbSimilar('movie', id)
                .then((s) => {
                    if (loadToken === token) similar = s;
                })
                .catch(() => {});
        } catch (caught) {
            if (loadToken !== token) return;
            err = caught instanceof Error ? caught.message : 'failed to load';
        } finally {
            if (loadToken === token) loading = false;
        }
    }

    function connectWs() {
        if (ws) return;
        const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
        ws = new WebSocket(`${proto}//${location.host}/ws/downloads`);
        ws.onmessage = (ev) => {
            try {
                const updates: WsUpdate[] = JSON.parse(ev.data);
                for (const u of updates) {
                    if (!media || u.media_id !== media.id) continue;
                    if (u.status === 'gone' || u.status === 'complete') {
                        activeDl = null;
                        api.getMediaByTmdb('movie', tmdbId)
                            .then((m) => {
                                media = m;
                            })
                            .catch(() => {});
                        continue;
                    }
                    activeDl = u;
                }
            } catch {}
        };
        ws.onclose = () => {
            ws = null;
            setTimeout(connectWs, 2000);
        };
        ws.onerror = () => {
            try {
                ws?.close();
            } catch {}
        };
    }

    async function addToLibrary() {
        if (media) return;
        adding = true;
        try {
            const added = await api.addMedia(tmdbId, 'movie');
            media = { ...added, episodes: [] };
        } catch (e) {
            err = e instanceof Error ? e.message : 'failed to add';
        } finally {
            adding = false;
        }
    }

    async function remove() {
        if (!media) return;
        if (!confirm(`remove "${media.title}" from library?`)) return;
        try {
            await api.deleteMedia(media.id);
            goto('/');
        } catch (caught) {
            err = caught instanceof Error ? caught.message : 'delete failed';
        }
    }

    async function cancelActiveDownload() {
        if (!activeDl) return;
        if (!confirm('cancel this download? partial files will be deleted.')) return;
        const id = activeDl.download_id;
        activeDl = null;
        if (id !== 'pending') {
            try {
                await api.cancelDownload(id);
            } catch (e) {
                alert(e instanceof Error ? e.message : 'cancel failed');
            }
        }
    }

    let magnetBusy = $state(false);
    async function addCustomMagnet() {
        if (magnetBusy || activeDl) return;
        const m = prompt('paste magnet link or torrent url:');
        if (!m || !m.trim()) return;
        magnetBusy = true;
        const showName = media?.title ?? detail?.title ?? 'movie';
        try {
            const dl = await api.createDownload({
                magnet: m.trim(),
                media_id: media?.id,
                tmdb_id: tmdbId,
                media_type: 'movie',
                title: `${showName} (custom)`
            });
            activeDl = {
                download_id: dl.id ?? 'pending',
                media_id: dl.media_id,
                status: dl.status ?? 'queued',
                progress: 0,
                state: 'starting',
                title: dl.title ?? `${showName} (custom)`
            };
            try {
                media = await api.getMediaByTmdb('movie', tmdbId);
            } catch {}
        } catch (caught) {
            alert(caught instanceof Error ? caught.message : 'failed to add');
        } finally {
            magnetBusy = false;
        }
    }

    let uploadInput = $state<HTMLInputElement | null>(null);
    let uploadBusy = $state(false);
    let uploadPct = $state(0);

    function uploadLocalFile() {
        if (uploadBusy) return;
        uploadInput?.click();
    }

    async function onUploadPicked(ev: Event) {
        const input = ev.target as HTMLInputElement;
        const file = input.files?.[0];
        input.value = '';
        if (!file) return;
        if (!media) {
            try {
                const added = await api.addMedia(tmdbId, 'movie');
                media = { ...added, episodes: [] };
            } catch (e) {
                alert(e instanceof Error ? e.message : 'failed to add to library');
                return;
            }
        }
        uploadBusy = true;
        uploadPct = 0;
        try {
            const fd = new FormData();
            fd.append('file', file);
            await new Promise<void>((resolve, reject) => {
                const xhr = new XMLHttpRequest();
                xhr.open('POST', `/api/media/${media!.id}/upload-local`);
                xhr.withCredentials = true;
                xhr.upload.onprogress = (e) => {
                    if (e.lengthComputable) uploadPct = Math.round((e.loaded / e.total) * 100);
                };
                xhr.onload = () => {
                    if (xhr.status >= 200 && xhr.status < 300) resolve();
                    else reject(new Error(`upload ${xhr.status}: ${xhr.responseText}`));
                };
                xhr.onerror = () => reject(new Error('upload failed'));
                xhr.send(fd);
            });
            try {
                media = await api.getMediaByTmdb('movie', tmdbId);
            } catch {}
        } catch (e) {
            alert(e instanceof Error ? e.message : 'upload failed');
        } finally {
            uploadBusy = false;
            uploadPct = 0;
        }
    }

    function fmtSpeed(bps?: number): string {
        if (!bps || bps <= 0) return '0 B/s';
        const u = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
        let i = 0;
        let n = bps;
        while (n >= 1024 && i < u.length - 1) {
            n /= 1024;
            i++;
        }
        return `${n.toFixed(n >= 10 || i === 0 ? 0 : 1)} ${u[i]}`;
    }

    const isAdmin = $derived(user?.role === 'admin');
    const addedByMe = $derived(!!media && !!user && media.added_by === user.id);
    const canManage = $derived(isAdmin || addedByMe);
    const inLibrary = $derived(!!media);
    const title = $derived(detail?.title ?? media?.title ?? 'movie');
    const year = $derived(detail?.year ?? (media?.year ? String(media.year) : null));
    const poster = $derived(detail?.poster_url ?? media?.poster_url ?? null);
    const overview = $derived(detail?.overview ?? media?.overview ?? null);
    const overviewShort = $derived(
        overview && overview.length > 320 ? overview.slice(0, 320).trimEnd() + '… ' : (overview ?? '')
    );
    const score = $derived(detail?.vote_average ? Math.round(detail.vote_average * 10) : 0);
    const scoreColor = $derived(score >= 70 ? '#22c55e' : score >= 50 ? '#eab308' : '#ef4444');
    const scoreR = 20;
    const scoreC = $derived(2 * Math.PI * scoreR);
    const scoreOffset = $derived(scoreC - (score / 100) * scoreC);
</script>

<svelte:head><title>{title} - pleasewatch</title></svelte:head>

{#if user}
    <div class="pw-page">
        <TopBar {user} back={true} />

        {#if loading}
            <section class="pw-section pw-empty">
                <div class="pw-empty-card"><div class="pw-empty-tag">// loading...</div></div>
            </section>
        {:else if err}
            <section class="pw-section">
                <div class="pw-error" style="max-width: 480px;">{err}</div>
            </section>
        {:else if detail || media}
            <div class="pw-v1-hero-wrap">
                <div class="pw-v1-hero-bg">
                    {#if detail?.backdrop_url}
                        <img class="pw-v1-hero-img pw-dt-hero-img" src={detail.backdrop_url} alt="" />
                    {:else if poster}
                        <img
                            class="pw-v1-hero-img"
                            src={poster}
                            alt=""
                            style="filter: blur(12px); transform: scale(1.25); opacity: 0.3;"
                        />
                    {/if}
                    <div class="pw-v1-hero-grad-x"></div>
                    <div class="pw-v1-hero-grad-y"></div>

                    <div class="pw-v1-hero-content">
                        <div class="pw-dt-layout">
                            <div class="pw-dt-cover-col">
                                <div class="pw-dt-cover">
                                    {#if poster}
                                        <img src={poster} alt={title} />
                                    {/if}
                                </div>
                            </div>
                            <div class="pw-dt-info">
                                <div class="pw-dt-bc">
                                    <span class="pw-dt-bc-arrow">{'>'}</span> movies <span class="pw-dt-bc-sep">/</span>
                                    <b>{title}</b>
                                </div>
                                <h1 class="pw-dt-title">{title}</h1>
                                <div class="pw-dt-meta">
                                    {#if year}<span class="pw-dt-meta-text">{year}</span>{/if}
                                    {#if year && detail?.runtime}<span class="pw-dt-meta-sep"></span>{/if}
                                    {#if detail?.runtime}<span class="pw-dt-meta-text">{detail.runtime} min</span>{/if}
                                    {#if detail?.genres?.length}
                                        <div class="pw-dt-tags pw-dt-tags-inline">
                                            {#each detail.genres.slice(0, 6) as g (g)}
                                                <span class="pw-dt-tag">{g}</span>
                                            {/each}
                                        </div>
                                    {/if}
                                </div>

                                <div class="pw-v1-score-row">
                                    {#if score > 0}
                                        <div class="pw-v1-score-ring">
                                            <svg viewBox="0 0 48 48">
                                                <circle
                                                    cx="24"
                                                    cy="24"
                                                    r={scoreR}
                                                    fill="rgba(0,0,0,0.5)"
                                                    stroke="rgba(255,255,255,0.1)"
                                                    stroke-width="3"
                                                />
                                                <circle
                                                    cx="24"
                                                    cy="24"
                                                    r={scoreR}
                                                    fill="none"
                                                    stroke={scoreColor}
                                                    stroke-width="3"
                                                    stroke-linecap="round"
                                                    stroke-dasharray={scoreC}
                                                    stroke-dashoffset={scoreOffset}
                                                />
                                            </svg>
                                            <span class="pw-v1-score-text">{score}%</span>
                                        </div>
                                        <span class="pw-v1-score-label">{t('media.user_score')}</span>
                                        <span class="pw-v1-ext-sep" aria-hidden="true"></span>
                                    {/if}
                                    <div class="pw-v1-ext-pills">
                                        <a
                                            class="pw-v1-ext-pill"
                                            href={`https://www.themoviedb.org/movie/${tmdbId}`}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            aria-label="open on TMDB"
                                            title="open on TMDB"
                                        >
                                            <img
                                                class="pw-v1-ext-logo pw-v1-ext-logo-tmdb"
                                                src="https://www.themoviedb.org/assets/2/v4/logos/v2/blue_square_2-d537fb228cf3ded904ef09b136fe3fec72548ebc1fea3fbbd1ad9e36364db38b.svg"
                                                alt="TMDB"
                                                loading="lazy"
                                                decoding="async"
                                            />
                                        </a>
                                        {#if detail?.imdb_id}
                                            <a
                                                class="pw-v1-ext-pill"
                                                href={`https://www.imdb.com/title/${detail.imdb_id}/`}
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                aria-label="open on IMDb"
                                                title="open on IMDb"
                                            >
                                                <img
                                                    class="pw-v1-ext-logo pw-v1-ext-logo-imdb"
                                                    src="https://upload.wikimedia.org/wikipedia/commons/6/69/IMDB_Logo_2016.svg"
                                                    alt="IMDb"
                                                    loading="lazy"
                                                    decoding="async"
                                                />
                                            </a>
                                        {/if}
                                        <a
                                            class="pw-v1-ext-pill pw-v1-ext-pill-filmweb"
                                            href={`https://www.filmweb.pl/search#/film?query=${encodeURIComponent(title)}`}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            aria-label="search on Filmweb"
                                            title="search on Filmweb"
                                        >
                                            <img
                                                class="pw-v1-ext-logo pw-v1-ext-logo-filmweb"
                                                src="https://fwcdn.pl/prt/static/images/fw/icons2/filmweb-touchbar-icon.svg"
                                                alt="Filmweb"
                                                loading="lazy"
                                                decoding="async"
                                            />
                                        </a>
                                    </div>
                                </div>

                                {#if overview}
                                    <p class="pw-dt-synopsis">
                                        {synopsisOpen ? overview : overviewShort}
                                        {#if overview.length > 320}
                                            <button
                                                class="pw-dt-readmore"
                                                onclick={() => (synopsisOpen = !synopsisOpen)}
                                            >
                                                {synopsisOpen ? 'show less' : 'read more'}
                                            </button>
                                        {/if}
                                    </p>
                                {/if}

                                <div class="pw-dt-actions">
                                    {#if inLibrary && media?.file_path}
                                        <button
                                            class="pw-v1-btn-watch"
                                            onclick={() => goto(`/watch/${media!.id}`, { replaceState: true })}
                                        >
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M8 5v14l11-7z" /></svg
                                            >
                                            Watch
                                        </button>
                                    {:else if activeDl}
                                        {@const dlPct = Math.round(activeDl.progress * 100)}
                                        {@const isProcessing =
                                            activeDl.progress >= 1 || activeDl.status === 'processing'}
                                        <button class="pw-v1-btn-watch pw-v1-btn-dl" disabled>
                                            <div class="pw-v1-btn-dl-bar" style="width: {dlPct}%"></div>
                                            <span class="pw-v1-btn-dl-text">
                                                {#if isProcessing}
                                                    Processing...
                                                {:else}
                                                    Downloading {dlPct}%
                                                {/if}
                                            </span>
                                        </button>
                                        {#if canManage}
                                            <button
                                                class="pw-v1-btn-lib"
                                                onclick={cancelActiveDownload}
                                                title="cancel download"
                                            >
                                                <svg
                                                    width="13"
                                                    height="13"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"><path d="M6 18L18 6M6 6l12 12" /></svg
                                                >
                                                Cancel
                                            </button>
                                        {/if}
                                    {:else if user}
                                        <button class="pw-v1-btn-watch" onclick={() => (pickerOpen = true)}>
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
                                            Download
                                        </button>
                                        <button
                                            class="pw-v1-btn-lib"
                                            onclick={addCustomMagnet}
                                            disabled={magnetBusy || !!activeDl}
                                            title="paste magnet link"
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
                                                ><path
                                                    d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"
                                                /><path
                                                    d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
                                                /></svg
                                            >
                                            {magnetBusy ? '...' : 'Magnet'}
                                        </button>
                                        {#if canManage || !inLibrary}
                                            <button
                                                class="pw-v1-btn-lib"
                                                onclick={uploadLocalFile}
                                                disabled={uploadBusy}
                                                title={uploadBusy ? `uploading ${uploadPct}%` : 'upload local file'}
                                            >
                                                {#if uploadBusy}
                                                    <span style="font-size:11px;font-weight:600;">{uploadPct}%</span>
                                                {:else}
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
                                                    Upload
                                                {/if}
                                            </button>
                                            <input
                                                bind:this={uploadInput}
                                                type="file"
                                                accept="video/*,.mkv,.mp4,.avi,.webm,.m4v,.mov"
                                                style="display:none"
                                                onchange={onUploadPicked}
                                            />
                                        {/if}
                                    {/if}
                                    <CollectionControl
                                        {tmdbId}
                                        kind="movie"
                                        {title}
                                        {year}
                                        posterUrl={poster}
                                        backdropUrl={detail?.backdrop_url ?? null}
                                    />
                                    {#if !inLibrary}
                                        <button class="pw-v1-btn-lib" onclick={addToLibrary} disabled={adding}>
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
                                            {t('media.add_to_library')}
                                        </button>
                                    {/if}
                                    {#if trailer}
                                        <button class="pw-v1-btn-lib" onclick={() => (trailerOpen = true)}>
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M8 5v14l11-7z" /></svg
                                            >
                                            {t('media.trailer')}
                                        </button>
                                    {/if}
                                    {#if inLibrary && canManage}
                                        <button class="pw-v1-btn-lib" onclick={remove}>
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" /></svg
                                            >
                                            In Library
                                        </button>
                                    {/if}
                                </div>

                                {#if media?.file_path && media?.subs_processing}
                                    <div class="pw-v1-subs-note">{t('media.subs_processing')}</div>
                                {/if}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {#if activeDl}
                <section class="pw-v1-body" style="padding-top: 12px; padding-bottom: 0;">
                    <div class="pw-dl-strip">
                        <div class="pw-dl-strip-head">
                            <span class="pw-dl-strip-tag">{activeDl.state ?? activeDl.status}</span>
                            <span class="pw-dl-strip-title">{activeDl.title ?? 'download in progress'}</span>
                            {#if activeDl.seeds !== undefined || activeDl.dlspeed !== undefined}
                                <span class="pw-dl-strip-stats">
                                    {#if activeDl.seeds !== undefined}{activeDl.seeds}↓{/if}
                                    {#if activeDl.peers !== undefined}
                                        · {activeDl.peers}↑{/if}
                                    {#if activeDl.dlspeed !== undefined}
                                        · {fmtSpeed(activeDl.dlspeed)}{/if}
                                </span>
                            {/if}
                            <span class="pw-dl-strip-pct">{Math.round(activeDl.progress * 100)}%</span>
                            {#if canManage}
                                <button
                                    class="pw-dl-strip-cancel"
                                    onclick={cancelActiveDownload}
                                    aria-label="cancel download"
                                    title="cancel download"
                                >
                                    <svg
                                        width="12"
                                        height="12"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="2.4"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"><path d="M6 18L18 6M6 6l12 12" /></svg
                                    >
                                </button>
                            {/if}
                        </div>
                        <div class="pw-dl-strip-bar">
                            <div class="pw-dl-strip-fill" style="width: {Math.round(activeDl.progress * 100)}%"></div>
                        </div>
                    </div>
                </section>
            {/if}

            {#if collection && collection.parts.length > 1}
                <section class="pw-v1-body">
                    <CollectionRow parts={collection.parts} collectionName={collection.name} currentTmdbId={tmdbId} />
                </section>
            {/if}

            {#if similar.length > 0}
                <section class="pw-v1-body">
                    <TmdbShelf title={t('media.similar')} items={similar} />
                </section>
            {/if}

            {#if detail?.cast?.length}
                <section class="pw-v1-body">
                    <CastRow cast={detail.cast} onSelect={(id) => (castOpenId = id)} />
                </section>
            {/if}
        {/if}

        <div style="height: 60px;"></div>
    </div>

    {#if trailerOpen && trailer}
        <TrailerModal videoKey={trailer.key} onClose={() => (trailerOpen = false)} />
    {/if}

    {#if castOpenId !== null}
        <CastModal personId={castOpenId} onClose={() => (castOpenId = null)} />
    {/if}

    {#if pickerOpen && (detail || media)}
        <TorrentPicker
            query={`${title}${year ? ` ${year}` : ''}`}
            mediaId={media?.id}
            {tmdbId}
            mediaType="movie"
            imdbId={detail?.imdb_id ?? undefined}
            kind="movie"
            onClose={() => (pickerOpen = false)}
            onStarted={(t) => {
                if (media) {
                    activeDl = {
                        download_id: 'pending',
                        media_id: media.id,
                        status: 'queued',
                        progress: 0,
                        state: 'starting',
                        title: t.title
                    };
                }
                api.getMediaByTmdb('movie', tmdbId)
                    .then((m) => (media = m))
                    .catch(() => {});
            }}
        />
    {/if}
{/if}
