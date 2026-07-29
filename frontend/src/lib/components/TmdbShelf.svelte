<script lang="ts">
    import { goto } from '$app/navigation';
    import type { TmdbSearchItem } from '$lib/api';
    import Icon from '$lib/components/Icon.svelte';

    let {
        title,
        items,
        width = 180,
        onPreview
    }: {
        title: string;
        items: TmdbSearchItem[];
        width?: number;
        onPreview?: (item: TmdbSearchItem) => void;
    } = $props();

    let scrollEl = $state<HTMLDivElement>();
    let canL = $state(false);
    let canR = $state(false);

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

    function open(item: TmdbSearchItem) {
        if (onPreview) {
            onPreview(item);
            return;
        }
        const route = item.media_type === 'tv' ? 'tv' : 'movie';
        goto(`/${route}/${item.tmdb_id}`);
    }

    function shelfPoster(u: string | null | undefined): string {
        if (!u) return '';
        return u.replace('/w500/', '/w342/');
    }
</script>

{#if items && items.length > 0}
    <div class="pw-shelf">
        <div class="pw-row-head">
            <h2 class="pw-h2">{title}<span class="pw-count">{items.length}</span></h2>
        </div>
        <div style="position: relative;">
            <div class="pw-shelf-fade pw-shelf-fade-left" class:is-visible={canL}></div>
            <div class="pw-shelf-fade pw-shelf-fade-right" class:is-visible={canR}></div>
            <button
                class="pw-shelf-arrow pw-shelf-arrow-left"
                class:is-enabled={canL}
                tabindex={canL ? 0 : -1}
                onclick={() => scrollBy(-1)}
                aria-label="scroll left"
            >
                <Icon name="chevron-left" class="w-3.5 h-3.5" strokeWidth={2.4} />
            </button>
            <button
                class="pw-shelf-arrow pw-shelf-arrow-right"
                class:is-enabled={canR}
                tabindex={canR ? 0 : -1}
                onclick={() => scrollBy(1)}
                aria-label="scroll right"
            >
                <Icon name="chevron-right" class="w-3.5 h-3.5" strokeWidth={2.4} />
            </button>
            <div class="pw-row-scroll" bind:this={scrollEl}>
                {#each items as item, i (item.tmdb_id + '-' + item.media_type)}
                    {@const eager = i < 6}
                    <div
                        class="pw-tmdb-card group"
                        style="width: {width}px;"
                        onclick={() => open(item)}
                        onkeydown={(e) => e.key === 'Enter' && open(item)}
                        role="button"
                        tabindex="0"
                    >
                        <div class="pw-card-frame pw-tmdb-frame">
                            {#if item.poster_url}
                                <img
                                    src={shelfPoster(item.poster_url)}
                                    alt={item.title}
                                    loading={eager ? 'eager' : 'lazy'}
                                    fetchpriority={eager ? 'high' : 'low'}
                                    decoding="async"
                                    class="pw-card-img pw-tmdb-img"
                                />
                            {:else}
                                <div class="pw-tmdb-empty">
                                    <Icon name="screen-text" class="w-12 h-12" strokeWidth={1.5} />
                                </div>
                            {/if}
                            <span class="pw-tmdb-kind">
                                {item.media_type === 'tv' ? 'series' : 'movie'}
                            </span>
                            <div class="pw-card-overlay pw-tmdb-overlay">
                                <Icon name="play" class="w-12 h-12 text-white" />
                            </div>
                        </div>
                        <h3 class="pw-tmdb-title">
                            {item.title}
                        </h3>
                        <div class="pw-tmdb-meta">
                            {#if item.year}<span>{item.year}</span>{/if}
                            {#if item.vote_average}<span>★ {item.vote_average.toFixed(1)}</span>{/if}
                        </div>
                    </div>
                {/each}
            </div>
        </div>
    </div>
{/if}

<style>
    .pw-tmdb-card {
        flex-shrink: 0;
        min-width: 0;
        cursor: pointer;
    }
    .pw-tmdb-frame {
        position: relative;
        aspect-ratio: 2 / 3;
        overflow: hidden;
        border-radius: 8px;
        background: #111827;
    }
    .pw-tmdb-img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .pw-tmdb-empty {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 100%;
        height: 100%;
        color: #374151;
    }
    .pw-tmdb-kind {
        position: absolute;
        z-index: 10;
        top: 8px;
        left: 8px;
        display: flex;
        align-items: center;
        height: 24px;
        padding: 0 8px;
        border-radius: 4px;
        background: rgba(0, 0, 0, 0.6);
        color: rgba(255, 255, 255, 0.9);
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }
    .pw-tmdb-overlay {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        pointer-events: none;
    }
    .pw-tmdb-title {
        margin: 8px 0 0;
        overflow: hidden;
        color: #e5e7eb;
        font-size: 14px;
        font-weight: 500;
        line-height: 1.3;
        text-overflow: ellipsis;
        white-space: nowrap;
        transition: color 0.15s ease;
    }
    .pw-tmdb-card:hover .pw-tmdb-title {
        color: #fff;
    }
    .pw-tmdb-meta {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-top: 2px;
        color: #6b7280;
        font-size: 12px;
    }
</style>
