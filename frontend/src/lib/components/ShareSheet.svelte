<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import { api, type ShareKind } from '$lib/api';
    import { sharePath } from '$lib/inbox.svelte';
    import { t } from '$lib/i18n.svelte';
    import { clickOutside } from '$lib/dismiss';

    let {
        kind,
        key,
        title,
        cover = null,
        onclose
    }: { kind: ShareKind; key: string; title: string; cover?: string | null; onclose: () => void } = $props();

    const UNDO_MS = 5000;

    let users = $state<{ id: string; username: string }[]>([]);
    let loaded = $state(false);
    let sent = $state<Record<string, 'wait' | 'busy' | 'ok' | 'err'>>({});
    let copied = $state(false);
    let note = $state('');
    let pop = $state<HTMLDivElement>();
    const timers = new Map<string, ReturnType<typeof setTimeout>>();
    const notes = new Map<string, string>();
    let opener: HTMLElement | null = null;

    onMount(async () => {
        opener = document.activeElement as HTMLElement | null;
        pop?.querySelector('button')?.focus();
        try {
            users = await api.shareUsers();
        } catch {
            users = [];
        }
        loaded = true;
    });

    onDestroy(() => {
        for (const [id, tm] of timers) {
            clearTimeout(tm);
            deliver(id);
        }
        opener?.focus();
    });

    function portal(node: HTMLElement) {
        document.body.appendChild(node);
        return { destroy: () => node.remove() };
    }

    function trap(ev: KeyboardEvent) {
        if (ev.key !== 'Tab' || !pop) return;
        const btns = [...pop.querySelectorAll<HTMLElement>('button:not(:disabled), input')];
        if (btns.length === 0) return;
        const first = btns[0];
        const last = btns[btns.length - 1];
        if (ev.shiftKey && document.activeElement === first) {
            ev.preventDefault();
            last.focus();
        } else if (!ev.shiftKey && document.activeElement === last) {
            ev.preventDefault();
            first.focus();
        }
    }

    async function copy() {
        const url = location.origin + sharePath(kind, key);
        try {
            await navigator.clipboard.writeText(url);
            copied = true;
            setTimeout(() => (copied = false), 1800);
        } catch {
            window.prompt(t('share.copy'), url);
        }
    }

    function send(id: string) {
        if (sent[id] === 'wait') {
            clearTimeout(timers.get(id));
            timers.delete(id);
            delete sent[id];
            return;
        }
        if (sent[id] === 'busy' || sent[id] === 'ok') return;
        sent[id] = 'wait';
        notes.set(id, note.trim());
        timers.set(id, setTimeout(() => deliver(id), UNDO_MS));
    }

    async function deliver(id: string) {
        timers.delete(id);
        sent[id] = 'busy';
        try {
            await api.shareSend({ to: id, kind, key, title, cover_url: cover, note: notes.get(id) || undefined });
            sent[id] = 'ok';
        } catch {
            sent[id] = 'err';
        }
    }
</script>

