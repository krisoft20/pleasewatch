<script lang="ts">
    import type { BookShelfItem } from '$lib/api';
    import { t, i18n } from '$lib/i18n.svelte';

    let { items, year, onClose }: { items: BookShelfItem[]; year: number; onClose: () => void } = $props();

    const inYear = $derived(
        items
            .filter((s) => s.status === 'read' && s.finished_at?.startsWith(String(year)))
            .sort((a, b) => (b.finished_at ?? '').localeCompare(a.finished_at ?? ''))
    );

    const totalPages = $derived(inYear.reduce((acc, s) => acc + (s.pages ?? 0), 0));

    const months = $derived.by(() => {
        const out = new Array(12).fill(0);
        for (const s of inYear) {
            const m = parseInt((s.finished_at ?? '').slice(5, 7), 10) - 1;
            if (m >= 0 && m < 12) out[m]++;
        }
        return out;
    });

    function monthName(i: number): string {
        return new Intl.DateTimeFormat(i18n.lang.toLowerCase(), { month: 'long' }).format(new Date(year, i, 1));
    }

    const bestMonth = $derived.by(() => {
        let bi = -1,
            bv = 0;
        months.forEach((n, i) => {
            if (n > bv) {
                bv = n;
                bi = i;
            }
        });
        return bi < 0 ? null : { name: monthName(bi), n: bv };
    });

    const longest = $derived.by(() => {
        const withPages = inYear.filter((s) => (s.pages ?? 0) > 0);
        if (!withPages.length) return null;
        return withPages.reduce((a, b) => ((a.pages ?? 0) >= (b.pages ?? 0) ? a : b));
    });

    const shortest = $derived.by(() => {
        const withPages = inYear.filter((s) => (s.pages ?? 0) > 0);
        if (!withPages.length) return null;
        return withPages.reduce((a, b) => ((a.pages ?? 0) <= (b.pages ?? 0) ? a : b));
    });

    function topOf(extract: (s: BookShelfItem) => string | null | undefined): { name: string; n: number } | null {
        const counts: Record<string, number> = {};
        for (const s of inYear) {
            const v = (extract(s) ?? '').split(',')[0].trim();
            if (v) counts[v] = (counts[v] ?? 0) + 1;
        }
        const keys = Object.keys(counts);
        if (!keys.length) return null;
        const best = keys.reduce((a, b) => (counts[a] >= counts[b] ? a : b));
        return { name: best, n: counts[best] };
    }

    const topAuthor = $derived(topOf((s) => s.authors));
    const topGenre = $derived(topOf((s) => s.subjects?.split(', ')[0]));

    function onKey(e: KeyboardEvent) {
        if (e.key === 'Escape') onClose();
    }
</script>

<svelte:window onkeydown={onKey} />

