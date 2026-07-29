<script lang="ts">
    import { onMount } from 'svelte';
    import { page } from '$app/state';
    import { goto } from '$app/navigation';
    import {
        api,
        type MediaWithEpisodes,
        type Episode,
        type TmdbDetail,
        type TmdbEpisode,
        type MediaSubtitle,
        type User
    } from '$lib/api';
    import Player from '$lib/components/Player.svelte';
    import { i18n, subLangCode } from '$lib/i18n.svelte';
    import { watchTogether } from '$lib/stores/watchTogether';
    import { get } from 'svelte/store';

    function announcePartyEpisode(epId: string) {
        const wt = get(watchTogether);
        if (wt.active && wt.isHost && wt.code) {
            api.setPartyEpisode(wt.code, epId).catch(() => {});
        }
    }

    let user = $state<User | null>(null);
    let media = $state<MediaWithEpisodes | null>(null);
    let detail = $state<TmdbDetail | null>(null);
    let cur = $state<Episode | null>(null);
    let subs = $state<MediaSubtitle[]>([]);
    let tmdbEps = $state<TmdbEpisode[]>([]);
    let loading = $state(true);
    let err = $state('');
    let resumeAt = $state(0);
    let subLoad = 0;

    const id = $derived(page.params.id ?? '');
    const epQuery = $derived(page.url.searchParams.get('ep'));

    let lastSavedPos = 0;
    let lastSavedAt = 0;

    async function loadSubs(ownerId: string) {
        const load = ++subLoad;
        try {
            const list = await api.listSubtitles(ownerId);
            if (load !== subLoad) return;
            subs = sortSubs(list).map((s) => ({ ...s, media_id: ownerId }));
        } catch {
            if (load === subLoad) subs = [];
        }
    }

    function sortSubs<T extends { language: string; is_default: boolean }>(list: T[]): T[] {
        const my = subLangCode(i18n.lang);
        return [...list].sort((a, b) => {
            const da = a.is_default ? 1 : 0;
            const db = b.is_default ? 1 : 0;
            if (da !== db) return db - da;
            const ea = a.language === 'eng' ? 1 : 0;
            const eb = b.language === 'eng' ? 1 : 0;
            if (ea !== eb) return eb - ea;
            const ma = a.language === my ? 1 : 0;
            const mb = b.language === my ? 1 : 0;
            if (ma !== mb) return mb - ma;
            return a.language.localeCompare(b.language);
        });
    }

    async function loadResume(mediaId: string, episodeId?: string) {
        try {
            const p = await api.getProgress(mediaId, episodeId);
            resumeAt = p && !p.completed && p.position > 5 ? p.position : 0;
        } catch {
            resumeAt = 0;
        }
    }

    function onProgress(position: number, duration: number) {
        if (!media || duration <= 0) return;
        const now = Date.now();
        const moved = Math.abs(position - lastSavedPos) > 5;
        const due = now - lastSavedAt > 10_000;
        if (!moved && !due) return;
        lastSavedPos = position;
        lastSavedAt = now;
        api.saveProgress({
            media_id: media.id,
            episode_id: cur?.id ?? null,
            position: Math.floor(position),
            duration: Math.floor(duration)
        }).catch(() => {});
    }

    onMount(async () => {
        const [u, m] = await Promise.all([api.me().catch(() => null), api.getMedia(id).catch(() => null)]);
        if (!u) {
            goto('/login');
            return;
        }
        user = u;
        if (!m) {
            err = 'media not found';
            loading = false;
            return;
        }
        media = m;

        let ownerId = m.id;
        if (m.media_type === 'tv') {
            const all = m.episodes ?? [];
            const pick = epQuery ? all.find((e) => e.id === epQuery) : all.find((e) => !!e.file_path);
            if (!pick?.file_path) {
                err = 'no playable episode';
                loading = false;
                return;
            }
            cur = pick;
            ownerId = pick.id;
        } else if (!m.file_path) {
            err = 'movie not downloaded';
            loading = false;
            return;
        }

        await loadResume(m.id, m.media_type === 'tv' ? ownerId : undefined);
        loading = false;
        if (m.media_type === 'tv' && cur) announcePartyEpisode(cur.id);

        loadSubs(ownerId);
        if (m.media_type === 'tv' && m.tmdb_id && cur) {
            const tmdbId = m.tmdb_id;
            const season = cur.season;
            api.tmdbDetail('tv', tmdbId)
                .then((d) => {
                    detail = d;
                })
                .catch(() => {});
            api.tmdbSeason(tmdbId, season)
                .then((eps) => {
                    tmdbEps = eps;
                })
                .catch(() => {});
        }
    });

    function back() {
        if (!media?.tmdb_id) {
            goto('/');
            return;
        }
        goto(media.media_type === 'tv' ? `/tv/${media.tmdb_id}` : `/movie/${media.tmdb_id}`);
    }

    async function pickEpisode(ep: Episode) {
        if (!media || !ep.file_path) return;
        await loadResume(media.id, ep.id);
        subs = [];
        cur = ep;
        announcePartyEpisode(ep.id);
        await loadSubs(ep.id);
        if (media.tmdb_id && tmdbEps[0]?.season_number !== ep.season) {
            try {
                tmdbEps = await api.tmdbSeason(media.tmdb_id, ep.season);
            } catch {}
        }
        const url = new URL(window.location.href);
        url.searchParams.set('ep', ep.id);
        history.replaceState(history.state, '', url.toString());
    }

    function nextReady(): Episode | null {
        if (!media || !cur) return null;
        const ready = (media.episodes ?? [])
            .filter((e) => !!e.file_path)
            .sort((a, b) => a.season - b.season || a.episode - b.episode);
        return (
            ready.find((e) => e.season === cur!.season && e.episode === cur!.episode + 1) ??
            ready.find((e) => e.season === cur!.season + 1 && e.episode === 1) ??
            null
        );
    }

    const playerLabel = $derived.by(() => {
        if (!cur) return null;
        const tEp = tmdbEps.find((e) => e.episode_number === cur!.episode);
        const head = `S${String(cur.season).padStart(2, '0')}E${String(cur.episode).padStart(2, '0')}`;
        return tEp ? `${head} - ${tEp.name}` : head;
    });

    const pageTitle = $derived.by(() => {
        if (!media) return 'watch';
        if (cur) {
            const head = `S${String(cur.season).padStart(2, '0')}E${String(cur.episode).padStart(2, '0')}`;
            return `${head} - ${media.title}`;
        }
        return media.title;
    });
