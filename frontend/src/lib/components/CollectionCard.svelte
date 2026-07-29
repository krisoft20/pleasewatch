<script lang="ts" module>
    import type { CollectionKind, CollectionStatus } from '$lib/api';

    export type CollectionCardItem = {
        key: string;
        kind: CollectionKind;
        tmdb_id: number;
        title: string;
        meta: string | null;
        poster_url: string | null;
        status: CollectionStatus;
        showcased: boolean;
        updated_at: string;
        completed_at: string | null;
    };
</script>

<script lang="ts">
    import { i18n, t } from '$lib/i18n.svelte';
    import Icon from './Icon.svelte';

    let {
        item,
        busy = false,
        onShowcase
    }: {
        item: CollectionCardItem;
        busy?: boolean;
        onShowcase: (item: CollectionCardItem) => void;
    } = $props();

    function href(): string {
        return `/${item.kind === 'movie' ? 'movie' : 'tv'}/${item.tmdb_id}`;
    }

    function poster(): string {
        return item.poster_url?.replace('/w500/', '/w342/') ?? '';
    }

    function finished(): string {
        if (!item.completed_at) return '';
        const value = item.completed_at;
        const date = new Date(value.includes('T') ? value : value.replace(' ', 'T') + 'Z');
        if (Number.isNaN(date.getTime())) return '';
        return date.toLocaleDateString(i18n.lang.toLowerCase(), {
            day: 'numeric',
            month: 'short',
            year: 'numeric'
        });
    }
</script>

<article class="collection-card">
    <div class="cover-wrap">
        <a class:pw-plat-frame={item.status === 'completed'} class="cover" href={href()} aria-label={item.title}>
            {#if item.poster_url}
                <img src={poster()} alt={item.title} loading="lazy" decoding="async" />
            {:else}
                <span class="no-poster">
                    <Icon name="screen-text" class="w-10 h-10" strokeWidth={1.3} />
                </span>
            {/if}

            <span class="kind">{t(`collection.kind.${item.kind}`)}</span>

            {#if item.status === 'completed'}
                <span class="pw-plat" aria-hidden="true">
                    <Icon name="check-alt" class="w-3 h-3" strokeWidth={3.4} />
                </span>
            {/if}
        </a>

        {#if item.status === 'completed'}
            <button
                class:active={item.showcased}
                class="pin"
                type="button"
                disabled={busy}
                onclick={() => onShowcase(item)}
                aria-label={item.showcased ? t('collection.showcase.remove') : t('collection.showcase.add')}
                title={item.showcased ? t('collection.showcase.remove') : t('collection.showcase.add')}
            >
                <Icon name="trophy" class="w-3.5 h-3.5" strokeWidth={2} />
            </button>
        {/if}
    </div>

    <a class="title" href={href()}>{item.title}</a>
    <div class="meta">
        {#if item.meta}<span>{item.meta}</span>{/if}
        {#if item.status === 'completed' && item.completed_at}
            <span>{t('collection.completed_on', { date: finished() })}</span>
        {/if}
    </div>
</article>

<style>
    .collection-card {
        min-width: 0;
    }

    .cover-wrap {
        position: relative;
    }

    .cover {
        position: relative;
        display: block;
        aspect-ratio: 2 / 3;
        overflow: hidden;
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 8px;
        background: #101114;
        transition: border-color 0.15s ease;
    }

    .cover:hover {
        border-color: rgba(255, 255, 255, 0.28);
    }

    .cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }

    .no-poster {
        display: grid;
        width: 100%;
        height: 100%;
        place-items: center;
        background: #111318;
        color: #3f4652;
    }

    .kind {
        position: absolute;
        top: 7px;
        left: 7px;
        padding: 4px 6px;
        border-radius: 4px;
        background: rgba(5, 6, 8, 0.7);
        color: rgba(255, 255, 255, 0.78);
        font-size: 9px;
        font-weight: 600;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    .pin {
        position: absolute;
        z-index: 6;
        top: 7px;
        right: 7px;
        display: grid;
        width: 28px;
        height: 28px;
        padding: 0;
        place-items: center;
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 50%;
        background: rgba(5, 6, 8, 0.78);
        color: rgba(255, 255, 255, 0.62);
        cursor: pointer;
    }

    .pin:hover,
    .pin.active {
        border-color: rgba(155, 232, 255, 0.48);
        color: #bfefff;
    }

    .pin:disabled {
        cursor: wait;
        opacity: 0.55;
    }

    .title {
        display: block;
        overflow: hidden;
        margin-top: 8px;
        color: #e5e7eb;
        font-size: 13px;
        font-weight: 500;
        line-height: 1.3;
        text-decoration: none;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .title:hover {
        color: #fff;
    }

    .meta {
        display: flex;
        min-height: 15px;
        margin-top: 3px;
        gap: 3px 8px;
        overflow: hidden;
        color: rgba(220, 220, 225, 0.4);
        font-size: 10.5px;
        line-height: 1.3;
        white-space: nowrap;
    }

    .meta span {
        overflow: hidden;
        text-overflow: ellipsis;
    }

    button:focus-visible,
    a:focus-visible {
        outline: 2px solid var(--pw-accent);
        outline-offset: 3px;
    }

    @media (prefers-reduced-motion: reduce) {
        .cover {
            transition: none;
        }
    }
</style>
