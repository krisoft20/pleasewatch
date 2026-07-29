<script lang="ts">
    import Icon from '$lib/components/Icon.svelte';

    let {
        masked = [],
        full = [],
        kind,
        onRemove
    }: {
        masked: string[];
        full?: string[];
        kind: string;
        onRemove: (masked: string) => void | Promise<void>;
    } = $props();

    let reveal = $state<Record<number, boolean>>({});
    let copied = $state<number | null>(null);

    function shown(i: number, mask: string): string {
        return reveal[i] ? full[i] || mask : mask;
    }

    async function copyKey(key: string, i: number) {
        try {
            await navigator.clipboard.writeText(key);
            copied = i;
            setTimeout(() => {
                if (copied === i) copied = null;
            }, 1400);
        } catch {}
    }
</script>

{#if masked.length > 0}
    <div class="pw-key-list">
        {#each masked as mask, i}
            {@const raw = full[i] || mask}
            <div class="pw-key-row">
                <span class="pw-key-dot"></span>
                <code class="pw-key-code">{shown(i, mask)}</code>
                <button
                    class="pw-key-icon"
                    onclick={() => (reveal = { ...reveal, [i]: !reveal[i] })}
                    title={reveal[i] ? 'hide' : 'reveal'}
                    aria-label={reveal[i] ? `hide ${kind} key` : `reveal ${kind} key`}
                >
                    <Icon name={reveal[i] ? 'eye-off' : 'eye'} class="w-3.5 h-3.5" />
                </button>
                <button
                    class="pw-key-icon"
                    onclick={() => copyKey(raw, i)}
                    title="copy"
                    aria-label={`copy ${kind} key`}
                >
                    <Icon
                        name={copied === i ? 'check-alt' : 'copy'}
                        class="w-3.5 h-3.5"
                        strokeWidth={copied === i ? 2.4 : 2}
                    />
                </button>
                <button
                    class="pw-key-remove"
                    onclick={() => onRemove(mask)}
                    title="remove"
                    aria-label={`remove ${kind} key ${i + 1}`}
                >
                    x
                </button>
            </div>
        {/each}
    </div>
{/if}

<style>
    .pw-key-list {
        display: grid;
        gap: 6px;
        margin: 0 0 12px;
    }
    .pw-key-row {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 8px 10px;
        border: 1px solid rgba(75, 85, 99, 0.45);
        border-radius: 6px;
        background: rgba(17, 24, 39, 0.55);
    }
    .pw-key-dot {
        width: 6px;
        height: 6px;
        border-radius: 999px;
        background: #4ade80;
        flex: 0 0 auto;
    }
    .pw-key-code {
        flex: 1 1 auto;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        color: #e5e7eb;
        font-size: 12px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        user-select: all;
    }
    .pw-key-icon,
    .pw-key-remove {
        border: 0;
        background: transparent;
        color: #6b7280;
        cursor: pointer;
        border-radius: 4px;
    }
    .pw-key-icon {
        width: 24px;
        height: 24px;
        display: grid;
        place-items: center;
    }
    .pw-key-icon:hover {
        color: #93c5fd;
        background: rgba(255, 255, 255, 0.05);
    }
    .pw-key-remove {
        padding: 2px 6px;
        font-size: 12px;
    }
    .pw-key-remove:hover {
        color: #f87171;
        background: rgba(248, 113, 113, 0.08);
    }
</style>
