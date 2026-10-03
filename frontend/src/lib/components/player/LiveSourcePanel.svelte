<script lang="ts">
    import { fly, fade } from 'svelte/transition';
    import { cubicOut } from 'svelte/easing';
    import { t } from '$lib/i18n';
    import Icon from '../Icon.svelte';
    import SelectRow from './SelectRow.svelte';

    type Level = { i: number; label: string; meta: string };
    type Props = {
        isMobile?: boolean;
        sourceName: string;
        sourceMirrors?: boolean;
        levels: Level[];
        activeLevel: number;
        onPickLevel: (i: number) => void;
        onReconnect: () => void;
        onClose: () => void;
    };
    let {
        isMobile = false,
        sourceName,
        sourceMirrors = false,
        levels,
        activeLevel,
        onPickLevel,
        onReconnect,
        onClose
    }: Props = $props();

    const sourceOptions = $derived([
        {
            key: 'active',
            label: sourceName,
            desc: sourceMirrors ? $t('live.source.mirrors') : $t('live.source.only')
        }
    ]);

    const qualityOptions = $derived([
        { key: 'auto', label: $t('live.quality.auto'), desc: $t('live.quality.auto.hint') },
        ...levels.map((l) => ({ key: String(l.i), label: l.label, desc: l.meta }))
    ]);

    function pick(k: string) {
        onPickLevel(k === 'auto' ? -1 : Number(k));
    }
</script>

{#snippet label(text: string, compact: boolean)}
    <p
        class={compact
            ? 'text-[10px] text-gray-500 font-medium uppercase tracking-wider mb-1.5 px-4 pt-3'
            : 'text-[11px] text-gray-500 font-medium uppercase tracking-wider mb-2 pt-4'}
    >
        {text}
    </p>
{/snippet}

{#snippet body(compact: boolean)}
    {@render label($t('live.source'), compact)}
    <SelectRow options={sourceOptions} selected="active" onSelect={() => {}} {compact} />
    <button
        onclick={() => {
            onClose();
            onReconnect();
        }}
        class={compact
            ? 'w-full flex items-center justify-between px-4 py-2.5 hover:bg-white/5 transition-colors'
            : 'w-full flex items-center justify-between py-4 border-b border-white/5 text-left'}
    >
        <div class="flex items-center gap-2">
            <Icon name="sync" class={compact ? 'w-4 h-4 text-primary-400' : 'w-5 h-5 text-primary-400'} />
            <p class={compact ? 'text-primary-400 text-[12px] font-medium' : 'text-primary-400 text-[15px]'}>
                {$t('live.reconnect')}
            </p>
        </div>
        <p class={compact ? 'text-gray-500 text-[10px]' : 'text-gray-500 text-[11px]'}>
            {$t('live.source.fresh')}
        </p>
    </button>

    {@render label($t('live.quality'), compact)}
    {#if levels.length < 2}
        <p class={compact ? 'text-gray-500 text-[11px] px-4 pb-3' : 'text-gray-500 text-[12px] pb-3'}>
            {$t('live.quality.none')}
        </p>
    {:else}
        <SelectRow
            options={qualityOptions}
            selected={activeLevel === -1 ? 'auto' : String(activeLevel)}
            onSelect={pick}
            {compact}
        />
    {/if}
{/snippet}

{#if isMobile}
    <div class="fixed inset-0 z-50">
        <button
            type="button"
            aria-label="close source picker"
            class="absolute inset-0 bg-black/60 border-0 p-0"
            transition:fade={{ duration: 200 }}
            onclick={onClose}
        ></button>
        <div
            class="absolute bottom-0 left-0 right-0 bg-black/70 backdrop-blur-2xl rounded-t-3xl shadow-[0_-10px_40px_rgba(0,0,0,0.5)] max-h-[70vh] overflow-hidden"
            in:fly={{ y: 500, duration: 400, easing: cubicOut }}
            out:fly={{ y: 500, duration: 300 }}
        >
            <div class="flex justify-center pt-3 pb-1">
                <div class="w-10 h-1.5 bg-gray-600 rounded-full"></div>
            </div>
            <div class="overflow-y-auto scrollbar-hide pb-10 px-4" style="max-height: calc(70vh - 80px);">
                {@render body(false)}
            </div>
        </div>
    </div>
{:else}
    <button
        type="button"
        aria-label="close source picker"
        class="absolute inset-0 z-30 bg-transparent border-0 p-0"
        onclick={onClose}
    ></button>
    <div in:fly={{ y: 8, duration: 200, easing: cubicOut }} class="absolute z-40 bottom-24 right-10 w-72">
        <div
            class="bg-black/60 backdrop-blur-2xl rounded-2xl shadow-[0_8px_32px_rgba(0,0,0,0.6)] ring-1 ring-white/10 overflow-hidden max-h-[55vh] overflow-y-auto scrollbar-hide"
        >
            <div class="bg-white/5 flex items-center justify-between px-4 pt-3 pb-2.5">
                <div class="flex items-center gap-2">
                    <Icon name="cloud" class="w-4 h-4 text-gray-400" />
                    <h3 class="text-white text-[13px] font-semibold">{$t('live.source')}</h3>
                </div>
                <button onclick={onClose} class="text-gray-500 hover:text-white transition-colors">
                    <Icon name="close" class="w-4 h-4" />
                </button>
            </div>
            <div class="pb-1">
                {@render body(true)}
            </div>
        </div>
    </div>
{/if}
