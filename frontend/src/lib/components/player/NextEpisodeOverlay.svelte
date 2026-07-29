<script lang="ts">
    import type { EpisodeRecord } from '$lib/types';

    let {
        episode: _episode,
        countdown,
        onPlayNow,
        onCancel
    }: {
        episode: EpisodeRecord;
        countdown: number;
        onPlayNow: () => void;
        onCancel: () => void;
    } = $props();

    let total = $state(1);
    let seededTotal = $state(false);

    $effect.pre(() => {
        if (seededTotal || countdown <= 0) return;
        total = countdown;
        seededTotal = true;
    });

    const fillPct = $derived(Math.max(0, Math.min(1, (total - countdown) / total)));

    function dismiss(e: Event) {
        e.stopPropagation();
        onCancel();
    }
</script>

<div class="pw-next-wrap">
    <button class="pw-next-x" onclick={dismiss} aria-label="cancel">
        <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.6"
            stroke-linecap="round"
            stroke-linejoin="round"><path d="M6 18L18 6M6 6l12 12" /></svg
        >
    </button>
    <button class="pw-next" onclick={onPlayNow} aria-label="play next episode">
        <span class="pw-next-fill" style="transform: scaleX({fillPct});"></span>
        <span class="pw-next-content">
            <svg class="pw-next-play" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M8 5v14l11-7z" />
            </svg>
            <span class="pw-next-label">NEXT EPISODE</span>
        </span>
    </button>
</div>

<style>
    .pw-next-wrap {
        position: absolute;
        bottom: 7rem;
        right: 1rem;
        z-index: 30;
        animation: pw-next-in 0.28s cubic-bezier(0.2, 0.7, 0.2, 1) both;
    }
    @media (min-width: 640px) {
        .pw-next-wrap {
            bottom: 8rem;
            right: 2rem;
        }
    }
    @keyframes pw-next-in {
        from {
            opacity: 0;
            transform: translateY(8px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    .pw-next {
        position: relative;
        display: inline-flex;
        align-items: center;
        padding: 0;
        border: none;
        cursor: pointer;
        background: linear-gradient(180deg, #ffffff 0%, #f4f4f6 100%);
        color: #08090b;
        border-radius: 6px;
        overflow: hidden;
        box-shadow:
            0 10px 28px -12px rgba(0, 0, 0, 0.7),
            0 2px 6px -2px rgba(0, 0, 0, 0.35),
            inset 0 1px 0 rgba(255, 255, 255, 0.8),
            inset 0 -1px 0 rgba(0, 0, 0, 0.06);
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, system-ui, sans-serif;
        transition:
            filter 0.15s ease,
            transform 0.1s ease;
    }
    .pw-next:hover {
        filter: brightness(0.97);
    }
    .pw-next:active {
        transform: translateY(1px);
    }

    .pw-next-fill {
        position: absolute;
        inset: 0;
        background: linear-gradient(180deg, rgba(15, 18, 22, 0.55) 0%, rgba(0, 0, 0, 0.5) 100%);
        transform-origin: right;
        transition: transform 1s linear;
        pointer-events: none;
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
    }

    .pw-next-content {
        position: relative;
        display: inline-flex;
        align-items: center;
        gap: 12px;
        padding: 13px 24px 13px 19px;
        z-index: 1;
    }
    .pw-next-play {
        width: 15px;
        height: 15px;
        flex-shrink: 0;
    }
    .pw-next-label {
        font-size: 14px;
        font-weight: 700;
        letter-spacing: 0.09em;
        line-height: 1;
    }
    @media (min-width: 640px) {
        .pw-next-content {
            padding: 14px 28px 14px 22px;
            gap: 13px;
        }
        .pw-next-play {
            width: 16px;
            height: 16px;
        }
        .pw-next-label {
            font-size: 15px;
        }
    }

    .pw-next-x {
        position: absolute;
        top: -8px;
        right: -8px;
        width: 20px;
        height: 20px;
        border-radius: 999px;
        background: rgba(0, 0, 0, 0.78);
        backdrop-filter: blur(8px);
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: rgba(255, 255, 255, 0.85);
        display: grid;
        place-items: center;
        cursor: pointer;
        padding: 0;
        z-index: 2;
        opacity: 0;
        transition:
            opacity 0.15s ease,
            background 0.15s ease,
            transform 0.1s ease;
    }
    .pw-next-wrap:hover .pw-next-x {
        opacity: 1;
    }
    .pw-next-x:hover {
        background: rgba(0, 0, 0, 0.9);
        color: #fff;
    }
    .pw-next-x:active {
        transform: scale(0.9);
    }
</style>
