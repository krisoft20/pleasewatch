<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type LiveGame } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import {
        abbrOf,
        countdown,
        darkLogo,
        FAMILIES,
        familyOfGame,
        sourceOf,
        favTeams,
        ink,
        livePick,
        pairColors
    } from '$lib/live.svelte';
    import Player from './Player.svelte';
    import YoutubeFrame from './YoutubeFrame.svelte';

    let { tab = 'live' }: { tab?: string } = $props();

    let games = $state<LiveGame[]>([]);
    let loading = $state(true);
    let fetchErr = $state<string | null>(null);
    let family = $state('all');
    let league = $state('all');
    let day = $state(0);
    let mineOnly = $state(false);
    let opening = $state<string | null>(null);
    let openErr = $state<string | null>(null);
    let stream = $state<{ url: string; game: LiveGame; youtube?: string } | null>(null);
    let retries = 0;
    let lastRetry = 0;
    let recovering = false;

    const view = $derived(tab === 'teams' ? 'teams' : tab === 'schedule' ? 'schedule' : 'live');

    $effect(() => {
        if (!livePick.id) return;
        const hit = games.find((g) => g.id === livePick.id);
        livePick.id = null;
        if (hit) watch(hit);
    });

    onMount(() => {
        load();
        const poll = setInterval(() => {
            if (!stream) load(true);
        }, 45000);
        return () => clearInterval(poll);
    });

    async function load(quiet = false) {
        if (!quiet) loading = true;
        fetchErr = null;
        try {
            const r = await api.liveSchedule();
            games = r.games;
        } catch (e) {
            if (!quiet) fetchErr = String((e as Error)?.message ?? e);
        } finally {
            loading = false;
        }
    }

    async function watch(g: LiveGame) {
        if (opening) return;
        opening = g.id;
        openErr = null;
        retries = 0;
        lastRetry = 0;
        recovering = false;
        try {
            const r = await api.liveResolve(g.source_url);
            stream = { url: r.master_url, game: g, youtube: r.youtube };
        } catch (e) {
            openErr = errText(e, g);
            console.error('[live] resolve', e);
        } finally {
            opening = null;
        }
    }

    async function onStreamError() {
        const cur = stream;
        if (!cur || recovering) return;
        recovering = true;
        try {
            if (retries >= 4) {
                stream = null;
                openErr = t('live.error');
                return;
            }
            const wait = 8000 - (Date.now() - lastRetry);
            if (wait > 0) await new Promise((r) => setTimeout(r, wait));
            if (stream !== cur) return;
            retries++;
            lastRetry = Date.now();
            const r = await api.liveResolve(cur.game.source_url);
            stream = { url: r.master_url + '?r=' + lastRetry, game: cur.game };
        } catch (e) {
            stream = null;
            openErr = errText(e, cur.game);
        } finally {
            recovering = false;
        }
    }

    function errText(e: unknown, g?: LiveGame): string {
        const msg = (e as Error)?.message;
        if (msg === 'youtube only') return t('live.error.youtube');
        if (msg === 'p2p only') return t('live.error.p2p');
        if (msg !== 'dead stream') return t('live.error');
        if (g && stateOf(g) === 'pre') {
            const when = countdown(g.starts_in ?? null);
            return when ? t('live.error.early', { when }) : t('live.error.notyet');
        }
        return t('live.error.dead');
    }

    const stateOf = (g: LiveGame) => g.espn?.state || (g.status === 'live' ? 'in' : 'pre');
    const groupOf = (g: LiveGame) => g.league || g.sport.toUpperCase();
    const isMine = (g: LiveGame) => favTeams.has(g.away) || favTeams.has(g.home) || favTeams.has(g.label);

    function bar(g: LiveGame) {
        const e = g.espn;
        const st = stateOf(g);
        const [fa, fb] = pairColors(g.away || g.label, g.home || g.label);
        const solo = !g.away;
        const scored = !!e && st !== 'pre';
        const a = {
            name: solo ? g.label : e?.away.name || g.away,
            abbr: e?.away.abbr || abbrOf(g.away || g.label),
            logo: e?.away.logo ?? null,
            color: e?.away.color || fa,
            record: (solo ? g.league : e?.away.record) ?? '',
            score: scored ? (e?.away.score ?? '') : '',
            mine: favTeams.has(g.away || g.label)
        };
        const b = {
            name: e?.home.name || g.home,
            abbr: e?.home.abbr || abbrOf(g.home),
            logo: e?.home.logo ?? null,
            color: e?.home.color || fb,
            record: e?.home.record ?? '',
            score: scored ? (e?.home.score ?? '') : '',
            mine: favTeams.has(g.home)
        };
        const lead = a.score && b.score ? (+a.score === +b.score ? null : +a.score > +b.score ? 'a' : 'b') : null;
        return {
            solo,
            live: st === 'in',
            final: st === 'post',
            a,
            b,
            lead,
            status:
                st === 'in'
                    ? t('live.status.live')
                    : st === 'post'
                      ? t('live.status.final')
                      : t('live.status.scheduled'),
            clock: st === 'pre' ? countdown(g.starts_in) || e?.detail || (g.start_hint ?? '') : e?.detail || '',
            network: e?.network ?? ''
        };
    }

    function follow(side: { name: string; color: string; logo: string | null }, g: LiveGame) {
        favTeams.toggle({
            name: side.name,
            color: side.color,
            logo: side.logo ?? undefined,
            league: g.league
        });
    }

    function startAt(g: LiveGame): Date | null {
        return g.starts_in === null ? null : new Date(Date.now() + g.starts_in * 60000);
    }
    const HHMM = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });
    const DAY = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
    function dayOffset(d: Date): number {
        const midnight = new Date();
        midnight.setHours(0, 0, 0, 0);
        return Math.floor((d.getTime() - midnight.getTime()) / 86400000);
    }

    const families = $derived.by(() => {
        const n = new Map<string, { total: number; live: number }>();
        for (const g of games) {
            const k = familyOfGame(g);
            const e = n.get(k) ?? { total: 0, live: 0 };
            e.total++;
            if (stateOf(g) === 'in') e.live++;
            n.set(k, e);
        }
        return FAMILIES.filter((f) => n.has(f.key)).map((f) => ({ ...f, ...n.get(f.key)! }));
    });

    const inFamily = $derived(family === 'all' ? games : games.filter((g) => familyOfGame(g) === family));

    const leagues = $derived.by(() => {
        const counts = new Map<string, number>();
        for (const g of inFamily) counts.set(groupOf(g), (counts.get(groupOf(g)) ?? 0) + 1);
        return [...counts.entries()].sort((a, b) => b[1] - a[1]);
    });

    const inLeague = $derived(league === 'all' ? inFamily : inFamily.filter((g) => groupOf(g) === league));

    $effect(() => {
        if (family !== 'all' && !families.some((f) => f.key === family)) family = 'all';
    });
    $effect(() => {
        if (league !== 'all' && !leagues.some(([n]) => n === league)) league = 'all';
    });

    function pickFamily(k: string) {
        family = k;
        league = 'all';
    }
    const liveNow = $derived(inLeague.filter((g) => stateOf(g) === 'in'));
    const soonAll = $derived(
        games.filter((g) => stateOf(g) === 'pre').sort((a, b) => (a.starts_in ?? 1e9) - (b.starts_in ?? 1e9))
    );
    const soon = $derived(inLeague.filter((g) => stateOf(g) === 'pre'));

    const ticker = $derived(
        games.filter((g) => g.espn && stateOf(g) === 'in' && g.espn.away.score && g.espn.home.score).slice(0, 14)
    );

    const rank = (g: LiveGame) => ({ in: 0, pre: 1, post: 2 })[stateOf(g)] ?? 1;
    const sections = $derived.by(() => {
        const by = new Map<string, LiveGame[]>();
        for (const g of inLeague) {
            const k = groupOf(g);
            if (!by.has(k)) by.set(k, []);
            by.get(k)!.push(g);
        }
        return [...by.entries()]
            .map(([name, list]) => ({
                name,
                games: list.sort((a, b) => rank(a) - rank(b) || (a.starts_in ?? 1e9) - (b.starts_in ?? 1e9))
            }))
            .sort(
                (x, y) =>
                    y.games.filter((g) => stateOf(g) === 'in').length -
                    x.games.filter((g) => stateOf(g) === 'in').length
            );
    });

    const myLive = $derived(games.filter((g) => stateOf(g) === 'in' && isMine(g)));
    const myNext = $derived(
        games
            .filter((g) => stateOf(g) === 'pre' && isMine(g))
            .sort((a, b) => (a.starts_in ?? 1e9) - (b.starts_in ?? 1e9))
    );

    const days = $derived.by(() => {
        const counts = new Map<number, number>();
        for (const g of soonAll) {
            const at = startAt(g);
            if (!at) continue;
            const d = dayOffset(at);
            if (d < 0 || d > 6) continue;
            counts.set(d, (counts.get(d) ?? 0) + 1);
        }
        return [...counts.entries()]
            .sort((a, b) => a[0] - b[0])
            .map(([offset, n]) => {
                const d = new Date();
                d.setDate(d.getDate() + offset);
                return { offset, n, label: offset === 0 ? t('live.today') : DAY.format(d) };
            });
    });

    const slots = $derived.by(() => {
        const by = new Map<string, { at: Date; games: LiveGame[] }>();
        for (const g of soonAll) {
            const at = startAt(g);
            if (!at || dayOffset(at) !== day) continue;
            if (mineOnly && !isMine(g)) continue;
            const key = HHMM.format(at);
            if (!by.has(key)) by.set(key, { at, games: [] });
            by.get(key)!.games.push(g);
        }
        return [...by.entries()]
            .sort((a, b) => a[1].at.getTime() - b[1].at.getTime())
            .map(([time, v]) => ({ time, rel: countdown(v.games[0].starts_in), games: v.games }));
    });
    const slotTotal = $derived(slots.reduce((n, s) => n + s.games.length, 0));
