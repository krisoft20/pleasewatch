<script lang="ts">
    import { t } from '$lib/i18n';
    import Icon from './Icon.svelte';

    type Props = { id: string; title: string; onBack: () => void };
    let { id, title, onBack }: Props = $props();

    const src = $derived(
        `https://www.youtube-nocookie.com/embed/${encodeURIComponent(id)}?autoplay=1&playsinline=1&rel=0`
    );

    function onKey(e: KeyboardEvent) {
        if (e.key === 'Escape') onBack();
    }
</script>

<svelte:window on:keydown={onKey} />

<div class="fixed inset-0 z-50 bg-black flex flex-col">
    <div class="flex items-center gap-3 px-4 py-3 shrink-0">
        <button onclick={onBack} class="text-white/80 hover:text-white transition-colors p-1 -m-1" aria-label="back">
            <Icon name="chevron-left" class="w-6 h-6" />
        </button>
        <h2 class="text-white text-[15px] font-semibold truncate">{title}</h2>
        <span
            class="ml-auto shrink-0 text-[10px] uppercase tracking-wider text-gray-400 border border-white/15 rounded px-2 py-0.5"
        >
            {$t('live.youtube.badge')}
        </span>
    </div>
    <div class="flex-1 min-h-0">
        <iframe
            {src}
            {title}
            class="w-full h-full border-0"
            allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
            allowfullscreen
        ></iframe>
    </div>
</div>
