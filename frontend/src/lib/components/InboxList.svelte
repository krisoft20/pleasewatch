<script lang="ts">
    import { inbox, sharePath } from '$lib/inbox.svelte';
    import { bookCoverSrc } from '$lib/bookCover';
    import { t } from '$lib/i18n.svelte';

    let { onpick }: { onpick: () => void } = $props();

    function ago(ts: string) {
        const s = (Date.now() - new Date(ts.replace(' ', 'T') + 'Z').getTime()) / 1000;
        if (s < 60) return t('inbox.now');
        if (s < 3600) return `${Math.floor(s / 60)}m`;
        if (s < 86400) return `${Math.floor(s / 3600)}h`;
        return `${Math.floor(s / 86400)}d`;
    }
</script>

<div class="pw-inbox-head">{t('inbox.title')}</div>
{#if inbox.items.length === 0}
    <p class="pw-inbox-empty">{t('inbox.empty')}</p>
{:else}
    <div class="pw-inbox-list">
        {#each inbox.items as s (s.id)}
            <a class="pw-inbox-row" class:is-new={inbox.fresh.has(s.id)} href={sharePath(s.kind, s.ref_key)} onclick={onpick}>
                {#if s.cover_url}
                    <img src={bookCoverSrc(s.cover_url)} alt="" loading="lazy" />
                {:else}
                    <span class="pw-inbox-nocover"></span>
                {/if}
                <span class="pw-inbox-txt">
                    <span class="pw-inbox-title">{s.title}</span>
                    {#if s.note}<span class="pw-inbox-note">“{s.note}”</span>{/if}
                    <span class="pw-inbox-from">{t('inbox.from', { name: s.from_name })} · {ago(s.created_at)}</span>
                </span>
            </a>
        {/each}
    </div>
{/if}

<style>
    .pw-inbox-head {
        padding: 12px 14px 8px;
        font-size: 13px;
        font-weight: 600;
        color: #ececef;
    }
    .pw-inbox-empty {
        margin: 0;
        padding: 6px 14px 16px;
        font-size: 12.5px;
        color: rgba(220, 220, 225, 0.5);
    }
    .pw-inbox-list {
        max-height: min(60vh, 420px);
        overflow-y: auto;
        padding: 0 4px 4px;
    }
    .pw-inbox-row {
        display: flex;
        align-items: center;
        gap: 11px;
        padding: 7px 10px;
        border-radius: 8px;
        text-decoration: none;
    }
    .pw-inbox-row:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .pw-inbox-row img,
    .pw-inbox-nocover {
        width: 32px;
        height: 48px;
        flex-shrink: 0;
        border-radius: 4px;
        object-fit: cover;
        background: rgba(255, 255, 255, 0.06);
    }
    .pw-inbox-txt {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
    }
    .pw-inbox-title {
        font-size: 13px;
        color: rgba(232, 232, 234, 0.82);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-inbox-row.is-new .pw-inbox-title {
        color: #ececef;
        font-weight: 600;
    }
    .pw-inbox-note {
        font-size: 12.5px;
        color: rgba(232, 232, 234, 0.75);
        display: -webkit-box;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .pw-inbox-from {
        font-size: 11.5px;
        color: rgba(220, 220, 225, 0.5);
    }
    .pw-inbox-row.is-new .pw-inbox-from {
        color: var(--pw-accent);
    }
</style>
