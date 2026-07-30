<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api } from '$lib/api';
    import Shell from '$lib/components/Shell.svelte';
    import Wordmark from '$lib/components/Wordmark.svelte';

    let userInput = $state<HTMLInputElement>();
    let u = $state('');
    let p = $state('');
    let busy = $state(false);
    let err = $state('');

    async function submit(e: SubmitEvent) {
        e.preventDefault();
        if (busy) return;
        err = '';
        busy = true;
        try {
            await api.login(u, p);
            goto('/', { replaceState: true });
        } catch (caught) {
            err = caught instanceof Error ? caught.message : 'login failed';
        } finally {
            busy = false;
        }
    }

    onMount(() => {
        userInput?.focus();
    });
</script>

<svelte:head><title>login - pleasewatch</title></svelte:head>

<div class="pw-shell">
    <Shell label="auth" />

    <div class="pw-stage">
        <Wordmark />

        <form class="pw-form" onsubmit={submit}>
            <div>
                <div class="pw-eyebrow"><span>›</span> sign in</div>
                <div class="pw-sub">a private library. enter to continue.</div>
            </div>

            <div>
                <label class="pw-label" for="u">username</label>
                <input
                    id="u"
                    class="pw-input"
                    type="text"
                    autocomplete="username"
                    bind:this={userInput}
                    spellcheck="false"
                    bind:value={u}
                />
            </div>

            <div>
                <label class="pw-label" for="p">password</label>
                <input id="p" class="pw-input" type="password" autocomplete="current-password" bind:value={p} />
            </div>

            {#if err}
                <div class="pw-error">{err}</div>
            {/if}

            <button type="submit" class="pw-btn" disabled={busy}>
                {busy ? 'signing in...' : 'sign in'}
            </button>

            <div class="pw-divider"></div>

            <div class="pw-foot">
                <span>no account? <a class="pw-link" href="/register">register</a></span>
            </div>
        </form>
    </div>
</div>
