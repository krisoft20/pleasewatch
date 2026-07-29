<script lang="ts">
    import { goto } from '$app/navigation';
    import type { TmdbSearchItem } from '$lib/api';
    import Icon from '$lib/components/Icon.svelte';

    let { items }: { items: TmdbSearchItem[] } = $props();

    const heroItems = $derived(items.filter((i) => !!i.backdrop_url).slice(0, 5));
    let idx = $state(0);
    let timer: ReturnType<typeof setInterval> | null = null;

    $effect(() => {
        if (heroItems.length <= 1) return;
        timer = setInterval(() => {
            idx = (idx + 1) % heroItems.length;
        }, 6000);
        return () => {
            if (timer) clearInterval(timer);
        };
    });

    function go(item: TmdbSearchItem) {
        const route = item.media_type === 'tv' ? 'tv' : 'movie';
        goto(`/${route}/${item.tmdb_id}`);
    }

    function pick(i: number) {
        idx = i;
        if (timer) {
            clearInterval(timer);
            timer = setInterval(() => {
                idx = (idx + 1) % heroItems.length;
            }, 6000);
        }
    }
</script>

{#if heroItems.length > 0}
    {@const hero = heroItems[idx]}
    <div class="pw-dh">
        {#key idx}
            <img class="pw-dh-img" src={hero.backdrop_url} alt="" fetchpriority="high" decoding="async" />
        {/key}
        <div class="pw-dh-grad-y"></div>
        <div class="pw-dh-grad-x"></div>

        <div class="pw-dh-content">
            <h1 class="pw-dh-title">{hero.title}</h1>
            <div class="pw-dh-meta">
                {#if hero.year}<span>{hero.year}</span>{/if}
                {#if hero.vote_average}
                    <span class="pw-dh-rating">★ {hero.vote_average.toFixed(1)}</span>
                {/if}
                <span class="pw-dh-chip">{hero.media_type === 'tv' ? 'TV' : 'Movie'}</span>
            </div>
            {#if hero.overview}
                <p class="pw-dh-overview">{hero.overview}</p>
            {/if}
            <button class="pw-dh-cta" onclick={() => go(hero)} type="button">
                <Icon name="play" class="w-4 h-4" />
                <span>View</span>
            </button>
        </div>

        {#if heroItems.length > 1}
            <div class="pw-dh-dots">
                {#each heroItems as _, i}
                    <button
                        type="button"
                        class="pw-dh-dot"
                        class:is-active={i === idx}
                        onclick={() => pick(i)}
                        aria-label="hero slide {i + 1}"
                    ></button>
                {/each}
            </div>
        {/if}
    </div>
{/if}

<style>
    .pw-dh {
        position: relative;
        width: 100%;
        height: clamp(360px, 52vw, 580px);
        overflow: hidden;
        background: #000;
        margin-bottom: 16px;
    }
    .pw-dh-img {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: cover;
        object-position: center 20%;
        animation: pw-dh-in 0.9s ease-out;
    }
    @keyframes pw-dh-in {
        from {
            opacity: 0;
            transform: scale(1.04);
        }
        to {
            opacity: 1;
            transform: scale(1);
        }
    }
    .pw-dh-grad-y {
        position: absolute;
        inset: 0;
        background: linear-gradient(to top, var(--pw-bg) 0%, rgba(7, 8, 10, 0.5) 45%, rgba(7, 8, 10, 0.2) 100%);
        pointer-events: none;
    }
    .pw-dh-grad-x {
        position: absolute;
        inset: 0;
        background: linear-gradient(
            to right,
            rgba(7, 8, 10, 0.92) 0%,
            rgba(7, 8, 10, 0.55) 35%,
            rgba(7, 8, 10, 0.15) 65%,
            transparent 100%
        );
        pointer-events: none;
    }
    .pw-dh-content {
        position: absolute;
        left: 0;
        right: 0;
        bottom: 56px;
        max-width: 1800px;
        margin: 0 auto;
        padding: 0 clamp(16px, 4vw, 36px);
        z-index: 2;
    }
    .pw-dh-title {
        font-size: clamp(28px, 4vw, 52px);
        font-weight: 600;
        color: #fff;
        margin: 0 0 10px;
        letter-spacing: -0.02em;
        line-height: 1.05;
        max-width: 48%;
        text-shadow: 0 2px 18px rgba(0, 0, 0, 0.55);
    }
    @media (max-width: 900px) {
        .pw-dh-title {
            max-width: 85%;
        }
    }

    .pw-dh-meta {
        display: flex;
        align-items: center;
        gap: 10px;
        font-size: 13px;
        color: rgba(232, 232, 234, 0.85);
        margin-bottom: 12px;
    }
    .pw-dh-rating {
        color: #f5c542;
    }
    .pw-dh-chip {
        background: rgba(255, 255, 255, 0.12);
        padding: 2px 8px;
        border-radius: 4px;
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: rgba(232, 232, 234, 0.9);
    }

    .pw-dh-overview {
        font-size: 14.5px;
        color: rgba(232, 232, 234, 0.82);
        line-height: 1.55;
        max-width: 44%;
        margin: 0 0 18px;
        display: -webkit-box;
        -webkit-line-clamp: 3;
        line-clamp: 3;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    @media (max-width: 900px) {
        .pw-dh-overview {
            max-width: 85%;
            -webkit-line-clamp: 2;
            line-clamp: 2;
        }
    }

    .pw-dh-cta {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        background: #fff;
        color: #08090b;
        font-weight: 600;
        font-size: 13.5px;
        padding: 10px 22px;
        border-radius: 7px;
        border: none;
        cursor: pointer;
        transition:
            background 0.15s ease,
            transform 0.08s ease;
    }
    .pw-dh-cta:hover {
        background: rgba(255, 255, 255, 0.88);
    }
    .pw-dh-cta:active {
        transform: translateY(1px);
    }

    .pw-dh-dots {
        position: absolute;
        bottom: 22px;
        right: clamp(16px, 4vw, 36px);
        display: flex;
        gap: 6px;
        z-index: 3;
    }
    .pw-dh-dot {
        width: 6px;
        height: 6px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.32);
        border: none;
        cursor: pointer;
        padding: 0;
        transition:
            background 0.15s ease,
            width 0.2s ease;
    }
    .pw-dh-dot:hover {
        background: rgba(255, 255, 255, 0.55);
    }
    .pw-dh-dot.is-active {
        background: #fff;
        width: 24px;
    }
</style>