</script>

<svelte:head><title>{pageTitle} - pleasewatch</title></svelte:head>

{#if loading}
    <div class="fixed inset-0 z-[300] bg-black flex items-center justify-center">
        <div class="animate-spin rounded-full h-10 w-10 border-t-2 border-b-2 border-white"></div>
    </div>
{:else if err}
    <div class="fixed inset-0 z-[300] bg-black flex items-center justify-center text-white">
        <div class="text-center">
            <p class="text-sm text-gray-300 mb-4">{err}</p>
            <button class="px-4 py-2 rounded-lg bg-white/10 hover:bg-white/15 text-sm" onclick={back}>back</button>
        </div>
    </div>
{:else if media}
    <Player
        src={api.streamUrl(cur?.id ?? media.id)}
        title={media.title}
        episodeLabel={playerLabel}
        releaseName={cur?.source_name ?? media.source_name ?? null}
        mediaId={cur?.id ?? media.id}
        showMediaId={media.id}
        currentSeason={cur?.season ?? 0}
        currentEpisodeNum={cur?.episode ?? 0}
        currentEpisodeId={cur?.id ?? null}
        tmdbId={media.tmdb_id ?? null}
        posterUrl={media.poster_url ?? null}
        subtitles={subs}
        episodes={(media.episodes ?? []).map((e) => ({ ...e, media_id: media!.id }))}
        tmdbEpisodes={tmdbEps}
        introStart={cur?.intro_start ?? null}
        introEnd={cur?.intro_end ?? null}
        creditsStart={cur?.credits_start ?? null}
        onBack={back}
        onEpisodeSelect={(ep) => pickEpisode(ep as Episode)}
        onNextEpisode={() => {
            const n = nextReady();
            if (n) pickEpisode(n);
        }}
        resumePosition={resumeAt}
        {onProgress}
    />
{/if}
