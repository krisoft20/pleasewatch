<script lang="ts">
    import '../app.css';
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import BottomBar from '$lib/components/BottomBar.svelte';
    import Footer from '$lib/components/Footer.svelte';
    import PwaInstallBanner from '$lib/components/PwaInstallBanner.svelte';

    let { children } = $props();

    function isTyping(t: EventTarget | null): boolean {
        if (!(t instanceof HTMLElement)) return false;
        const tag = t.tagName.toLowerCase();
        if (tag === 'input' || tag === 'textarea' || tag === 'select') return true;
        if (t.isContentEditable) return true;
        return false;
    }

    let lastG = 0;

    function onKey(e: KeyboardEvent) {
        if (isTyping(e.target)) return;
        if (e.metaKey || e.ctrlKey || e.altKey) return;

        if (e.key === '/') {
            const input = document.querySelector<HTMLInputElement>(
                'input[placeholder*="search" i], input[type="search"]'
            );
            if (input) {
                e.preventDefault();
                input.focus();
                return;
            }
        }
        if (e.key === 'g') {
            lastG = Date.now();
            return;
        }
        if (lastG && Date.now() - lastG < 1200) {
            lastG = 0;
            if (e.key === 'l') {
                e.preventDefault();
                goto('/');
                return;
            }
            if (e.key === 'd') {
                e.preventDefault();
                goto('/?tab=discover');
                return;
            }
            if (e.key === 'a') {
                e.preventDefault();
                goto('/admin');
                return;
            }
        }
    }

    function onPreloadError(e: Event) {
        e.preventDefault();
        const key = 'pw-preload-reload';
        const last = Number(sessionStorage.getItem(key) || 0);
        if (Date.now() - last < 10000) return;
        sessionStorage.setItem(key, String(Date.now()));
        window.location.reload();
    }

    onMount(() => {
        window.addEventListener('keydown', onKey);
        window.addEventListener('vite:preloadError', onPreloadError);
        return () => {
            window.removeEventListener('keydown', onKey);
            window.removeEventListener('vite:preloadError', onPreloadError);
        };
    });
</script>

{@render children()}
<Footer />
<BottomBar />
<PwaInstallBanner />
