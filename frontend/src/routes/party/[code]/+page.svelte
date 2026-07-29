<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { page } from '$app/state';
    import { goto } from '$app/navigation';
    import { api, type PartyInfo, type MediaSubtitle } from '$lib/api';
    import { joinSession, leaveSession, setRemoteCallbacks, watchTogether } from '$lib/stores/watchTogether';
    import Player from '$lib/components/Player.svelte';

    const code = $derived(page.params.code ?? '');

    let info = $state<PartyInfo | null>(null);
    let subs = $state<MediaSubtitle[]>([]);
    let name = $state('');
    let nameInput = $state<HTMLInputElement>();
    let joined = $state(false);
    let loading = $state(true);
    let err = $state('');

    onMount(async () => {
        try {
            info = await api.partyInfo(code);
        } catch {
            err = 'session not found or expired';
            loading = false;
            return;
        }
        try {
            const r = await fetch(`/api/party/${code}/subs`);
            if (r.ok) subs = await r.json();
        } catch {}
        const me = await api.me().catch(() => null);
        if (me?.username) {
            name = me.username;
            loading = false;
            join();
            return;
        }
        name = localStorage.getItem('party-name') || '';
        loading = false;
        await tick();
        nameInput?.focus();
    });

    function join() {
        const n = name.trim() || 'Guest';
        try {
            localStorage.setItem('party-name', n);
        } catch {}
        joinSession(code, n, false);
        joined = true;
        setRemoteCallbacks({
            onEpisodeSwitch: async (_ep) => {
                try {
                    info = await api.partyInfo(code);
                    const r = await fetch(`/api/party/${code}/subs`);
                    if (r.ok) subs = await r.json();
                } catch {}
            }
        });
    }

    let lastSavedPos = 0;
    let lastSavedAt = 0;
    function onProgress(position: number, duration: number) {
        if (!info || duration <= 0) return;
        const now = Date.now();
        const moved = Math.abs(position - lastSavedPos) > 5;
        const due = now - lastSavedAt > 10_000;
        if (!moved && !due) return;
        lastSavedPos = position;
        lastSavedAt = now;
        api.saveProgress({
            media_id: info.media_id,
            episode_id: info.episode_id,
            position: Math.floor(position),
            duration: Math.floor(duration)
        }).catch(() => {});
    }

    function back() {
        leaveSession();
        if (history.length > 1) history.back();
        else goto('/');
    }

    const playerLabel = $derived.by(() => {
        if (!info) return null;
        if (info.episode_season != null && info.episode_number != null) {
            return (
                `S${String(info.episode_season).padStart(2, '0')}E${String(info.episode_number).padStart(2, '0')}` +
                (info.episode_title ? ` - ${info.episode_title}` : '')
            );
        }
        return null;
    });

    const pageTitle = $derived.by(() => {
        if (!info) return 'party';
        if (info.episode_season != null && info.episode_number != null) {
            const head = `S${String(info.episode_season).padStart(2, '0')}E${String(info.episode_number).padStart(2, '0')}`;
            return `${head} - ${info.media_title} - party`;
        }
        return `${info.media_title} - party`;
    });
</script>

<svelte:head><title>{pageTitle} - pleasewatch</title></svelte:head>

{#if loading}
    <div class="fixed inset-0 z-[300] bg-black flex items-center justify-center">
        <div class="animate-spin rounded-full h-10 w-10 border-t-2 border-b-2 border-white"></div>
    </div>
{:else if err}
    <div class="fixed inset-0 z-[300] bg-black flex items-center justify-center text-white">
        <div class="text-center">
            <p class="text-sm text-gray-300 mb-4">{err}</p>
            <button class="px-4 py-2 rounded-lg bg-white/10 hover:bg-white/15 text-sm" onclick={back}>back</button>
        </div>
    </div>
{:else if !joined && info}
    <div class="pw-shell">
        <div class="pw-mesh"></div>
        <div class="pw-grain"></div>
        <div class="pw-vignette"></div>

        <div class="pw-stage">
            <div class="pw-pending">
                <div>
                    <div class="pw-eyebrow"><span>›</span> watch together</div>
                    <h2 class="pw-h2-lg" style="margin-top: 8px;">{info.media_title}</h2>
                    {#if playerLabel}
                        <p class="pw-sub">{playerLabel}</p>
                    {/if}
                </div>

                <dl>
                    <div class="pw-row"><span>code</span><span class="pw-mono">{info.code}</span></div>
                    <div class="pw-row"><span>watchers</span><span class="pw-mono">{info.participants}</span></div>
                </dl>

                <div>
                    <label class="pw-label" for="n">your name</label>
                    <input
                        id="n"
                        class="pw-input"
                        type="text"
                        placeholder="Guest"
                        bind:this={nameInput}
                        bind:value={name}
                        onkeydown={(e) => e.key === 'Enter' && join()}
                        spellcheck="false"
                    />
                </div>

                <button class="pw-btn" type="button" onclick={join}>join party</button>
            </div>
        </div>
    </div>
{:else if joined && info && !$watchTogether.synced}
    <div class="fixed inset-0 z-[300] bg-black flex items-center justify-center">
        <div class="animate-spin rounded-full h-10 w-10 border-t-2 border-b-2 border-white"></div>
    </div>
{:else if joined && info}
    {#key info.stream_id}
        <Player
            src={`/api/party/${code}/stream?ep=${info.stream_id}`}
            title={info.media_title}
            episodeLabel={playerLabel}
            mediaId={info.stream_id}
            showMediaId={info.media_id}
            posterUrl={info.poster_url ?? null}
            subtitles={subs}
            resumePosition={$watchTogether.syncedTime}
            subUrlBuilder={(sub) => `/api/party/${code}/subs/${sub.id}`}
            partyMode={true}
            onBack={back}
            {onProgress}
        />
    {/key}
{/if}
