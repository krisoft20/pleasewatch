<script lang="ts">
    let {
        hasNext,
        onSkipCredits,
        onNextEpisode,
        onDismiss
    }: {
        hasNext: boolean;
        onSkipCredits: () => void;
        onNextEpisode: () => void;
        onDismiss: () => void;
    } = $props();

    function dismiss(e: Event) {
        e.stopPropagation();
        onDismiss();
    }
</script>

<div class="pw-cr-wrap">
    <button class="pw-cr-x" onclick={dismiss} aria-label="dismiss">
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
    <div class="pw-cr-row">
        <button class="pw-cr-btn pw-cr-btn-ghost" onclick={onSkipCredits} aria-label="skip credits">
            <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M5 4l10 8-10 8V4zm12 0h2v16h-2z" />
            </svg>
            <span>SKIP CREDITS</span>
        </button>
        {#if hasNext}
            <button class="pw-cr-btn pw-cr-btn-primary" onclick={onNextEpisode} aria-label="next episode">
                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <path d="M8 5v14l11-7z" />
                </svg>
                <span>NEXT EPISODE</span>
            </button>
        {/if}
    </div>
</div>

<style>
    .pw-cr-wrap {
        position: absolute;
        bottom: 7rem;
        right: 1rem;
        z-index: 30;
        animation: pw-cr-in 0.28s cubic-bezier(0.2, 0.7, 0.2, 1) both;
    }
    @media (min-width: 640px) {
        .pw-cr-wrap {
            bottom: 8rem;
            right: 2rem;
        }
    }
    @keyframes pw-cr-in {
        from {
            opacity: 0;
            transform: translateY(8px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    .pw-cr-row {
        display: flex;
        gap: 10px;
        align-items: stretch;
    }

    .pw-cr-btn {
        display: inline-flex;
        align-items: center;
        gap: 10px;
        padding: 13px 22px;
        border: none;
        border-radius: 6px;
        cursor: pointer;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, system-ui, sans-serif;
        font-size: 14px;
        font-weight: 700;
        letter-spacing: 0.09em;
        line-height: 1;
        transition:
            filter 0.15s ease,
            transform 0.1s ease,
            background 0.15s ease;
        box-shadow:
            0 10px 28px -12px rgba(0, 0, 0, 0.7),
            0 2px 6px -2px rgba(0, 0, 0, 0.35);
    }
    .pw-cr-btn svg {
        width: 15px;
        height: 15px;
        flex-shrink: 0;
    }
    .pw-cr-btn:hover {
        filter: brightness(1.06);
    }
    .pw-cr-btn:active {
        transform: translateY(1px);
    }

    .pw-cr-btn-ghost {
        background: rgba(20, 22, 26, 0.85);
        backdrop-filter: blur(8px);
        color: rgba(255, 255, 255, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.18);
    }
    .pw-cr-btn-primary {
        background: linear-gradient(180deg, #ffffff 0%, #f4f4f6 100%);
        color: #08090b;
        box-shadow:
            0 10px 28px -12px rgba(0, 0, 0, 0.7),
            0 2px 6px -2px rgba(0, 0, 0, 0.35),
            inset 0 1px 0 rgba(255, 255, 255, 0.8),
            inset 0 -1px 0 rgba(0, 0, 0, 0.06);
    }

    @media (min-width: 640px) {
        .pw-cr-btn {
            padding: 14px 26px;
            font-size: 15px;
        }
        .pw-cr-btn svg {
            width: 16px;
            height: 16px;
        }
    }

    .pw-cr-x {
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
    .pw-cr-wrap:hover .pw-cr-x {
        opacity: 1;
    }
    .pw-cr-x:hover {
        background: rgba(0, 0, 0, 0.9);
        color: #fff;
    }
    .pw-cr-x:active {
        transform: scale(0.9);
    }
</style>
