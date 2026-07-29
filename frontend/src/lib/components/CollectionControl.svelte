<script lang="ts">
    import { api, type CollectionItem, type CollectionKind, type CollectionStatus } from '$lib/api';
    import { clickOutside } from '$lib/dismiss';
    import { t } from '$lib/i18n.svelte';
    import Icon from './Icon.svelte';

    type Props = {
        tmdbId: number;
        kind: CollectionKind;
        title: string;
        year: string | null;
        posterUrl: string | null;
        backdropUrl: string | null;
    };

    const choices: { value: CollectionStatus; label: string }[] = [
        { value: 'planned', label: 'collection.planned' },
        { value: 'in_progress', label: 'collection.in_progress' },
        { value: 'completed', label: 'collection.completed' }
    ];

    let { tmdbId, kind, title, year, posterUrl, backdropUrl }: Props = $props();
    let item = $state<CollectionItem | null>(null);
    let loaded = $state(false);
    let busy = $state(false);
    let open = $state(false);
    let error = $state('');
    let loadToken = 0;

    const currentLabel = $derived(
        item
            ? t(choices.find((choice) => choice.value === item?.status)?.label ?? 'collection.control.add')
            : t('collection.control.add')
    );

    $effect(() => {
        void load(tmdbId, kind);
    });

    async function load(id: number, mediaKind: CollectionKind) {
        const token = ++loadToken;
        loaded = false;
        item = null;
        open = false;
        error = '';

        try {
            const collection = await api.collectionList();
            if (token !== loadToken) return;
            item =
                collection.find((entry) => entry.tmdb_id === id && entry.kind === mediaKind) ??
                collection.find((entry) => entry.tmdb_id === id && entry.kind !== 'movie' && mediaKind !== 'movie') ??
                null;
        } catch (e) {
            if (token !== loadToken) return;
            error = e instanceof Error ? e.message : t('collection.error.load');
        } finally {
            if (token === loadToken) loaded = true;
        }
    }

    async function choose(status: CollectionStatus) {
        if (busy) return;
        if (item?.status === status) {
            open = false;
            return;
        }

        const id = tmdbId;
        const routeKind = kind;
        const mediaKind = item?.kind ?? routeKind;
        busy = true;
        error = '';

        try {
            const saved = item
                ? await api.collectionUpdate(mediaKind, id, { status })
                : await api.collectionSave({
                      tmdb_id: id,
                      kind: mediaKind,
                      title,
                      year,
                      poster_url: posterUrl,
                      backdrop_url: backdropUrl,
                      status
                  });

            if (id !== tmdbId || routeKind !== kind) return;
            item = saved;
            open = false;
        } catch (e) {
            error = e instanceof Error ? e.message : t('collection.error.update');
        } finally {
            if (id === tmdbId && routeKind === kind) busy = false;
        }
    }

    async function remove() {
        if (!item || busy) return;
        const id = tmdbId;
        const routeKind = kind;
        busy = true;
        error = '';

        try {
            await api.collectionRemove(item.kind, id);
            if (id !== tmdbId || routeKind !== kind) return;
            item = null;
            open = false;
        } catch (e) {
            error = e instanceof Error ? e.message : t('collection.control.remove_error');
        } finally {
            if (id === tmdbId && routeKind === kind) busy = false;
        }
    }
</script>

