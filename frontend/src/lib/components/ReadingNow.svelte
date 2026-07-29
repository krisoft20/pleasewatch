<script lang="ts">
    import { goto } from '$app/navigation';
    import { t, plural } from '$lib/i18n.svelte';

    type Item = {
        ol_key: string;
        title: string;
        cover_url: string | null;
        authors?: string | null;
        pages?: number | null;
        percent: number | null;
    };

    const CAP = 3;
    let { title, items }: { title: string; items: Item[] } = $props();
    let expanded = $state(false);
    const visible = $derived(expanded ? items : items.slice(0, CAP));
    const hidden = $derived(Math.max(0, items.length - CAP));

    function pagesLeft(s: Item): number | null {
        if (!s.pages || s.percent == null) return null;
        return Math.max(0, Math.round(s.pages * (1 - s.percent)));
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
    <div class="pw-rn-list">
        {#each visible as s (s.ol_key)}
            {@const left = pagesLeft(s)}
            <div class="pw-rn-card">
                <a class="pw-rn-cover" href={`/book/${s.ol_key}`}>
                    {#if s.cover_url}
                        <img
                            src={s.cover_url}
                            alt={s.title}
                            loading="eager"
                            onerror={(event) => retryCoverFromOpenLibrary(event, s.cover_url!)}
                        />
                    {/if}
                </a>
                <div class="pw-rn-info">
                    <a class="pw-rn-title" href={`/book/${s.ol_key}`}>{s.title}</a>
                    {#if s.authors}<div class="pw-rn-author">{s.authors}</div>{/if}
                    <div class="pw-rn-bar"><div style:width="{Math.round((s.percent ?? 0) * 100)}%"></div></div>
                    <div class="pw-rn-meta">
                        <span>{Math.round((s.percent ?? 0) * 100)}%</span>
                        {#if left != null}
                            <span>·</span>
                            <span>{plural('books.reader.left', left)}</span>
                        {/if}
                    </div>
                </div>
                <button class="pw-v1-btn-watch pw-rn-go" onclick={() => goto(`/read-book/${s.ol_key}`)}>
                    <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        ><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" /><path
                            d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"
                        /></svg
                    >
                    <span class="pw-rn-go-label">{t('books.action.continue')}</span>
                </button>
            </div>
        {/each}
    </div>
    {#if hidden > 0}
        <button class="pw-rn-more" onclick={() => (expanded = !expanded)}>
            {expanded ? t('books.show_less') : `${t('books.show_more')} (+${hidden})`}
        </button>
    {/if}
</div>

<style>
    .pw-rn-list {
        display: flex;
        flex-direction: column;
        gap: 10px;
    }
    .pw-rn-card {
        display: flex;
        align-items: center;
        gap: 12px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-radius: 16px;
        padding: 11px;
        max-width: 760px;
    }
    .pw-rn-cover {
        width: 50px;
        aspect-ratio: 2/3;
        border-radius: 8px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.06);
        flex-shrink: 0;
    }
    .pw-rn-cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .pw-rn-info {
        flex: 1;
        min-width: 0;
    }
    .pw-rn-title {
        display: block;
        font-size: 14px;
        font-weight: 600;
        color: #ececef;
        text-decoration: none;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-rn-title:hover {
        text-decoration: underline;
        text-underline-offset: 3px;
    }
    .pw-rn-author {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.45);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-rn-bar {
        height: 4px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.08);
        overflow: hidden;
        margin-top: 8px;
        max-width: 320px;
    }
    .pw-rn-bar > div {
        height: 100%;
        background: linear-gradient(90deg, var(--pw-accent), color-mix(in oklch, var(--pw-accent) 70%, white 30%));
        border-radius: 999px;
    }
    .pw-rn-meta {
        display: flex;
        gap: 6px;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        margin-top: 5px;
        font-variant-numeric: tabular-nums;
    }
    .pw-rn-go {
        flex-shrink: 0;
    }
    @media (max-width: 560px) {
        .pw-rn-go {
            padding: 0 13px;
        }
        .pw-rn-go-label {
            display: none;
        }
    }
    .pw-rn-more {
        margin-top: 10px;
        align-self: flex-start;
        background: transparent;
        border: 0;
        color: rgba(220, 220, 225, 0.55);
        font-size: 12.5px;
        cursor: pointer;
        padding: 6px 0;
    }
    .pw-rn-more:hover {
        color: #ececef;
    }
</style>
