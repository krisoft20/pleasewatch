<script lang="ts">
    type Props = {
        value: number;
        min: number;
        max: number;
        step: number;
        format: (v: number) => string;
        onChange: (v: number) => void;
        onReset?: () => void;
        resetLabel?: string;
        presets?: { value: number; label?: string; sublabel?: string }[];
        compact?: boolean;
    };
    let { value, min, max, step, format, onChange, onReset, resetLabel, presets, compact = false }: Props = $props();

    function clamp(v: number): number {
        return Math.max(min, Math.min(max, v));
    }
</script>

<div class="flex flex-col items-center {compact ? 'py-4' : 'py-6'}">
    <p class="text-white {compact ? 'text-2xl mb-4' : 'text-4xl mb-6'} font-bold">{format(value)}</p>
    <div class="flex items-center {compact ? 'gap-3 px-3' : 'gap-4 px-2'} w-full mb-4">
        <button
            onclick={() => onChange(clamp(value - step))}
            class="{compact
                ? 'w-7 h-7 text-sm'
                : 'w-9 h-9 text-lg'} rounded-full bg-white/10 flex items-center justify-center text-white font-bold active:bg-white/20 hover:bg-white/20"
            >-</button
        >
        <input
            type="range"
            {min}
            {max}
            {step}
            {value}
            oninput={(e) => onChange(Number((e.target as HTMLInputElement).value))}
            class="flex-1 h-1 bg-white/20 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:shadow-lg"
        />
        <button
            onclick={() => onChange(clamp(value + step))}
            class="{compact
                ? 'w-7 h-7 text-sm'
                : 'w-9 h-9 text-lg'} rounded-full bg-white/10 flex items-center justify-center text-white font-bold active:bg-white/20 hover:bg-white/20"
            >+</button
        >
    </div>
    {#if presets && presets.length > 0}
        <div class="flex {compact ? 'gap-1.5' : 'gap-2'} w-full">
            {#each presets as p}
                <button
                    onclick={() => onChange(p.value)}
                    class="flex-1 {compact ? 'py-1.5 text-[11px]' : 'py-2.5 text-[13px]'} rounded-{compact
                        ? 'lg'
                        : 'xl'} font-medium transition-all {value === p.value
                        ? 'bg-white text-black'
                        : 'bg-white/10 text-gray-300 hover:bg-white/15 active:bg-white/20'}"
                >
                    {p.label ?? p.value}
                    {#if p.sublabel}<br /><span
                            class="text-[10px] {value === p.value ? 'text-gray-600' : 'text-gray-500'}"
                            >{p.sublabel}</span
                        >{/if}
                </button>
            {/each}
        </div>
    {/if}
    {#if onReset && resetLabel}
        <button
            onclick={onReset}
            class="text-gray-400 {compact ? 'text-[11px]' : 'text-[13px]'} active:text-white hover:text-white mt-2"
        >
            {resetLabel}
        </button>
    {/if}
</div>