<div class:open class="collection-control" use:clickOutside={() => (open = false)}>
    <button
        class:completed={item?.status === 'completed'}
        class="collection-trigger"
        type="button"
        disabled={!loaded || busy}
        aria-haspopup="menu"
        aria-expanded={open}
        onclick={() => (open = !open)}
    >
        <span class:completed={item?.status === 'completed'} class="state-icon" aria-hidden="true">
            {#if item?.status === 'completed'}
                <Icon name="check-alt" class="w-3.5 h-3.5" strokeWidth={3} />
            {:else if item?.status === 'in_progress'}
                <Icon name="clock" class="w-4 h-4" strokeWidth={1.8} />
            {:else}
                <Icon name="plus" class="w-4 h-4" strokeWidth={1.8} />
            {/if}
        </span>
        <span>{busy ? t('collection.control.saving') : loaded ? currentLabel : t('collection.control.loading')}</span>
        <Icon name="chevron-down" class="chevron" strokeWidth={1.8} />
    </button>

    {#if open}
        <div class="collection-menu" role="menu" aria-label={t('collection.control.status')}>
            {#each choices as choice (choice.value)}
                <button
                    class:selected={item?.status === choice.value}
                    type="button"
                    role="menuitemradio"
                    aria-checked={item?.status === choice.value}
                    disabled={busy}
                    onclick={() => choose(choice.value)}
                >
                    <span class="radio" aria-hidden="true">
                        {#if item?.status === choice.value}
                            <Icon name="check-alt" class="w-3 h-3" strokeWidth={3} />
                        {/if}
                    </span>
                    <span>{t(choice.label)}</span>
                </button>
            {/each}

            {#if item}
                <button class="remove" type="button" role="menuitem" disabled={busy} onclick={remove}>
                    <Icon name="trash" class="w-3.5 h-3.5" strokeWidth={1.8} />
                    <span>{t('collection.control.remove')}</span>
                </button>
            {/if}

            {#if error}
                <div class="collection-error" role="status">{error}</div>
            {/if}
        </div>
    {/if}
</div>

<svelte:window
    onkeydown={(e) => {
        if (e.key === 'Escape') open = false;
    }}
/>

<style>
    .collection-control {
        position: relative;
        z-index: 2;
        display: inline-flex;
    }

    .collection-control.open {
        z-index: 100;
    }

    .collection-trigger {
        display: inline-flex;
        align-items: center;
        gap: 7px;
        height: 38px;
        padding: 0 12px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.08);
        color: rgba(255, 255, 255, 0.9);
        font: inherit;
        font-size: 14px;
        font-weight: 500;
        white-space: nowrap;
        cursor: pointer;
    }

    .collection-trigger:hover,
    .collection-trigger:focus-visible {
        border-color: rgba(255, 255, 255, 0.25);
        background: rgba(255, 255, 255, 0.12);
    }

    .collection-trigger.completed {
        border-color: rgba(155, 232, 255, 0.45);
    }

    .collection-trigger:disabled {
        cursor: default;
        opacity: 0.6;
    }

    .state-icon {
        display: grid;
        width: 18px;
        height: 18px;
        place-items: center;
        color: rgba(255, 255, 255, 0.75);
    }

    .state-icon.completed {
        border-radius: 50%;
        background: #9be8ff;
        color: #0b1116;
    }

    :global(.chevron) {
        width: 13px;
        height: 13px;
        margin-left: 1px;
        opacity: 0.55;
        transition: transform 0.15s ease;
    }

    .open :global(.chevron) {
        transform: rotate(180deg);
    }

    .collection-menu {
        position: absolute;
        top: calc(100% + 8px);
        right: 0;
        width: 220px;
        padding: 6px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        background: #0c0e13;
        box-shadow: 0 18px 45px rgba(0, 0, 0, 0.5);
    }

    .collection-menu button {
        display: flex;
        align-items: center;
        gap: 9px;
        width: 100%;
        min-height: 38px;
        padding: 7px 8px;
        border: 0;
        border-radius: 7px;
        background: transparent;
        color: rgba(255, 255, 255, 0.76);
        font: inherit;
        font-size: 12px;
        text-align: left;
        cursor: pointer;
    }

    .collection-menu button:hover,
    .collection-menu button:focus-visible {
        background: rgba(255, 255, 255, 0.07);
        color: #fff;
        outline: none;
    }

    .collection-menu button.selected {
        color: #dff7ff;
    }

    .collection-menu button:disabled {
        cursor: default;
        opacity: 0.55;
    }

    .radio {
        display: grid;
        width: 18px;
        height: 18px;
        flex: 0 0 auto;
        place-items: center;
        border: 1px solid rgba(255, 255, 255, 0.18);
        border-radius: 50%;
    }

    .selected .radio {
        border-color: #9be8ff;
        background: #9be8ff;
        color: #0b1116;
    }

    .collection-menu .remove {
        margin-top: 5px;
        padding-top: 9px;
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 0 0 7px 7px;
        color: rgba(248, 113, 113, 0.75);
    }

    .collection-menu .remove:hover,
    .collection-menu .remove:focus-visible {
        color: #fca5a5;
    }

    .collection-error {
        padding: 6px 8px 3px;
        color: #fca5a5;
        font-size: 10.5px;
        line-height: 1.35;
    }

    .collection-trigger:focus-visible {
        outline: 2px solid var(--pw-accent);
        outline-offset: 2px;
    }

    @media (max-width: 640px) {
        .collection-control {
            position: static;
        }

        .collection-menu {
            position: fixed;
            top: auto;
            right: 12px;
            bottom: calc(64px + env(safe-area-inset-bottom));
            left: 12px;
            width: auto;
            max-width: 420px;
            margin: 0 auto;
        }
    }

    @media (prefers-reduced-motion: reduce) {
        :global(.chevron) {
            transition: none;
        }
    }
</style>
