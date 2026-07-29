<script lang="ts">
    import Icon, { type IconName } from '../Icon.svelte';

    type Props = {
        icon?: IconName;
        label: string;
        value?: string;
        onClick: () => void;
        compact?: boolean;
    };
    let { icon, label, value, onClick, compact = false }: Props = $props();
</script>

<button
    onclick={onClick}
    class={compact
        ? 'w-full flex items-center justify-between px-3 py-2.5 hover:bg-white/5 rounded-lg transition-colors'
        : 'w-full flex items-center justify-between py-3.5 border-b border-white/5'}
>
    <div class="flex items-center {compact ? 'gap-2' : 'gap-3'}">
        {#if icon}
            <Icon name={icon} class={compact ? 'w-3.5 h-3.5 text-gray-500' : 'w-5 h-5 text-gray-500'} />
        {/if}
        <p class="text-white {compact ? 'text-[12px]' : 'text-[14px]'} font-medium">{label}</p>
    </div>
    <div class="flex items-center {compact ? 'gap-1' : 'gap-1.5'}">
        {#if value}<span class="text-gray-400 {compact ? 'text-[10px]' : 'text-[13px]'} capitalize">{value}</span>{/if}
        <Icon name="chevron-right" class={compact ? 'w-3.5 h-3.5 text-gray-500' : 'w-4 h-4 text-gray-500'} />
    </div>
</button>
