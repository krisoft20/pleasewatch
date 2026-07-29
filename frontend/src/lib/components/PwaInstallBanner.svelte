<script lang="ts">
    import { onMount } from 'svelte';

    let visible = $state(false);
    let mode = $state<'android' | 'ios' | null>(null);
    let deferredPrompt: any = null;

    function isStandalone(): boolean {
        if (typeof window === 'undefined') return false;
        if (window.matchMedia?.('(display-mode: standalone)').matches) return true;
        if ((navigator as any).standalone === true) return true;
        return false;
    }

    function isIos(): boolean {
        return /iPhone|iPad|iPod/i.test(navigator.userAgent);
    }

    function isSafari(): boolean {
        const ua = navigator.userAgent;
        return /Safari/i.test(ua) && !/CriOS|FxiOS|EdgiOS/i.test(ua);
    }

    onMount(() => {
        if (isStandalone()) return;
        if (localStorage.getItem('pwa_install_dismissed')) return;

        if (isIos() && isSafari()) {
            mode = 'ios';
            setTimeout(() => (visible = true), 800);
            return;
        }

        window.addEventListener('beforeinstallprompt', (e: any) => {
            e.preventDefault();
            deferredPrompt = e;
            mode = 'android';
            visible = true;
        });
    });

    async function install() {
        if (!deferredPrompt) return;
        deferredPrompt.prompt();
        const choice = await deferredPrompt.userChoice;
        if (choice.outcome === 'accepted') {
            localStorage.setItem('pwa_install_dismissed', '1');
            visible = false;
        }
        deferredPrompt = null;
    }

    function dismiss() {
        localStorage.setItem('pwa_install_dismissed', '1');
        visible = false;
    }
</script>

{#if visible}
    <div class="pw-pwa-banner" class:is-ios={mode === 'ios'}>
        <div class="pw-pwa-inner">
            {#if mode === 'android'}
                <span class="pw-pwa-icon" aria-hidden="true">
                    <svg
                        width="20"
                        height="20"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                        <polyline points="7 10 12 15 17 10" />
                        <line x1="12" y1="15" x2="12" y2="3" />
                    </svg>
                </span>
                <div class="pw-pwa-text">
                    <strong>install pleasewatch</strong>
                    <span>no browser bar, opens like an app</span>
                </div>
                <button class="pw-pwa-cta" onclick={install}>install</button>
            {:else}
                <span class="pw-pwa-icon" aria-hidden="true">
                    <svg
                        width="20"
                        height="20"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <path d="M12 3v12" />
                        <polyline points="7 8 12 3 17 8" />
                        <path d="M5 12v7a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-7" />
                    </svg>
                </span>
                <div class="pw-pwa-text">
                    <strong>add to home screen</strong>
                    <span>tap the share icon, then add to home screen</span>
                </div>
            {/if}
            <button class="pw-pwa-x" onclick={dismiss} aria-label="dismiss">
                <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"><path d="M6 18L18 6M6 6l12 12" /></svg
                >
            </button>
        </div>
    </div>
{/if}

<style>
    .pw-pwa-banner {
        position: fixed;
        top: env(safe-area-inset-top, 0);
        left: 0;
        right: 0;
        z-index: 100;
        background: linear-gradient(180deg, #1d1222 0%, #14091a 100%);
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
        animation: pw-pwa-slide 0.32s cubic-bezier(0.2, 0.7, 0.2, 1) both;
    }
    @keyframes pw-pwa-slide {
        from {
            transform: translateY(-100%);
            opacity: 0;
        }
        to {
            transform: translateY(0);
            opacity: 1;
        }
    }
    .pw-pwa-inner {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 10px 14px;
        max-width: 720px;
        margin: 0 auto;
        color: #fff;
    }
    .pw-pwa-icon {
        flex-shrink: 0;
        width: 32px;
        height: 32px;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.06);
        display: grid;
        place-items: center;
        color: var(--pw-accent, #c084fc);
    }
    .pw-pwa-text {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 1px;
        line-height: 1.3;
    }
    .pw-pwa-text strong {
        font-size: 13px;
        font-weight: 600;
        letter-spacing: 0.01em;
    }
    .pw-pwa-text span {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.6);
    }
    .pw-pwa-cta {
        flex-shrink: 0;
        background: var(--pw-accent, #c084fc);
        color: #0a0612;
        border: none;
        padding: 7px 14px;
        border-radius: 6px;
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0.02em;
        cursor: pointer;
        text-transform: lowercase;
    }
    .pw-pwa-cta:hover {
        filter: brightness(1.1);
    }
    .pw-pwa-cta:active {
        transform: translateY(1px);
    }
    .pw-pwa-x {
        flex-shrink: 0;
        width: 26px;
        height: 26px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.06);
        border: none;
        color: rgba(255, 255, 255, 0.55);
        display: grid;
        place-items: center;
        cursor: pointer;
        padding: 0;
        transition:
            background 0.15s ease,
            color 0.15s ease;
    }
    .pw-pwa-x:hover {
        background: rgba(255, 255, 255, 0.12);
        color: #fff;
    }
</style>
