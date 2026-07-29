<script lang="ts">
    import type { User } from '$lib/api';
    import { t } from '$lib/i18n.svelte';

    type ReadingItem = { ol_key: string; title: string; percent: number | null };

    let {
        user,
        reading,
        waiting = 0,
        thisYearRead,
        year,
        onYearReview
    }: {
        user: User | null;
        reading: ReadingItem[];
        waiting?: number;
        thisYearRead?: number;
        year?: number;
        onYearReview?: () => void;
    } = $props();

    const greet = $derived.by(() => {
        const h = new Date().getHours();
        if (h < 6) return 'late';
        if (h < 12) return 'morning';
        if (h < 17) return 'afternoon';
        if (h < 22) return 'evening';
        return 'late';
    });

    const sub = $derived.by(() => {
        if (reading.length === 1) {
            const current = reading[0];
            const percent = Math.round((current.percent ?? 0) * 100);
            if (percent < 5) return { k: 'books.greet.single.start', v: { title: current.title } };
            if (percent >= 75) return { k: 'books.greet.single.finish', v: { title: current.title, percent } };
            return { k: 'books.greet.single.middle', v: { title: current.title, percent } };
        }
        if (reading.length > 1) {
            const current = [...reading].sort((a, b) => (b.percent ?? 0) - (a.percent ?? 0))[0];
            return {
                k: 'books.greet.many',
                v: { title: current.title, percent: Math.round((current.percent ?? 0) * 100), n: reading.length }
            };
        }
        if (waiting > 0) return { k: 'books.greet.waiting', v: { n: waiting } };
        return { k: 'books.greet.empty', v: {} };
    });

    const name = $derived(user?.username ?? '');
</script>

<div class="pw-greet">
    <h2 class="pw-greet-hi">
        {t('books.greet.' + greet)}{#if name}, <span class="pw-greet-name">{name}</span>{/if}{#if greet === 'late'}<span class="pw-greet-late-mark">?</span>{/if}
    </h2>
    <div class="pw-greet-sub-row">
        <p class="pw-greet-sub">{t(sub.k, sub.v)}</p>
        {#if thisYearRead && thisYearRead > 0 && year && onYearReview}
            <button class="pw-greet-pill" onclick={onYearReview}>
                <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"
                    ><path d="M12 2l3 7h7l-5.5 4 2 7L12 16l-6.5 4 2-7L2 9h7z" /></svg
                >
                {t('books.greet.year_pill', { year, n: thisYearRead })}
            </button>
        {/if}
    </div>
</div>

<style>
    .pw-greet {
        max-width: 760px;
        margin-bottom: 22px;
    }
    .pw-greet-hi {
        font-family: 'Lora', Georgia, 'Iowan Old Style', 'Palatino Linotype', ui-serif, serif;
        font-style: italic;
        font-weight: 500;
        font-size: 24px;
        color: #ececef;
        margin: 0;
        letter-spacing: -0.01em;
    }
    @media (min-width: 640px) {
        .pw-greet-hi {
            font-size: 28px;
        }
    }
    .pw-greet-name {
        font-style: normal;
        font-weight: 600;
        color: color-mix(in oklch, var(--pw-accent) 70%, #ececef);
    }
    .pw-greet-late-mark {
        color: #9c160b;
        font-style: normal;
        font-weight: 700;
        margin-left: 0.08em;
    }
    .pw-greet-sub-row {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 10px;
        margin-top: 6px;
    }
    .pw-greet-sub {
        margin: 0;
        font-size: 13px;
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-greet-pill {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        background: color-mix(in oklch, var(--pw-accent) 14%, rgba(255, 255, 255, 0.04));
        border: 1px solid color-mix(in oklch, var(--pw-accent) 35%, transparent);
        color: color-mix(in oklch, var(--pw-accent) 60%, #ececef);
        border-radius: 999px;
        padding: 4px 11px 4px 8px;
        font-size: 11.5px;
        cursor: pointer;
        transition: background 0.15s ease;
        white-space: nowrap;
    }
    .pw-greet-pill:hover {
        background: color-mix(in oklch, var(--pw-accent) 22%, rgba(255, 255, 255, 0.04));
    }
    .pw-greet-pill svg {
        color: var(--pw-accent);
    }
</style>