<div class="pw-yir-scrim" role="presentation" onclick={onClose}>
    <div class="pw-yir" role="presentation" onclick={(e) => e.stopPropagation()}>
        <button class="pw-yir-x" onclick={onClose} aria-label="close">
            <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"><path d="M18 6L6 18M6 6l12 12" /></svg
            >
        </button>

        <div class="pw-yir-head">
            <span class="pw-yir-eyebrow">{t('books.yir.eyebrow')}</span>
            <h1 class="pw-yir-year">{year}</h1>
        </div>

        {#if inYear.length === 0}
            <p class="pw-yir-empty">{t('books.yir.empty')}</p>
        {:else}
            <div class="pw-yir-headline">
                <div>
                    <b>{inYear.length}</b>
                    <span>{t('books.yir.books')}</span>
                </div>
                <div>
                    <b>{totalPages.toLocaleString()}</b>
                    <span>{t('books.yir.pages')}</span>
                </div>
                {#if bestMonth}
                    <div>
                        <b>{bestMonth.name}</b>
                        <span>{t('books.yir.best_month')}</span>
                    </div>
                {/if}
            </div>

            <div class="pw-yir-collage">
                {#each inYear as s, i (s.ol_key)}
                    <a
                        class="pw-yir-cover"
                        href={`/book/${s.ol_key}`}
                        style:animation-delay="{Math.min(i * 35, 1400)}ms"
                    >
                        {#if s.cover_url}<img src={s.cover_url} alt={s.title} loading="lazy" />{/if}
                    </a>
                {/each}
            </div>

            <div class="pw-yir-supers">
                {#if longest}
                    <div class="pw-yir-super">
                        <span class="pw-yir-skey">{t('books.yir.longest')}</span>
                        <span class="pw-yir-sval">{longest.title}</span>
                        <span class="pw-yir-snote">{t('books.meta.pages.other', { n: longest.pages ?? 0 })}</span>
                    </div>
                {/if}
                {#if shortest && shortest.ol_key !== longest?.ol_key}
                    <div class="pw-yir-super">
                        <span class="pw-yir-skey">{t('books.yir.shortest')}</span>
                        <span class="pw-yir-sval">{shortest.title}</span>
                        <span class="pw-yir-snote">{t('books.meta.pages.other', { n: shortest.pages ?? 0 })}</span>
                    </div>
                {/if}
                {#if topAuthor && topAuthor.n > 1}
                    <div class="pw-yir-super">
                        <span class="pw-yir-skey">{t('books.yir.top_author')}</span>
                        <span class="pw-yir-sval">{topAuthor.name}</span>
                        <span class="pw-yir-snote">{t('books.yir.n_books', { n: topAuthor.n })}</span>
                    </div>
                {/if}
                {#if topGenre && topGenre.n > 1}
                    <div class="pw-yir-super">
                        <span class="pw-yir-skey">{t('books.yir.top_genre')}</span>
                        <span class="pw-yir-sval">{topGenre.name}</span>
                        <span class="pw-yir-snote">{t('books.yir.n_books', { n: topGenre.n })}</span>
                    </div>
                {/if}
            </div>
        {/if}
    </div>
</div>

<style>
    .pw-yir-scrim {
        position: fixed;
        inset: 0;
        z-index: 200;
        background: rgba(0, 0, 0, 0.72);
        backdrop-filter: blur(6px);
        display: flex;
        justify-content: center;
        align-items: flex-start;
        padding: 24px 12px;
        overflow-y: auto;
        animation: pw-yir-fade 0.25s ease;
    }
    @keyframes pw-yir-fade {
        from {
            opacity: 0;
        }
    }
    .pw-yir {
        position: relative;
        width: 100%;
        max-width: 720px;
        background:
            radial-gradient(
                circle at top left,
                color-mix(in oklch, var(--pw-accent) 16%, transparent),
                transparent 55%
            ),
            linear-gradient(160deg, rgba(20, 16, 28, 0.95), rgba(12, 13, 17, 0.98));
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 22px;
        padding: 28px 22px 26px;
        margin: auto;
        animation: pw-yir-rise 0.35s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    @keyframes pw-yir-rise {
        from {
            transform: translateY(20px);
            opacity: 0;
        }
    }
    @media (min-width: 640px) {
        .pw-yir {
            padding: 40px 36px 36px;
        }
    }
    .pw-yir-x {
        position: absolute;
        top: 14px;
        right: 14px;
        width: 34px;
        height: 34px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: rgba(232, 232, 234, 0.8);
        display: grid;
        place-items: center;
        cursor: pointer;
    }
    .pw-yir-head {
        text-align: center;
        margin-bottom: 24px;
    }
    .pw-yir-eyebrow {
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.16em;
        text-transform: uppercase;
        color: color-mix(in oklch, var(--pw-accent) 60%, #ececef);
    }
    .pw-yir-year {
        font-family: 'Lora', Georgia, ui-serif, serif;
        font-weight: 600;
        font-size: 64px;
        margin: 4px 0 0;
        color: #ececef;
        letter-spacing: -0.02em;
        line-height: 1;
    }
    @media (min-width: 640px) {
        .pw-yir-year {
            font-size: 84px;
        }
    }
    .pw-yir-empty {
        text-align: center;
        color: rgba(220, 220, 225, 0.5);
        font-size: 14px;
        padding: 30px 0;
    }
    .pw-yir-headline {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 0;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-radius: 16px;
        margin-bottom: 24px;
    }
    .pw-yir-headline > div {
        padding: 14px 10px;
        display: flex;
        flex-direction: column;
        gap: 4px;
        align-items: center;
        text-align: center;
        min-width: 0;
    }
    .pw-yir-headline > div + div {
        border-left: 1px solid rgba(255, 255, 255, 0.06);
    }
    .pw-yir-headline b {
        font-family: 'Lora', Georgia, ui-serif, serif;
        font-size: 22px;
        font-weight: 600;
        color: #ececef;
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 100%;
    }
    .pw-yir-headline span {
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.45);
        text-transform: uppercase;
        letter-spacing: 0.04em;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 100%;
    }
    .pw-yir-collage {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(64px, 1fr));
        gap: 8px;
        margin-bottom: 26px;
    }
    @media (min-width: 640px) {
        .pw-yir-collage {
            grid-template-columns: repeat(auto-fill, minmax(78px, 1fr));
            gap: 10px;
        }
    }
    .pw-yir-cover {
        display: block;
        aspect-ratio: 2/3;
        border-radius: 6px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.06);
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.35);
        opacity: 0;
        animation: pw-yir-pop 0.5s cubic-bezier(0.2, 0.7, 0.2, 1) forwards;
    }
    .pw-yir-cover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    @keyframes pw-yir-pop {
        from {
            transform: translateY(8px) scale(0.94);
            opacity: 0;
        }
        to {
            transform: none;
            opacity: 1;
        }
    }
    .pw-yir-supers {
        display: flex;
        flex-direction: column;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        padding-top: 4px;
    }
    .pw-yir-super {
        display: grid;
        grid-template-columns: 90px 1fr auto;
        align-items: baseline;
        gap: 10px;
        padding: 11px 2px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    }
    .pw-yir-super:last-child {
        border-bottom: 0;
    }
    .pw-yir-skey {
        font-size: 10.5px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: rgba(220, 220, 225, 0.45);
    }
    .pw-yir-sval {
        font-family: 'Lora', Georgia, ui-serif, serif;
        font-size: 15px;
        color: #ececef;
        min-width: 0;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-yir-snote {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        text-align: right;
        white-space: nowrap;
    }
</style>
