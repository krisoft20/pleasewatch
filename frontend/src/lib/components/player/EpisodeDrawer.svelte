<script lang="ts">
    import { onMount } from 'svelte';
    import { fly, fade } from 'svelte/transition';
    import { cubicOut } from 'svelte/easing';
    import type { EpisodeRecord, TmdbEpisode, WatchProgress } from '$lib/types';
    import { api } from '$lib/api';
    import Icon from '../Icon.svelte';

    let {
        episodes,
        currentEpisodeId,
        currentSeason,
        currentEpisodeNum,
        tmdbEpisodes = [],
        tmdbId = null,
        episodeProgress = new Map(),
        isMobile = false,
        onSelect,
        onClose
    }: {
        episodes: EpisodeRecord[];
        currentEpisodeId: string | null;
        currentSeason: number;
        currentEpisodeNum: number;
        tmdbEpisodes?: TmdbEpisode[];
        tmdbId?: number | null;
        episodeProgress?: Map<string, WatchProgress>;
        isMobile?: boolean;
        onSelect: (ep: EpisodeRecord) => void;
        onClose: () => void;
    } = $props();

    function initialSeason() {
        return currentSeason;
    }

    function initialTmdbEpisodes() {
        return tmdbEpisodes;
    }

    let drawerSeason = $state(initialSeason());
    let drawerTmdbEpisodes = $state<TmdbEpisode[]>(initialTmdbEpisodes());
    let showSeasonDropdown = $state(false);
    let mobileListEl = $state<HTMLDivElement | null>(null);
    let desktopListEl = $state<HTMLDivElement | null>(null);

    const seasons = $derived([...new Set(episodes.map((e) => e.season))].sort((a, b) => a - b));
    const drawerEpisodes = $derived(seasons.length <= 1 ? episodes : episodes.filter((e) => e.season === drawerSeason));

    onMount(() => {
        scrollToCurrent();
    });

    async function switchSeason(season: number) {
        drawerSeason = season;
        showSeasonDropdown = false;

        if (tmdbId && season !== currentSeason) {
            try {
                drawerTmdbEpisodes = await api.getSeasonEpisodes(tmdbId, season);
            } catch {
                drawerTmdbEpisodes = [];
            }
        } else {
            drawerTmdbEpisodes = tmdbEpisodes;
        }

        scrollToCurrent();
    }

    function scrollToCurrent() {
        setTimeout(() => {
            const el = mobileListEl || desktopListEl;
            if (!el) return;

            const current = el.querySelector('[data-current="true"]') as HTMLElement | null;
            current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
        }, 100);
    }

    function selectEpisode(ep: EpisodeRecord) {
        if (ep.status !== 'ready') return;
        onClose();
        onSelect(ep);
    }

    function metaFor(ep: EpisodeRecord) {
        return drawerTmdbEpisodes.find((t) => t.episode_number === ep.episode);
    }

    function progressPct(progress: WatchProgress | undefined) {
        if (!progress || progress.duration <= 0) return 0;
        return (progress.position / progress.duration) * 100;
    }
</script>

