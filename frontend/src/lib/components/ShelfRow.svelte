<script lang="ts">
    import type { Media } from '$lib/api';
    import Poster from './Poster.svelte';

    let {
        title,
        items,
        width = 200,
        onRemove,
        limit,
        layout = 'row',
        progress,
        showActivity = false
    }: {
        title: string;
        items: Media[];
        width?: number;
        onRemove?: (m: Media) => void;
        limit?: number;
        layout?: 'row' | 'grid';
        progress?: Record<string, number>;
        showActivity?: boolean;
    } = $props();

    const visible = $derived(limit ? items.slice(0, limit) : items);

    let scrollEl = $state<HTMLDivElement>();
    let canL = $state(false);
    let canR = $state(false);

    function update() {
        if (!scrollEl) return;
        const { scrollLeft, scrollWidth, clientWidth } = scrollEl;
        canL = scrollLeft > 16;
        canR = scrollWidth - (scrollLeft + clientWidth) > 16;
    }

    function scrollBy(dir: number) {
        if (!scrollEl) return;
        const delta = scrollEl.clientWidth * 0.85 * dir;
        scrollEl.scrollBy({ left: delta, behavior: 'smooth' });
    }

    $effect(() => {
        if (layout !== 'row') return;
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

    const showArrows = $derived(layout === 'row');
</script>

<div class="pw-shelf">
    <div class="pw-row-head">
        <h2 class="pw-h2">{title}<span class="pw-count">{items.length}</span></h2>
    </div>
    {#if layout === 'grid'}
        <div class="pw-shelf-grid" style="--pw-poster-w: {width}px;">
            {#each visible as item, i (item.id)}
                <Poster {item} {width} {onRemove} priority={i < 8} pct={progress?.[item.id] ?? 0} {showActivity} />
            {/each}
        </div>
    {:else}
        <div style="position: relative;">
            {#if showArrows}
                <div class="pw-shelf-fade pw-shelf-fade-left" class:is-visible={canL}></div>
                <div class="pw-shelf-fade pw-shelf-fade-right" class:is-visible={canR}></div>
                <button
                    class="pw-shelf-arrow pw-shelf-arrow-left"
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
                    class="pw-shelf-arrow pw-shelf-arrow-right"
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
            {/if}
            <div class="pw-row-scroll" bind:this={scrollEl}>
                {#each visible as item, i (item.id)}
                    <Poster {item} {width} {onRemove} priority={i < 6} pct={progress?.[item.id] ?? 0} {showActivity} />
                {/each}
            </div>
        </div>
    {/if}
</div>
