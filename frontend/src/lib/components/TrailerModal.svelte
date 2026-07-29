<script lang="ts">
    let { videoKey, onClose }: { videoKey: string; onClose: () => void } = $props();

    $effect(() => {
        const onKey = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        document.addEventListener('keydown', onKey);
        return () => document.removeEventListener('keydown', onKey);
    });

    function onBackdrop(e: MouseEvent) {
        if (e.target === e.currentTarget) onClose();
    }
</script>

<div
    class="pw-trailer-bg"
    onclick={onBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
>
    <div class="pw-trailer-frame">
        <button class="pw-trailer-close" onclick={onClose} aria-label="close">
            <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.4"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
        </button>
        <iframe
            src="https://www.youtube.com/embed/{videoKey}?autoplay=1&rel=0"
            title="trailer"
            frameborder="0"
            allow="autoplay; encrypted-media; fullscreen"
            allowfullscreen
        ></iframe>
    </div>
</div>

<style>
    .pw-trailer-bg {
        position: fixed;
        inset: 0;
        z-index: 200;
        background: rgba(0, 0, 0, 0.85);
        backdrop-filter: blur(8px);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
    }
    .pw-trailer-frame {
        position: relative;
        width: 100%;
        max-width: 1100px;
        aspect-ratio: 16 / 9;
        background: #000;
        border-radius: 8px;
        overflow: hidden;
        box-shadow: 0 24px 60px -8px rgba(0, 0, 0, 0.8);
    }
    .pw-trailer-frame iframe {
        width: 100%;
        height: 100%;
        border: 0;
        display: block;
    }
    .pw-trailer-close {
        position: absolute;
        top: -40px;
        right: 0;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: #fff;
        width: 32px;
        height: 32px;
        border-radius: 999px;
        display: grid;
        place-items: center;
        cursor: pointer;
        transition: background 0.15s ease;
    }
    .pw-trailer-close:hover {
        background: rgba(255, 255, 255, 0.15);
    }
</style>
