<script lang="ts">
    import Icon from '../Icon.svelte';

    type Option = { key: string; label: string; desc?: string; swatch?: string };
    type Props = {
        options: Option[];
        selected: string;
        onSelect: (k: string) => void;
        compact?: boolean;
    };
    let { options, selected, onSelect, compact = false }: Props = $props();
</script>

{#each options as opt}
    {#if compact}
        <button
            onclick={() => onSelect(opt.key)}
            class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-white/5 transition-colors"
        >
            <div class="flex items-center gap-2">
                {#if opt.swatch}<div
                        class="w-3 h-3 rounded-full border border-white/10"
                        style="background: {opt.swatch};"
                    ></div>{/if}
                <p class="text-white text-[12px] font-medium">{opt.label}</p>
            </div>
            {#if selected === opt.key}
                <Icon name="check" class="w-4 h-4 text-primary-400" />
            {/if}
        </button>
    {:else}
        <button
            onclick={() => onSelect(opt.key)}
            class="w-full flex items-center justify-between py-4 border-b border-white/5 text-left"
        >
            <div class="flex items-center gap-3">
                {#if opt.swatch}<div
                        class="w-4 h-4 rounded-full border border-white/10"
                        style="background: {opt.swatch};"
                    ></div>{/if}
                <div>
                    <p class="text-white text-[14px] font-medium">{opt.label}</p>
                    {#if opt.desc}<p class="text-gray-500 text-[12px]">{opt.desc}</p>{/if}
                </div>
            </div>
            {#if selected === opt.key}
                <Icon name="check" class="w-5 h-5 text-primary-400 flex-shrink-0" />
            {/if}
        </button>
    {/if}
{/each}
