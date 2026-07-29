<script lang="ts">
    import type { TmdbCastMember } from '$lib/api';
    import { t } from '$lib/i18n.svelte';

    let { cast, onSelect }: { cast: TmdbCastMember[]; onSelect: (id: number) => void } = $props();

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
</script>

<div class="pw-cast-section">
    <h2 class="pw-cast-h">{t('media.cast')}</h2>
    <div class="pw-cast-wrap">
        <div class="pw-cast-fade pw-cast-fade-left" class:is-visible={canL}></div>
        <div class="pw-cast-fade pw-cast-fade-right" class:is-visible={canR}></div>
        <button
            class="pw-cast-arrow pw-cast-arrow-left"
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
            class="pw-cast-arrow pw-cast-arrow-right"
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
        <div class="pw-cast-row" bind:this={scrollEl}>
            {#each cast as person (person.id)}
                <button
                    class="pw-cast-card"
                    type="button"
                    onclick={() => onSelect(person.id)}
                    aria-label="more about {person.name}"
                >
                    <div class="pw-cast-photo">
                        {#if person.photo_url}
                            <img src={person.photo_url} alt={person.name} loading="lazy" />
                        {:else}
                            <svg
                                width="32"
                                height="32"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.5"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                ><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></svg
                            >
                        {/if}
                    </div>
                    <div class="pw-cast-name">{person.name}</div>
                    <div class="pw-cast-char">{person.character}</div>
                </button>
            {/each}
        </div>
    </div>
</div>

<style>
    .pw-cast-wrap {
        position: relative;
    }

    .pw-cast-arrow {
        position: absolute;
        top: 50%;
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
        transition:
            opacity 0.15s ease,
            background 0.15s ease,
            transform 0.12s cubic-bezier(0.2, 0.7, 0.2, 1);
        backdrop-filter: blur(8px);
    }
    .pw-cast-wrap:hover .pw-cast-arrow.is-enabled {
        opacity: 1;
        pointer-events: auto;
    }
    .pw-cast-arrow.is-enabled:hover {
        background: rgba(8, 9, 12, 0.95);
        border-color: rgba(255, 255, 255, 0.18);
        color: #fff;
    }
    .pw-cast-arrow.is-enabled:active {
        transform: translateY(-50%) scale(0.94);
    }
    .pw-cast-arrow-left {
        left: -8px;
    }
    .pw-cast-arrow-right {
        right: -8px;
    }
    @media (max-width: 640px) {
        .pw-cast-arrow {
            display: none;
        }
    }

    .pw-cast-fade {
        position: absolute;
        top: 0;
        bottom: 8px;
        width: 60px;
        pointer-events: none;
        z-index: 2;
        opacity: 0;
        transition: opacity 0.15s ease;
    }
    .pw-cast-fade.is-visible {
        opacity: 1;
    }
    .pw-cast-fade-left {
        left: 0;
        background: linear-gradient(to right, var(--pw-bg) 10%, transparent);
    }
    .pw-cast-fade-right {
        right: 0;
        background: linear-gradient(to left, var(--pw-bg) 10%, transparent);
    }
</style>
