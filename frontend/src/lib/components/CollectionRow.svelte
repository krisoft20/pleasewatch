<script lang="ts">
    import { goto } from '$app/navigation';
    import { api, type TmdbCollectionPart, type MediaWithEpisodes } from '$lib/api';

    let {
        parts,
        collectionName,
        currentTmdbId
    }: {
        parts: TmdbCollectionPart[];
        collectionName: string;
        currentTmdbId: number;
    } = $props();

    let scrollEl = $state<HTMLDivElement>();
    let canL = $state(false);
    let canR = $state(false);
    let statusByTmdb = $state<Map<number, 'ready' | 'added' | 'none'>>(new Map());

    function update() {
        if (!scrollEl) return;
        const { scrollLeft, scrollWidth, clientWidth } = scrollEl;
        canL = scrollLeft > 4;
        canR = scrollLeft + clientWidth < scrollWidth - 4;
    }

    function scrollBy(dir: number) {
        if (!scrollEl) return;
        const delta = scrollEl.clientWidth * 0.85 * dir;
        scrollEl.scrollBy({ left: delta, behavior: 'smooth' });
    }

    $effect(() => {
        const el = scrollEl;
        if (!el) return;
        update();
        el.addEventListener('scroll', update, { passive: true });
        const ro = new ResizeObserver(update);
        ro.observe(el);
        return () => {
            el.removeEventListener('scroll', update);
            ro.disconnect();
        };
    });

    $effect(() => {
        const next = new Map<number, 'ready' | 'added' | 'none'>();
        statusByTmdb = next;
        for (const p of parts) {
            api.getMediaByTmdb('movie', p.tmdb_id)
                .then((m: MediaWithEpisodes | null) => {
                    if (!m) {
                        next.set(p.tmdb_id, 'none');
                    } else if (m.file_path) {
                        next.set(p.tmdb_id, 'ready');
                    } else {
                        next.set(p.tmdb_id, 'added');
                    }
                    statusByTmdb = new Map(next);
                })
                .catch(() => {
                    next.set(p.tmdb_id, 'none');
                    statusByTmdb = new Map(next);
                });
        }
    });

    function go(id: number) {
        goto(`/movie/${id}`);
    }
</script>

