<script lang="ts">
    import { goto } from '$app/navigation';

    type Item = { md_id: string; title: string; cover_url: string | null };

    let {
        title,
        items,
        width = 200,
        onRemove,
        sub
    }: {
        title: string;
        items: Item[];
        width?: number;
        onRemove?: (m: Item) => void;
        sub?: Record<string, string>;
    } = $props();

    function open(m: Item) {
        goto(`/manga/${m.md_id}`);
    }
</script>

<div class="pw-shelf">
    <div class="pw-row-head">
        <h2 class="pw-h2">{title}<span class="pw-count">{items.length}</span></h2>
    </div>
    <div class="pw-shelf-grid" style="--pw-poster-w: {width}px;">
        {#each items as m, i (m.md_id)}
            <div
                class="group flex-shrink-0 cursor-pointer"
                style="width: {width}px;"
                onclick={() => open(m)}
                onkeydown={(e) => e.key === 'Enter' && open(m)}
                role="button"
                tabindex="0"
            >
                <div class="pw-card-frame aspect-[2/3] relative rounded-lg overflow-hidden bg-gray-900">
                    {#if m.cover_url}
                        <img
                            src={m.cover_url}
                            alt={m.title}
                            loading={i < 8 ? 'eager' : 'lazy'}
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
                                    d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2zM22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"
                                />
                            </svg>
                        </div>
                    {/if}

                    {#if onRemove}
                        <button
                            onclick={(e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                onRemove(m);
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
                </div>

                <h3 class="mt-2 text-sm font-medium text-gray-200 truncate group-hover:text-white transition-colors">
                    {m.title}
                </h3>
                {#if sub?.[m.md_id]}
                    <div class="text-xs text-gray-500">{sub[m.md_id]}</div>
                {/if}
            </div>
        {/each}
    </div>
</div>
