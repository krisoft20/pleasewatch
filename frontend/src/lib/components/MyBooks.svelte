<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type BookShelfItem, type User } from '$lib/api';
    import { t, i18n } from '$lib/i18n.svelte';
    import BookShelf from './BookShelf.svelte';
    import BookGreeting from './BookGreeting.svelte';
    import DailyQuote from './DailyQuote.svelte';
    import ReadingNow from './ReadingNow.svelte';
    import YearInReview from './YearInReview.svelte';

    let items = $state<BookShelfItem[]>([]);
    let user = $state<User | null>(null);
    let readTotal = $state(0);
    let readYear = $state(0);
    let goal = $state<number | null>(null);
    let loaded = $state(false);
    let goalOpen = $state(false);
    let yirOpen = $state(false);
    let goalDraft = $state(12);
    let coverRefreshTimer: ReturnType<typeof setTimeout> | undefined;

    const year = new Date().getFullYear();

    onMount(() => {
        api.me()
            .then((u) => (user = u))
            .catch(() => {});
        api.bookShelf()
            .catch(() => ({ items: [] as BookShelfItem[], read_total: 0, read_year: 0, goal: null }))
            .then((s) => {
                items = s.items;
                readTotal = s.read_total;
                readYear = s.read_year;
                goal = s.goal;
                if (s.items.some((book) => !book.cover_url)) {
                    // The shelf request starts server-side cover recovery. Re-read it once the
                    // replacement cover IDs have had a moment to come back from Open Library.
                    coverRefreshTimer = setTimeout(() => {
                        api.bookShelf()
                            .then((fresh) => {
                                items = fresh.items;
                                readTotal = fresh.read_total;
                                readYear = fresh.read_year;
                                goal = fresh.goal;
                            })
                            .catch(() => {});
                    }, 1800);
                }
            })
            .finally(() => (loaded = true));

        return () => {
            if (coverRefreshTimer) clearTimeout(coverRefreshTimer);
        };
    });

    const reading = $derived(items.filter((s) => s.status === 'reading'));
    const want = $derived(items.filter((s) => s.status === 'want'));
    const read = $derived(
        [...items.filter((s) => s.status === 'read')].sort((a, b) =>
            (b.finished_at ?? '').localeCompare(a.finished_at ?? '')
        )
    );
    const pagesYear = $derived(
        read.filter((s) => s.finished_at?.startsWith(String(year))).reduce((acc, s) => acc + (s.pages ?? 0), 0)
    );

    const months = $derived.by(() => {
        const out = new Array(12).fill(0);
        for (const s of read) {
            if (!s.finished_at?.startsWith(String(year))) continue;
            const m = parseInt(s.finished_at.slice(5, 7), 10) - 1;
            if (m >= 0 && m < 12) out[m]++;
        }
        return out;
    });
    const monthMax = $derived(Math.max(1, ...months));

    function monthLetter(i: number): string {
        return new Intl.DateTimeFormat(i18n.lang.toLowerCase(), { month: 'narrow' }).format(new Date(year, i, 1));
    }

    function fmtMonth(d: string): string {
        const dt = new Date(d.replace(' ', 'T') + 'Z');
        return isNaN(dt.getTime())
            ? ''
            : dt.toLocaleDateString(i18n.lang.toLowerCase(), { month: 'long', year: 'numeric' });
    }

    function fmtDay(d: string): string {
        const dt = new Date(d.replace(' ', 'T') + 'Z');
        return isNaN(dt.getTime())
            ? ''
            : dt.toLocaleDateString(i18n.lang.toLowerCase(), { day: 'numeric', month: 'short' });
    }

    const history = $derived.by(() => {
        const groups: { label: string; rows: BookShelfItem[] }[] = [];
        for (const s of read) {
            if (!s.finished_at) continue;
            const label = fmtMonth(s.finished_at);
            const last = groups[groups.length - 1];
            if (last && last.label === label) last.rows.push(s);
            else groups.push({ label, rows: [s] });
        }
        return groups;
    });

    function openGoal() {
        goalDraft = goal ?? 12;
        goalOpen = true;
    }

    async function saveGoal() {
        const g = goalDraft;
        goalOpen = false;
        try {
            await api.bookGoalSet(g);
            goal = g > 0 ? g : null;
        } catch (e) {
            console.error('[books] goal save failed', e);
        }
    }

    const ringPct = $derived(goal ? Math.min(1, readYear / goal) : 0);
    const RING_R = 26;
    const RING_C = 2 * Math.PI * RING_R;
</script>