<div class="pw-share-scrim" use:portal>
    <div
        class="pw-share-pop"
        role="dialog"
        aria-modal="true"
        aria-labelledby="pw-share-h"
        tabindex="-1"
        bind:this={pop}
        onkeydown={trap}
        use:clickOutside={onclose}
    >
        <div class="pw-share-head">
            <span id="pw-share-h">{t('share.title')}</span>
            <span class="pw-share-what">{title}</span>
        </div>

        <button class="pw-share-row" onclick={copy}>
            <svg
                width="15"
                height="15"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                ><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path
                    d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
                /></svg
            >
            <span class="pw-share-name" aria-live="polite">{copied ? t('share.copied') : t('share.copy')}</span>
        </button>

        <input
            class="pw-share-note"
            bind:value={note}
            maxlength="200"
            placeholder={t('share.note')}
            aria-label={t('share.note')}
        />

        <div class="pw-share-label">{t('share.send_to')}</div>
        <div class="pw-share-list" aria-live="polite">
            {#if !loaded}
                <p class="pw-share-dim">{t('common.loading')}</p>
            {:else if users.length === 0}
                <p class="pw-share-dim">{t('share.nobody')}</p>
            {/if}
            {#each users as u (u.id)}
                <button class="pw-share-row" disabled={sent[u.id] === 'busy'} onclick={() => send(u.id)}>
                    <span class="pw-share-ava" aria-hidden="true">{u.username.charAt(0).toUpperCase()}</span>
                    <span class="pw-share-name">{u.username}</span>
                    {#if sent[u.id] === 'wait'}
                        <span class="pw-share-state">{t('share.sent')} · <b>{t('share.undo')}</b></span>
                    {:else if sent[u.id] === 'ok'}
                        <span class="pw-share-state is-ok">{t('share.sent')}</span>
                    {:else if sent[u.id] === 'err'}
                        <span class="pw-share-state is-err">{t('share.failed')}</span>
                    {:else if sent[u.id] === 'busy'}
                        <span class="pw-share-state">...</span>
                    {/if}
                </button>
            {/each}
        </div>
    </div>
</div>

<style>
    .pw-share-scrim {
        position: fixed;
        inset: 0;
        z-index: 120;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 16px;
    }
    .pw-share-pop {
        width: 100%;
        max-width: 340px;
        max-height: min(72vh, 520px);
        display: flex;
        flex-direction: column;
        background: rgba(12, 13, 17, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        padding-bottom: 8px;
        overflow: hidden;
    }
    .pw-share-head {
        display: flex;
        flex-direction: column;
        gap: 2px;
        padding: 14px 16px 10px;
        font-size: 14px;
        font-weight: 600;
        color: #ececef;
    }
    .pw-share-what {
        font-size: 12px;
        font-weight: 400;
        color: rgba(220, 220, 225, 0.5);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-share-note {
        margin: 6px 16px 0;
        padding: 9px 12px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 10px;
        color: #ececef;
        font-size: 13.5px;
        outline: none;
    }
    .pw-share-note::placeholder {
        color: rgba(220, 220, 225, 0.4);
    }
    .pw-share-note:focus {
        border-color: color-mix(in oklch, var(--pw-accent) 55%, transparent);
    }
    .pw-share-label {
        padding: 12px 16px 4px;
        font-size: 11.5px;
        color: rgba(220, 220, 225, 0.45);
    }
    .pw-share-list {
        overflow-y: auto;
        min-height: 48px;
    }
    .pw-share-dim {
        margin: 0;
        padding: 8px 16px;
        font-size: 12.5px;
        color: rgba(220, 220, 225, 0.5);
    }
    .pw-share-row {
        display: flex;
        align-items: center;
        gap: 11px;
        width: calc(100% - 16px);
        margin: 0 8px;
        padding: 10px 10px;
        border-radius: 10px;
        background: none;
        border: 0;
        color: rgba(220, 220, 225, 0.8);
        font-size: 14px;
        text-align: left;
        cursor: pointer;
    }
    .pw-share-row:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .pw-share-row:focus-visible {
        outline: 2px solid var(--pw-accent);
        outline-offset: -2px;
    }
    .pw-share-pop:focus {
        outline: none;
    }
    .pw-share-state b {
        font-weight: 600;
        color: var(--pw-accent);
    }
    .pw-share-ava {
        width: 26px;
        height: 26px;
        flex-shrink: 0;
        border-radius: 999px;
        display: grid;
        place-items: center;
        font-size: 11px;
        font-weight: 600;
        color: var(--pw-accent);
        background: color-mix(in oklch, var(--pw-accent) 22%, #1a1a1f);
    }
    .pw-share-name {
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .pw-share-state {
        font-size: 12px;
        color: rgba(220, 220, 225, 0.5);
    }
    .pw-share-state.is-ok {
        color: var(--pw-accent);
    }
    .pw-share-state.is-err {
        color: #f87171;
    }
    @media (max-width: 640px) {
        .pw-share-scrim {
            align-items: flex-end;
            padding: 0;
        }
        .pw-share-pop {
            max-width: none;
            max-height: 80vh;
            border-radius: 16px 16px 0 0;
            border-bottom: 0;
            padding-bottom: calc(10px + env(safe-area-inset-bottom));
            animation: pw-share-up 0.22s cubic-bezier(0.2, 0.7, 0.2, 1);
        }
        .pw-share-note {
            font-size: 16px;
        }
        .pw-share-row {
            padding: 12px 10px;
        }
    }
    @keyframes pw-share-up {
        from {
            transform: translateY(40px);
            opacity: 0;
        }
    }
</style>
