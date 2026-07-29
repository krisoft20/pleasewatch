<script lang="ts">
    import Icon, { type IconName } from '../Icon.svelte';

    type Props = {
        icon: IconName;
        label: string;
        desc?: string;
        value: boolean;
        onChange: () => void;
        compact?: boolean;
    };
    let { icon, label, desc, value, onChange, compact = false }: Props = $props();
</script>

<div class="flex items-center justify-between {compact ? 'py-2' : 'py-3.5'}">
    <div class="flex items-center {compact ? 'gap-2' : 'gap-3'}">
        <Icon name={icon} class={compact ? 'w-3.5 h-3.5 text-gray-500' : 'w-5 h-5 text-gray-500'} />
        <div>
            <p class="text-white {compact ? 'text-[12px]' : 'text-[14px]'} font-medium">{label}</p>
            {#if desc && !compact}<p class="text-gray-500 text-[12px]">{desc}</p>{/if}
        </div>
    </div>
    <button
        onclick={onChange}
        aria-label={label}
        class="{compact
            ? 'w-8 h-[18px]'
            : 'w-11 h-6'} rounded-full transition-all duration-200 relative flex-shrink-0 {value
            ? 'bg-primary-500'
            : 'bg-white/15'}"
    >
        <div
            class="absolute top-0.5 rounded-full bg-white shadow-md transition-all duration-200 {compact
                ? 'w-[14px] h-[14px]'
                : 'w-5 h-5'} {value ? (compact ? 'left-[16px]' : 'left-[22px]') : 'left-0.5'}"
        ></div>
    </button>
</div>
