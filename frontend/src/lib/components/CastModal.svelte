<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api, type TmdbPersonDetail } from '$lib/api';
    import { t, i18n } from '$lib/i18n.svelte';

    let { personId, onClose }: { personId: number; onClose: () => void } = $props();

    let detail = $state<TmdbPersonDetail | null>(null);
    let err = $state('');
    let loading = $state(true);
    let lightbox = $state(false);

    function hiResPhoto(url: string): string {
        return url.replace(/\/t\/p\/w\d+\//, '/t/p/original/');
    }

    onMount(async () => {
        try {
            detail = await api.tmdbPerson(personId);
        } catch (e) {
            err = e instanceof Error ? e.message : 'failed to load';
        } finally {
            loading = false;
        }
    });

    function onBackdrop(ev: MouseEvent) {
        if (ev.target === ev.currentTarget) onClose();
    }

    function openCredit(c: { tmdb_id: number; media_type: string }) {
        if (c.media_type !== 'movie' && c.media_type !== 'tv') return;
        onClose();
        goto(`/${c.media_type}/${c.tmdb_id}`);
    }

    function dateLocale(): string {
        return i18n.lang === 'PL' ? 'pl-PL' : i18n.lang === 'DE' ? 'de-DE' : 'en-US';
    }
    function fmtDate(s: string): string {
        const d = new Date(s);
        return new Intl.DateTimeFormat(dateLocale(), { day: 'numeric', month: 'long', year: 'numeric' }).format(d);
    }
    function fmtAge(birthday: string | null, deathday: string | null): string | null {
        if (!birthday) return null;
        const b = new Date(birthday);
        const end = deathday ? new Date(deathday) : new Date();
        const yrs = Math.floor((end.getTime() - b.getTime()) / (365.25 * 24 * 3600 * 1000));
        if (deathday) {
            return `${fmtDate(birthday)} – ${fmtDate(deathday)} · ${t('cast.died_at', { n: yrs })}`;
        }
        return `${fmtDate(birthday)} (${yrs})`;
    }
</script>

<div
    class="pw-cm-bg"
    onclick={onBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
>
    <div class="pw-cm-card">
        <button class="pw-cm-x" onclick={onClose} aria-label="close">
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
            <div class="pw-cm-loading">{t('common.loading')}</div>
        {:else if err}
            <div class="pw-cm-err">{err}</div>
        {:else if detail}
            <div class="pw-cm-head">
                <button
                    class="pw-cm-photo"
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
                <div class="pw-cm-meta">
                    <h2 class="pw-cm-name">{detail.name}</h2>
                    <div class="pw-cm-sub">
                        {#if detail.known_for_department}<span>{detail.known_for_department}</span>{/if}
                        {#if fmtAge(detail.birthday, detail.deathday)}
                            <span class="pw-cm-dot">·</span>
                            <span>{fmtAge(detail.birthday, detail.deathday)}</span>
                        {/if}
                        {#if detail.place_of_birth}
                            <span class="pw-cm-dot">·</span>
                            <span>{detail.place_of_birth}</span>
                        {/if}
                    </div>
                    {#if detail.total_credits > 0 || detail.career_start}
                        <div class="pw-cm-sub pw-cm-sub-2">
                            {#if detail.total_credits > 0}
                                <span>{detail.total_credits} credits</span>
                            {/if}
                            {#if detail.career_start}
                                {#if detail.total_credits > 0}<span class="pw-cm-dot">·</span>{/if}
                                <span>
                                    {detail.career_start}{#if detail.career_end && detail.career_end !== detail.career_start}–{detail.deathday
                                            ? detail.career_end
                                            : 'present'}{/if}
                                </span>
                            {/if}
                        </div>
                    {/if}
                    {#if detail.also_known_as.length > 0 && !detail.also_known_as.includes(detail.name)}
                        <div class="pw-cm-aka">
                            <span class="pw-cm-aka-label">aka</span>
                            {detail.also_known_as.slice(0, 3).join(' · ')}
                        </div>
                    {/if}
                </div>
            </div>

            {#if detail.biography}
                <div class="pw-cm-bio">{detail.biography}</div>
            {/if}

            {#if detail.credits.length > 0}
                <h3 class="pw-cm-h3">{t('cast.known_for')}</h3>
                <div class="pw-cm-credits">
                    {#each detail.credits as c, i (c.tmdb_id + '-' + c.media_type + '-' + i)}
                        <button class="pw-cm-credit" type="button" onclick={() => openCredit(c)} title={c.title}>
                            <div class="pw-cm-credit-poster">
                                {#if c.poster_url}
                                    <img src={c.poster_url} alt="" loading="lazy" />
                                {:else}
                                    <div class="pw-cm-credit-blank"></div>
                                {/if}
                            </div>
                            <div class="pw-cm-credit-title">{c.title}</div>
                            <div class="pw-cm-credit-meta">
                                {#if c.year}<span>{c.year}</span>{/if}
                                {#if c.character}<span class="pw-cm-credit-char">as {c.character}</span>{/if}
                            </div>
                        </button>
                    {/each}
                </div>
            {/if}
        {/if}
    </div>

    {#if lightbox && detail?.photo_url}
        <div
            class="pw-cm-lb"
            onclick={() => (lightbox = false)}
            onkeydown={(e) => e.key === 'Escape' && (lightbox = false)}
            role="dialog"
            aria-modal="true"
            tabindex="-1"
        >
            <img src={hiResPhoto(detail.photo_url)} alt={detail.name} />
        </div>
    {/if}
</div>

<style>
    .pw-cm-bg {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        backdrop-filter: blur(8px);
        z-index: 200;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        animation: pw-cm-fade 0.15s ease-out;
    }
    @keyframes pw-cm-fade {
        from {
            opacity: 0;
        }
        to {
            opacity: 1;
        }
    }

    .pw-cm-card {
        position: relative;
        background: #0e0f12;
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 12px;
        width: 100%;
        max-width: 720px;
        max-height: 85vh;
        overflow-y: auto;
        padding: 28px;
        animation: pw-cm-pop 0.2s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    @keyframes pw-cm-pop {
        from {
            opacity: 0;
            transform: translateY(8px) scale(0.98);
        }
        to {
            opacity: 1;
            transform: translateY(0) scale(1);
        }
    }

    .pw-cm-x {
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
    .pw-cm-x:hover {
        background: rgba(255, 255, 255, 0.08);
        color: #fff;
    }

    .pw-cm-loading,
    .pw-cm-err {
        padding: 40px 0;
        text-align: center;
        color: rgba(220, 220, 225, 0.5);
        font-size: 14px;
    }
    .pw-cm-err {
        color: oklch(0.78 0.13 28);
    }

    .pw-cm-head {
        display: flex;
        gap: 20px;
        align-items: flex-start;
        margin-bottom: 18px;
    }
    .pw-cm-photo {
        width: 96px;
        height: 128px;
        flex-shrink: 0;
        border-radius: 10px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.04);
        display: grid;
        place-items: center;
        color: rgba(255, 255, 255, 0.15);
        padding: 0;
        border: none;
        cursor: pointer;
        transition:
            box-shadow 0.15s ease,
            transform 0.15s ease;
    }
    .pw-cm-photo:not(:disabled):hover {
        box-shadow: 0 0 0 2px color-mix(in oklch, var(--pw-accent) 55%, transparent);
        transform: translateY(-1px);
    }
    .pw-cm-photo:disabled {
        cursor: default;
    }

    .pw-cm-lb {
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
        animation: pw-cm-fade 0.15s ease-out;
    }
    .pw-cm-lb img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        border-radius: 8px;
        box-shadow: 0 24px 60px -20px rgba(0, 0, 0, 0.8);
    }
    .pw-cm-photo img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        object-position: center top;
    }

    .pw-cm-meta {
        flex: 1;
        min-width: 0;
        padding-top: 4px;
    }
    .pw-cm-name {
        font-size: 22px;
        font-weight: 500;
        color: #ececef;
        margin: 0 0 6px;
        letter-spacing: -0.015em;
    }
    .pw-cm-sub {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.55);
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
    }
    .pw-cm-sub-2 {
        margin-top: 4px;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.42);
    }
    .pw-cm-aka {
        margin-top: 6px;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.45);
    }
    .pw-cm-aka-label {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.32);
        letter-spacing: 0.06em;
        text-transform: uppercase;
        margin-right: 6px;
    }
    .pw-cm-dot {
        opacity: 0.4;
    }

    .pw-cm-bio {
        font-size: 14px;
        color: rgba(220, 220, 225, 0.75);
        line-height: 1.6;
        margin-bottom: 22px;
        white-space: pre-line;
    }

    .pw-cm-h3 {
        font-size: 13px;
        color: rgba(220, 220, 225, 0.55);
        font-weight: 500;
        margin: 0 0 12px;
        letter-spacing: 0.02em;
        text-transform: uppercase;
    }

    .pw-cm-credits {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
        gap: 12px;
    }
    .pw-cm-credit {
        background: none;
        border: none;
        padding: 0;
        text-align: left;
        color: inherit;
        cursor: pointer;
    }
    .pw-cm-credit-poster {
        aspect-ratio: 2 / 3;
        background: rgba(255, 255, 255, 0.04);
        border-radius: 6px;
        overflow: hidden;
        transition:
            transform 0.2s cubic-bezier(0.2, 0.7, 0.2, 1),
            box-shadow 0.2s ease;
    }
    .pw-cm-credit-poster img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }
    .pw-cm-credit-blank {
        width: 100%;
        height: 100%;
    }
    .pw-cm-credit:hover .pw-cm-credit-poster {
        transform: translateY(-2px);
        box-shadow: 0 0 0 1px color-mix(in oklch, var(--pw-accent) 50%, transparent);
    }
    .pw-cm-credit-title {
        margin-top: 6px;
        font-size: 12.5px;
        color: #e8e8ea;
        font-weight: 500;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-cm-credit-meta {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.5);
        display: flex;
        gap: 6px;
        white-space: nowrap;
        overflow: hidden;
    }
    .pw-cm-credit-char {
        overflow: hidden;
        text-overflow: ellipsis;
    }
</style>
