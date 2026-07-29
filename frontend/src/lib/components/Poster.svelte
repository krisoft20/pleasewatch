<script lang="ts">
    import type { Media } from '$lib/api';
    import { goto } from '$app/navigation';

    let {
        item,
        width = 200,
        onRemove,
        priority = false,
        pct = 0,
        showActivity = false
    }: {
        item: Media;
        width?: number;
        onRemove?: (m: Media) => void;
        priority?: boolean;
        pct?: number;
        showActivity?: boolean;
    } = $props();

    const kind = $derived(item.is_anime ? 'anime' : item.media_type === 'tv' ? 'series' : 'movie');

    function open() {
        const route = item.media_type === 'tv' ? 'tv' : 'movie';
        const id = item.tmdb_id ?? item.id;
        goto(`/${route}/${id}`);
    }

    function remove(ev: MouseEvent) {
        ev.preventDefault();
        ev.stopPropagation();
        onRemove?.(item);
    }

    function fmtDuration(secs?: number | null): string {
        if (!secs) return '';
        const h = Math.floor(secs / 3600);
        const m = Math.floor((secs % 3600) / 60);
        return h > 0 ? `${h}h ${m}m` : `${m}m`;
    }
</script>

<div
    class="group flex-shrink-0 cursor-pointer"
    style="width: {width}px;"
    onclick={open}
    onkeydown={(e) => e.key === 'Enter' && open()}
    role="button"
    tabindex="0"
>
    <div class="pw-card-frame aspect-[2/3] relative rounded-lg overflow-hidden bg-gray-900">
        {#if item.poster_url}
            <img
                src={item.poster_url.replace('/w500/', '/w342/')}
                alt={item.title}
                loading={priority ? 'eager' : 'lazy'}
                fetchpriority={priority ? 'high' : 'low'}
                decoding="async"
                class="pw-card-img w-full h-full object-cover"
            />
        {:else}
            <div class="w-full h-full flex items-center justify-center text-gray-700">
                <svg class="w-12 h-12" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M7 4v16M17 4v16M3 8h4m10 0h4M3 12h18M3 16h4m10 0h4M4 20h16a1 1 0 001-1V5a1 1 0 00-1-1H4a1 1 0 00-1 1v14a1 1 0 001 1z"
                    />
                </svg>
            </div>
        {/if}

        <span
            class="absolute top-2 left-2 h-6 flex items-center text-[10px] font-medium text-white/90 px-2 rounded bg-black/60 uppercase tracking-wide z-10"
        >
            {kind}
        </span>

        {#if showActivity && item.activity_label}
            <span class="pw-card-activity">
                new episode {item.activity_label}
            </span>
        {/if}

        {#if onRemove}
            <button
                onclick={remove}
                class="absolute top-2 right-2 w-6 h-6 bg-black/65 hover:bg-red-600 rounded-full flex items-center justify-center transition-colors opacity-0 group-hover:opacity-100 z-10"
                aria-label="remove"
            >
                <svg class="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="3">
                    <path d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
        {/if}

        <div class="pw-card-overlay absolute inset-0 flex items-center justify-center pointer-events-none">
            <svg class="w-12 h-12 text-white" fill="currentColor" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z" />
            </svg>
        </div>

        {#if pct > 0}
            <div class="pw-card-bar"><div style="width: {pct}%"></div></div>
        {/if}
    </div>

    <h3 class="mt-2 text-sm font-medium text-gray-200 truncate group-hover:text-white transition-colors">
        {item.title}
    </h3>
    <div class="flex items-center gap-2 text-xs text-gray-500">
        {#if item.year}<span>{item.year}</span>{/if}
        {#if item.duration}<span>{fmtDuration(item.duration)}</span>{/if}
    </div>
</div>
