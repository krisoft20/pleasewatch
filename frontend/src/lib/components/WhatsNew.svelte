<script lang="ts">
    import { onMount } from 'svelte';
    import { fade } from 'svelte/transition';
    import { api, type TvChannel } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import { category } from '$lib/category.svelte';

    const RELEASE = '2026-10b';
    const PICKS = ['Polsat', 'TVP Info', 'TV Puls', 'Polsat News', 'TV4', 'Puls 2'];

    let open = $state(false);
    let logos = $state<TvChannel[]>([]);

    onMount(() => {
        try {
            if (localStorage.getItem('pw-whatsnew') === RELEASE) return;
        } catch {
            return;
        }
        open = true;
        api.tvChannels()
            .then((r) => {
                logos = PICKS.map((n) => r.channels.find((c) => c.name === n && c.logo))
                    .filter((c): c is TvChannel => !!c)
                    .slice(0, 5);
            })
            .catch(() => {});
    });

    function close() {
        open = false;
        try {
            localStorage.setItem('pw-whatsnew', RELEASE);
        } catch {}
    }
</script>

<svelte:window onkeydown={(e) => open && e.key === 'Escape' && close()} />

{#if open}
    <div
        class="wn-bg"
        transition:fade={{ duration: 150 }}
        onclick={(e) => e.target === e.currentTarget && close()}
        role="presentation"
    >
        <div class="wn" role="dialog" aria-modal="true" aria-labelledby="wn-title">
            <div class="wn-head">
                <h2 id="wn-title">{t('whatsnew.title')}</h2>
                <span></span>
            </div>

            <section>
                <div class="wn-row">
                    <b>{t('whatsnew.tv.title')}</b>
                    <button
                        class="pw-chip"
                        onclick={() => {
                            category.set('tv');
                            close();
                        }}>{t('whatsnew.tv.cta')}</button
                    >
                </div>
                <p>{t('whatsnew.tv.body')}</p>
                {#if logos.length > 0}
                    <div class="wn-logos">
                        {#each logos as c (c.id)}
                            <span
                                ><img
                                    src={c.logo?.replace(/^http:/, 'https:')}
                                    alt={c.name}
                                    referrerpolicy="no-referrer"
                                /></span
                            >
                        {/each}
                    </div>
                {/if}
            </section>

            <section>
                <b>{t('whatsnew.subs.title')}</b>
                <p>{t('whatsnew.subs.body')}</p>
                <div class="wn-screen">
                    <div class="wn-sub2">I'll be there in an <i>hour</i>.</div>
                    <div class="wn-sub">Będę tam za <i>godzinę</i>.</div>
                </div>
            </section>

            <section>
                <b>{t('whatsnew.ext.title')}</b>
                <p>{t('whatsnew.ext.body')}</p>
                <div class="wn-torrent">
                    <span class="wn-pill">ext/tpb</span>
                    <span class="wn-name">Dune.Part.Two.2024.1080p.BluRay.x264</span>
                    <span class="wn-seeds">S:611</span>
                </div>
            </section>

            <button class="pw-btn" onclick={close}>{t('whatsnew.close')}</button>
        </div>
    </div>
{/if}

<style>
    .wn-bg {
        position: fixed;
        inset: 0;
        z-index: 300;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 16px;
        background: rgba(4, 5, 8, 0.7);
        backdrop-filter: blur(6px);
    }
    .wn {
        width: 100%;
        max-width: 500px;
        max-height: calc(100dvh - 32px);
        overflow-y: auto;
        padding: 18px 20px 20px;
        border-radius: 12px;
        background: #0f1014;
        border: 1px solid var(--pw-line);
    }
    .wn-head {
        display: flex;
        align-items: center;
        gap: 10px;
        margin-bottom: 4px;
    }
    .wn-head h2 {
        margin: 0;
        font-family: 'Barlow Condensed', 'Arial Narrow', system-ui, sans-serif;
        font-size: 22px;
        font-weight: 700;
        letter-spacing: 0.06em;
        text-transform: uppercase;
        color: #f4f4f6;
    }
    .wn-head span {
        flex: 1;
        height: 2px;
        background: linear-gradient(90deg, rgba(226, 64, 44, 0.75), rgba(255, 255, 255, 0.05));
    }
    section {
        padding: 14px 0;
        border-bottom: 1px solid var(--pw-line-soft);
    }
    section b {
        font-size: 14.5px;
        color: var(--pw-fg);
    }
    section p {
        margin: 3px 0 0;
        font-size: 13px;
        line-height: 1.45;
        color: var(--pw-fg-dim);
    }
    .wn-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
    }
    .wn-logos {
        display: grid;
        grid-template-columns: repeat(5, 1fr);
        gap: 6px;
        margin-top: 10px;
    }
    .wn-logos span {
        display: flex;
        align-items: center;
        justify-content: center;
        aspect-ratio: 16 / 9;
        border-radius: 6px;
        background: radial-gradient(120% 90% at 50% 0%, #2c2d34, #17181c);
    }
    .wn-logos img {
        max-width: 70%;
        max-height: 58%;
        object-fit: contain;
        filter: drop-shadow(0 0 0.6px rgba(255, 255, 255, 0.7));
    }
    .wn-screen {
        margin-top: 10px;
        padding: 26px 12px 12px;
        border-radius: 6px;
        background: linear-gradient(180deg, #1b1d24, #050506);
        text-align: center;
        font-family: 'Segoe UI', Tahoma, sans-serif;
        font-weight: 700;
        text-shadow: 0 1px 2px #000;
    }
    .wn-sub2 {
        font-size: 12.5px;
        color: rgba(255, 255, 255, 0.82);
    }
    .wn-sub {
        margin-top: 2px;
        font-size: 16px;
        color: #fff;
    }
    .wn-screen i {
        font-style: normal;
        border-radius: 0.15em;
        background-color: rgba(255, 196, 0, 0.42);
        box-shadow: 0 0 0 0.08em rgba(255, 196, 0, 0.42);
    }
    .wn-torrent {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-top: 10px;
        padding: 8px 10px;
        border-radius: 6px;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid var(--pw-line-soft);
        font-size: 12px;
    }
    .wn-pill {
        flex-shrink: 0;
        padding: 1px 7px;
        border-radius: 999px;
        font-size: 11px;
        color: oklch(0.82 0.14 300);
        background: color-mix(in oklch, oklch(0.65 0.16 300) 22%, transparent);
    }
    .wn-name {
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: #e9e9ee;
    }
    .wn-seeds {
        flex-shrink: 0;
        color: oklch(0.78 0.14 150);
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    }
    .pw-btn {
        margin-top: 16px;
    }
</style>
