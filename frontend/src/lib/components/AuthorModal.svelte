<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api, type AuthorDetail } from '$lib/api';
    import { bookCoverSrc, retryBookCover, validateBookCover } from '$lib/bookCover';
    import { t, i18n } from '$lib/i18n.svelte';

    let { olid, onClose }: { olid: string; onClose: () => void } = $props();

    let detail = $state<AuthorDetail | null>(null);
    let err = $state('');
    let loading = $state(true);
    let lightbox = $state(false);

    onMount(async () => {
        try {
            detail = await api.bookAuthor(olid);
        } catch (e) {
            err = e instanceof Error ? e.message : 'failed to load';
        } finally {
            loading = false;
        }
    });

    function onBackdrop(ev: MouseEvent) {
        if (ev.target === ev.currentTarget) onClose();
    }

    function openWork(ol_key: string) {
        onClose();
        goto(`/book/${ol_key}`);
    }

    function dateLocale(): string {
        return i18n.lang === 'PL' ? 'pl-PL' : i18n.lang === 'DE' ? 'de-DE' : 'en-US';
    }

    function fmtYears(birth: string | null, death: string | null): string | null {
        if (!birth && !death) return null;
        if (birth && death) return `${birth} – ${death}`;
        if (birth) return `${t('books.author.born')} ${birth}`;
        if (death) return `${t('books.author.died')} ${death}`;
        return null;
    }

    function ageOrLifespan(birth: string | null, death: string | null): string | null {
        const by = yearOf(birth);
        if (!by) return null;
        const dy = yearOf(death);
        if (dy) return `${dy - by} ${t('books.author.years')}`;
        const now = new Date().toLocaleDateString(dateLocale(), { year: 'numeric' });
        const cur = Number(now.match(/\d{4}/)?.[0] ?? 0);
        if (!cur) return null;
        return `${cur - by} ${t('books.author.years')}`;
    }

    function yearOf(s: string | null): number | null {
        if (!s) return null;
        const m = s.match(/\d{4}/);
        return m ? Number(m[0]) : null;
    }
</script>

