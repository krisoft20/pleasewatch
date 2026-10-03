<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type TvChannel } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import Player from './Player.svelte';

    const COUNTRY: Record<string, string> = { pl: 'Polska', uk: 'UK', us: 'USA' };

    let channels = $state<TvChannel[]>([]);
    let pending = $state<string[]>([]);
    let loading = $state(true);
    let fetchErr = $state<string | null>(null);
    let country = $state('pl');
    let group = $state('all');
    let q = $state('');
    let opening = $state<string | null>(null);
    let openErr = $state<string | null>(null);
    let stream = $state<{ url: string; ch: TvChannel } | null>(null);
    let retries = 0;
    let recovering = false;

    onMount(() => {
        load();
        const poll = setInterval(() => {
            if (pending.length > 0 && !stream) load(true);
        }, 10000);
        return () => clearInterval(poll);
    });

    async function load(quiet = false) {
        if (!quiet) loading = true;
        fetchErr = null;
        try {
            const r = await api.tvChannels();
            channels = r.channels;
            pending = r.pending;
            if (!channels.some((c) => c.country === country) && channels.length > 0 && !pending.includes(country)) {
                country = channels[0].country;
            }
        } catch (e) {
            if (!quiet) fetchErr = String((e as Error)?.message ?? e);
        } finally {
            loading = false;
        }
    }

    async function watch(ch: TvChannel) {
        if (opening) return;
        opening = ch.id;
        openErr = null;
        retries = 0;
        try {
            const r = await api.tvPlay(ch.id);
            stream = { url: r.master_url, ch };
        } catch {
            openErr = t('tv.error', { name: ch.name });
        } finally {
            opening = null;
        }
    }

    async function onStreamError() {
        const cur = stream;
        if (!cur || recovering) return;
        recovering = true;
        try {
            if (retries >= 3) {
                stream = null;
                openErr = t('tv.error', { name: cur.ch.name });
                return;
            }
            retries++;
            await new Promise((r) => setTimeout(r, 4000));
            if (stream !== cur) return;
            const r = await api.tvPlay(cur.ch.id);
            stream = { url: r.master_url + '?r=' + Date.now(), ch: cur.ch };
        } catch {
            stream = null;
            openErr = t('tv.error', { name: cur.ch.name });
        } finally {
            recovering = false;
        }
    }

    const countries = $derived.by(() => {
        const n = new Map<string, number>();
        for (const c of channels) n.set(c.country, (n.get(c.country) ?? 0) + 1);
        return [...n.entries()];
    });
    const inCountry = $derived(channels.filter((c) => c.country === country));
    const groups = $derived.by(() => {
        const n = new Map<string, number>();
        for (const c of inCountry) for (const g of splitGroups(c)) n.set(g, (n.get(g) ?? 0) + 1);
        return [...n.entries()].sort((a, b) => b[1] - a[1]);
    });
    const shown = $derived.by(() => {
        const needle = q.trim().toLowerCase();
        return inCountry.filter(
            (c) =>
                (group === 'all' || splitGroups(c).includes(group)) &&
                (!needle || c.name.toLowerCase().includes(needle))
        );
    });
    const sections = $derived.by(() => {
        const by = new Map<string, TvChannel[]>();
        for (const c of shown) {
            const g = group === 'all' ? splitGroups(c)[0] : group;
            if (!by.has(g)) by.set(g, []);
            by.get(g)!.push(c);
        }
        const other = t('tv.other');
        return [...by.entries()].sort((a, b) => +(a[0] === other) - +(b[0] === other) || b[1].length - a[1].length);
    });

    function splitGroups(c: TvChannel): string[] {
        const g = (c.group ?? '')
            .split(';')
            .map((s) => s.trim())
            .filter(Boolean);
        return g.length ? g : [t('tv.other')];
    }

    function pickCountry(c: string) {
        country = c;
        group = 'all';
    }
</script>

