<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { page } from '$app/state';
    import { api, type SeriesDetail, type User } from '$lib/api';
    import { bookCoverSrc, retryBookCover, validateBookCover } from '$lib/bookCover';
    import { t } from '$lib/i18n.svelte';
    import TopBar from '$lib/components/TopBar.svelte';

    const rawName = $derived(decodeURIComponent(page.params.name ?? ''));

    let user = $state<User | null>(null);
    let detail = $state<SeriesDetail | null>(null);
    let loading = $state(true);
    let err = $state('');

    onMount(async () => {
        try {
            user = await api.me();
        } catch {
            goto('/login', { replaceState: true });
            return;
        }
        try {
            detail = await api.bookSeriesDetail(rawName);
        } catch (e) {
            console.error('[series] detail failed', e);
            err = 'could not load series';
        } finally {
            loading = false;
        }
    });

    const yearRange = $derived.by(() => {
        if (!detail) return '';
        const a = detail.year_min;
        const b = detail.year_max;
        if (a && b) return a === b ? `${a}` : `${a}–${b}`;
        return a ? `${a}` : b ? `${b}` : '';
    });

    const ownedCount = $derived(detail?.books.filter((b) => b.in_library).length ?? 0);
    const heroCover = $derived(detail?.cover_url ?? detail?.books[0]?.cover_url ?? null);
    const titleTxt = $derived(detail?.name ?? rawName);
</script>

<svelte:head><title>{titleTxt} - pleasewatch</title></svelte:head>