<div
    class="pw-am-bg"
    onclick={onBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
>
    <div class="pw-am-card">
        <button class="pw-am-x" onclick={onClose} aria-label="close">
            <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <path d="M6 18L18 6M6 6l12 12" />
            </svg>
        </button>

        {#if loading}
            <div class="pw-am-loading">{t('common.loading')}</div>
        {:else if err}
            <div class="pw-am-err">{err}</div>
        {:else if detail}
            <div class="pw-am-head">
                <button
                    class="pw-am-photo"
                    type="button"
                    onclick={() => detail?.photo_url && (lightbox = true)}
                    disabled={!detail.photo_url}
                    aria-label="view photo"
                >
                    {#if detail.photo_url}
                        <img src={detail.photo_url} alt={detail.name} loading="lazy" />
                    {:else}
                        <svg
                            width="48"
                            height="48"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.4"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></svg
                        >
                    {/if}
                </button>
                <div class="pw-am-meta">
                    <h2 class="pw-am-name">{detail.name}</h2>
                    {#if fmtYears(detail.birth_date, detail.death_date)}
                        <div class="pw-am-sub">
                            <span>{fmtYears(detail.birth_date, detail.death_date)}</span>
                            {#if ageOrLifespan(detail.birth_date, detail.death_date)}
                                <span class="pw-am-dot">·</span>
                                <span>{ageOrLifespan(detail.birth_date, detail.death_date)}</span>
                            {/if}
                        </div>
                    {/if}
                    {#if detail.top_works.length > 0}
                        <div class="pw-am-sub pw-am-sub-2">
                            <span>{t('books.author.works_count', { n: detail.top_works.length })}</span>
                        </div>
                    {/if}
                </div>
            </div>

            {#if detail.bio}
                <div class="pw-am-bio">{detail.bio}</div>
            {/if}

            {#if detail.top_works.length > 0}
                <h3 class="pw-am-h3">{t('books.author.known_for')}</h3>
                <div class="pw-am-works">
                    {#each detail.top_works as w (w.ol_key)}
                        <button class="pw-am-work" type="button" onclick={() => openWork(w.ol_key)} title={w.title}>
                            <div class="pw-am-work-poster">
                                {#if w.cover_url}
                                    <img
                                        src={bookCoverSrc(w.cover_url)}
                                        alt=""
                                        loading="lazy"
                                        onload={(event) => validateBookCover(event, w.cover_url!)}
                                        onerror={(event) => retryBookCover(event, w.cover_url!)}
                                    />
                                {:else}
                                    <div class="pw-am-work-blank"></div>
                                {/if}
                                {#if w.in_library}
                                    <span class="pw-am-owned" aria-label={t('books.author.in_library')}>
                                        <svg
                                            width="10"
                                            height="10"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="3.4"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                                        >
                                    </span>
                                {/if}
                            </div>
                            <div class="pw-am-work-title">{w.title}</div>
                            {#if w.year}
                                <div class="pw-am-work-meta"><span>{w.year}</span></div>
                            {/if}
                        </button>
                    {/each}
                </div>
            {/if}
        {/if}
    </div>

    {#if lightbox && detail?.photo_url}
        <div
            class="pw-am-lb"
            onclick={() => (lightbox = false)}
            onkeydown={(e) => e.key === 'Escape' && (lightbox = false)}
            role="dialog"
            aria-modal="true"
            tabindex="-1"
        >
            <img src={detail.photo_url} alt={detail.name} />
        </div>
    {/if}
</div>

<style>
    .pw-am-bg {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        backdrop-filter: blur(8px);
        z-index: 200;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        animation: pw-am-fade 0.15s ease-out;
    }
    @keyframes pw-am-fade {
        from {
            opacity: 0;
        }
        to {
            opacity: 1;
        }
    }
    .pw-am-card {
        position: relative;
        background: #0e0f12;
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 12px;
        width: 100%;
        max-width: 720px;
        max-height: 85vh;
        overflow-y: auto;
        padding: 28px;
        animation: pw-am-pop 0.2s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    @keyframes pw-am-pop {
        from {
            opacity: 0;
            transform: translateY(8px) scale(0.98);
        }
        to {
            opacity: 1;
            transform: translateY(0) scale(1);
        }
    }
    .pw-am-x {
        position: absolute;
        top: 12px;
        right: 12px;
        width: 32px;
        height: 32px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 999px;
        color: rgba(232, 232, 234, 0.65);
        display: grid;
        place-items: center;
        cursor: pointer;
        transition:
            background 0.14s ease,
            color 0.14s ease;
    }
    .pw-am-x:hover {
        background: rgba(255, 255, 255, 0.08);
        color: #fff;
    }
    .pw-am-loading,
    .pw-am-err {
        padding: 40px 0;
        text-align: center;
        color: rgba(220, 220, 225, 0.5);
        font-size: 14px;
    }
    .pw-am-err {
        color: oklch(0.78 0.13 28);
    }
    .pw-am-head {
        display: flex;
        gap: 20px;
        align-items: flex-start;
        margin-bottom: 18px;
    }
    .pw-am-photo {
        width: 112px;
        height: 112px;
        flex-shrink: 0;
        border-radius: 999px;
        overflow: hidden;
        background: linear-gradient(
            135deg,
            color-mix(in oklch, var(--pw-accent) 38%, transparent),
            color-mix(in oklch, var(--pw-accent) 14%, transparent)
        );
        display: grid;
        place-items: center;
        color: rgba(255, 255, 255, 0.6);
        padding: 0;
        border: none;
        cursor: pointer;
        transition:
            box-shadow 0.15s ease,
            transform 0.15s ease;
    }
    .pw-am-photo:not(:disabled):hover {
        box-shadow: 0 0 0 2px color-mix(in oklch, var(--pw-accent) 55%, transparent);
        transform: translateY(-1px);
    }
    .pw-am-photo:disabled {
        cursor: default;
    }
    .pw-am-photo img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        object-position: center top;
    }
    .pw-am-meta {
        flex: 1;
        min-width: 0;
        padding-top: 6px;
    }
    .pw-am-name {
        font-size: 24px;
        font-weight: 500;
        color: #ececef;
        margin: 0 0 6px;
        letter-spacing: -0.015em;
    }
    .pw-am-sub {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.55);
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
    }
    .pw-am-sub-2 {
        margin-top: 4px;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.42);
    }
    .pw-am-dot {
        opacity: 0.4;
    }
    .pw-am-bio {
        font-size: 14px;
        color: rgba(220, 220, 225, 0.75);
        line-height: 1.6;
        margin-bottom: 22px;
        white-space: pre-line;
    }
    .pw-am-h3 {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.55);
        font-weight: 500;
        margin: 0 0 12px;
        letter-spacing: 0.02em;
        text-transform: uppercase;
    }
    .pw-am-works {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
        gap: 12px;
    }
    .pw-am-work {
        background: none;
        border: none;
        padding: 0;
        text-align: left;
        color: inherit;
        cursor: pointer;
    }
    .pw-am-work-poster {
        position: relative;
        aspect-ratio: 2 / 3;
        background: rgba(255, 255, 255, 0.04);
        border-radius: 6px;
        overflow: hidden;
        transition:
            transform 0.2s cubic-bezier(0.2, 0.7, 0.2, 1),
            box-shadow 0.2s ease;
    }
    .pw-am-work-poster img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-am-work-blank {
        width: 100%;
        height: 100%;
    }
    .pw-am-work:hover .pw-am-work-poster {
        transform: translateY(-2px);
        box-shadow: 0 0 0 1px color-mix(in oklch, var(--pw-accent) 50%, transparent);
    }
    .pw-am-owned {
        position: absolute;
        bottom: 6px;
        right: 6px;
        width: 20px;
        height: 20px;
        border-radius: 999px;
        background: rgba(120, 200, 150, 0.92);
        color: #0b1116;
        display: grid;
        place-items: center;
        backdrop-filter: blur(4px);
    }
    .pw-am-work-title {
        margin-top: 6px;
        font-size: 12.5px;
        color: #e8e8ea;
        font-weight: 500;
        overflow: hidden;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
    }
    .pw-am-work-meta {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.5);
        margin-top: 2px;
    }
    .pw-am-lb {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.9);
        backdrop-filter: blur(12px);
        z-index: 300;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        cursor: zoom-out;
        animation: pw-am-fade 0.15s ease-out;
    }
    .pw-am-lb img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        border-radius: 12px;
        box-shadow: 0 24px 60px -20px rgba(0, 0, 0, 0.8);
    }
</style>
