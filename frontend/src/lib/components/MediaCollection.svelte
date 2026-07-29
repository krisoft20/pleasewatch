<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type CollectionItem, type CollectionKind, type CollectionStatus } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import CollectionCard, { type CollectionCardItem } from './CollectionCard.svelte';

    type Filter = 'all' | CollectionKind;

    const filters: { key: Filter; label: string }[] = [
        { key: 'all', label: 'collection.filter.all' },
        { key: 'movie', label: 'collection.filter.movies' },
        { key: 'tv', label: 'collection.filter.series' },
        { key: 'anime', label: 'collection.filter.anime' }
    ];

    const shelves: { status: CollectionStatus; label: string }[] = [
        { status: 'in_progress', label: 'collection.in_progress' },
        { status: 'planned', label: 'collection.planned' },
        { status: 'completed', label: 'collection.completed' }
    ];

    let items = $state<CollectionCardItem[]>([]);
    let loaded = $state(false);
    let filter = $state<Filter>('all');
    let busyKey = $state<string | null>(null);
    let notice = $state('');

    onMount(() => {
        api.collectionList()
            .then((media) => {
                items = media.map(normalizeMedia);
            })
            .catch((e) => {
                console.error('[collection] load failed', e);
                notice = t('collection.error.load');
            })
            .finally(() => (loaded = true));
    });

    const visible = $derived(filter === 'all' ? items : items.filter((item) => item.kind === filter));
    const showcase = $derived(
        [...items.filter((item) => item.showcased && item.status === 'completed')].sort(byRecent)
    );

    function normalizeMedia(item: CollectionItem): CollectionCardItem {
        return {
            key: `${item.kind}:${item.tmdb_id}`,
            kind: item.kind,
            tmdb_id: item.tmdb_id,
            title: item.title,
            meta: item.year,
            poster_url: item.poster_url,
            status: item.status,
            showcased: item.showcased,
            updated_at: item.updated_at,
            completed_at: item.completed_at
        };
    }

    function byRecent(a: CollectionCardItem, b: CollectionCardItem): number {
        const left = a.completed_at ?? a.updated_at;
        const right = b.completed_at ?? b.updated_at;
        return right.localeCompare(left);
    }

    function rows(status: CollectionStatus): CollectionCardItem[] {
        return visible.filter((item) => item.status === status).sort(byRecent);
    }

    function filterCount(kind: Filter): number {
        return kind === 'all' ? items.length : items.filter((item) => item.kind === kind).length;
    }

    async function toggleShowcase(item: CollectionCardItem) {
        if (busyKey) return;
        busyKey = item.key;
        notice = '';

        try {
            const saved = await api.collectionUpdate(item.kind, item.tmdb_id, {
                showcased: !item.showcased
            });
            items = items.map((row) => (row.key === item.key ? normalizeMedia(saved) : row));
        } catch (e) {
            const message = e instanceof Error ? e.message : '';
            notice =
                message.includes('five') || message.includes('5')
                    ? t('collection.error.showcase_full')
                    : t('collection.error.update');
        } finally {
            busyKey = null;
        }
    }
</script>

{#snippet grid(rows: CollectionCardItem[])}
    <div class="collection-grid">
        {#each rows as item (item.key)}
            <CollectionCard {item} busy={busyKey === item.key} onShowcase={toggleShowcase} />
        {/each}
    </div>
{/snippet}

{#if !loaded}
    <section class="pw-section pw-empty">
        <div class="pw-empty-card"><div class="pw-empty-tag">{t('lib.loading')}</div></div>
    </section>
{:else}
    <main class="pw-section collection">
        <div class="collection-head">
            <h1>{t('collection.title')}<span>{items.length}</span></h1>
        </div>

        <div class="pw-chiprow" aria-label={t('collection.filter.label')}>
            {#each filters as option (option.key)}
                <button
                    class:is-active={filter === option.key}
                    class="pw-chip"
                    type="button"
                    aria-pressed={filter === option.key}
                    onclick={() => (filter = option.key)}
                >
                    {t(option.label)}
                    <span class="pw-chip-n">{filterCount(option.key)}</span>
                </button>
            {/each}
        </div>

        {#if notice}
            <div class="collection-notice" role="status">{notice}</div>
        {/if}

        {#if items.length === 0}
            <div class="collection-empty">
                <h2>{t('collection.empty.title')}</h2>
                <p>{t('collection.empty.body')}</p>
            </div>
        {:else if visible.length === 0}
            <div class="collection-empty">
                <h2>{t('collection.filter.empty_title')}</h2>
                <p>{t('collection.filter.empty_body')}</p>
            </div>
        {:else}
            {#if showcase.length > 0}
                <section class="collection-shelf">
                    <div class="pw-row-head">
                        <h2 class="pw-h2">
                            {t('collection.showcase.title')}
                            <span class="pw-count">{showcase.length}/5</span>
                        </h2>
                    </div>
                    {@render grid(showcase)}
                </section>
            {/if}

            {#each shelves as shelf (shelf.status)}
                {@const shelfRows = rows(shelf.status)}
                {#if shelfRows.length > 0}
                    <section class="collection-shelf">
                        <div class="pw-row-head">
                            <h2 class="pw-h2">
                                {t(shelf.label)}
                                <span class="pw-count">{shelfRows.length}</span>
                            </h2>
                        </div>
                        {@render grid(shelfRows)}
                    </section>
                {/if}
            {/each}
        {/if}
    </main>
{/if}

<style>
    .collection {
        padding-top: 10px;
        padding-bottom: 72px;
    }

    .collection-head {
        display: flex;
        align-items: center;
        min-height: 42px;
    }

    .collection-head h1 {
        display: inline-flex;
        align-items: baseline;
        gap: 10px;
        margin: 0;
        color: #f4f4f6;
        font-size: 26px;
        font-weight: 600;
        letter-spacing: -0.025em;
    }

    .collection-head h1 span {
        color: rgba(220, 220, 225, 0.42);
        font-size: 13px;
        font-weight: 500;
        letter-spacing: 0;
    }

    .collection-shelf {
        margin-top: 30px;
    }

    .collection-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
        gap: 20px 14px;
    }

    .collection-notice {
        margin-top: 14px;
        color: #e8cf91;
        font-size: 12px;
    }

    .collection-empty {
        max-width: 460px;
        padding: 52px 0;
    }

    .collection-empty h2 {
        margin: 0;
        color: #e8e8eb;
        font-size: 17px;
        font-weight: 600;
    }

    .collection-empty p {
        margin: 6px 0 0;
        color: rgba(220, 220, 225, 0.46);
        font-size: 13px;
        line-height: 1.5;
    }

    button:focus-visible {
        outline: 2px solid var(--pw-accent);
        outline-offset: 3px;
    }

    @media (max-width: 640px) {
        .collection {
            padding-top: 4px;
        }

        .collection-head h1 {
            font-size: 23px;
        }

        .pw-chiprow {
            flex-wrap: nowrap;
            overflow-x: auto;
            padding-bottom: 2px;
            scrollbar-width: none;
        }

        .pw-chiprow::-webkit-scrollbar {
            display: none;
        }

        .collection-grid {
            grid-template-columns: repeat(2, minmax(0, 1fr));
            gap: 20px 12px;
        }
    }
</style>
