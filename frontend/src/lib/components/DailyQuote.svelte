<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api, type DailyQuote } from '$lib/api';
    import { bookCoverSrc, retryBookCover, validateBookCover } from '$lib/bookCover';
    import { t } from '$lib/i18n.svelte';

    let quote = $state<DailyQuote | null>(null);
    let tried = $state(false);

    onMount(async () => {
        try {
            quote = await api.bookDaily();
        } catch {
        } finally {
            tried = true;
        }
    });

    function open() {
        if (!quote) return;
        goto(`/read-book/${quote.ol_key}?cfi=${encodeURIComponent(quote.cfi)}`);
    }
</script>

{#if tried && quote}
    <button class="pw-dq" onclick={open}>
        <span class="pw-dq-label">{t('books.quote.label')}</span>
        <blockquote class="pw-dq-text">{quote.snippet}</blockquote>
        <div class="pw-dq-foot">
            {#if quote.cover_url}
                <img
                    class="pw-dq-cover"
                    src={bookCoverSrc(quote.cover_url)}
                    alt=""
                    loading="lazy"
                    onload={(event) => validateBookCover(event, quote!.cover_url!)}
                    onerror={(event) => retryBookCover(event, quote!.cover_url!)}
                />
            {/if}
            <div class="pw-dq-meta">
                <span class="pw-dq-title">{quote.title}</span>
                {#if quote.authors}<span class="pw-dq-author">{quote.authors}</span>{/if}
            </div>
            <svg
                class="pw-dq-chev"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.2"
                stroke-linecap="round"
                stroke-linejoin="round"><polyline points="9 18 15 12 9 6" /></svg
            >
        </div>
    </button>
{/if}

<style>
    .pw-dq {
        position: relative;
        width: 100%;
        max-width: 760px;
        text-align: left;
        background: linear-gradient(
            155deg,
            color-mix(in oklch, var(--pw-accent) 6%, rgba(255, 255, 255, 0.02)) 0%,
            rgba(255, 255, 255, 0.02) 60%
        );
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-radius: 18px;
        padding: 16px 18px 14px;
        margin-bottom: 24px;
        cursor: pointer;
        transition:
            border-color 0.15s ease,
            background 0.15s ease;
        display: flex;
        flex-direction: column;
        gap: 10px;
    }
    .pw-dq:hover {
        border-color: rgba(255, 255, 255, 0.14);
        background: linear-gradient(
            155deg,
            color-mix(in oklch, var(--pw-accent) 10%, rgba(255, 255, 255, 0.03)) 0%,
            rgba(255, 255, 255, 0.03) 60%
        );
    }
    .pw-dq-label {
        font-size: 10.5px;
        font-weight: 600;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: color-mix(in oklch, var(--pw-accent) 55%, rgba(220, 220, 225, 0.5));
    }
    .pw-dq-text {
        font-family: 'Lora', Georgia, 'Iowan Old Style', 'Palatino Linotype', ui-serif, serif;
        font-style: italic;
        font-size: 17px;
        line-height: 1.5;
        color: #ececef;
        margin: 0;
        padding-left: 14px;
        border-left: 2px solid color-mix(in oklch, var(--pw-accent) 45%, transparent);
        display: -webkit-box;
        -webkit-line-clamp: 4;
        line-clamp: 4;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    @media (min-width: 640px) {
        .pw-dq-text {
            font-size: 18.5px;
        }
    }
    .pw-dq-text::before {
        content: '\201C';
        margin-right: 2px;
        opacity: 0.5;
    }
    .pw-dq-text::after {
        content: '\201D';
        margin-left: 2px;
        opacity: 0.5;
    }
    .pw-dq-foot {
        display: flex;
        align-items: center;
        gap: 10px;
        padding-left: 16px;
    }
    .pw-dq-cover {
        width: 22px;
        aspect-ratio: 2/3;
        border-radius: 3px;
        object-fit: cover;
        flex-shrink: 0;
    }
    .pw-dq-meta {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
    }
    .pw-dq-title {
        font-size: 12px;
        font-weight: 600;
        color: rgba(232, 232, 234, 0.85);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-dq-author {
        font-size: 11px;
        color: rgba(220, 220, 225, 0.45);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-dq-chev {
        color: rgba(220, 220, 225, 0.35);
        flex-shrink: 0;
    }
</style>
