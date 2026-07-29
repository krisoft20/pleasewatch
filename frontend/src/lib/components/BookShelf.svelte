<script lang="ts">
    import { goto, preloadData } from '$app/navigation';

    type Item = { ol_key: string; title: string; cover_url: string | null; authors?: string | null };

    let {
        title,
        items,
        width = 160,
        onRemove,
        sub,
        done
    }: {
        title: string;
        items: Item[];
        width?: number;
        onRemove?: (b: Item) => void;
        sub?: Record<string, string>;
        done?: Set<string>;
    } = $props();

    function open(b: Item) {
        goto(`/book/${b.ol_key}`);
    }

    function retryCoverFromOpenLibrary(event: Event, coverUrl: string) {
        const image = event.currentTarget as HTMLImageElement;
        const coverId = coverUrl.match(/\/api\/books\/cover\/(\d+)/)?.[1];
        if (!coverId || image.dataset.openLibraryFallback) return;
        image.dataset.openLibraryFallback = 'true';
        image.src = `https://covers.openlibrary.org/b/id/${coverId}-L.jpg`;
    }
</script>

<div class="pw-shelf">
    <div class="pw-row-head">
        <h2 class="pw-h2">{title}<span class="pw-count">{items.length}</span></h2>
    </div>
    <div class="pw-shelf-grid" style="--pw-poster-w: {width}px;">
        {#each items as b, i (b.ol_key)}
            <div
                class="group flex-shrink-0 cursor-pointer"
                style="width: {width}px;"
                onclick={() => open(b)}
                onkeydown={(e) => e.key === 'Enter' && open(b)}
                onpointerenter={() => preloadData(`/book/${b.ol_key}`)}
                role="button"
                tabindex="0"
            >
                <div
                    class="pw-card-frame aspect-[2/3] relative rounded-lg overflow-hidden bg-gray-900"
                    class:pw-plat-frame={done?.has(b.ol_key)}
                >
                    {#if b.cover_url}
                        <img
                            src={b.cover_url}
                            alt={b.title}
                            loading={i < 8 ? 'eager' : 'lazy'}
                            decoding="async"
                            onerror={(event) => retryCoverFromOpenLibrary(event, b.cover_url!)}
                            class="pw-card-img w-full h-full object-cover"
                        />
                    {:else}
                        <div class="w-full h-full flex items-center justify-center text-gray-700">
                            <svg class="w-12 h-12" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    stroke-width="1.5"
                                    d="M4 4h12a2 2 0 0 1 2 2v14H6a2 2 0 0 1-2-2zM4 4v16"
                                />
                            </svg>
                        </div>
                    {/if}

                    {#if onRemove}
                        <button
                            onclick={(e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                onRemove(b);
                            }}
                            class="absolute top-2 right-2 w-6 h-6 bg-black/65 hover:bg-red-600 rounded-full flex items-center justify-center transition-colors opacity-0 group-hover:opacity-100 z-10"
                            aria-label="remove"
                        >
                            <svg
                                class="w-3 h-3 text-white"
                                fill="none"
                                stroke="currentColor"
                                viewBox="0 0 24 24"
                                stroke-width="3"
                            >
                                <path d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    {/if}

                    {#if done?.has(b.ol_key)}
                        <span class="pw-plat">
                            <svg
                                width="12"
                                height="12"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="#0b1116"
                                stroke-width="3.4"
                                stroke-linecap="round"
                                stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                            >
                        </span>
                    {/if}
                </div>

                <h3 class="mt-2 text-sm font-medium text-gray-200 truncate group-hover:text-white transition-colors">
                    {b.title}
                </h3>
                {#if sub?.[b.ol_key]}
                    <div class="text-xs text-gray-500">{sub[b.ol_key]}</div>
                {:else if b.authors}
                    <div class="text-xs text-gray-500 truncate">{b.authors}</div>
                {/if}
            </div>
        {/each}
    </div>
</div>