{#if user}
    <div class="pw-page pw-sr-page">
        <TopBar {user} back={true} />

        {#if loading}
            <section class="pw-section pw-empty">
                <div class="pw-empty-card"><div class="pw-empty-tag">// loading...</div></div>
            </section>
        {:else if err || !detail}
            <section class="pw-section">
                <div class="pw-error" style="max-width: 480px;">{err || 'series not found'}</div>
            </section>
        {:else}
            <div class="pw-v1-hero-wrap pw-sr-hero">
                <div class="pw-v1-hero-bg">
                    {#if heroCover}
                        <img class="pw-v1-hero-img pw-sr-hero-img" src={heroCover} alt="" />
                    {/if}
                    <div class="pw-v1-hero-grad-x"></div>
                    <div class="pw-v1-hero-grad-y"></div>

                    <div class="pw-v1-hero-content">
                        <div class="pw-sr-head">
                            <div class="pw-sr-eyebrow">SERIES</div>
                            <h1 class="pw-sr-title">{detail.name}</h1>
                            {#if detail.author}
                                <div class="pw-sr-by">by <b>{detail.author}</b></div>
                            {/if}
                            <div class="pw-sr-meta">
                                <span class="pw-sr-meta-stat"><b>{detail.books.length}</b> books</span>
                                {#if ownedCount > 0}
                                    <span class="pw-sr-meta-dot">·</span>
                                    <span class="pw-sr-meta-stat"><b>{ownedCount}</b> in library</span>
                                {/if}
                                {#if yearRange}
                                    <span class="pw-sr-meta-dot">·</span>
                                    <span class="pw-sr-meta-soft">{yearRange}</span>
                                {/if}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <section class="pw-section pw-sr-grid-sec">
                <div class="pw-sr-grid">
                    {#each detail.books as b, i (b.ol_key)}
                        <a class="pw-sr-card" href={`/book/${b.ol_key}`} data-sveltekit-preload-data="hover">
                            <div class="pw-sr-card-cover">
                                <span class="pw-sr-card-num">#{i + 1}</span>
                                {#if b.cover_url}
                                    <img
                                        src={bookCoverSrc(b.cover_url)}
                                        alt={b.title}
                                        loading="lazy"
                                        decoding="async"
                                        onload={(event) => validateBookCover(event, b.cover_url!)}
                                        onerror={(event) => retryBookCover(event, b.cover_url!)}
                                    />
                                {:else}
                                    <div class="pw-sr-card-empty">
                                        <svg
                                            width="32"
                                            height="32"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="1.2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><path d="M4 4h12a2 2 0 0 1 2 2v14H6a2 2 0 0 1-2-2zM4 4v16" /></svg
                                        >
                                        <span class="pw-sr-card-empty-t">{b.title}</span>
                                    </div>
                                {/if}
                                {#if b.in_library}
                                    <span class="pw-sr-card-owned" aria-label="in library">
                                        <svg
                                            width="11"
                                            height="11"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="3"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                        >
                                    </span>
                                {/if}
                            </div>
                            <div class="pw-sr-card-title">{b.title}</div>
                            {#if b.year}<div class="pw-sr-card-year">{b.year}</div>{/if}
                        </a>
                    {/each}
                </div>
            </section>
        {/if}

        <div style="height: 60px;"></div>
    </div>
{/if}

<style>
    .pw-sr-hero {
        isolation: isolate;
        overflow: hidden;
    }
    .pw-sr-hero-img {
        filter: blur(52px) saturate(112%) brightness(0.5);
        transform: scale(1.14);
    }
    .pw-sr-head {
        max-width: 760px;
    }
    .pw-sr-eyebrow {
        font-family: ui-monospace, monospace;
        font-size: 10.5px;
        font-weight: 600;
        letter-spacing: 0.2em;
        color: var(--pw-accent);
        margin-bottom: 8px;
    }
    .pw-sr-title {
        font-size: clamp(32px, 5vw, 60px);
        font-weight: 300;
        letter-spacing: -0.025em;
        line-height: 1.02;
        color: #f4f4f6;
        margin: 0 0 14px;
    }
    .pw-sr-by {
        font-size: 15px;
        color: rgba(220, 220, 225, 0.62);
        margin-bottom: 14px;
    }
    .pw-sr-by :global(b) {
        color: #d8d8da;
        font-weight: 500;
    }
    .pw-sr-meta {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 8px;
        font-size: 13px;
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-sr-meta-stat :global(b) {
        color: #ececef;
        font-weight: 600;
    }
    .pw-sr-meta-soft {
        font-family: ui-monospace, monospace;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.4);
    }
    .pw-sr-meta-dot {
        opacity: 0.4;
    }

    .pw-sr-grid-sec {
        padding-top: 8px;
    }
    .pw-sr-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
        gap: 24px 18px;
    }
    .pw-sr-card {
        text-decoration: none;
        color: inherit;
        min-width: 0;
        display: flex;
        flex-direction: column;
    }
    .pw-sr-card-cover {
        position: relative;
        width: 100%;
        aspect-ratio: 2/3;
        border-radius: 10px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.03);
        box-shadow: 0 14px 30px -18px rgba(0, 0, 0, 0.9);
        transition:
            transform 0.25s cubic-bezier(0.2, 0.7, 0.2, 1),
            box-shadow 0.25s ease;
    }
    .pw-sr-card:hover .pw-sr-card-cover {
        transform: translateY(-5px);
        box-shadow:
            0 24px 40px -20px rgba(0, 0, 0, 0.95),
            0 0 0 1px rgba(255, 255, 255, 0.1);
    }
    .pw-sr-card-cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-sr-card-num {
        position: absolute;
        top: 6px;
        left: 6px;
        z-index: 2;
        font-family: ui-monospace, monospace;
        font-size: 9.5px;
        font-weight: 600;
        letter-spacing: 0.02em;
        padding: 1px 5px;
        border-radius: 3px;
        color: rgba(255, 255, 255, 0.85);
        background: rgba(8, 9, 12, 0.55);
        backdrop-filter: blur(6px);
        font-variant-numeric: tabular-nums;
    }
    .pw-sr-card-empty {
        width: 100%;
        height: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        padding: 12px 10px;
        color: rgba(220, 220, 225, 0.32);
        background:
            linear-gradient(135deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.01) 60%),
            radial-gradient(circle at 50% 30%, rgba(140, 110, 200, 0.12), transparent 60%);
    }
    .pw-sr-card-empty-t {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.5);
        text-align: center;
        line-height: 1.3;
        display: -webkit-box;
        -webkit-line-clamp: 3;
        line-clamp: 3;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .pw-sr-card-owned {
        position: absolute;
        bottom: 8px;
        right: 8px;
        width: 20px;
        height: 20px;
        border-radius: 999px;
        background: rgba(120, 200, 150, 0.95);
        color: #0b1116;
        display: grid;
        place-items: center;
        box-shadow: 0 4px 12px -4px rgba(0, 0, 0, 0.6);
    }
    .pw-sr-card-title {
        margin-top: 10px;
        font-size: 13px;
        font-weight: 500;
        color: #ececef;
        line-height: 1.32;
        overflow: hidden;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
    }
    .pw-sr-card:hover .pw-sr-card-title {
        color: #fff;
    }
    .pw-sr-card-year {
        font-family: ui-monospace, monospace;
        font-size: 10px;
        color: rgba(220, 220, 225, 0.4);
        margin-top: 3px;
        letter-spacing: 0.04em;
    }
    @media (max-width: 720px) {
        .pw-sr-grid {
            grid-template-columns: repeat(auto-fill, minmax(108px, 1fr));
            gap: 18px 12px;
        }
        .pw-sr-card-num {
            font-size: 9px;
            padding: 1px 4px;
            top: 5px;
            left: 5px;
        }
    }
</style>