{#snippet plat()}
    <span class="pw-plat">
        <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="#0b1116"
            stroke-width="3.4"
            stroke-linecap="round"
            stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
        >
    </span>
{/snippet}

{#if !loaded}
    <section class="pw-section pw-empty">
        <div class="pw-empty-card"><div class="pw-empty-tag">{t('lib.loading')}</div></div>
    </section>
{:else if items.length === 0}
    <section class="pw-section pw-empty">
        <div class="pw-empty-card">
            <div class="pw-empty-tag">{t('personal.tag')}</div>
            <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('personal.title')}</h2>
            <p class="pw-empty-text">{t('books.empty.body')}</p>
        </div>
    </section>
{:else}
    <div class="pw-section pw-section-tight">
        <BookGreeting
            {user}
            reading={reading.map((s) => ({ ol_key: s.ol_key, title: s.title, percent: s.percent }))}
            waiting={want.length}
            thisYearRead={readYear}
            {year}
            onYearReview={() => (yirOpen = true)}
        />
        <DailyQuote />
        <div class="pw-mb-hero">
            <button class="pw-mb-hgoal" onclick={openGoal}>
                <svg width="68" height="68" viewBox="0 0 64 64">
                    <circle cx="32" cy="32" r={RING_R} fill="none" stroke="rgba(255,255,255,0.08)" stroke-width="5" />
                    <circle
                        cx="32"
                        cy="32"
                        r={RING_R}
                        fill="none"
                        stroke="var(--pw-accent)"
                        stroke-width="5"
                        stroke-linecap="round"
                        stroke-dasharray={RING_C}
                        stroke-dashoffset={RING_C * (1 - ringPct)}
                        transform="rotate(-90 32 32)"
                    />
                    <text x="32" y="38" text-anchor="middle" class="pw-mb-ring-num">{readYear}</text>
                </svg>
                <div class="pw-mb-goal-txt">
                    <div class="pw-mb-goal-title">{year} · {t('books.goal.title')}</div>
                    {#if goal}
                        <div class="pw-mb-goal-sub">{t('books.goal.of', { n: readYear, goal })}</div>
                    {:else}
                        <div class="pw-mb-goal-cta">{t('books.goal.set')}</div>
                    {/if}
                </div>
                <svg
                    class="pw-mb-hchev"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.2"
                    stroke-linecap="round"
                    stroke-linejoin="round"><polyline points="9 18 15 12 9 6" /></svg
                >
            </button>

            <div class="pw-mb-hstats">
                <div>
                    <b>{readYear}</b>
                    <span>{t('books.stats.year')}</span>
                </div>
                <div>
                    <b>{pagesYear.toLocaleString()}</b>
                    <span>{t('books.stats.pages')}</span>
                </div>
                <div>
                    <b>{readTotal}</b>
                    <span>{t('books.stats.alltime')}</span>
                </div>
            </div>

            {#if read.length > 0}
                <div class="pw-mb-hchart">
                    {#each months as n, i (i)}
                        <div class="pw-mb-col">
                            <div
                                class="pw-mb-bar"
                                class:zero={n === 0}
                                style:height={n === 0 ? '3px' : `${Math.max(12, (n / monthMax) * 100)}%`}
                            ></div>
                            <span>{monthLetter(i)}</span>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>

        {#if reading.length > 0}
            <ReadingNow title={t('books.my.reading')} items={reading} />
        {/if}

        {#if want.length > 0}
            <BookShelf title={t('books.my.toread')} items={want} width={140} />
        {/if}

        {#if history.length > 0}
            <h2 class="pw-h2 pw-mb-h">{t('books.my.completed')}<span class="pw-count">{read.length}</span></h2>
            {#each history as g (g.label)}
                <div class="pw-mb-month">{g.label}</div>
                <div class="pw-mb-grid">
                    {#each g.rows as s (s.ol_key)}
                        <div class="pw-mb-gitem">
                            <a href={`/book/${s.ol_key}`}>
                                <span class="pw-mb-gcover">
                                    {#if s.cover_url}<img src={s.cover_url} alt={s.title} loading="lazy" />{/if}
                                    {@render plat()}
                                </span>
                                <span class="pw-mb-gtitle">{s.title}</span>
                                <span class="pw-mb-gdate">{s.finished_at ? fmtDay(s.finished_at) : ''}</span>
                            </a>
                        </div>
                    {/each}
                </div>
            {/each}
        {/if}
    </div>

    {#if yirOpen}
        <YearInReview {items} {year} onClose={() => (yirOpen = false)} />
    {/if}

    {#if goalOpen}
        <div class="pw-mb-scrim" role="presentation" onclick={() => (goalOpen = false)}>
            <div class="pw-mb-pop" role="presentation" onclick={(e) => e.stopPropagation()}>
                <div class="pw-mb-pop-title">{t('books.goal.title')}</div>
                <div class="pw-mb-step">
                    <button onclick={() => (goalDraft = Math.max(0, goalDraft - 1))}>-</button>
                    <span>{goalDraft}</span>
                    <button onclick={() => (goalDraft = Math.min(999, goalDraft + 1))}>+</button>
                </div>
                <button class="pw-v1-btn-watch pw-mb-save" onclick={saveGoal}>{t('books.marks.save')}</button>
            </div>
        </div>
    {/if}
{/if}

<style>
    .pw-mb-hero {
        max-width: 620px;
        background: linear-gradient(
            160deg,
            color-mix(in oklch, var(--pw-accent) 8%, rgba(255, 255, 255, 0.03)),
            rgba(255, 255, 255, 0.025) 55%
        );
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 20px;
        margin-bottom: 24px;
        overflow: hidden;
    }
    .pw-mb-hgoal {
        display: flex;
        align-items: center;
        gap: 14px;
        width: 100%;
        background: none;
        border: 0;
        padding: 14px 16px;
        cursor: pointer;
        text-align: left;
        transition: background 0.15s ease;
    }
    .pw-mb-hgoal:hover {
        background: rgba(255, 255, 255, 0.03);
    }
    .pw-mb-goal-txt {
        min-width: 0;
        flex: 1;
    }
    .pw-mb-hchev {
        color: rgba(220, 220, 225, 0.35);
        flex-shrink: 0;
    }
    .pw-mb-ring-num {
        fill: #ececef;
        font-size: 17px;
        font-weight: 600;
        font-variant-numeric: tabular-nums;
    }
    .pw-mb-goal-title {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.5);
        white-space: nowrap;
    }
    .pw-mb-goal-sub {
        font-size: 16px;
        color: #ececef;
        font-weight: 600;
        margin-top: 3px;
    }
    .pw-mb-goal-cta {
        font-size: 13px;
        color: var(--pw-accent);
        margin-top: 3px;
    }
    .pw-mb-hstats {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .pw-mb-hstats > div {
        padding: 12px 16px;
        display: flex;
        flex-direction: column;
        gap: 1px;
        min-width: 0;
    }
    .pw-mb-hstats > div + div {
        border-left: 1px solid rgba(255, 255, 255, 0.06);
    }
    .pw-mb-hstats b {
        font-size: 20px;
        font-weight: 700;
        color: #ececef;
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-mb-hstats span {
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.45);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-mb-hchart {
        display: flex;
        align-items: flex-end;
        gap: 5px;
        height: 64px;
        padding: 10px 16px 10px;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .pw-mb-col {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: flex-end;
        gap: 4px;
        height: 100%;
        min-width: 0;
    }
    .pw-mb-bar {
        width: 100%;
        max-width: 24px;
        background: linear-gradient(180deg, color-mix(in oklch, var(--pw-accent) 70%, white 30%), var(--pw-accent));
        border-radius: 4px 4px 2px 2px;
        opacity: 0.9;
    }
    .pw-mb-bar.zero {
        background: rgba(255, 255, 255, 0.07);
    }
    .pw-mb-col span {
        font-size: 9px;
        color: rgba(220, 220, 225, 0.35);
        text-transform: uppercase;
    }
    .pw-mb-h {
        margin: 0 0 10px;
    }
    .pw-mb-gitem a {
        min-width: 0;
        text-decoration: none;
    }
    .pw-mb-month {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.4);
        letter-spacing: 0.04em;
        padding: 10px 2px 10px;
    }
    .pw-mb-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
        gap: 12px 10px;
        margin-bottom: 10px;
        max-width: 1100px;
    }
    .pw-mb-gitem {
        position: relative;
        min-width: 0;
    }
    .pw-mb-gcover {
        position: relative;
        display: block;
        aspect-ratio: 2/3;
        border-radius: 10px;
        overflow: hidden;
        background: rgba(255, 255, 255, 0.06);
    }
    .pw-mb-gcover img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .pw-mb-gtitle {
        display: block;
        margin-top: 6px;
        font-size: 12px;
        color: rgba(232, 232, 234, 0.85);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-mb-gdate {
        display: block;
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.4);
        font-variant-numeric: tabular-nums;
    }
    .pw-mb-scrim {
        position: fixed;
        inset: 0;
        z-index: 120;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 16px;
    }
    .pw-mb-pop {
        width: 100%;
        max-width: 280px;
        background: rgba(12, 13, 17, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        padding: 18px;
        display: flex;
        flex-direction: column;
        gap: 16px;
        animation: pw-mb-in 0.18s cubic-bezier(0.2, 0.7, 0.2, 1);
    }
    @keyframes pw-mb-in {
        from {
            transform: translateY(14px);
            opacity: 0;
        }
    }
    .pw-mb-pop-title {
        font-size: 14px;
        font-weight: 600;
        color: #ececef;
    }
    .pw-mb-step {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 18px;
    }
    .pw-mb-step button {
        width: 44px;
        height: 44px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #ececef;
        font-size: 18px;
        cursor: pointer;
    }
    .pw-mb-step span {
        font-size: 26px;
        font-weight: 700;
        color: #ececef;
        min-width: 56px;
        text-align: center;
        font-variant-numeric: tabular-nums;
    }
    .pw-mb-save {
        justify-content: center;
    }
</style>
