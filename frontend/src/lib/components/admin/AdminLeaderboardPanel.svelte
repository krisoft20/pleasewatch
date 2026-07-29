<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type WatchStats } from '$lib/api';
    import Icon from '$lib/components/Icon.svelte';
    import { fmtWatchTime, type CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    let { onCount }: Props = $props();

    let watchStats = $state<WatchStats | null>(null);
    let leaderboardRefreshing = $state(false);

    let podium = $derived(watchStats?.leaderboard.slice(0, 3) ?? []);
    let leaderboardRest = $derived(watchStats?.leaderboard.slice(3) ?? []);
    let avgWatch = $derived(
        watchStats?.leaderboard.length ? Math.round(watchStats.total_watch_seconds / watchStats.leaderboard.length) : 0
    );

    onMount(() => {
        refreshLeaderboard().catch(() => {});
    });

    async function refreshLeaderboard() {
        leaderboardRefreshing = true;
        try {
            watchStats = await api.watchStats();
            onCount?.(watchStats.leaderboard.length || null);
        } finally {
            leaderboardRefreshing = false;
        }
    }

    function leaderName(row: WatchStats['leaderboard'][number]): string {
        return row.username || '(no name)';
    }

    function leaderInitial(row: WatchStats['leaderboard'][number]): string {
        const name = leaderName(row).trim();
        return name === '(no name)' ? '?' : name.charAt(0).toUpperCase();
    }

    function leaderWidth(secs: number): number {
        const max = watchStats?.leaderboard[0]?.watch_seconds || 1;
        return Math.max(4, Math.min(100, (secs / max) * 100));
    }
</script>

{#if !watchStats}
    <div class="pw-leader-loading">loading watch stats...</div>
{:else if watchStats.leaderboard.length === 0}
    <section class="pw-leader-empty">
        <p class="pw-leader-kicker">watch leaderboard</p>
        <h2>no watch time yet</h2>
        <span>start a title in the player and this fills from watch ticks.</span>
        <button onclick={refreshLeaderboard} disabled={leaderboardRefreshing}>refresh</button>
    </section>
{:else}
    <div class="pw-leader-page">
        <section class="pw-leader-hero">
            <div>
                <p class="pw-leader-kicker">watch leaderboard</p>
                <h2>{fmtWatchTime(watchStats.total_watch_seconds)} tracked</h2>
                <span>
                    {watchStats.leaderboard.length} viewers, {fmtWatchTime(avgWatch)} avg watch time
                </span>
            </div>
            <button class="pw-leader-refresh" onclick={refreshLeaderboard} disabled={leaderboardRefreshing}>
                <Icon name="sync" class={`w-4 h-4 ${leaderboardRefreshing ? 'pw-admin-spin' : ''}`} strokeWidth={1.8} />
                refresh
            </button>
        </section>

        <section class="pw-leader-podium" aria-label="Top three viewers">
            {#each podium as row, i (row.user_id)}
                <article class="pw-leader-card" class:gold={i === 0} class:silver={i === 1} class:bronze={i === 2}>
                    <div class="pw-leader-rank">
                        <span>#{i + 1}</span>
                        {#if i === 0}
                            <Icon name="trophy" class="w-8 h-8" strokeWidth={2} />
                        {/if}
                    </div>
                    <div class="pw-leader-avatar">{leaderInitial(row)}</div>
                    <div class="pw-leader-name">
                        <strong>{leaderName(row)}</strong>
                        <small>{row.completed_episodes.toLocaleString()} completed</small>
                    </div>
                    <div class="pw-leader-time">{fmtWatchTime(row.watch_seconds)}</div>
                    <div class="pw-leader-meter">
                        <span style={`width:${leaderWidth(row.watch_seconds)}%`}></span>
                    </div>
                </article>
            {/each}
        </section>

        <section class="pw-leader-table">
            <div class="pw-leader-table-head">
                <div>
                    <p class="pw-leader-kicker">full ranking</p>
                    <h3>{leaderboardRest.length > 0 ? 'the chase pack' : 'top three only so far'}</h3>
                </div>
            </div>
            {#if leaderboardRest.length > 0}
                <div class="pw-leader-rows">
                    {#each leaderboardRest as row, i (row.user_id)}
                        <div class="pw-leader-row">
                            <span class="rank">#{i + 4}</span>
                            <span class="avatar">{leaderInitial(row)}</span>
                            <span class="name">{leaderName(row)}</span>
                            <span class="bar"><i style={`width:${leaderWidth(row.watch_seconds)}%`}></i></span>
                            <span class="done">{row.completed_episodes.toLocaleString()} done</span>
                            <strong>{fmtWatchTime(row.watch_seconds)}</strong>
                        </div>
                    {/each}
                </div>
            {:else}
                <p class="pw-leader-note">no one below the podium yet.</p>
            {/if}
        </section>
    </div>
{/if}

<style>
    .pw-leader-page {
        display: grid;
        gap: 16px;
    }

    .pw-leader-hero,
    .pw-leader-card,
    .pw-leader-table,
    .pw-leader-empty,
    .pw-leader-loading {
        border: 1px solid rgba(255, 255, 255, 0.075);
        background: #0e131b;
        border-radius: 14px;
    }

    .pw-leader-hero {
        position: relative;
        overflow: hidden;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 18px;
        padding: 24px;
        background:
            radial-gradient(circle at 18% 0%, rgba(213, 166, 72, 0.18), transparent 28%),
            linear-gradient(135deg, rgba(91, 141, 239, 0.12), transparent 52%), #0e131b;
    }

    .pw-leader-hero::after {
        content: '';
        position: absolute;
        right: 18px;
        bottom: -50px;
        width: 280px;
        height: 120px;
        border-radius: 50%;
        background: radial-gradient(ellipse, rgba(213, 166, 72, 0.12), transparent 68%);
        pointer-events: none;
    }

    .pw-leader-kicker {
        margin: 0 0 6px;
        color: #8c7b58;
        font:
            650 10px/1.2 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        letter-spacing: 0.16em;
        text-transform: uppercase;
    }

    .pw-leader-hero h2,
    .pw-leader-empty h2,
    .pw-leader-table h3 {
        margin: 0;
        color: #f4f1ea;
        letter-spacing: -0.035em;
    }

    .pw-leader-hero h2 {
        font-size: clamp(25px, 4vw, 42px);
        line-height: 1;
        font-weight: 720;
    }

    .pw-leader-hero span,
    .pw-leader-empty span {
        display: block;
        margin-top: 9px;
        color: #808b9d;
        font:
            11px/1.5 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-leader-refresh,
    .pw-leader-empty button {
        position: relative;
        z-index: 1;
        display: inline-flex;
        align-items: center;
        gap: 7px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.035);
        color: #b8c0cd;
        padding: 8px 11px;
        font-size: 11px;
        cursor: pointer;
    }

    .pw-leader-refresh:hover,
    .pw-leader-empty button:hover {
        border-color: rgba(255, 255, 255, 0.18);
        color: #f1f3f7;
        background: rgba(255, 255, 255, 0.055);
    }

    .pw-leader-refresh:disabled,
    .pw-leader-empty button:disabled {
        opacity: 0.55;
        cursor: wait;
    }

    .pw-leader-podium {
        display: grid;
        grid-template-columns: minmax(0, 1.25fr) repeat(2, minmax(0, 0.9fr));
        align-items: end;
        gap: 12px;
    }

    .pw-leader-card {
        position: relative;
        overflow: hidden;
        min-height: 188px;
        padding: 18px;
        background: linear-gradient(180deg, rgba(255, 255, 255, 0.035), rgba(255, 255, 255, 0.01)), #101720;
    }

    .pw-leader-card::before {
        content: '';
        position: absolute;
        inset: 0 0 auto;
        height: 3px;
        background: #667389;
    }

    .pw-leader-card.gold {
        min-height: 232px;
        background:
            radial-gradient(circle at 78% 16%, rgba(213, 166, 72, 0.26), transparent 34%),
            linear-gradient(180deg, rgba(213, 166, 72, 0.08), rgba(255, 255, 255, 0.01)), #101720;
    }

    .pw-leader-card.gold::before {
        background: linear-gradient(90deg, #9e7530, #f2d38a, #9e7530);
    }

    .pw-leader-card.silver::before {
        background: linear-gradient(90deg, #778193, #d9e0ec, #778193);
    }

    .pw-leader-card.bronze::before {
        background: linear-gradient(90deg, #8b563b, #d39267);
    }

    .pw-leader-rank {
        display: flex;
        align-items: center;
        justify-content: space-between;
        color: #8d98aa;
    }

    .pw-leader-rank span {
        font:
            700 12px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        letter-spacing: 0.08em;
    }

    .pw-leader-card.gold .pw-leader-rank {
        color: #e6c36f;
    }

    .pw-leader-avatar {
        display: grid;
        place-items: center;
        width: 48px;
        height: 48px;
        margin-top: 18px;
        border-radius: 15px;
        background: rgba(255, 255, 255, 0.06);
        color: #ecf0f7;
        font-weight: 760;
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    }

    .pw-leader-card.gold .pw-leader-avatar {
        width: 58px;
        height: 58px;
        border-radius: 18px;
        background: rgba(213, 166, 72, 0.16);
        color: #f6dd9b;
    }

    .pw-leader-name {
        margin-top: 15px;
    }

    .pw-leader-name strong,
    .pw-leader-name small,
    .pw-leader-time {
        display: block;
    }

    .pw-leader-name strong {
        overflow: hidden;
        color: #f1f4f8;
        font-size: 18px;
        letter-spacing: -0.02em;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pw-leader-name small {
        margin-top: 4px;
        color: #6e7a8e;
        font-size: 10px;
    }

    .pw-leader-time {
        margin-top: 18px;
        color: #dce3ed;
        font:
            700 21px/1 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-leader-card.gold .pw-leader-time {
        color: #f2d38a;
        font-size: 28px;
    }

    .pw-leader-meter {
        height: 5px;
        margin-top: 16px;
        overflow: hidden;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.07);
    }

    .pw-leader-meter span {
        display: block;
        height: 100%;
        border-radius: inherit;
        background: #74829a;
    }

    .pw-leader-card.gold .pw-leader-meter span {
        background: linear-gradient(90deg, #ad7d32, #f2d38a);
    }

    .pw-leader-card.silver .pw-leader-meter span {
        background: linear-gradient(90deg, #7d899d, #d7dfed);
    }

    .pw-leader-card.bronze .pw-leader-meter span {
        background: linear-gradient(90deg, #8b563b, #d39267);
    }

    .pw-leader-table {
        padding: 18px;
    }

    .pw-leader-table-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        margin-bottom: 14px;
    }

    .pw-leader-table h3 {
        font-size: 15px;
        font-weight: 640;
    }

    .pw-leader-rows {
        display: grid;
        gap: 7px;
    }

    .pw-leader-row {
        display: grid;
        grid-template-columns: 44px 34px minmax(130px, 1fr) minmax(120px, 1.2fr) 82px 82px;
        align-items: center;
        gap: 11px;
        border: 1px solid rgba(255, 255, 255, 0.055);
        border-radius: 9px;
        background: #111822;
        padding: 9px 11px;
    }

    .pw-leader-row .rank {
        color: #657186;
        font:
            700 11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-leader-row .avatar {
        display: grid;
        place-items: center;
        width: 30px;
        height: 30px;
        border-radius: 9px;
        background: rgba(91, 141, 239, 0.11);
        color: #9bb8ef;
        font-size: 11px;
        font-weight: 720;
    }

    .pw-leader-row .name {
        min-width: 0;
        overflow: hidden;
        color: #dfe4ec;
        font-size: 12px;
        font-weight: 620;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pw-leader-row .bar {
        height: 5px;
        overflow: hidden;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.06);
    }

    .pw-leader-row .bar i {
        display: block;
        height: 100%;
        border-radius: inherit;
        background: #5b8def;
    }

    .pw-leader-row .done {
        color: #687487;
        font-size: 10px;
    }

    .pw-leader-row strong {
        color: #d9e0ea;
        font:
            700 11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        text-align: right;
    }

    .pw-leader-empty,
    .pw-leader-loading {
        padding: 44px 24px;
        text-align: center;
    }

    .pw-leader-empty h2 {
        font-size: 26px;
    }

    .pw-leader-empty button {
        margin-top: 18px;
    }

    .pw-leader-loading,
    .pw-leader-note {
        color: #6e7a8e;
        font:
            11px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    :global(.pw-admin-spin) {
        animation: pw-admin-spin 0.8s linear infinite;
    }

    @keyframes pw-admin-spin {
        to {
            transform: rotate(360deg);
        }
    }

    @media (max-width: 900px) {
        .pw-leader-podium {
            grid-template-columns: 1fr;
            align-items: stretch;
        }

        .pw-leader-card,
        .pw-leader-card.gold {
            min-height: 0;
        }

        .pw-leader-row {
            grid-template-columns: 38px 32px minmax(110px, 1fr) 90px 78px;
        }

        .pw-leader-row .bar {
            display: none;
        }
    }

    @media (max-width: 680px) {
        .pw-leader-hero {
            align-items: flex-start;
            flex-direction: column;
            padding: 18px;
        }

        .pw-leader-hero h2 {
            font-size: 30px;
        }

        .pw-leader-card {
            padding: 15px;
        }

        .pw-leader-table {
            padding: 14px;
        }

        .pw-leader-row {
            grid-template-columns: 34px 30px minmax(0, 1fr) 74px;
            gap: 8px;
        }

        .pw-leader-row .done {
            display: none;
        }
    }
</style>
