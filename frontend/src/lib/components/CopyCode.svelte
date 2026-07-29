<script lang="ts">
    let { code, multiline = false }: { code: string; multiline?: boolean } = $props();
    let copied = $state(false);

    async function copy() {
        try {
            await navigator.clipboard.writeText(code);
            copied = true;
            setTimeout(() => (copied = false), 1400);
        } catch {}
    }
</script>

{#if multiline}
    <div class="pw-cc-block">
        <pre class="pw-cc-pre">{code}</pre>
        <button class="pw-cc-btn" onclick={copy} aria-label="copy">
            {#if copied}
                <svg
                    width="13"
                    height="13"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                >
            {:else}
                <svg
                    width="13"
                    height="13"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    ><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path
                        d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
                    /></svg
                >
            {/if}
        </button>
    </div>
{:else}
    <span class="pw-cc-inline">
        <code>{code}</code>
        <button class="pw-cc-btn-inline" onclick={copy} aria-label="copy">
            {#if copied}
                <svg
                    width="11"
                    height="11"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                >
            {:else}
                <svg
                    width="11"
                    height="11"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    ><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path
                        d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
                    /></svg
                >
            {/if}
        </button>
    </span>
{/if}

<style>
    .pw-cc-block {
        position: relative;
        margin: 8px 0 0;
    }
    .pw-cc-pre {
        margin: 0;
        padding: 12px 44px 12px 14px;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 12.5px;
        color: #ececef;
        overflow-x: auto;
        white-space: pre-wrap;
        word-break: break-all;
    }
    .pw-cc-btn {
        position: absolute;
        top: 8px;
        right: 8px;
        width: 28px;
        height: 28px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: rgba(220, 220, 225, 0.65);
        border-radius: 5px;
        display: grid;
        place-items: center;
        cursor: pointer;
        transition:
            background 0.15s ease,
            color 0.15s ease;
    }
    .pw-cc-btn:hover {
        background: rgba(255, 255, 255, 0.1);
        color: #ececef;
    }
    .pw-cc-inline {
        display: inline-flex;
        align-items: center;
        gap: 4px;
        white-space: nowrap;
    }
    .pw-cc-inline code {
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 4px;
        padding: 1px 6px;
        font-size: 12px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        color: rgba(232, 232, 234, 0.9);
    }
    .pw-cc-btn-inline {
        background: transparent;
        border: none;
        color: rgba(220, 220, 225, 0.4);
        padding: 2px;
        display: inline-grid;
        place-items: center;
        cursor: pointer;
        border-radius: 3px;
        transition:
            color 0.15s ease,
            background 0.15s ease;
    }
    .pw-cc-btn-inline:hover {
        color: #ececef;
        background: rgba(255, 255, 255, 0.06);
    }
</style>
