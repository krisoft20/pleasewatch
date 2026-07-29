<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api } from '$lib/api';
    import Shell from '$lib/components/Shell.svelte';
    import Wordmark from '$lib/components/Wordmark.svelte';

    let userInput = $state<HTMLInputElement>();
    let u = $state('');
    let e = $state('');
    let p = $state('');
    let busy = $state(false);
    let err = $state('');
    let ok = $state<{ username: string; email: string } | null>(null);

    const queued = $derived(new Date().toISOString().slice(0, 16).replace('T', ' '));

    async function submit(ev: SubmitEvent) {
        ev.preventDefault();
        if (busy) return;
        err = '';
        busy = true;
        try {
            const r = await api.register(u, e, p);
            if (r.role === 'admin') {
                goto('/login');
                return;
            }
            ok = { username: u, email: e };
        } catch (caught) {
            err = caught instanceof Error ? caught.message : 'register failed';
        } finally {
            busy = false;
        }
    }

    onMount(() => {
        userInput?.focus();
    });
</script>

<svelte:head><title>register - pleasewatch</title></svelte:head>

<div class="pw-shell">
    <Shell label={ok ? 'pending' : 'register'} />

    <div class="pw-stage">
        <Wordmark />

        {#if ok}
            <div class="pw-pending">
                <div class="pw-eyebrow"><span>›</span> request submitted</div>
                <h2>waiting for the owner<br />to let you in.</h2>
                <dl>
                    <div class="pw-row"><span>user</span><span>{ok.username}</span></div>
                    <div class="pw-row"><span>email</span><span>{ok.email}</span></div>
                    <div class="pw-row is-hot">
                        <span>status</span><span>pending</span>
                    </div>
                    <div class="pw-row"><span>queued</span><span>{queued}</span></div>
                </dl>
                <p>wait for admin to approve. no email gets sent.</p>
                <a class="pw-link pw-back" href="/login">← back to sign in</a>
            </div>
        {:else}
            <form class="pw-form" onsubmit={submit}>
                <div>
                    <div class="pw-eyebrow"><span>›</span> request access</div>
                    <div class="pw-sub">a private library. by invitation.</div>
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
                    <label class="pw-label" for="e">email</label>
                    <input id="e" class="pw-input" type="email" autocomplete="email" bind:value={e} />
                </div>

                <div>
                    <label class="pw-label" for="p">password</label>
                    <input id="p" class="pw-input" type="password" autocomplete="new-password" bind:value={p} />
                </div>

                {#if err}
                    <div class="pw-error">{err}</div>
                {/if}

                <button type="submit" class="pw-btn" disabled={busy}>
                    {busy ? 'creating...' : 'create account'}
                </button>

                <div class="pw-divider"></div>

                <div class="pw-foot">
                    <span>have an account? <a class="pw-link" href="/login">sign in</a></span>
                </div>
            </form>
        {/if}
    </div>
</div>
