<script lang="ts">
    import { onMount } from 'svelte';
    import { fade } from 'svelte/transition';
    import { page } from '$app/state';
    import { goto } from '$app/navigation';
    import {
        api,
        type MediaWithEpisodes,
        type TmdbDetail,
        type TmdbEpisode,
        type TmdbSearchItem,
        type TmdbVideo,
        type User,
        type WatchProgress
    } from '$lib/api';
    import TopBar from '$lib/components/TopBar.svelte';
    import TrailerModal from '$lib/components/TrailerModal.svelte';
    import TorrentPicker from '$lib/components/TorrentPicker.svelte';
    import CastModal from '$lib/components/CastModal.svelte';
    import CastRow from '$lib/components/CastRow.svelte';
    import CollectionControl from '$lib/components/CollectionControl.svelte';
    import TmdbShelf from '$lib/components/TmdbShelf.svelte';
    import { t, plural } from '$lib/i18n.svelte';

    type WsUpdate = {
        download_id: string;
        media_id: string;
        status: string;
        progress: number;
        state: string | null;
        title: string | null;
        episodes: { season: number; episode: number; progress: number; name: string }[];
        seeds?: number;
        peers?: number;
        dlspeed?: number;
    };
    type EpisodePick = {
        season: number;
        episode: number;
        display: number;
        episodeId?: string;
        query: string;
    };

    let user = $state<User | null>(null);
    let media = $state<MediaWithEpisodes | null>(null);
    let detail = $state<TmdbDetail | null>(null);
    let loading = $state(true);
    let err = $state('');
    let adding = $state(false);
    let activeDl = $state<WsUpdate | null>(null);
    let dlByEp = $state<Map<string, WsUpdate>>(new Map());
    let epProgress = $state<Map<string, number>>(new Map());
    let watched = $state<Map<string, WatchProgress>>(new Map());
    let ws: WebSocket | null = null;

    let activeSeason = $state<number | null>(null);
    let episodes = $state<TmdbEpisode[]>([]);
    let loadingEps = $state(false);
    const cache = new Map<number, TmdbEpisode[]>();

    let trailer = $state<TmdbVideo | null>(null);
    let trailerOpen = $state(false);
    let similar = $state<TmdbSearchItem[]>([]);
    let pickerEpisode = $state<EpisodePick | null>(null);
    let castOpenId = $state<number | null>(null);
    let pickerSeasonPack = $state(false);
    let synopsisOpen = $state(false);

    const tmdbId = $derived(Number(page.params.tmdb_id));

    const omdbSeasons = $derived(detail?.omdb_seasons ?? null);
    const omdbOverride = $derived(!!omdbSeasons && omdbSeasons.length > 1);
    function omdbOffset(virtualSeason: number): number {
        if (!omdbSeasons) return 0;
        let offset = 0;
        for (let i = 0; i < virtualSeason - 1 && i < omdbSeasons.length; i++) offset += omdbSeasons[i];
        return offset;
    }
    function realSeasonFor(virtualSeason: number): number {
        return omdbOverride ? 1 : virtualSeason;
    }
    function displayEpisodeFor(virtualSeason: number, episode: number): number {
        return omdbOverride ? episode - omdbOffset(virtualSeason) : episode;
    }
    function epBelongsToVirtual(realSeason: number, realEp: number, virtual: number): boolean {
        if (!omdbOverride || !omdbSeasons) return realSeason === virtual;
        if (realSeason !== 1) return false;
        const lo = omdbOffset(virtual) + 1;
        const hi = omdbOffset(virtual) + (omdbSeasons[virtual - 1] ?? 0);
        return realEp >= lo && realEp <= hi;
    }
    function epBelongsToView(realSeason: number, realEp: number, virtual: number): boolean {
        if (!omdbOverride || !omdbSeasons) return realSeason === virtual;
        if (epBelongsToVirtual(realSeason, realEp, virtual)) return true;
        if (realSeason !== virtual) return false;
        if (virtual === 1 && realEp > (omdbSeasons[0] ?? 0)) return false;
        return true;
    }
    function episodeSlots(virtualSeason: number, episode: number) {
        const display = displayEpisodeFor(virtualSeason, episode);
        const slots = [{ season: virtualSeason, episode: display }];
        const real = realSeasonFor(virtualSeason);
        if (real !== virtualSeason || display !== episode) {
            slots.push({ season: real, episode });
        }
        return slots;
    }
    function epKey(season: number, episode: number) {
        return `S${String(season).padStart(2, '0')}E${String(episode).padStart(2, '0')}`;
    }
    function findEpisodeFile(virtualSeason: number, episode: number) {
        for (const slot of episodeSlots(virtualSeason, episode)) {
            const hit = media?.episodes?.find((e) => e.season === slot.season && e.episode === slot.episode);
            if (hit) return hit;
        }
        return undefined;
    }
    function seasonEpisodeFiles(season: number) {
        return (media?.episodes ?? []).filter((e) => epBelongsToView(e.season, e.episode, season));
    }
    function progressForEpisode(virtualSeason: number, episode: number) {
        for (const slot of episodeSlots(virtualSeason, episode)) {
            const hit = epProgress.get(epKey(slot.season, slot.episode));
            if (hit !== undefined) return hit;
        }
        return undefined;
    }
    function downloadForEpisode(virtualSeason: number, episode: number) {
        for (const slot of episodeSlots(virtualSeason, episode)) {
            const hit = dlByEp.get(epKey(slot.season, slot.episode));
            if (hit) return hit;
        }
        return undefined;
    }

    let loadToken = 0;

    function storageKeyFor(id: number) {
        return `pw-show-${id}-season`;
    }

    function readSavedSeasonFor(id: number): number | null {
        try {
            const v = localStorage.getItem(storageKeyFor(id));
            return v ? Number(v) : null;
        } catch {
            return null;
        }
    }

    function saveSeason(n: number) {
        try {
            localStorage.setItem(storageKeyFor(tmdbId), String(n));
        } catch {}
    }

    onMount(async () => {
        try {
            user = await api.me();
        } catch {
            goto('/login');
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

    onMount(() => {
        const onFocus = () => {
            if (media) loadWatched();
        };
        window.addEventListener('focus', onFocus);
        return () => window.removeEventListener('focus', onFocus);
    });

    async function loadTmdb(id: number) {
        const token = ++loadToken;
        media = null;
        detail = null;
        trailer = null;
        similar = [];
        activeSeason = null;
        episodes = [];
        cache.clear();
        epProgress = new Map();
        watched = new Map();
        activeDl = null;
        err = '';
        loading = true;

        try {
            const [d, m] = await Promise.all([
                api.tmdbDetail('tv', id),
                api.getMediaByTmdb('tv', id).catch(() => null)
            ]);
            if (loadToken !== token) return;
            detail = d;
            media = m;

            api.tmdbVideos('tv', id)
                .then((vs) => {
                    if (loadToken === token) trailer = vs[0] ?? null;
                })
                .catch(() => {});

            api.tmdbSimilar('tv', id)
                .then((s) => {
                    if (loadToken === token) similar = s;
                })
                .catch(() => {});

            const rawSeasons = d?.seasons ?? [];
            const ovr = d?.omdb_seasons ?? null;
            const pickFrom: number[] =
                ovr && ovr.length > 1 ? ovr.map((_, i) => i + 1) : rawSeasons.map((s) => s.season_number);
            const localSeasons = Array.from(new Set((m?.episodes ?? []).map((e) => e.season))).filter((s) => s > 0);
            for (const s of localSeasons) {
                if (!pickFrom.includes(s)) pickFrom.push(s);
            }
            pickFrom.sort((a, b) => a - b);
            if (pickFrom.length > 0) {
                const saved = readSavedSeasonFor(id);
                const validSaved = saved !== null && pickFrom.includes(saved);
                activeSeason = validSaved ? saved : pickFrom[0];

                const idle: (cb: () => void) => void =
                    typeof (window as any).requestIdleCallback === 'function'
                        ? (cb) => (window as any).requestIdleCallback(cb)
                        : (cb) => setTimeout(cb, 400);
                idle(() => {
                    if (loadToken === token) prefetchOtherSeasons(id);
                });
            }
        } catch (caught) {
            if (loadToken !== token) return;
            err = caught instanceof Error ? caught.message : 'failed to load';
        } finally {
            if (loadToken === token) loading = false;
        }

        if (loadToken === token) {
            seedActiveDownload();
            loadWatched();
        }
    }

    async function loadWatched() {
        if (!media) return;
        try {
            const list = await api.listMediaProgress(media.id);
            const m = new Map<string, WatchProgress>();
            for (const p of list) {
                if (p.episode_id) m.set(p.episode_id, p);
            }
            watched = m;
        } catch {}
    }

    async function toggleWatched(epId: string, duration: number | null | undefined) {
        if (!media) return;
        const cur = watched.get(epId);
        const next = !cur?.completed;
        const optimistic = new Map(watched);
        if (next) {
            const dur = duration ?? cur?.duration ?? 1;
            optimistic.set(epId, {
                id: cur?.id ?? '',
                user_id: '',
                media_id: media.id,
                episode_id: epId,
                position: dur,
                duration: dur,
                completed: true,
                dismissed: false,
                updated_at: new Date().toISOString()
            });
        } else {
            optimistic.delete(epId);
        }
        watched = optimistic;
        try {
            await api.markWatched(media.id, next, epId, duration ?? undefined);
        } catch {
            loadWatched();
        }
    }

    function fmtTs(s: number): string {
        if (!isFinite(s) || s <= 0) return '0:00';
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        const sec = Math.floor(s % 60);
        if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
        return `${m}:${String(sec).padStart(2, '0')}`;
    }

    function airingLabel(dateStr: string | null | undefined): string | null {
        if (!dateStr) return null;
        const air = new Date(`${dateStr}T00:00:00`);
        if (isNaN(air.getTime())) return null;
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        const days = Math.round((air.getTime() - today.getTime()) / 86_400_000);
        if (days <= 0) return null;
        if (days === 1) return 'tomorrow';
        if (days < 14) return `in ${days} days`;
        return dateStr;
    }

    async function seedActiveDownload() {
        if (!media) return;
        try {
            const dls = await api.listDownloads();
            const mine = dls.find(
                (d) =>
                    d.media_id === media!.id &&
                    (d.status === 'queued' || d.status === 'downloading' || d.status === 'processing')
            );
            if (mine) {
                activeDl = {
                    download_id: mine.id,
                    media_id: mine.media_id,
                    status: mine.status,
                    progress: mine.progress,
                    state: mine.state,
                    title: mine.title,
                    episodes: []
                };
            }
        } catch {}
    }

    function connectWs() {
        if (ws) return;
        const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
        ws = new WebSocket(`${proto}//${location.host}/ws/downloads`);
        ws.onmessage = (ev) => {
            try {
                const updates: WsUpdate[] = JSON.parse(ev.data);
                let dirty = false;
                for (const u of updates) {
                    if (!media || u.media_id !== media.id) continue;
                    if (u.status === 'gone' || u.status === 'complete') {
                        activeDl = null;
                        epProgress = new Map();
                        const purged = new Map(dlByEp);
                        for (const [k, dl] of purged) {
                            if (dl.download_id === u.download_id) purged.delete(k);
                        }
                        dlByEp = purged;
                        api.getMediaByTmdb('tv', tmdbId)
                            .then((m) => {
                                media = m;
                            })
                            .catch(() => {});
                        continue;
                    }
                    activeDl = u;
                    const next = new Map(epProgress);
                    const nextMap = new Map(dlByEp);
                    for (const e of u.episodes ?? []) {
                        const k = `S${String(e.season).padStart(2, '0')}E${String(e.episode).padStart(2, '0')}`;
                        next.set(k, e.progress);
                        nextMap.set(k, u);
                    }
                    epProgress = next;
                    dlByEp = nextMap;
                    dirty = true;
                }
                if (dirty) {
                    api.getMediaByTmdb('tv', tmdbId)
                        .then((m) => {
                            media = m;
                        })
                        .catch(() => {});
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
            const added = await api.addMedia(tmdbId, 'tv');
            media = { ...added, episodes: [] };
        } catch (e) {
            err = e instanceof Error ? e.message : 'failed to add';
        } finally {
            adding = false;
        }
    }

    async function prefetchOtherSeasons(id: number) {
        if (!detail?.seasons) return;
        for (const s of detail.seasons) {
            if (cache.has(s.season_number)) continue;
            if (s.season_number === activeSeason) continue;
            try {
                const eps = await api.tmdbSeason(id, s.season_number);
                if (id !== tmdbId) return;
                cache.set(s.season_number, eps);
            } catch {}
        }
    }

    $effect(() => {
        if (!detail || activeSeason === null) return;
        const season = activeSeason;
        const id = tmdbId;
        saveSeason(season);

        const fetchSeason = omdbOverride ? 1 : season;
        const cached = cache.get(fetchSeason);

        const applySlice = (raw: TmdbEpisode[]) => {
            if (!omdbOverride || !omdbSeasons) return raw;
            const start = omdbOffset(season);
            const end = start + (omdbSeasons[season - 1] ?? 0);
            return raw.slice(start, end);
        };

        if (cached) {
            episodes = applySlice(cached);
            loadingEps = false;
            return;
        }

        loadingEps = true;
        episodes = [];
        api.tmdbSeason(id, fetchSeason)
            .then((eps) => {
                if (id !== tmdbId || activeSeason !== season) return;
                cache.set(fetchSeason, eps);
                episodes = applySlice(eps);
            })
            .catch(() => {})
            .finally(() => {
                if (id === tmdbId && activeSeason === season) loadingEps = false;
            });
    });

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

    let magnetBusy = $state(false);
    async function addCustomMagnet() {
        if (magnetBusy) return;
        const m = prompt('paste magnet link or torrent url:');
        if (!m || !m.trim()) return;
        magnetBusy = true;
        const showName = media?.title ?? detail?.title ?? 'show';
        try {
            const dl = await api.createDownload({
                magnet: m.trim(),
                media_id: media?.id,
                tmdb_id: tmdbId,
                media_type: 'tv',
                season: activeSeason ?? null,
                title: `${showName} S${String(activeSeason ?? 0).padStart(2, '0')} (custom)`
            });
            activeDl = {
                download_id: dl.id ?? 'pending',
                media_id: dl.media_id,
                status: dl.status ?? 'queued',
                progress: 0,
                state: 'starting',
                title: dl.title ?? `${showName} S${String(activeSeason ?? 0).padStart(2, '0')} (custom)`,
                episodes: []
            };
            try {
                media = await api.getMediaByTmdb('tv', tmdbId);
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
        if (!media || uploadBusy) return;
        uploadInput?.click();
    }

    async function onUploadPicked(ev: Event) {
        const input = ev.target as HTMLInputElement;
        const file = input.files?.[0];
        input.value = '';
        if (!file || !media) return;

        uploadBusy = true;
        uploadPct = 0;
        const fd = new FormData();
        fd.append('file', file);
        if (activeSeason != null) fd.append('season', String(activeSeason));

        try {
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
                media = await api.getMediaByTmdb('tv', tmdbId);
            } catch {}
        } catch (e) {
            alert(e instanceof Error ? e.message : 'upload failed');
        } finally {
            uploadBusy = false;
            uploadPct = 0;
        }
    }

    async function removeEpisodeFile(episodeId: string) {
        if (!confirm('remove this episode file?')) return;
        try {
            await api.deleteEpisodeFile(episodeId);
            if (media) {
                media = await api.getMediaByTmdb('tv', tmdbId);
            }
        } catch (e) {
            alert(e instanceof Error ? e.message : 'remove failed');
        }
    }

    async function cancelActiveDownload(downloadId?: string) {
        const id = downloadId ?? activeDl?.download_id;
        if (!id) return;
        if (!confirm('cancel this download? partial files will be deleted.')) return;
        if (activeDl?.download_id === id) activeDl = null;
        epProgress = new Map();
        const purged = new Map(dlByEp);
        for (const [k, dl] of purged) {
            if (dl.download_id === id) purged.delete(k);
        }
        dlByEp = purged;
        if (id !== 'pending') {
            try {
                await api.cancelDownload(id);
            } catch (e) {
                alert(e instanceof Error ? e.message : 'cancel failed');
            }
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

    async function removeSeason() {
        if (!media || activeSeason === null) return;
        const seasonEps = seasonEpisodeFiles(activeSeason);
        const ready = seasonEps.filter((e) => !!e.file_path).length;
        const total = seasonEps.length + (activeDl ? 1 : 0);
        if (total === 0) return;
        const msg =
            ready > 0
                ? `remove ${ready} downloaded episode${ready === 1 ? '' : 's'} + cancel any in-flight download for season ${activeSeason}?`
                : `cancel the in-flight download for season ${activeSeason}?`;
        if (!confirm(msg)) return;

        try {
            const dls = await api.listDownloads();
            const seasonEpIds = new Set(seasonEps.map((e) => e.id));
            for (const d of dls) {
                if (d.media_id !== media.id) continue;
                if (d.status !== 'queued' && d.status !== 'downloading' && d.status !== 'processing') continue;
                if (d.episode_id) {
                    if (!seasonEpIds.has(d.episode_id)) continue;
                } else {
                    const m = d.title?.match(/[Ss](\d{1,2})(?!\d)/);
                    if (!m) continue;
                    if (parseInt(m[1], 10) !== activeSeason) continue;
                }
                try {
                    await api.cancelDownload(d.id);
                } catch {}
            }
        } catch {}

        for (const ep of seasonEps) {
            try {
                await api.deleteEpisodeFile(ep.id);
            } catch {}
        }

        if (activeDl && activeDl.media_id === media.id) {
            const m = activeDl.title?.match(/[Ss](\d{1,2})(?!\d)/);
            const dlSeason = m ? parseInt(m[1], 10) : null;
            if (dlSeason === activeSeason) {
                activeDl = null;
                epProgress = new Map();
            }
        }
        try {
            media = await api.getMediaByTmdb('tv', tmdbId);
        } catch {}
    }

    function readyInSeason(season: number): number {
        return seasonEpisodeFiles(season).filter((e) => !!e.file_path).length;
    }

    const seasonDownloadActive = $derived.by(() => {
        if (activeSeason === null) return false;
        if (activeDl && media && activeDl.media_id === media.id) {
            const m = activeDl.title?.match(/[Ss](\d{1,2})(?!\d)/);
            const dlSeason = m ? parseInt(m[1], 10) : null;
            if (dlSeason !== null && dlSeason === activeSeason) return true;
        }
        return seasonEpisodeFiles(activeSeason).some((e) => e.status === 'processing' || e.status === 'downloading');
    });
    const seasonHasAny = $derived(activeSeason !== null && (readyInSeason(activeSeason) > 0 || seasonDownloadActive));

    const isAdmin = $derived(user?.role === 'admin');
    const addedByMe = $derived(!!media && !!user && media.added_by === user.id);
    const canManage = $derived(isAdmin || addedByMe);
    const inLibrary = $derived(!!media);
    const title = $derived(detail?.title ?? media?.title ?? 'show');
    const year = $derived(detail?.year ?? (media?.year ? String(media.year) : null));
    const poster = $derived(detail?.poster_url ?? media?.poster_url ?? null);
    const collectionKind = $derived(media?.is_anime || detail?.is_anime ? 'anime' : 'tv');
    const overview = $derived(detail?.overview ?? media?.overview ?? null);
    const overviewShort = $derived(
        overview && overview.length > 320 ? overview.slice(0, 320).trimEnd() + '… ' : (overview ?? '')
    );
    const localSeasonCounts = $derived.by(() => {
        const counts = new Map<number, number>();
        for (const ep of media?.episodes ?? []) {
            if (ep.season <= 0) continue;
            counts.set(ep.season, (counts.get(ep.season) ?? 0) + 1);
        }
        return counts;
    });
    const seasons = $derived.by(() => {
        const base =
            omdbOverride && omdbSeasons
                ? omdbSeasons.map((count, i) => ({
                      season_number: i + 1,
                      name: `Season ${i + 1}`,
                      episode_count: count,
                      overview: null,
                      poster_url: null
                  }))
                : (detail?.seasons ?? []).map((s) => ({ ...s }));

        const seen = new Set(base.map((s) => s.season_number));
        for (const [season, count] of localSeasonCounts) {
            const existing = base.find((s) => s.season_number === season);
            if (existing) {
                existing.episode_count = Math.max(existing.episode_count, count);
                continue;
            }
            base.push({
                season_number: season,
                name: `Season ${season}`,
                episode_count: count,
                overview: null,
                poster_url: null
            });
        }
        return base.sort((a, b) => a.season_number - b.season_number);
    });

    function openEpisodePicker(ep: number, episodeId?: string) {
        if (activeSeason === null) return;
        if (!detail && !media) return;

        const season = activeSeason;
        const display = displayEpisodeFor(season, ep);
        pickerEpisode = {
            season,
            episode: ep,
            display,
            episodeId,
            query: `${title} S${String(season).padStart(2, '0')}E${String(display).padStart(2, '0')}`
        };
    }
    const expectedEps = $derived(seasons.find((s) => s.season_number === activeSeason)?.episode_count ?? 0);
    const seasonComplete = $derived(
        activeSeason !== null && expectedEps > 0 && readyInSeason(activeSeason) >= expectedEps
    );
    const orphanEps = $derived.by(() => {
        if (activeSeason === null || !media?.episodes) return [];
        const shown = new Set<string>();
        for (const ep of episodes) {
            for (const slot of episodeSlots(activeSeason, ep.episode_number)) {
                shown.add(epKey(slot.season, slot.episode));
            }
        }
        return media.episodes
            .filter((e) => epBelongsToView(e.season, e.episode, activeSeason!) && !!e.file_path)
            .filter((e) => !shown.has(epKey(e.season, e.episode)))
            .sort((a, b) => a.episode - b.episode);
    });
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
                                    <span class="pw-dt-bc-arrow">{'>'}</span> shows <span class="pw-dt-bc-sep">/</span>
                                    <b>{title}</b>
                                </div>
                                <h1 class="pw-dt-title">{title}</h1>
                                <div class="pw-dt-meta">
                                    {#if year}<span class="pw-dt-meta-text">{year}</span>{/if}
                                    {#if year && seasons.length > 0}<span class="pw-dt-meta-sep"></span>{/if}
                                    {#if seasons.length > 0}<span class="pw-dt-meta-text"
                                            >{plural('media.seasons', seasons.length)}</span
                                        >{/if}
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
                                            href={`https://www.themoviedb.org/tv/${tmdbId}`}
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
                                            href={`https://www.filmweb.pl/search#/serial?query=${encodeURIComponent(title)}`}
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
                                    {#if inLibrary}
                                        <button class="pw-v1-btn-watch">
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"
                                                ><path d="M8 5v14l11-7z" /></svg
                                            >
                                            Watch
                                        </button>
                                    {:else}
                                        <button class="pw-v1-btn-watch" onclick={addToLibrary} disabled={adding}>
                                            <svg
                                                width="14"
                                                height="14"
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
                                            {adding ? 'adding...' : t('media.add_to_library')}
                                        </button>
                                    {/if}
                                    <CollectionControl
                                        {tmdbId}
                                        kind={collectionKind}
                                        {title}
                                        {year}
                                        posterUrl={poster}
                                        backdropUrl={detail?.backdrop_url ?? null}
                                    />
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

                                {#if media?.subs_processing}
                                    <div class="pw-v1-subs-note">{t('media.subs_processing')}</div>
                                {/if}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <section class="pw-v1-body">
                {#if seasons.length > 0}
                    <div class="pw-season-row" style="margin-top: 4px;">
                        <div class="pw-season-picker-mobile">
                            <select
                                class="pw-season-select"
                                value={activeSeason}
                                onchange={(e) => (activeSeason = Number((e.currentTarget as HTMLSelectElement).value))}
                                aria-label="season"
                            >
                                {#each seasons as s (s.season_number)}
                                    {@const ready = readyInSeason(s.season_number)}
                                    <option value={s.season_number}>
                                        {s.name}
                                        {ready > 0 ? `(${ready}/${s.episode_count})` : `(${s.episode_count})`}
                                    </option>
                                {/each}
                            </select>
                            <svg
                                class="pw-season-picker-caret"
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <polyline points="6 9 12 15 18 9" />
                            </svg>
                        </div>
                        <div class="pw-season-tabs">
                            {#each seasons as s (s.season_number)}
                                {@const ready = readyInSeason(s.season_number)}
                                <button
                                    class="pw-season-tab"
                                    class:is-active={activeSeason === s.season_number}
                                    onclick={() => (activeSeason = s.season_number)}
                                >
                                    {s.name}
                                    <span class="pw-season-count" class:is-ready={ready > 0}>
                                        {ready > 0 ? `${ready}/${s.episode_count}` : s.episode_count}
                                    </span>
                                </button>
                            {/each}
                        </div>
                        {#if activeSeason !== null && user}
                            {@const ready = readyInSeason(activeSeason)}
                            <div class="pw-season-actions">
                                {#if !seasonComplete && !seasonDownloadActive}
                                    <button
                                        class="pw-season-action"
                                        onclick={() => (pickerSeasonPack = true)}
                                        aria-label="download season"
                                    >
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
                                        {t('media.download_season')}
                                    </button>
                                {/if}
                                {#if seasonHasAny && canManage}
                                    <button
                                        class="pw-season-icon is-danger"
                                        onclick={removeSeason}
                                        aria-label={`${t('media.remove_season')}${ready > 0 ? ` (${ready})` : ''}`}
                                        title={`${t('media.remove_season')}${ready > 0 ? ` (${ready})` : ''}`}
                                    >
                                        <svg
                                            width="14"
                                            height="14"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><polyline points="3 6 5 6 21 6" /><path
                                                d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"
                                            /><path d="M10 11v6M14 11v6" /><path
                                                d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"
                                            /></svg
                                        >
                                    </button>
                                {/if}
                                <button
                                    class="pw-season-icon"
                                    onclick={addCustomMagnet}
                                    disabled={magnetBusy}
                                    aria-label="paste magnet link"
                                    title="paste magnet link"
                                >
                                    <svg
                                        width="14"
                                        height="14"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="2"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path
                                            d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
                                        /></svg
                                    >
                                </button>
                                {#if canManage}
                                    <button
                                        class="pw-season-icon"
                                        onclick={uploadLocalFile}
                                        disabled={uploadBusy}
                                        aria-label="upload local file"
                                        title={uploadBusy ? `uploading ${uploadPct}%` : 'upload local file'}
                                    >
                                        {#if uploadBusy}
                                            <span style="font-size:10px;font-weight:600;">{uploadPct}%</span>
                                        {:else}
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
                                                    points="17 8 12 3 7 8"
                                                /><line x1="12" y1="3" x2="12" y2="15" /></svg
                                            >
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
                            </div>
                        {/if}
                    </div>

                    {#key activeSeason}
                        <div in:fade={{ duration: 180, delay: 60 }} out:fade={{ duration: 100 }}>
                            {#if loadingEps && expectedEps > 0}
                                <div class="pw-episodes-grid">
                                    {#each Array(expectedEps) as _, i (i)}
                                        <div class="pw-ep-card pw-ep-skel">
                                            <div class="pw-ep-still"></div>
                                            <div class="pw-ep-body">
                                                <div class="pw-ep-skel-line" style="width: 70%;"></div>
                                                <div class="pw-ep-skel-line" style="width: 45%; margin-top: 6px;"></div>
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {:else if loadingEps}
                                <div class="pw-empty-tag" style="padding: 24px 0;">// loading episodes...</div>
                            {:else if episodes.length === 0 && orphanEps.length === 0}
                                <div class="pw-empty-tag" style="padding: 24px 0;">// no episodes</div>
                            {:else}
                                <div class="pw-episodes-grid">
                                    {#each episodes as ep (ep.episode_number)}
                                        {@const stored = findEpisodeFile(activeSeason ?? -1, ep.episode_number)}
                                        {@const displayEp = displayEpisodeFor(activeSeason ?? 1, ep.episode_number)}
                                        {@const epDl = progressForEpisode(activeSeason ?? -1, ep.episode_number)}
                                        {@const epDlInfo = downloadForEpisode(activeSeason ?? -1, ep.episode_number)}
                                        {@const isReady = !!stored?.file_path}
                                        {@const isProcessing =
                                            stored?.status === 'processing' ||
                                            (epDl !== undefined && epDl >= 1 && !isReady)}
                                        {@const isDownloading =
                                            !isReady && !isProcessing && epDl !== undefined && epDl < 1}
                                        {@const dlPct = epDl !== undefined ? Math.round(epDl * 100) : 0}
                                        {@const ringR = 18}
                                        {@const ringC = 2 * Math.PI * ringR}
                                        {@const wp = stored ? watched.get(stored.id) : undefined}
                                        {@const isWatched = wp?.completed === true}
                                        {@const watchPct =
                                            wp && wp.duration > 0
                                                ? Math.min(100, Math.round((wp.position / wp.duration) * 100))
                                                : 0}
                                        {@const inProgress =
                                            isReady && !isWatched && wp && wp.position > 5 && watchPct < 95}
                                        {@const airing = airingLabel(ep.air_date)}
                                        <div
                                            class="pw-ep-card"
                                            class:is-ready={isReady}
                                            class:is-watched={isWatched}
                                            onclick={() => {
                                                if (isReady && stored) goto(`/watch/${media!.id}?ep=${stored.id}`);
                                            }}
                                            onkeydown={(e) => {
                                                if ((e.key === 'Enter' || e.key === ' ') && isReady && stored) {
                                                    e.preventDefault();
                                                    goto(`/watch/${media!.id}?ep=${stored.id}`);
                                                }
                                            }}
                                            role="button"
                                            aria-disabled={!isReady}
                                            tabindex={isReady ? 0 : -1}
                                        >
                                            <div class="pw-ep-still">
                                                {#if ep.still_url}<img
                                                        src={ep.still_url}
                                                        alt={ep.name}
                                                        loading="lazy"
                                                        decoding="async"
                                                        class:dim={isDownloading || isProcessing || isWatched}
                                                    />{/if}
                                                <div class="pw-ep-num">E{String(displayEp).padStart(2, '0')}</div>

                                                {#if isReady}
                                                    <div class="pw-ep-overlay pw-ep-overlay-hover">
                                                        <div class="pw-ep-play-big">
                                                            <svg
                                                                width="22"
                                                                height="22"
                                                                viewBox="0 0 24 24"
                                                                fill="currentColor"><path d="M8 5v14l11-7z" /></svg
                                                            >
                                                        </div>
                                                    </div>
                                                    <button
                                                        class="pw-ep-mark"
                                                        class:on={isWatched}
                                                        onclick={(e) => {
                                                            e.stopPropagation();
                                                            toggleWatched(stored.id, stored.duration);
                                                        }}
                                                        aria-label={isWatched
                                                            ? t('media.mark_unwatched')
                                                            : t('media.mark_watched')}
                                                    >
                                                        {isWatched ? t('media.watched') : t('media.mark_watched')}
                                                    </button>
                                                    {#if canManage}
                                                        <button
                                                            class="pw-ep-x"
                                                            onclick={(e) => {
                                                                e.stopPropagation();
                                                                removeEpisodeFile(stored.id);
                                                            }}
                                                            aria-label="remove file"
                                                            title="remove file"
                                                        >
                                                            <svg
                                                                width="11"
                                                                height="11"
                                                                viewBox="0 0 24 24"
                                                                fill="none"
                                                                stroke="currentColor"
                                                                stroke-width="2.4"
                                                                stroke-linecap="round"
                                                                stroke-linejoin="round"
                                                                ><path d="M6 18L18 6M6 6l12 12" /></svg
                                                            >
                                                        </button>
                                                    {/if}
                                                    {#if isWatched || inProgress}
                                                        <div class="pw-ep-progress" class:done={isWatched}>
                                                            <div
                                                                class="pw-ep-progress-fill"
                                                                style="width: {isWatched ? 100 : watchPct}%"
                                                            ></div>
                                                        </div>
                                                    {/if}
                                                {:else if isProcessing}
                                                    <div class="pw-ep-overlay pw-ep-overlay-busy">
                                                        <div class="pw-ep-busy">
                                                            <div class="pw-ep-spin"></div>
                                                            <span class="pw-ep-dots">...</span>
                                                        </div>
                                                    </div>
                                                {:else if isDownloading}
                                                    <div class="pw-ep-overlay pw-ep-overlay-busy">
                                                        <div class="pw-ep-busy">
                                                            <svg class="pw-ep-ring" viewBox="0 0 48 48">
                                                                <circle
                                                                    cx="24"
                                                                    cy="24"
                                                                    r={ringR}
                                                                    fill="none"
                                                                    stroke="rgba(255,255,255,0.12)"
                                                                    stroke-width="3"
                                                                />
                                                                <circle
                                                                    cx="24"
                                                                    cy="24"
                                                                    r={ringR}
                                                                    fill="none"
                                                                    stroke="var(--pw-accent)"
                                                                    stroke-width="3"
                                                                    stroke-linecap="round"
                                                                    stroke-dasharray={ringC}
                                                                    stroke-dashoffset={ringC * (1 - dlPct / 100)}
                                                                />
                                                            </svg>
                                                            <span class="pw-ep-pct">{dlPct}%</span>
                                                            {#if epDlInfo && (epDlInfo.seeds !== undefined || epDlInfo.dlspeed !== undefined)}
                                                                <span class="pw-ep-stats">
                                                                    {#if epDlInfo.seeds !== undefined}{epDlInfo.seeds}↓{/if}
                                                                    {#if epDlInfo.peers !== undefined}
                                                                        · {epDlInfo.peers}↑{/if}
                                                                    {#if epDlInfo.dlspeed !== undefined}
                                                                        · {fmtSpeed(epDlInfo.dlspeed)}{/if}
                                                                </span>
                                                            {/if}
                                                        </div>
                                                    </div>
                                                    {#if canManage}
                                                        <button
                                                            class="pw-ep-x"
                                                            onclick={(e) => {
                                                                e.stopPropagation();
                                                                cancelActiveDownload(epDlInfo?.download_id);
                                                            }}
                                                            aria-label="cancel download"
                                                            title="cancel download"
                                                        >
                                                            <svg
                                                                width="11"
                                                                height="11"
                                                                viewBox="0 0 24 24"
                                                                fill="none"
                                                                stroke="currentColor"
                                                                stroke-width="2.4"
                                                                stroke-linecap="round"
                                                                stroke-linejoin="round"
                                                                ><path d="M6 18L18 6M6 6l12 12" /></svg
                                                            >
                                                        </button>
                                                    {/if}
                                                {:else if user}
                                                    <button
                                                        class="pw-ep-dl"
                                                        onclick={(e) => {
                                                            e.stopPropagation();
                                                            openEpisodePicker(ep.episode_number);
                                                        }}
                                                        aria-label="download episode"
                                                    >
                                                        <svg
                                                            width="14"
                                                            height="14"
                                                            viewBox="0 0 24 24"
                                                            fill="none"
                                                            stroke="currentColor"
                                                            stroke-width="2"
                                                            stroke-linecap="round"
                                                            stroke-linejoin="round"
                                                            ><path
                                                                d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"
                                                            /><polyline points="7 10 12 15 17 10" /><line
                                                                x1="12"
                                                                y1="15"
                                                                x2="12"
                                                                y2="3"
                                                            /></svg
                                                        >
                                                    </button>
                                                {/if}
                                                {#if airing && !isReady && !isProcessing && !isDownloading}
                                                    <div class="pw-ep-airing-pill">{airing}</div>
                                                {/if}
                                            </div>
                                            <div class="pw-ep-body">
                                                <div class="pw-ep-title">{ep.name}</div>
                                                {#if inProgress && wp}
                                                    <p class="pw-ep-ts">{fmtTs(wp.position)} / {fmtTs(wp.duration)}</p>
                                                {/if}
                                                {#if ep.overview}<p class="pw-ep-overview">{ep.overview}</p>{/if}
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {/if}

                            {#if orphanEps.length > 0}
                                <div class="pw-orphan-divider">
                                    <span class="pw-orphan-label">downloaded, not in tmdb ({orphanEps.length})</span>
                                </div>
                                <div class="pw-episodes-grid">
                                    {#each orphanEps as ep (ep.id)}
                                        {@const wp = watched.get(ep.id)}
                                        {@const isWatched = wp?.completed === true}
                                        {@const watchPct =
                                            wp && wp.duration > 0
                                                ? Math.min(100, Math.round((wp.position / wp.duration) * 100))
                                                : 0}
                                        {@const inProgress = !isWatched && wp && wp.position > 5 && watchPct < 95}
                                        {@const shortSrc =
                                            ep.source_name ??
                                            `S${String(ep.season).padStart(2, '0')}E${String(ep.episode).padStart(2, '0')}`}
                                        <div
                                            class="pw-ep-card is-ready"
                                            class:is-watched={isWatched}
                                            onclick={() => {
                                                if (media) goto(`/watch/${media.id}?ep=${ep.id}`);
                                            }}
                                            onkeydown={(e) => {
                                                if ((e.key === 'Enter' || e.key === ' ') && media) {
                                                    e.preventDefault();
                                                    goto(`/watch/${media.id}?ep=${ep.id}`);
                                                }
                                            }}
                                            role="button"
                                            tabindex="0"
                                        >
                                            <div class="pw-ep-still pw-ep-still-orphan">
                                                <div class="pw-ep-num">E{String(ep.episode).padStart(2, '0')}</div>
                                                <div class="pw-ep-overlay pw-ep-overlay-hover">
                                                    <div class="pw-ep-play-big">
                                                        <svg
                                                            width="22"
                                                            height="22"
                                                            viewBox="0 0 24 24"
                                                            fill="currentColor"><path d="M8 5v14l11-7z" /></svg
                                                        >
                                                    </div>
                                                </div>
                                                <button
                                                    class="pw-ep-mark"
                                                    class:on={isWatched}
                                                    onclick={(e) => {
                                                        e.stopPropagation();
                                                        toggleWatched(ep.id, ep.duration);
                                                    }}
                                                    aria-label={isWatched
                                                        ? t('media.mark_unwatched')
                                                        : t('media.mark_watched')}
                                                >
                                                    {isWatched ? t('media.watched') : t('media.mark_watched')}
                                                </button>
                                                {#if canManage}
                                                    <button
                                                        class="pw-ep-x"
                                                        onclick={(e) => {
                                                            e.stopPropagation();
                                                            removeEpisodeFile(ep.id);
                                                        }}
                                                        aria-label="remove file"
                                                        title="remove file"
                                                    >
                                                        <svg
                                                            width="11"
                                                            height="11"
                                                            viewBox="0 0 24 24"
                                                            fill="none"
                                                            stroke="currentColor"
                                                            stroke-width="2.4"
                                                            stroke-linecap="round"
                                                            stroke-linejoin="round"
                                                            ><path d="M6 18L18 6M6 6l12 12" /></svg
                                                        >
                                                    </button>
                                                {/if}
                                                {#if isWatched || inProgress}
                                                    <div class="pw-ep-progress" class:done={isWatched}>
                                                        <div
                                                            class="pw-ep-progress-fill"
                                                            style="width: {isWatched ? 100 : watchPct}%"
                                                        ></div>
                                                    </div>
                                                {/if}
                                            </div>
                                            <div class="pw-ep-body">
                                                <div class="pw-ep-title">{shortSrc}</div>
                                                {#if inProgress && wp}
                                                    <p class="pw-ep-ts">{fmtTs(wp.position)} / {fmtTs(wp.duration)}</p>
                                                {/if}
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    {/key}
                {/if}

                {#if similar.length > 0}
                    <TmdbShelf title={t('media.similar')} items={similar} />
                {/if}

                {#if detail?.cast?.length}
                    <CastRow cast={detail.cast} onSelect={(id) => (castOpenId = id)} />
                {/if}
            </section>
        {/if}

        <div style="height: 60px;"></div>
    </div>

    {#if trailerOpen && trailer}
        <TrailerModal videoKey={trailer.key} onClose={() => (trailerOpen = false)} />
    {/if}

    {#if castOpenId !== null}
        <CastModal personId={castOpenId} onClose={() => (castOpenId = null)} />
    {/if}

    {#if pickerSeasonPack && (detail || media) && activeSeason !== null}
        <TorrentPicker
            query={`${title} S${String(activeSeason).padStart(2, '0')}`}
            mediaId={media?.id}
            {tmdbId}
            mediaType="tv"
            season={activeSeason}
            imdbId={detail?.imdb_id ?? undefined}
            kind={collectionKind}
            onClose={() => (pickerSeasonPack = false)}
            onStarted={async (t) => {
                if (media) {
                    activeDl = {
                        download_id: 'pending',
                        media_id: media.id,
                        status: 'queued',
                        progress: 0,
                        state: 'starting',
                        title: t.title,
                        episodes: []
                    };
                }
                try {
                    media = await api.getMediaByTmdb('tv', tmdbId);
                } catch {}
                seedActiveDownload();
            }}
        />
    {/if}

    {#if pickerEpisode}
        <TorrentPicker
            query={pickerEpisode?.query ?? title}
            mediaId={media?.id}
            {tmdbId}
            mediaType="tv"
            episodeId={pickerEpisode?.episodeId}
            season={pickerEpisode?.season}
            episode={pickerEpisode?.display}
            imdbId={detail?.imdb_id ?? undefined}
            kind={collectionKind}
            onClose={() => (pickerEpisode = null)}
            onStarted={async (t) => {
                const pick = pickerEpisode;
                if (pick) {
                    const k = epKey(pick.season, pick.display);
                    const next = new Map(epProgress);
                    next.set(k, 0.01);
                    epProgress = next;
                }
                if (media) {
                    activeDl = {
                        download_id: 'pending',
                        media_id: media.id,
                        status: 'queued',
                        progress: 0,
                        state: 'starting',
                        title: t.title,
                        episodes: []
                    };
                }
                try {
                    media = await api.getMediaByTmdb('tv', tmdbId);
                } catch {}
                seedActiveDownload();
            }}
        />
    {/if}
{/if}
