<script lang="ts">
    import { api, type TmdbDetail, type TmdbSearchItem, type TmdbVideo } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import Icon from '$lib/components/Icon.svelte';

    let {
        item,
        genreNames = {},
        onClose
    }: {
        item: TmdbSearchItem;
        genreNames?: Record<number, string>;
        onClose: () => void;
    } = $props();

    let detail = $state<TmdbDetail | null>(null);
    let videos = $state<TmdbVideo[]>([]);
    let busy = $state(false);
    let seq = 0;

    const trailer = $derived(videos.find((v) => v.kind === 'Trailer') ?? videos[0] ?? null);
    const title = $derived(detail?.title ?? item.title);
    const backdrop = $derived(detail?.backdrop_url ?? item.backdrop_url);
    const poster = $derived(detail?.poster_url ?? item.poster_url);
    const genres = $derived(
        detail?.genres?.length
            ? detail.genres
            : (item.genre_ids ?? []).map((id) => genreNames[id]).filter(Boolean)
    );

    $effect(() => {
        const current = item;
        const mine = ++seq;
        detail = null;
        videos = [];
        busy = true;
        Promise.all([
            api.tmdbDetail(current.media_type, current.tmdb_id),
            api.tmdbVideos(current.media_type, current.tmdb_id).catch(() => [] as TmdbVideo[])
        ])
            .then(([d, v]) => {
                if (mine !== seq) return;
                detail = d;
                videos = v;
            })
            .catch((e) => {
                if (mine === seq) console.error('[discover] preview failed', e);
            })
            .finally(() => {
                if (mine === seq) busy = false;
            });
    });

    function titleHref(it: TmdbSearchItem) {
        return it.media_type === 'tv' ? `/tv/${it.tmdb_id}` : `/movie/${it.tmdb_id}`;
    }

    function openTrailer() {
        if (!trailer || typeof window === 'undefined') return;
        window.open(`https://www.youtube.com/watch?v=${trailer.key}`, '_blank', 'noopener');
    }

    function onKey(e: KeyboardEvent) {
        if (e.key === 'Escape') onClose();
    }

    function onBackdrop(e: MouseEvent) {
        if (e.target === e.currentTarget) onClose();
    }
</script>

<svelte:window onkeydown={onKey} />