<div class="pw-coll-section">
    <h2 class="pw-coll-h">{collectionName}</h2>
    <div class="pw-coll-wrap">
        <div class="pw-coll-fade pw-coll-fade-left" class:is-visible={canL}></div>
        <div class="pw-coll-fade pw-coll-fade-right" class:is-visible={canR}></div>
        <button
            class="pw-coll-arrow pw-coll-arrow-left"
            class:is-enabled={canL}
            tabindex={canL ? 0 : -1}
            onclick={() => scrollBy(-1)}
            aria-label="scroll left"
        >
            <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.4"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <polyline points="15 18 9 12 15 6" />
            </svg>
        </button>
        <button
            class="pw-coll-arrow pw-coll-arrow-right"
            class:is-enabled={canR}
            tabindex={canR ? 0 : -1}
            onclick={() => scrollBy(1)}
            aria-label="scroll right"
        >
            <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.4"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <polyline points="9 18 15 12 9 6" />
            </svg>
        </button>
        <div class="pw-coll-row" bind:this={scrollEl}>
            {#each parts as part (part.tmdb_id)}
                {@const isCurrent = part.tmdb_id === currentTmdbId}
                {@const status = statusByTmdb.get(part.tmdb_id) ?? 'none'}
                <button
                    class="pw-coll-card"
                    class:is-current={isCurrent}
                    type="button"
                    onclick={() => go(part.tmdb_id)}
                    aria-label={part.title}
                >
                    <div class="pw-coll-poster">
                        {#if part.poster_url}
                            <img src={part.poster_url} alt={part.title} loading="lazy" />
                        {:else}
                            <div class="pw-coll-poster-blank">
                                <svg
                                    width="36"
                                    height="36"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="1.4"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M3 9h18M9 3v18" /></svg
                                >
                            </div>
                        {/if}
                        {#if isCurrent}
                            <div class="pw-coll-badge pw-coll-badge-current">// you're here</div>
                        {:else if status === 'ready'}
                            <div class="pw-coll-badge pw-coll-badge-ready">ready</div>
                        {:else if status === 'added'}
                            <div class="pw-coll-badge pw-coll-badge-pending">added</div>
                        {:else}
                            <div class="pw-coll-badge pw-coll-badge-missing">+ add</div>
                        {/if}
                    </div>
                    <div class="pw-coll-title">{part.title}</div>
                    {#if part.year}<div class="pw-coll-year">{part.year}</div>{/if}
                </button>
            {/each}
        </div>
    </div>
</div>

<style>
    .pw-coll-section {
        padding: 8px 0 4px;
    }
    .pw-coll-h {
        font-size: 14px;
        font-weight: 600;
        color: rgba(232, 232, 234, 0.92);
        margin: 0 0 14px;
        letter-spacing: 0.01em;
    }

    .pw-coll-wrap {
        position: relative;
    }

    .pw-coll-row {
        display: flex;
        gap: 14px;
        overflow-x: auto;
        scroll-snap-type: x proximity;
        padding: 6px 6px 14px;
        margin: -6px -6px 0;
        scrollbar-width: none;
    }
    .pw-coll-row::-webkit-scrollbar {
        display: none;
    }

    .pw-coll-card {
        flex: 0 0 140px;
        scroll-snap-align: start;
        background: none;
        border: none;
        padding: 0;
        cursor: pointer;
        text-align: left;
        color: inherit;
        transition: transform 0.14s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    .pw-coll-card:hover {
        transform: translateY(-2px);
    }
    .pw-coll-card:active {
        transform: translateY(0);
    }

    .pw-coll-poster {
        position: relative;
        width: 140px;
        height: 210px;
        border-radius: 10px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.04);
        box-shadow: 0 4px 18px -8px rgba(0, 0, 0, 0.5);
    }
    .pw-coll-poster img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-coll-poster-blank {
        width: 100%;
        height: 100%;
        display: grid;
        place-items: center;
        color: rgba(255, 255, 255, 0.2);
    }

    .pw-coll-card.is-current {
        position: relative;
        z-index: 2;
    }
    .pw-coll-card.is-current .pw-coll-poster {
        box-shadow:
            0 0 0 2px rgba(110, 200, 255, 0.7),
            0 8px 24px -10px rgba(110, 200, 255, 0.5);
    }

    .pw-coll-badge {
        position: absolute;
        bottom: 6px;
        left: 6px;
        font-size: 10px;
        font-weight: 600;
        padding: 3px 7px;
        border-radius: 4px;
        letter-spacing: 0.02em;
        backdrop-filter: blur(6px);
    }
    .pw-coll-badge-ready {
        background: rgba(34, 197, 94, 0.85);
        color: #fff;
    }
    .pw-coll-badge-pending {
        background: rgba(234, 179, 8, 0.78);
        color: #08090b;
    }
    .pw-coll-badge-missing {
        background: rgba(8, 9, 12, 0.78);
        color: rgba(232, 232, 234, 0.88);
        border: 1px solid rgba(255, 255, 255, 0.12);
    }
    .pw-coll-badge-current {
        background: rgba(110, 200, 255, 0.92);
        color: #08090b;
    }

    .pw-coll-title {
        margin-top: 8px;
        font-size: 13px;
        font-weight: 500;
        color: rgba(232, 232, 234, 0.95);
        line-height: 1.25;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .pw-coll-year {
        margin-top: 2px;
        font-size: 11px;
        color: rgba(180, 180, 184, 0.7);
    }

    .pw-coll-arrow {
        position: absolute;
        top: 105px;
        transform: translateY(-50%);
        width: 36px;
        height: 36px;
        border-radius: 999px;
        background: rgba(8, 9, 12, 0.85);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: rgba(232, 232, 234, 0.85);
        display: grid;
        place-items: center;
        cursor: pointer;
        z-index: 3;
        opacity: 0;
        pointer-events: none;
        transition: opacity 0.15s ease;
        backdrop-filter: blur(8px);
    }
    .pw-coll-wrap:hover .pw-coll-arrow.is-enabled {
        opacity: 1;
        pointer-events: auto;
    }
    .pw-coll-arrow.is-enabled:hover {
        background: rgba(8, 9, 12, 0.95);
        color: #fff;
    }
    .pw-coll-arrow-left {
        left: -8px;
    }
    .pw-coll-arrow-right {
        right: -8px;
    }
    @media (max-width: 640px) {
        .pw-coll-arrow {
            display: none;
        }
        .pw-coll-card {
            flex-basis: 116px;
        }
        .pw-coll-poster {
            width: 116px;
            height: 174px;
        }
    }

    .pw-coll-fade {
        position: absolute;
        top: 0;
        bottom: 8px;
        width: 60px;
        pointer-events: none;
        z-index: 2;
        opacity: 0;
        transition: opacity 0.15s ease;
    }
    .pw-coll-fade.is-visible {
        opacity: 1;
    }
    .pw-coll-fade-left {
        left: 0;
        background: linear-gradient(to right, var(--pw-bg) 10%, transparent);
    }
    .pw-coll-fade-right {
        right: 0;
        background: linear-gradient(to left, var(--pw-bg) 10%, transparent);
    }
</style>
