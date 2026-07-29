<script lang="ts">
    import { goto } from '$app/navigation';
    import { api, type ContinueItem } from '$lib/api';
    import { t } from '$lib/i18n.svelte';

    let { items = $bindable<ContinueItem[]>([]) }: { items: ContinueItem[] } = $props();

    let scrollEl = $state<HTMLDivElement>();
    let canL = $state(false);
    let canR = $state(false);

    const CARD_W = 280;

    function update() {
        if (!scrollEl) return;
        const { scrollLeft, scrollWidth, clientWidth } = scrollEl;
        canL = scrollLeft > 50;
        canR = scrollLeft + clientWidth < scrollWidth - 50;
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

    function open(item: ContinueItem) {
        if (item.episode_id) {
            goto(`/watch/${item.media_id}?ep=${item.episode_id}`);
            return;
        }
        goto(`/watch/${item.media_id}`);
    }

    const prefetched = new Set<string>();
    let hoverTimer: ReturnType<typeof setTimeout> | null = null;

    function streamUrlFor(item: ContinueItem): string {
        return `/api/stream/${item.episode_id ?? item.media_id}`;
    }

    function hoverStart(item: ContinueItem) {
        if (hoverTimer) clearTimeout(hoverTimer);
        const url = streamUrlFor(item);
        if (prefetched.has(url)) return;
        hoverTimer = setTimeout(() => {
            prefetched.add(url);
            fetch(url, {
                headers: { Range: 'bytes=0-65535' },
                credentials: 'include'
            }).catch(() => prefetched.delete(url));
        }, 200);
    }

    function hoverEnd() {
        if (hoverTimer) {
            clearTimeout(hoverTimer);
            hoverTimer = null;
        }
    }

    async function dismiss(ev: MouseEvent, item: ContinueItem) {
        ev.preventDefault();
        ev.stopPropagation();
        const prev = items;
        items = items.filter((i) => i.media_id !== item.media_id);
        try {
            await api.dismissContinue(item.media_id);
        } catch {
            items = prev;
        }
    }

    function fmtLeft(pos: number, dur: number): string {
        if (!dur || dur <= 0) return '';
        const left = Math.max(0, dur - pos);
        const h = Math.floor(left / 3600);
        const m = Math.floor((left % 3600) / 60);
        if (h > 0) return `${h}h ${m}m`;
        if (m > 0) return `${m}m`;
        return `${Math.max(0, Math.floor(left))}s`;
    }

    function epTag(it: ContinueItem): string | null {
        if (it.episode_season == null || it.episode_number == null) return null;
        return t('cw.season_ep', { s: it.episode_season, e: it.episode_number });
    }

    function thumbId(it: ContinueItem): string {
        return it.episode_id ?? it.media_id;
    }
    function imgFor(it: ContinueItem): string {
        if (it.position === 0 && it.episode_still_url) return it.episode_still_url;
        return api.thumbUrl(thumbId(it), it.position);
    }
    function imgFallback(it: ContinueItem): string | null {
        return it.episode_still_url ?? it.poster_url ?? null;
    }
    function pct(it: ContinueItem): number {
        if (it.duration <= 0) return 0;
        return Math.min(100, Math.round((it.position / it.duration) * 100));
    }
</script>

{#if items.length > 0}
    <div class="pw-shelf pw-cw">
        <div class="pw-row-head">
            <h2 class="pw-h2">{t('shelves.continue')}<span class="pw-count">{items.length}</span></h2>
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
            <div class="pw-row-scroll pw-cw-row" bind:this={scrollEl}>
                {#each items as item, i (item.media_id + '-' + (item.episode_id ?? ''))}
                    {@const fallback = imgFallback(item)}
                    {@const tag = epTag(item)}
                    {@const eager = i < 5}
                    <div
                        class="pw-cw-card group"
                        style="width: {CARD_W}px;"
                        onmouseenter={() => hoverStart(item)}
                        onmouseleave={hoverEnd}
                        role="presentation"
                    >
                        <div
                            class="pw-cw-frame"
                            style={fallback
                                ? `background-image: url('${fallback}'); background-size: cover; background-position: center;`
                                : ''}
                            onclick={() => open(item)}
                            onkeydown={(e) => {
                                if (e.key === 'Enter' || e.key === ' ') {
                                    e.preventDefault();
                                    open(item);
                                }
                            }}
                            role="button"
                            tabindex="0"
                            aria-label="continue {item.media_title}"
                        >
                            <img
                                src={imgFor(item)}
                                alt=""
                                loading={eager ? 'eager' : 'lazy'}
                                fetchpriority={eager ? 'high' : 'low'}
                                decoding="async"
                                onerror={(e) => {
                                    const el = e.currentTarget as HTMLImageElement;
                                    el.style.opacity = '0';
                                }}
                            />
                            <div class="pw-cw-veil"></div>
                            <div class="pw-cw-play" aria-hidden="true">
                                <svg
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="#fff"
                                    stroke-width="1.6"
                                    stroke-linejoin="round"
                                    stroke-linecap="round"
                                >
                                    <path d="M8 5l11 7-11 7z" />
                                </svg>
                            </div>
                            <button
                                class="pw-cw-x"
                                onclick={(e) => dismiss(e, item)}
                                type="button"
                                aria-label="dismiss"
                                title="dismiss"
                            >
                                <svg
                                    width="10"
                                    height="10"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.4"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                >
                                    <path d="M6 18L18 6M6 6l12 12" />
                                </svg>
                            </button>
                            {#if item.position > 0}
                                <div class="pw-cw-bar"><div style="width: {pct(item)}%"></div></div>
                            {:else}
                                <span class="pw-cw-up">{t('cw.next_up')}</span>
                            {/if}
                        </div>
                        <h3 class="pw-cw-title">{item.media_title}</h3>
                        <div class="pw-cw-meta">
                            {#if tag}<span>{tag}</span><span class="pw-cw-dot">-</span>{/if}
                            {#if item.position > 0}
                                <span>{t('cw.left', { t: fmtLeft(item.position, item.duration) })}</span>
                            {:else if item.duration > 0}
                                <span>{fmtLeft(0, item.duration)}</span>
                            {/if}
                        </div>
                    </div>
                {/each}
            </div>
        </div>
    </div>
{/if}

<style>
    .pw-cw-row {
        gap: 16px;
        padding: 6px 6px 22px;
        scroll-padding: 0 6px;
    }

    .pw-cw-card {
        flex-shrink: 0;
        scroll-snap-align: start;
    }

    .pw-cw-frame {
        position: relative;
        width: 100%;
        aspect-ratio: 16 / 9;
        border-radius: 8px;
        overflow: hidden;
        background: #111;
        cursor: pointer;
        box-shadow: 0 8px 24px -12px rgba(0, 0, 0, 0.8);
        transition:
            transform 0.25s cubic-bezier(0.2, 0.7, 0.2, 1),
            box-shadow 0.25s ease;
    }
    .pw-cw-frame img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
        transition:
            transform 0.5s cubic-bezier(0.2, 0.7, 0.2, 1),
            filter 0.25s ease;
    }
    .pw-cw-frame:hover,
    .pw-cw-frame:focus-visible {
        outline: none;
        transform: translateY(-3px);
        box-shadow:
            0 18px 40px -16px rgba(0, 0, 0, 0.9),
            0 0 0 1.5px color-mix(in oklch, var(--pw-accent) 60%, transparent);
    }
    .pw-cw-frame:hover img {
        transform: scale(1.03);
        filter: brightness(1.05);
    }

    .pw-cw-veil {
        position: absolute;
        inset: 0;
        background: linear-gradient(to top, rgba(0, 0, 0, 0.55) 0%, transparent 50%);
        pointer-events: none;
    }

    .pw-cw-play {
        position: absolute;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        width: 44px;
        height: 44px;
        display: grid;
        place-items: center;
        pointer-events: none;
        opacity: 0;
        transition: opacity 0.15s ease;
    }
    .pw-cw-frame:hover .pw-cw-play {
        opacity: 1;
    }
    .pw-cw-play svg {
        width: 60%;
        height: 60%;
        filter: drop-shadow(0 0 10px color-mix(in oklch, var(--pw-accent) 55%, transparent))
            drop-shadow(0 2px 6px rgba(0, 0, 0, 0.7));
    }

    .pw-cw-x {
        position: absolute;
        top: 6px;
        right: 6px;
        width: 22px;
        height: 22px;
        border-radius: 999px;
        background: rgba(0, 0, 0, 0.65);
        backdrop-filter: blur(4px);
        color: rgba(232, 232, 234, 0.7);
        display: grid;
        place-items: center;
        border: none;
        cursor: pointer;
        opacity: 0;
        transition:
            opacity 0.15s ease,
            background 0.15s ease,
            color 0.15s ease;
        z-index: 2;
    }
    .pw-cw-frame:hover .pw-cw-x {
        opacity: 1;
    }
    .pw-cw-x:hover {
        background: oklch(0.65 0.21 25);
        color: #fff;
    }

    .pw-cw-up {
        position: absolute;
        left: 8px;
        bottom: 8px;
        font-size: 10.5px;
        font-weight: 600;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: #fff;
        background: color-mix(in oklch, var(--pw-accent) 80%, #000);
        padding: 3px 7px;
        border-radius: 4px;
        box-shadow: 0 2px 6px rgba(0, 0, 0, 0.45);
    }
    .pw-cw-bar {
        position: absolute;
        left: 0;
        right: 0;
        bottom: 0;
        height: 3px;
        background: rgba(0, 0, 0, 0.5);
    }
    .pw-cw-bar > div {
        height: 100%;
        background: var(--pw-accent);
    }

    .pw-cw-title {
        margin: 10px 0 2px;
        font-size: 14px;
        color: #ececef;
        font-weight: 500;
        letter-spacing: -0.005em;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        transition: color 0.15s ease;
    }
    .pw-cw-frame:hover + .pw-cw-title {
        color: #fff;
    }

    .pw-cw-meta {
        font-size: 12.5px;
        color: rgba(220, 220, 225, 0.55);
        display: flex;
        gap: 6px;
        font-weight: 400;
    }
    .pw-cw-dot {
        opacity: 0.45;
    }
</style>