</script>

{#snippet crest(team: { abbr: string; logo: string | null; color: string })}
    {#if team.logo}
        <img class="lv-logo" src={darkLogo(team.logo)} alt="" loading="lazy" />
    {:else}
        <span class="lv-chip" style="background:{team.color};color:{ink(team.color)}">{team.abbr}</span>
    {/if}
{/snippet}

{#snippet gameBar(g: LiveGame)}
    {@const v = bar(g)}
    <div
        class="lv-bar"
        class:is-solo={v.solo}
        class:is-final={v.final}
        style="--ca:{v.a.color};--cb:{v.b.color};--ia:{ink(v.a.color)};--ib:{ink(v.b.color)}"
        role="button"
        tabindex="0"
        onclick={() => watch(g)}
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && watch(g)}
    >
        <span class="lv-field lv-field-a"></span>
        {#if !v.solo}<span class="lv-field lv-field-b"></span>{/if}
        <span class="lv-shade"></span>

        {#if !v.solo}
            <div class="lv-mid">
                <span class="lv-flag" class:is-live={v.live}>
                    {#if v.live}<span class="lv-dot"></span>{/if}{v.status}
                </span>
                {#if v.clock}<span class="lv-clock">{v.clock}</span>{/if}
            </div>
        {/if}

        <div class="lv-bar-in">
            <div class="lv-side lv-side-a">
                {@render crest(v.a)}
                <div class="lv-id">
                    <div class="lv-name">{v.a.name}</div>
                    {#if v.a.record}<div class="lv-rec">{v.a.record}</div>{/if}
                </div>
                {#if v.a.mine}<span class="lv-star">★</span>{/if}
                {#if v.a.score}<span class="lv-score" class:is-dim={v.lead === 'b'}>{v.a.score}</span>{/if}
            </div>

            {#if v.solo}
                <div class="lv-solo-meta">
                    <span class="lv-flag" class:is-live={v.live}>
                        {#if v.live}<span class="lv-dot"></span>{/if}{v.status}
                    </span>
                    {#if v.clock}<span class="lv-clock">{v.clock}</span>{/if}
                </div>
            {:else}
                <div class="lv-gap"></div>
                <div class="lv-side lv-side-b">
                    {#if v.b.score}<span class="lv-score" class:is-dim={v.lead === 'a'}>{v.b.score}</span>{/if}
                    {#if v.b.mine}<span class="lv-star">★</span>{/if}
                    <div class="lv-id lv-id-r">
                        <div class="lv-name">{v.b.name}</div>
                        {#if v.b.record}<div class="lv-rec">{v.b.record}</div>{/if}
                    </div>
                    {@render crest(v.b)}
                </div>
            {/if}

            <div class="lv-rail">
                {#if !v.solo}
                    <span class="lv-rail-meta">
                        <span class="lv-flag" class:is-live={v.live}>
                            {#if v.live}<span class="lv-dot"></span>{/if}{v.status}
                        </span>
                        {#if v.clock}<span class="lv-clock">{v.clock}</span>{/if}
                    </span>
                {/if}
                {#if v.network}<span class="lv-net">{v.network}</span>{/if}
                <span class="lv-play">
                    {#if opening === g.id}
                        <span class="lv-spin"></span>
                    {:else}
                        <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"
                            ><path d="M8 5v14l11-7z" /></svg
                        >
                    {/if}
                </span>
            </div>
        </div>

        {#if !v.solo}
            <button
                class="lv-follow"
                class:is-on={v.a.mine}
                aria-label={t('live.follow')}
                onclick={(e) => {
                    e.stopPropagation();
                    follow(v.a, g);
                }}>★</button
            >
        {/if}
    </div>
{/snippet}

{#snippet sectionHead(name: string, n: number, note = '')}
    <div class="lv-head">
        <span class="lv-head-name">{name}</span>
        <span class="lv-head-n">{n}</span>
        {#if note}<span class="lv-head-note">{note}</span>{/if}
        <span class="lv-head-rule"></span>
    </div>
{/snippet}

<div class="pw-filter-bar">
    <div class="pw-filter-bar-inner">
        {#if view === 'live'}
            <div class="lv-filter">
                <div class="lv-chips">
                    <button class="pw-chip lv-fam" class:is-active={family === 'all'} onclick={() => pickFamily('all')}>
                        {t('live.filter.all')}<span class="pw-chip-n">{games.length}</span>
                    </button>
                    {#each families as f (f.key)}
                        <button
                            class="pw-chip lv-fam"
                            class:is-active={family === f.key}
                            style="--fam:{f.tint}"
                            onclick={() => pickFamily(f.key)}
                        >
                            <span class="lv-fam-bar"></span>{f.label}<span class="pw-chip-n">{f.total}</span>
                            {#if f.live > 0}<span class="lv-fam-live">{f.live}</span>{/if}
                        </button>
                    {/each}
                </div>
                {#if family !== 'all' && leagues.length > 1}
                    <div class="lv-chips lv-subs">
                        <button
                            class="pw-chip lv-lg"
                            class:is-active={league === 'all'}
                            onclick={() => (league = 'all')}
                        >
                            {t('live.filter.allLeagues')}<span class="pw-chip-n">{inFamily.length}</span>
                        </button>
                        {#each leagues as [name, n] (name)}
                            <button
                                class="pw-chip lv-lg"
                                class:is-active={league === name}
                                onclick={() => (league = name)}
                            >
                                {name}<span class="pw-chip-n">{n}</span>
                            </button>
                        {/each}
                    </div>
                {/if}
            </div>
        {:else if view === 'schedule'}
            <div class="lv-chips">
                {#each days as d (d.offset)}
                    <button class="pw-chip" class:is-active={day === d.offset} onclick={() => (day = d.offset)}>
                        {d.label}<span class="pw-chip-n">{d.n}</span>
                    </button>
                {/each}
                <button class="pw-chip lv-mine-chip" class:is-active={mineOnly} onclick={() => (mineOnly = !mineOnly)}
                    >{t('live.schedule.mineOnly')}</button
                >
            </div>
        {:else}
            <div class="lv-chips"><span class="lv-bar-note">{t('live.teams.note')}</span></div>
        {/if}
        <div class="lv-bar-meta">
            <span>{t('live.meta', { live: liveNow.length, soon: soon.length })}</span>
            <button class="pw-see-all" onclick={() => load()} disabled={loading}>{t('live.refresh')}</button>
        </div>
    </div>
</div>

{#if view === 'live' && ticker.length > 0}
    <div class="lv-ticker">
        <div class="lv-ticker-flag"><span class="lv-dot"></span>{t('live.status.live')}</div>
        <div class="lv-ticker-rail">
            {#each ticker as g (g.id)}
                {@const v = bar(g)}
                <button class="lv-tick" onclick={() => watch(g)}>
                    <span class="lv-tick-chip" style="background:{v.a.color};color:{ink(v.a.color)}">{v.a.abbr}</span>
                    <span class="lv-tick-score">{v.a.score}</span>
                    <span class="lv-tick-sep">/</span>
                    <span class="lv-tick-chip" style="background:{v.b.color};color:{ink(v.b.color)}">{v.b.abbr}</span>
                    <span class="lv-tick-score">{v.b.score}</span>
                    <span class="lv-tick-clock">{v.clock}</span>
                </button>
            {/each}
        </div>
    </div>
{/if}

{#if openErr}
    <section class="pw-section pw-section-tight"><p class="lv-note is-err">{openErr}</p></section>
{/if}

{#if loading && games.length === 0}
    <section class="pw-section pw-section-tight"><p class="lv-note">{t('live.loading')}</p></section>
{:else if fetchErr}
    <section class="pw-section pw-section-tight"><p class="lv-note is-err">{fetchErr}</p></section>
{:else if view === 'live'}
    {#if sections.length === 0}
        <section class="pw-section pw-empty">
            <div class="pw-empty-card">
                <div class="pw-empty-tag">{t('live.title')}</div>
                <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('live.empty')}</h2>
                <p class="pw-empty-text">{t('live.sports.hint')}</p>
            </div>
        </section>
    {:else}
        <section class="pw-section pw-section-tight">
            {#each sections as sec (sec.name)}
                <div class="lv-group">
                    {@render sectionHead(sec.name, sec.games.length)}
                    <div class="lv-bars">
                        {#each sec.games as g (g.id)}
                            {@render gameBar(g)}
                        {/each}
                    </div>
                </div>
            {/each}
        </section>
    {/if}
{:else if view === 'teams'}
    <section class="pw-section pw-section-tight">
        {@render sectionHead(t('live.tab.teams'), favTeams.teams.length)}
        {#if favTeams.teams.length === 0}
            <div class="pw-empty-card">
                <div class="pw-empty-tag">{t('live.tab.teams')}</div>
                <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('live.teams.none')}</h2>
                <p class="pw-empty-text">{t('live.teams.hint')}</p>
            </div>
        {:else}
            <div class="lv-roster">
                {#each favTeams.teams as team (team.name)}
                    <div class="lv-team" style="--c:{team.color ?? '#5a5a63'}">
                        {@render crest({
                            abbr: abbrOf(team.name),
                            logo: team.logo ?? null,
                            color: team.color ?? '#5a5a63'
                        })}
                        <div class="lv-team-id">
                            <div class="lv-team-name">{team.name}</div>
                            {#if team.league}<div class="lv-team-sub">{team.league}</div>{/if}
                        </div>
                        <button
                            class="lv-team-drop"
                            onclick={() => favTeams.remove(team.name)}
                            aria-label={t('live.unfollow')}
                        >
                            <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                            >
                                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
                            </svg>
                        </button>
                    </div>
                {/each}
            </div>
        {/if}
    </section>

    {#if favTeams.teams.length}
        <section class="pw-section pw-section-tight">
            <div class="lv-group">
                {@render sectionHead(t('live.teams.playing'), myLive.length)}
                {#if myLive.length === 0}
                    <p class="lv-note">{t('live.empty.mine')}</p>
                {:else}
                    <div class="lv-bars">
                        {#each myLive as g (g.id)}
                            {@render gameBar(g)}
                        {/each}
                    </div>
                {/if}
            </div>
            <div class="lv-group">
                {@render sectionHead(t('live.teams.next'), myNext.length)}
                {#if myNext.length === 0}
                    <p class="lv-note">{t('live.teams.nonext')}</p>
                {:else}
                    <div class="lv-bars">
                        {#each myNext as g (g.id)}
                            {@render gameBar(g)}
                        {/each}
                    </div>
                {/if}
            </div>
        </section>
    {/if}
{:else}
    <section class="pw-section pw-section-tight">
        {#if slots.length === 0}
            <div class="pw-empty-card">
                <div class="pw-empty-tag">{t('live.tab.schedule')}</div>
                <h2 class="pw-h2-lg" style="margin-top: 8px;">{t('live.schedule.none')}</h2>
                <p class="pw-empty-text">{t('live.sports.hint')}</p>
            </div>
        {:else}
            {@render sectionHead(days.find((d) => d.offset === day)?.label ?? t('live.today'), slotTotal)}
            {#each slots as sl (sl.time)}
                <div class="lv-slot">
                    <div class="lv-slot-time">
                        <div class="lv-slot-hh">{sl.time}</div>
                        <div class="lv-slot-rel">{sl.rel}</div>
                    </div>
                    <div class="lv-bars lv-slot-bars">
                        {#each sl.games as g (g.id)}
                            {@render gameBar(g)}
                        {/each}
                    </div>
                </div>
            {/each}
        {/if}
    </section>
{/if}

{#if stream?.youtube}
    <YoutubeFrame id={stream.youtube} title={stream.game.label} onBack={() => (stream = null)} />
{:else if stream}
    <Player
        src={stream.url}
        title={stream.game.label}
        onBack={() => (stream = null)}
        {onStreamError}
        live
        sourceName={sourceOf(stream.game.source_url).name}
        sourceMirrors={sourceOf(stream.game.source_url).mirrors}
    />
{/if}

<style>
    .lv-chips {
        display: flex;
        gap: 6px;
        flex-wrap: wrap;
        min-width: 0;
        align-items: center;
    }
    .lv-filter {
        display: flex;
        flex-direction: column;
        gap: 7px;
        min-width: 0;
    }
    .lv-fam {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 14px;
        font-weight: 600;
        letter-spacing: 0.05em;
        text-transform: uppercase;
        padding: 4px 11px 4px 8px;
        border-radius: 4px;
        gap: 7px;
    }
    .lv-fam-bar {
        width: 3px;
        align-self: stretch;
        margin: 1px 0;
        border-radius: 2px;
        background: var(--fam, #b9b9c2);
    }
    .lv-fam.is-active {
        background: var(--fam, rgba(255, 255, 255, 0.94));
        color: #08090b;
    }
    .lv-fam.is-active .lv-fam-bar {
        background: rgba(8, 9, 11, 0.55);
    }
    .lv-fam-live {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0;
        padding: 1px 4px;
        border-radius: 3px;
        background: #e0392f;
        color: #fff;
        font-variant-numeric: tabular-nums;
    }
    .lv-fam.is-active .lv-fam-live {
        background: rgba(8, 9, 11, 0.72);
    }
    .lv-subs {
        padding-left: 2px;
    }
    .lv-lg {
        font-size: 11px;
        padding: 3px 9px;
        border-radius: 4px;
    }
    .lv-bar-note {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.32);
    }
    .lv-mine-chip.is-active {
        background: color-mix(in oklch, oklch(0.82 0.14 72) 16%, transparent);
        border-color: color-mix(in oklch, oklch(0.82 0.14 72) 40%, transparent);
        color: oklch(0.86 0.12 72);
    }
    .lv-bar-meta {
        display: flex;
        align-items: center;
        gap: 14px;
        margin-left: auto;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        white-space: nowrap;
    }
    .lv-note {
        margin: 8px 0 14px;
        font-size: 13px;
        color: var(--pw-fg-dim);
    }
    .lv-note.is-err {
        color: oklch(0.8 0.13 28);
    }

    .lv-ticker {
        display: flex;
        align-items: stretch;
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
        background: #0b0c0f;
    }
    .lv-ticker-flag {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 0 26px 0 clamp(16px, 4vw, 48px);
        background: #e2402c;
        color: #fff;
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 15px;
        font-weight: 700;
        letter-spacing: 0.14em;
        text-transform: uppercase;
        clip-path: polygon(0 0, 100% 0, calc(100% - 13px) 100%, 0 100%);
        flex-shrink: 0;
    }
    .lv-ticker-rail {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: center;
        gap: 20px;
        padding: 8px 16px;
        overflow-x: auto;
        scrollbar-width: none;
    }
    .lv-ticker-rail::-webkit-scrollbar {
        display: none;
    }
    .lv-tick {
        display: flex;
        align-items: center;
        gap: 7px;
        flex-shrink: 0;
        border: 0;
        background: none;
        padding: 0;
        cursor: pointer;
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
    }
    .lv-tick:hover .lv-tick-score {
        color: #fff;
    }
    .lv-tick-chip {
        display: inline-grid;
        place-items: center;
        min-width: 34px;
        height: 20px;
        padding: 0 6px;
        border-radius: 4px;
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0.05em;
    }
    .lv-tick-score {
        font-size: 16px;
        font-weight: 600;
        color: #ececef;
        font-variant-numeric: tabular-nums;
    }
    .lv-tick-sep {
        color: rgba(220, 220, 225, 0.22);
    }
    .lv-tick-clock {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 9.5px;
        color: rgba(220, 220, 225, 0.34);
        margin-left: 2px;
        white-space: nowrap;
    }

    .lv-group {
        margin-bottom: 18px;
    }
    .lv-head {
        display: flex;
        align-items: center;
        gap: 10px;
        margin: 14px 0 8px;
    }
    .lv-head-name {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 20px;
        font-weight: 700;
        letter-spacing: 0.06em;
        text-transform: uppercase;
        color: #f4f4f6;
    }
    .lv-head-n,
    .lv-head-note {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        color: rgba(220, 220, 225, 0.3);
    }
    .lv-head-rule {
        flex: 1;
        height: 2px;
        background: linear-gradient(90deg, rgba(226, 64, 44, 0.75), rgba(255, 255, 255, 0.05));
    }

    .lv-bars {
        display: flex;
        flex-direction: column;
        gap: 5px;
    }
    .lv-bar {
        position: relative;
        overflow: hidden;
        height: 66px;
        border-radius: 7px;
        background: #0d0e12;
        cursor: pointer;
        transition: filter 0.15s ease;
    }
    .lv-bar:hover {
        filter: brightness(1.12);
    }
    .lv-bar.is-final {
        opacity: 0.55;
    }
    .lv-field {
        position: absolute;
        inset: 0;
    }
    .lv-field-a {
        background: var(--ca);
        clip-path: polygon(0 0, calc(50% + 13px) 0, calc(50% - 17px) 100%, 0 100%);
    }
    .is-solo .lv-field-a {
        clip-path: polygon(0 0, calc(100% - 15px) 0, calc(100% - 45px) 100%, 0 100%);
    }
    .lv-field-b {
        background: var(--cb);
        clip-path: polygon(calc(50% + 17px) 0, 100% 0, 100% 100%, calc(50% - 13px) 100%);
    }
    .lv-shade {
        position: absolute;
        inset: 0;
        background: linear-gradient(
            90deg,
            rgba(0, 0, 0, 0.34) 0%,
            rgba(0, 0, 0, 0.04) 26%,
            rgba(0, 0, 0, 0.42) 50%,
            rgba(0, 0, 0, 0.04) 74%,
            rgba(0, 0, 0, 0.34) 100%
        );
    }

    .lv-mid {
        position: absolute;
        left: 50%;
        top: 0;
        bottom: 0;
        transform: translateX(-50%);
        z-index: 3;
        width: 124px;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 3px;
    }
    .lv-bar-in {
        position: relative;
        z-index: 2;
        display: flex;
        align-items: center;
        height: 100%;
        padding: 0 14px;
    }
    .lv-gap {
        width: 124px;
        flex-shrink: 0;
    }
    .lv-side {
        display: flex;
        align-items: center;
        gap: 11px;
        flex: 1;
        min-width: 0;
    }
    .lv-side-a {
        color: var(--ia);
    }
    .lv-side-b {
        justify-content: flex-end;
        color: var(--ib);
    }
    .lv-id {
        min-width: 0;
    }
    .lv-id-r {
        text-align: right;
    }
    .lv-name {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 21px;
        font-weight: 600;
        line-height: 1.05;
        letter-spacing: 0.01em;
        color: currentColor;
        text-shadow: 0 1px 8px rgba(0, 0, 0, 0.45);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .lv-rec {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 9px;
        letter-spacing: 0.04em;
        opacity: 0.62;
        margin-top: 1px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .lv-score {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 40px;
        font-weight: 700;
        line-height: 1;
        font-variant-numeric: tabular-nums;
        flex-shrink: 0;
        min-width: 46px;
        text-align: center;
        text-shadow: 0 2px 12px rgba(0, 0, 0, 0.5);
    }
    .lv-score.is-dim {
        opacity: 0.62;
    }
    .lv-star {
        color: #ffcf4a;
        font-size: 11px;
        flex-shrink: 0;
    }
    .lv-flag {
        display: inline-flex;
        align-items: center;
        height: 17px;
        padding: 0 8px;
        border-radius: 3px;
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 11px;
        font-weight: 700;
        letter-spacing: 0.12em;
        text-transform: uppercase;
        background: rgba(0, 0, 0, 0.45);
        color: rgba(255, 255, 255, 0.75);
        white-space: nowrap;
    }
    .lv-flag.is-live {
        background: #e2402c;
        color: #fff;
    }
    .lv-dot {
        width: 4px;
        height: 4px;
        border-radius: 999px;
        margin-right: 5px;
        background: #fff;
        animation: lv-pulse 1.6s ease-in-out infinite;
    }
    @keyframes lv-pulse {
        0%,
        100% {
            opacity: 0.3;
        }
        50% {
            opacity: 1;
        }
    }
    .lv-clock {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 15px;
        font-weight: 600;
        letter-spacing: 0.07em;
        color: #fff;
        text-shadow: 0 1px 6px rgba(0, 0, 0, 0.7);
        white-space: nowrap;
    }
    .lv-solo-meta {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 3px;
        flex-shrink: 0;
        color: #fff;
    }
    .lv-rail {
        width: 104px;
        flex-shrink: 0;
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: 7px;
        padding-left: 12px;
        margin-left: 12px;
        border-left: 1px solid rgba(255, 255, 255, 0.18);
    }
    .lv-rail-meta {
        display: none;
    }
    .lv-net {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 13px;
        font-weight: 600;
        letter-spacing: 0.08em;
        color: rgba(255, 255, 255, 0.82);
        white-space: nowrap;
    }
    .lv-play {
        width: 30px;
        height: 30px;
        border-radius: 6px;
        background: rgba(0, 0, 0, 0.42);
        display: grid;
        place-items: center;
        color: #fff;
        flex-shrink: 0;
    }
    .lv-spin {
        width: 13px;
        height: 13px;
        border-radius: 999px;
        border: 2px solid rgba(255, 255, 255, 0.25);
        border-top-color: #fff;
        animation: lv-spin 0.7s linear infinite;
    }
    @keyframes lv-spin {
        to {
            transform: rotate(360deg);
        }
    }
    .lv-follow {
        position: absolute;
        z-index: 4;
        left: 8px;
        top: 6px;
        width: 20px;
        height: 20px;
        border: 0;
        border-radius: 4px;
        background: rgba(0, 0, 0, 0.35);
        color: rgba(255, 255, 255, 0.45);
        font-size: 11px;
        line-height: 1;
        cursor: pointer;
        opacity: 0;
        transition: opacity 0.12s ease;
    }
    .lv-bar:hover .lv-follow,
    .lv-follow.is-on {
        opacity: 1;
    }
    .lv-follow.is-on {
        color: #ffcf4a;
    }

    .lv-logo {
        width: 34px;
        height: 34px;
        object-fit: contain;
        flex-shrink: 0;
        padding: 3px;
        border-radius: 8px;
        background: rgba(6, 7, 9, 0.42);
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
        filter: drop-shadow(0 0 1.5px rgba(255, 255, 255, 0.55));
    }
    .lv-chip {
        width: 34px;
        height: 34px;
        border-radius: 7px;
        display: grid;
        place-items: center;
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 13px;
        font-weight: 700;
        letter-spacing: 0.04em;
        flex-shrink: 0;
        box-shadow:
            inset 0 0 0 1.5px rgba(255, 255, 255, 0.4),
            0 2px 10px rgba(0, 0, 0, 0.35);
    }

    .lv-roster {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        margin-bottom: 6px;
    }
    .lv-team {
        display: flex;
        align-items: center;
        gap: 11px;
        padding: 9px 12px 9px 10px;
        border-radius: 8px;
        background: color-mix(in srgb, var(--c) 22%, #0d0e12);
        box-shadow: inset 3px 0 0 var(--c);
        min-width: 210px;
    }
    .lv-team-id {
        flex: 1;
        min-width: 0;
    }
    .lv-team-name {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 18px;
        font-weight: 600;
        color: #fff;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .lv-team-sub {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 9px;
        color: rgba(255, 255, 255, 0.5);
        margin-top: 1px;
    }
    .lv-team-drop {
        display: grid;
        place-items: center;
        width: 22px;
        height: 22px;
        border: 0;
        border-radius: 5px;
        background: transparent;
        color: rgba(255, 255, 255, 0.35);
        cursor: pointer;
        flex-shrink: 0;
    }
    .lv-team-drop:hover {
        color: #fff;
        background: rgba(0, 0, 0, 0.3);
    }

    .lv-slot {
        display: flex;
        gap: 18px;
        margin-bottom: 16px;
    }
    .lv-slot-time {
        width: 74px;
        flex-shrink: 0;
        padding-top: 4px;
    }
    .lv-slot-hh {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 19px;
        font-weight: 700;
        letter-spacing: 0.05em;
        color: #ececef;
    }
    .lv-slot-rel {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        color: rgba(220, 220, 225, 0.3);
        margin-top: 2px;
    }
    .lv-slot-bars {
        flex: 1;
        min-width: 0;
    }

    @media (max-width: 1000px) {
        .lv-rail {
            width: auto;
            padding-left: 10px;
            margin-left: 10px;
        }
        .lv-net {
            display: none;
        }
        .lv-rec {
            display: none;
        }
    }

    @media (max-width: 760px) {
        .lv-bar {
            height: auto;
            border-radius: 9px;
        }
        .lv-field,
        .lv-shade,
        .lv-mid {
            display: none;
        }
        .lv-bar-in {
            flex-direction: column;
            align-items: stretch;
            padding: 0;
            gap: 0;
        }
        .lv-gap {
            display: none;
        }
        .lv-side {
            position: relative;
            height: 44px;
            padding: 0 12px;
            flex: none;
            overflow: hidden;
        }
        .lv-side::before {
            content: '';
            position: absolute;
            inset: 0;
            z-index: -1;
            clip-path: polygon(0 0, calc(74% + 8px) 0, calc(74% - 8px) 100%, 0 100%);
        }
        .lv-side-a::before {
            background: var(--ca);
        }
        .lv-side-b::before {
            background: var(--cb);
        }
        .lv-side-b {
            flex-direction: row;
            justify-content: flex-start;
            padding-left: 12px;
            color: var(--ib);
        }
        .lv-side-a {
            padding-right: 12px;
        }
        .is-solo .lv-side::before {
            clip-path: polygon(0 0, calc(100% - 14px) 0, calc(100% - 34px) 100%, 0 100%);
        }
        .is-solo .lv-side-a {
            padding-right: 40px;
        }
        .is-solo .lv-bar-in {
            flex-direction: row;
            flex-wrap: wrap;
        }
        .is-solo .lv-side {
            width: 100%;
        }
        .is-solo .lv-solo-meta {
            flex: 1;
        }
        .is-solo .lv-rail {
            margin-left: auto;
        }
        .lv-id-r {
            text-align: left;
        }
        .lv-side-b .lv-id {
            order: 3;
        }
        .lv-side-b .lv-chip,
        .lv-side-b .lv-logo {
            order: 1;
        }
        .lv-side-b .lv-star {
            order: 4;
        }
        .lv-side-b .lv-score {
            order: 9;
            margin-left: auto;
        }
        .lv-side-a .lv-score {
            margin-left: auto;
        }
        .lv-name {
            font-size: 19px;
        }
        .lv-score {
            font-size: 26px;
            min-width: 0;
            text-align: right;
            color: #f4f4f6;
            text-shadow: none;
        }
        .lv-filter {
            flex: 1;
            min-width: 0;
        }
        .lv-chips {
            flex: 1;
            min-width: 0;
            flex-wrap: nowrap;
            overflow-x: auto;
            scrollbar-width: none;
        }
        .lv-filter .lv-chips {
            flex: none;
        }
        .lv-chips::-webkit-scrollbar {
            display: none;
        }
        .lv-bar-meta > span {
            display: none;
        }
        .lv-logo,
        .lv-chip {
            width: 26px;
            height: 26px;
            font-size: 11px;
        }
        .lv-logo {
            padding: 2px;
            border-radius: 6px;
        }
        .lv-solo-meta {
            flex-direction: row;
            align-items: center;
            gap: 8px;
            padding: 7px 12px;
            background: #0d0e12;
        }
        .lv-rail {
            width: auto;
            justify-content: flex-start;
            gap: 8px;
            padding: 7px 12px;
            margin-left: 0;
            border-left: 0;
            background: #0d0e12;
        }
        .lv-rail-meta {
            display: flex;
            align-items: center;
            gap: 8px;
        }
        .lv-play {
            margin-left: auto;
            border-radius: 999px;
            background: var(--pw-accent);
            color: #08090b;
        }
        .lv-clock {
            font-size: 14px;
            color: #ececef;
            text-shadow: none;
        }
        .lv-follow {
            top: 12px;
        }
        .lv-slot {
            flex-direction: column;
            gap: 8px;
        }
        .lv-slot-time {
            width: auto;
            display: flex;
            align-items: baseline;
            gap: 8px;
        }
        .lv-slot-rel {
            margin-top: 0;
        }
        .lv-ticker-flag {
            font-size: 13px;
            padding-right: 20px;
        }
    }
</style>