{#if isMobile}
    <div class="fixed inset-0 z-50">
        <button
            type="button"
            class="absolute inset-0 bg-black/60"
            transition:fade={{ duration: 200 }}
            onclick={onClose}
            aria-label="close episodes"
        ></button>
        <div
            class="absolute bottom-0 left-0 right-0 bg-black/70 backdrop-blur-2xl rounded-t-3xl shadow-[0_-10px_40px_rgba(0,0,0,0.5)] max-h-[65vh] overflow-hidden"
            in:fly={{ y: 500, duration: 400, easing: cubicOut }}
            out:fly={{ y: 500, duration: 300 }}
        >
            <div class="flex justify-center pt-3 pb-1">
                <div class="w-10 h-1.5 bg-gray-600 rounded-full"></div>
            </div>
            <div class="px-6 pt-2 pb-3 flex items-center justify-between">
                {#if seasons.length > 1}
                    <div class="relative">
                        <button
                            onclick={() => {
                                showSeasonDropdown = !showSeasonDropdown;
                            }}
                            class="flex items-center gap-2 text-white text-xl font-bold"
                        >
                            Season {drawerSeason}
                            <Icon
                                name="chevron-down"
                                class="w-5 h-5 text-gray-400 transition-transform {showSeasonDropdown
                                    ? 'rotate-180'
                                    : ''}"
                            />
                        </button>
                        {#if showSeasonDropdown}
                            <div
                                class="pw-season-menu"
                                in:fly={{ y: -8, duration: 150 }}
                            >
                                {#each seasons as season}
                                    <button
                                        onclick={() => switchSeason(season)}
                                        class="w-full px-4 py-2.5 text-left text-sm font-medium flex items-center justify-between transition-colors
											{season === drawerSeason ? 'text-white bg-white/10' : 'text-gray-400 active:bg-white/5'}"
                                    >
                                        Season {season}
                                        {#if season === drawerSeason}
                                            <Icon name="check" class="w-4 h-4 text-primary-400" />
                                        {/if}
                                    </button>
                                {/each}
                            </div>
                        {/if}
                    </div>
                {:else}
                    <h3 class="text-white text-xl font-bold">
                        {#if currentSeason > 0}Season {currentSeason}{:else}Episodes{/if}
                    </h3>
                {/if}
            </div>
            <div
                bind:this={mobileListEl}
                class="overflow-y-auto scrollbar-hide px-4 pb-8"
                style="max-height: calc(65vh - 80px);"
            >
                {#each drawerEpisodes as ep (ep.id)}
                    {@const isCurrent =
                        ep.id === currentEpisodeId ||
                        (currentEpisodeNum > 0 && ep.episode === currentEpisodeNum && ep.season === currentSeason)}
                    {@const tmdb = metaFor(ep)}
                    {@const epProg = episodeProgress.get(ep.id)}
                    {@const isWatched = epProg?.completed || ep.status === 'cleaned'}
                    {@const pct = progressPct(epProg)}
                    <button
                        data-current={isCurrent}
                        onclick={() => selectEpisode(ep)}
                        disabled={ep.status !== 'ready'}
                        class="w-full flex gap-3 px-2 py-3 rounded-2xl transition-all text-left {ep.status !== 'ready'
                            ? 'opacity-30'
                            : ''}
							{isCurrent ? 'bg-primary-500/15 ring-1 ring-primary-500/40' : 'active:bg-white/5'}"
                    >
                        <div
                            class="relative flex-shrink-0 w-[120px] aspect-video rounded-lg overflow-hidden {isCurrent
                                ? 'ring-2 ring-primary-500'
                                : 'bg-white/5'}"
                        >
                            {#if tmdb?.still_url}
                                <img
                                    src={tmdb.still_url}
                                    alt=""
                                    class="w-full h-full object-cover {isWatched && !isCurrent ? 'opacity-50' : ''}"
                                />
                            {:else}
                                <div
                                    class="w-full h-full flex items-center justify-center text-gray-600 text-lg font-bold"
                                >
                                    {ep.episode}
                                </div>
                            {/if}
                            {#if isCurrent}
                                <div class="absolute inset-0 bg-black/40 flex items-center justify-center">
                                    <div class="flex gap-[3px] items-end h-4">
                                        <div class="w-[3px] rounded-full bg-primary-400 animate-bar1"></div>
                                        <div class="w-[3px] rounded-full bg-primary-400 animate-bar2"></div>
                                        <div class="w-[3px] rounded-full bg-primary-400 animate-bar3"></div>
                                    </div>
                                </div>
                            {:else if isWatched}
                                <div
                                    class="absolute top-1 left-1 w-5 h-5 rounded-full bg-blue-500 flex items-center justify-center shadow-lg"
                                >
                                    <Icon name="check" class="w-3 h-3 text-white" />
                                </div>
                            {/if}
                            {#if tmdb?.runtime}
                                <span
                                    class="absolute bottom-1 right-1 text-[9px] font-medium text-white bg-black/70 px-1 py-0.5 rounded"
                                    >{tmdb.runtime}m</span
                                >
                            {/if}
                            {#if pct > 0 && !isWatched}
                                <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-black/60">
                                    <div class="h-full bg-primary-500" style="width: {pct}%"></div>
                                </div>
                            {/if}
                        </div>
                        <div class="flex-1 min-w-0 py-0.5">
                            <p
                                class="text-[14px] font-semibold truncate {isCurrent
                                    ? 'text-white'
                                    : isWatched
                                      ? 'text-gray-500'
                                      : 'text-gray-200'}"
                            >
                                <span class="text-gray-500">{ep.episode}.</span>
                                {tmdb?.name || ep.title || `Episode ${ep.episode}`}
                            </p>
                            <div class="flex items-center gap-2 mt-0.5">
                                {#if tmdb?.air_date}
                                    <span class="text-[11px] text-gray-500">{tmdb.air_date}</span>
                                {/if}
                                {#if ep.status !== 'ready'}
                                    <span class="text-[11px] text-gray-500 capitalize">{ep.status}</span>
                                {/if}
                                {#if isWatched}
                                    <span class="text-[11px] text-blue-400">Watched</span>
                                {/if}
                            </div>
                            {#if tmdb?.overview}
                                <p class="text-[12px] text-gray-400 line-clamp-2 mt-1 leading-relaxed">
                                    {tmdb.overview}
                                </p>
                            {/if}
                        </div>
                    </button>
                {/each}
            </div>
        </div>
    </div>
{:else}
    <button type="button" class="absolute inset-0 z-30" onclick={onClose} aria-label="close episodes"></button>
    <div in:fly={{ y: 8, duration: 200, easing: cubicOut }} class="absolute z-40 bottom-24 right-10 w-80">
        <div
            class="bg-black/60 backdrop-blur-2xl rounded-2xl shadow-[0_8px_32px_rgba(0,0,0,0.6)] ring-1 ring-white/10 overflow-hidden"
        >
            <div class="bg-white/5 px-4 pt-3.5 pb-2.5 flex items-center justify-between">
                {#if seasons.length > 1}
                    <div class="relative">
                        <button
                            onclick={() => {
                                showSeasonDropdown = !showSeasonDropdown;
                            }}
                            class="flex items-center gap-1.5 text-white text-[13px] font-semibold"
                        >
                            Season {drawerSeason}
                            <Icon
                                name="chevron-down"
                                class="w-3.5 h-3.5 text-gray-400 transition-transform {showSeasonDropdown
                                    ? 'rotate-180'
                                    : ''}"
                            />
                        </button>
                        {#if showSeasonDropdown}
                            <div
                                class="pw-season-menu pw-season-menu-small"
                                in:fly={{ y: -6, duration: 150 }}
                            >
                                {#each seasons as season}
                                    <button
                                        onclick={() => switchSeason(season)}
                                        class="w-full px-3 py-2 text-left text-xs font-medium flex items-center justify-between transition-colors
											{season === drawerSeason ? 'text-white bg-white/10' : 'text-gray-400 hover:bg-white/5 hover:text-white'}"
                                    >
                                        Season {season}
                                        {#if season === drawerSeason}
                                            <Icon name="check" class="w-3.5 h-3.5 text-primary-400" />
                                        {/if}
                                    </button>
                                {/each}
                            </div>
                        {/if}
                    </div>
                {:else}
                    <h3 class="text-white text-[13px] font-semibold">Episodes</h3>
                {/if}
                <button onclick={onClose} class="text-gray-500 hover:text-white transition-colors">
                    <Icon name="close" class="w-4 h-4" />
                </button>
            </div>
            <div bind:this={desktopListEl} class="max-h-[45vh] overflow-y-auto scrollbar-hide pb-2">
                {#each drawerEpisodes as ep (ep.id)}
                    {@const isCurrent =
                        ep.id === currentEpisodeId ||
                        (currentEpisodeNum > 0 && ep.episode === currentEpisodeNum && ep.season === currentSeason)}
                    {@const tmdb = metaFor(ep)}
                    {@const epProg = episodeProgress.get(ep.id)}
                    {@const isWatched = epProg?.completed || ep.status === 'cleaned'}
                    {@const pct = progressPct(epProg)}
                    <button
                        data-current={isCurrent}
                        onclick={() => selectEpisode(ep)}
                        disabled={ep.status !== 'ready'}
                        class="w-full flex gap-3 px-3 py-2.5 transition-all text-left {isCurrent
                            ? 'bg-white/10'
                            : 'hover:bg-white/5'} {ep.status !== 'ready' ? 'opacity-30 cursor-not-allowed' : ''}"
                    >
                        <div
                            class="relative flex-shrink-0 w-[100px] aspect-video rounded-lg overflow-hidden bg-white/5"
                        >
                            {#if tmdb?.still_url}
                                <img
                                    src={tmdb.still_url}
                                    alt=""
                                    class="w-full h-full object-cover {isWatched && !isCurrent ? 'opacity-50' : ''}"
                                />
                            {:else}
                                <div
                                    class="w-full h-full flex items-center justify-center text-gray-600 text-sm font-bold"
                                >
                                    {ep.episode}
                                </div>
                            {/if}
                            {#if isCurrent}
                                <div class="absolute inset-0 bg-black/40 flex items-center justify-center">
                                    <div class="flex gap-[3px] items-end h-3">
                                        <div class="w-[2px] rounded-full bg-primary-400 animate-bar1"></div>
                                        <div class="w-[2px] rounded-full bg-primary-400 animate-bar2"></div>
                                        <div class="w-[2px] rounded-full bg-primary-400 animate-bar3"></div>
                                    </div>
                                </div>
                            {:else if isWatched}
                                <div
                                    class="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-blue-500 flex items-center justify-center shadow-lg"
                                >
                                    <Icon name="check" class="w-2.5 h-2.5 text-white" />
                                </div>
                            {/if}
                            {#if tmdb?.runtime}
                                <span
                                    class="absolute bottom-0.5 right-0.5 text-[8px] text-white bg-black/70 px-1 rounded"
                                    >{tmdb.runtime}m</span
                                >
                            {/if}
                            {#if pct > 0 && !isWatched}
                                <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-black/60">
                                    <div class="h-full bg-primary-500" style="width: {pct}%"></div>
                                </div>
                            {/if}
                        </div>
                        <div class="flex-1 min-w-0 py-0.5">
                            <p
                                class="text-[12px] font-medium truncate {isCurrent
                                    ? 'text-white'
                                    : isWatched
                                      ? 'text-gray-500'
                                      : 'text-gray-300'}"
                            >
                                <span class="text-gray-500">{ep.episode}.</span>
                                {tmdb?.name || ep.title || `Episode ${ep.episode}`}
                            </p>
                            {#if tmdb?.overview}
                                <p class="text-[10px] text-gray-500 line-clamp-2 mt-0.5 leading-relaxed">
                                    {tmdb.overview}
                                </p>
                            {/if}
                        </div>
                        {#if isCurrent}
                            <div class="flex gap-[3px] items-end h-4 flex-shrink-0">
                                <div class="w-[3px] rounded-full bg-primary-400 animate-bar1"></div>
                                <div class="w-[3px] rounded-full bg-primary-400 animate-bar2"></div>
                                <div class="w-[3px] rounded-full bg-primary-400 animate-bar3"></div>
                            </div>
                        {/if}
                    </button>
                {/each}
            </div>
        </div>
    </div>
{/if}

<style>
    @keyframes bar1 {
        0%,
        100% {
            height: 4px;
        }
        50% {
            height: 16px;
        }
    }
    @keyframes bar2 {
        0%,
        100% {
            height: 12px;
        }
        50% {
            height: 4px;
        }
    }
    @keyframes bar3 {
        0%,
        100% {
            height: 8px;
        }
        50% {
            height: 14px;
        }
    }
    .animate-bar1 {
        animation: bar1 0.8s ease infinite;
    }
    .animate-bar2 {
        animation: bar2 0.8s ease infinite 0.2s;
    }
    .animate-bar3 {
        animation: bar3 0.8s ease infinite 0.4s;
    }
    .pw-season-menu {
        position: absolute;
        top: 100%;
        left: 0;
        z-index: 10;
        min-width: 160px;
        margin-top: 0.5rem;
        max-height: min(320px, 44vh);
        overflow-y: auto;
        overscroll-behavior: contain;
        -webkit-overflow-scrolling: touch;
        background: #2c2c2e;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 0.75rem;
        box-shadow: 0 24px 48px rgba(0, 0, 0, 0.35);
    }
    .pw-season-menu-small {
        min-width: 130px;
        margin-top: 0.375rem;
        border-radius: 0.5rem;
    }
</style>