<div
    class="pw-drawer-shade"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={onBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
>
    <aside class="pw-drawer">
        <button class="pw-drawer-close" type="button" onclick={onClose} aria-label="close">
            <Icon name="close" class="w-5 h-5" strokeWidth={2} />
        </button>

        <div class="pw-drawer-media">
            {#if backdrop}
                <img class="pw-drawer-backdrop" src={backdrop} alt="" loading="eager" decoding="async" />
            {/if}
            <div class="pw-drawer-grad"></div>
            {#if poster}
                <img class="pw-drawer-poster" src={poster} alt={title} loading="eager" decoding="async" />
            {/if}
        </div>

        <div class="pw-drawer-body">
            <div class="pw-drawer-type">{item.media_type === 'tv' ? 'series' : 'movie'}</div>
            <h2>{title}</h2>
            <div class="pw-drawer-meta">
                {#if detail?.year ?? item.year}<span>{detail?.year ?? item.year}</span>{/if}
                {#if detail?.vote_average ?? item.vote_average}
                    <span>{(detail?.vote_average ?? item.vote_average)?.toFixed(1)}</span>
                {/if}
                {#if busy}<span>{t('common.loading')}</span>{/if}
            </div>

            {#if genres.length > 0}
                <div class="pw-drawer-genres">
                    {#each genres as g}
                        <span>{g}</span>
                    {/each}
                </div>
            {/if}

            {#if detail?.overview ?? item.overview}
                <p>{detail?.overview ?? item.overview}</p>
            {:else}
                <p class="pw-drawer-muted">no overview</p>
            {/if}

            <div class="pw-drawer-actions">
                <a class="pw-drawer-main" href={titleHref(item)}>
                    <Icon name="external-link" class="w-4 h-4" strokeWidth={2} />
                    <span>{t('discover.open')}</span>
                </a>
                {#if trailer}
                    <button type="button" class="pw-drawer-ghost" onclick={openTrailer}>
                        <Icon name="play" class="w-4 h-4" />
                        <span>{t('media.trailer')}</span>
                    </button>
                {/if}
            </div>
        </div>
    </aside>
</div>

<style>
    .pw-drawer-shade {
        position: fixed;
        inset: 0;
        z-index: 80;
        display: flex;
        justify-content: flex-end;
        background: rgba(0, 0, 0, 0.58);
        backdrop-filter: blur(10px);
    }
    .pw-drawer {
        position: relative;
        width: min(460px, 100vw);
        height: 100%;
        overflow-y: auto;
        background: #080a0e;
        border-left: 1px solid rgba(255, 255, 255, 0.1);
        box-shadow: -24px 0 70px rgba(0, 0, 0, 0.44);
    }
    .pw-drawer-close {
        position: absolute;
        top: 14px;
        right: 14px;
        z-index: 3;
        display: grid;
        place-items: center;
        width: 36px;
        height: 36px;
        border-radius: 999px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: #fff;
        background: rgba(0, 0, 0, 0.45);
        cursor: pointer;
    }
    .pw-drawer-media {
        position: relative;
        min-height: 280px;
        background: #050608;
    }
    .pw-drawer-backdrop {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: cover;
        opacity: 0.65;
    }
    .pw-drawer-grad {
        position: absolute;
        inset: 0;
        background: linear-gradient(to bottom, rgba(8, 10, 14, 0.05), #080a0e 92%);
    }
    .pw-drawer-poster {
        position: absolute;
        left: 22px;
        bottom: 0;
        width: 132px;
        aspect-ratio: 2 / 3;
        object-fit: cover;
        border-radius: 8px;
        border: 1px solid rgba(255, 255, 255, 0.15);
        box-shadow: 0 18px 50px rgba(0, 0, 0, 0.55);
        transform: translateY(38px);
    }
    .pw-drawer-body {
        padding: 54px 22px 28px;
    }
    .pw-drawer-type {
        margin-bottom: 8px;
        color: #67b7ff;
        font-size: 11px;
        font-weight: 800;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }
    .pw-drawer h2 {
        margin: 0;
        color: #f5f5f6;
        font-size: 30px;
        line-height: 1.08;
    }
    .pw-drawer-meta {
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-top: 11px;
        color: rgba(235, 238, 244, 0.58);
        font-size: 13px;
    }
    .pw-drawer-genres {
        display: flex;
        flex-wrap: wrap;
        gap: 7px;
        margin-top: 16px;
    }
    .pw-drawer-genres span {
        padding: 5px 8px;
        border-radius: 999px;
        color: rgba(235, 238, 244, 0.78);
        background: rgba(255, 255, 255, 0.07);
        font-size: 12px;
    }
    .pw-drawer p {
        margin: 18px 0 0;
        color: rgba(235, 238, 244, 0.75);
        font-size: 14px;
        line-height: 1.55;
    }
    .pw-drawer-muted {
        color: rgba(235, 238, 244, 0.42);
    }
    .pw-drawer-actions {
        display: flex;
        gap: 10px;
        margin-top: 22px;
    }
    .pw-drawer-actions button,
    .pw-drawer-actions a {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 8px;
        height: 38px;
        padding: 0 14px;
        border-radius: 7px;
        font-weight: 700;
        font-size: 13px;
        cursor: pointer;
        text-decoration: none;
    }
    .pw-drawer-main {
        border: none;
        color: #061018;
        background: #f5f5f6;
    }
    .pw-drawer-ghost {
        color: rgba(245, 245, 246, 0.9);
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.1);
    }
    @media (max-width: 640px) {
        .pw-drawer-shade {
            align-items: flex-end;
            justify-content: center;
            padding: 0 8px max(0px, env(safe-area-inset-bottom));
        }
        .pw-drawer {
            width: 100%;
            height: min(86dvh, 720px);
            max-height: calc(100dvh - 22px);
            border-left: 0;
            border-top: 1px solid rgba(255, 255, 255, 0.12);
            border-radius: 18px 18px 0 0;
            box-shadow: 0 -24px 70px rgba(0, 0, 0, 0.5);
            animation: pw-drawer-up 0.18s ease-out;
        }
        .pw-drawer-media {
            min-height: 220px;
        }
        .pw-drawer-body {
            padding: 50px 18px calc(24px + env(safe-area-inset-bottom));
        }
        .pw-drawer h2 {
            font-size: 26px;
        }
    }
    @keyframes pw-drawer-up {
        from {
            transform: translateY(26px);
            opacity: 0.86;
        }
        to {
            transform: translateY(0);
            opacity: 1;
        }
    }
</style>