{#snippet sectionHead(name: string, n: number)}
    <div class="lv-head">
        <span class="lv-head-name">{name}</span>
        <span class="lv-head-n">{n}</span>
        <span class="lv-head-rule"></span>
    </div>
{/snippet}

<div class="pw-filter-bar">
    <div class="pw-filter-bar-inner">
        <div class="tv-filter">
            <div class="tv-row">
                <div class="tv-chips">
                    {#each countries as [c, n] (c)}
                        <button
                            class="pw-chip tv-country"
                            class:is-active={country === c}
                            onclick={() => pickCountry(c)}
                        >
                            {COUNTRY[c] ?? c.toUpperCase()}<span class="pw-chip-n">{n}</span>
                        </button>
                    {/each}
                    {#each pending as c (c)}
                        <span class="pw-chip tv-country is-pending"
                            >{COUNTRY[c] ?? c.toUpperCase()}<span class="tv-spin"></span></span
                        >
                    {/each}
                </div>
                <input class="pw-input tv-search" type="search" placeholder={t('tv.search')} bind:value={q} />
            </div>
            {#if groups.length > 1}
                <div class="tv-chips">
                    <button class="pw-chip" class:is-active={group === 'all'} onclick={() => (group = 'all')}>
                        {t('live.filter.all')}<span class="pw-chip-n">{inCountry.length}</span>
                    </button>
                    {#each groups as [g, n] (g)}
                        <button class="pw-chip" class:is-active={group === g} onclick={() => (group = g)}>
                            {g}<span class="pw-chip-n">{n}</span>
                        </button>
                    {/each}
                </div>
            {/if}
        </div>
    </div>
</div>

{#if openErr}
    <section class="pw-section pw-section-tight"><p class="tv-note is-err">{openErr}</p></section>
{/if}

{#if loading && channels.length === 0}
    <section class="pw-section pw-section-tight"><p class="tv-note">{t('tv.loading')}</p></section>
{:else if fetchErr}
    <section class="pw-section pw-section-tight"><p class="tv-note is-err">{fetchErr}</p></section>
{:else if inCountry.length === 0}
    <section class="pw-section pw-section-tight">
        <p class="tv-note">{pending.includes(country) || channels.length === 0 ? t('tv.pending') : t('tv.empty')}</p>
    </section>
{:else}
    <section class="pw-section pw-section-tight">
        {#if shown.length === 0}
            <p class="tv-note">{t('tv.empty')}</p>
        {/if}
        {#each sections as [name, list] (name)}
            <div class="tv-group">
                {@render sectionHead(name, list.length)}
                <div class="tv-grid">
                    {#each list as ch (ch.id)}
                        <button class="tv-tile" disabled={opening !== null} onclick={() => watch(ch)}>
                            <span class="tv-plate">
                                {#if ch.logo}
                                    <img
                                        src={ch.logo.replace(/^http:/, 'https:')}
                                        alt=""
                                        loading="lazy"
                                        referrerpolicy="no-referrer"
                                    />
                                {:else}
                                    <span class="tv-mono">{ch.name.slice(0, 3)}</span>
                                {/if}
                                {#if opening === ch.id}<span class="tv-opening"><span class="tv-spin"></span></span
                                    >{/if}
                            </span>
                            <span class="tv-name">{ch.name}</span>
                        </button>
                    {/each}
                </div>
            </div>
        {/each}
    </section>
{/if}

{#if stream}
    <Player
        src={stream.url}
        title={stream.ch.name}
        onBack={() => (stream = null)}
        {onStreamError}
        live
        sourceName="iptv-org"
    />
{/if}

<style>
    .tv-filter {
        display: flex;
        flex-direction: column;
        gap: 7px;
        min-width: 0;
        width: 100%;
    }
    .tv-row {
        display: flex;
        gap: 10px;
        align-items: center;
        flex-wrap: wrap;
    }
    .tv-chips {
        display: flex;
        gap: 6px;
        flex-wrap: wrap;
        align-items: center;
        min-width: 0;
    }
    .tv-country {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-weight: 700;
        letter-spacing: 0.05em;
        text-transform: uppercase;
    }
    .tv-country.is-pending {
        opacity: 0.55;
        gap: 6px;
    }
    .tv-search {
        margin-left: auto;
        width: 220px;
        max-width: 100%;
    }
    @media (max-width: 640px) {
        .tv-search {
            margin-left: 0;
            width: 100%;
        }
    }

    .tv-note {
        margin: 8px 0 14px;
        font-size: 13px;
        color: var(--pw-fg-dim);
    }
    .tv-note.is-err {
        color: oklch(0.8 0.13 28);
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
    .lv-head-n {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        color: var(--pw-fg-dim);
    }
    .lv-head-rule {
        flex: 1;
        height: 2px;
        background: linear-gradient(90deg, rgba(226, 64, 44, 0.75), rgba(255, 255, 255, 0.05));
    }

    .tv-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
        gap: 10px;
    }
    @media (max-width: 480px) {
        .tv-grid {
            grid-template-columns: repeat(3, minmax(0, 1fr));
            gap: 8px;
        }
    }
    .tv-tile {
        display: flex;
        flex-direction: column;
        gap: 6px;
        padding: 0;
        border: 0;
        background: none;
        cursor: pointer;
        text-align: left;
        min-width: 0;
    }
    .tv-tile:disabled {
        cursor: progress;
    }
    .tv-plate {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
        aspect-ratio: 16 / 9;
        border-radius: 8px;
        background: radial-gradient(120% 90% at 50% 0%, #2c2d34, #17181c);
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
        overflow: hidden;
        outline: 2px solid transparent;
        outline-offset: 2px;
        transition:
            outline-color 0.15s,
            transform 0.15s;
    }
    .tv-tile:hover .tv-plate,
    .tv-tile:focus-visible .tv-plate {
        outline-color: rgba(226, 64, 44, 0.85);
        transform: translateY(-2px);
    }
    .tv-plate img {
        max-width: 70%;
        max-height: 58%;
        object-fit: contain;
        filter: drop-shadow(0 0 0.6px rgba(255, 255, 255, 0.7)) drop-shadow(0 0 5px rgba(255, 255, 255, 0.12));
    }
    .tv-mono {
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 26px;
        font-weight: 700;
        text-transform: uppercase;
        color: #c9cad2;
    }
    .tv-opening {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(10, 11, 14, 0.55);
    }
    .tv-name {
        font-size: 12.5px;
        color: #e9e9ee;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .tv-spin {
        width: 12px;
        height: 12px;
        border-radius: 50%;
        border: 2px solid rgba(255, 255, 255, 0.25);
        border-top-color: #fff;
        animation: tv-spin 0.8s linear infinite;
        display: inline-block;
    }
    @keyframes tv-spin {
        to {
            transform: rotate(360deg);
        }
    }
</style>
