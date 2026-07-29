<script lang="ts">
    import { fly, fade } from 'svelte/transition';
    import { cubicOut } from 'svelte/easing';
    import { t } from '$lib/i18n';
    import Icon, { type IconName } from '../Icon.svelte';
    import NavRow from './NavRow.svelte';
    import SelectRow from './SelectRow.svelte';
    import SliderPage from './SliderPage.svelte';
    import ToggleRow from './ToggleRow.svelte';

    type FitMode = 'contain' | 'cover' | 'fill';
    type LayoutMode = 'auto' | 'mobile' | 'desktop';
    type Page =
        | 'main'
        | 'speed'
        | 'skip'
        | 'fit'
        | 'layout'
        | 'substyle'
        | 'sub-size'
        | 'sub-color'
        | 'sub-bg'
        | 'sub-sync'
        | 'sub-pos';

    const FIT_OPTIONS = [
        { key: 'contain', label: 'Fit', desc: 'Show entire video with bars' },
        { key: 'cover', label: 'Zoom', desc: 'Fill screen, may crop edges' },
        { key: 'fill', label: 'Stretch', desc: 'Stretch to fill, may distort' }
    ];
    const LAYOUT_OPTIONS = [
        { key: 'auto', label: 'Auto', desc: 'Detect from screen size' },
        { key: 'mobile', label: 'Mobile', desc: 'Touch controls, bottom sheet panels' },
        { key: 'desktop', label: 'Desktop', desc: 'Mouse controls, dropdown panels' }
    ];
    const SKIP_STEPS = [1, 2, 3, 4, 5, 10, 15, 20];
    const SPEED_PRESETS = [
        { value: 0.5, label: '0.5' },
        { value: 0.75, label: '0.75' },
        { value: 1, label: '1.0', sublabel: 'Normal' },
        { value: 1.25, label: '1.25' },
        { value: 1.5, label: '1.5' },
        { value: 2, label: '2' }
    ];
    const BG_COLORS = [
        { key: 'black', color: '#000' },
        { key: 'white', color: '#fff' },
        { key: 'yellow', color: '#b49600' },
        { key: 'cyan', color: '#009696' },
        { key: 'green', color: '#007800' }
    ];
    const BG_OPACITY = [
        { key: '50', label: '50%' },
        { key: '75', label: '75%' },
        { key: '100', label: '100%' }
    ];

    let {
        isMobile,
        isIPhone,
        partyActive,
        unreadCount,
        currentSpeed,
        skipDuration = $bindable(5),
        autoNextEpisode = $bindable(true),
        autoSkipIntro = $bindable(false),
        useNativeIOSFs = $bindable(false),
        layoutMode,
        videoFit,
        subSize = $bindable('medium'),
        subColor = $bindable('white'),
        subBg = $bindable('transparent'),
        subBgColor = $bindable('black'),
        subBgOpacity = $bindable('75'),
        subBgJoin = $bindable(false),
        subSync = $bindable(0),
        subYOffset = $bindable(0),
        onClose,
        onClip,
        onOpenChat,
        onStartParty,
        onLeaveParty,
        onSetSpeed,
        onSetVideoFit,
        onSetLayoutMode
    }: {
        isMobile: boolean;
        isIPhone: boolean;
        partyActive: boolean;
        unreadCount: number;
        currentSpeed: number;
        skipDuration: number;
        autoNextEpisode: boolean;
        autoSkipIntro: boolean;
        useNativeIOSFs: boolean;
        layoutMode: LayoutMode;
        videoFit: FitMode;
        subSize: string;
        subColor: string;
        subBg: string;
        subBgColor: string;
        subBgOpacity: string;
        subBgJoin: boolean;
        subSync: number;
        subYOffset: number;
        onClose: () => void;
        onClip: () => void;
        onOpenChat: () => void;
        onStartParty: () => void | Promise<void>;
        onLeaveParty: () => void;
        onSetSpeed: (speed: number) => void;
        onSetVideoFit: (fit: FitMode) => void;
        onSetLayoutMode: (mode: LayoutMode) => void;
    } = $props();

    let settingsPage = $state<Page>('main');

    const settingsTitle = $derived.by(() => {
        switch (settingsPage) {
            case 'speed':
                return $t('player.playback_speed');
            case 'skip':
                return $t('player.skip_duration');
            case 'fit':
                return $t('player.video_fit');
            case 'layout':
                return $t('player.player_layout');
            case 'substyle':
                return $t('player.subtitles');
            case 'sub-size':
                return $t('player.subtitle_size');
            case 'sub-color':
                return $t('player.font_color');
            case 'sub-bg':
                return $t('player.background');
            case 'sub-sync':
                return $t('player.sync_offset');
            case 'sub-pos':
                return $t('player.position');
            default:
                return $t('player.settings');
        }
    });

    const fitLabel = $derived(videoFit === 'contain' ? 'Fit' : videoFit === 'cover' ? 'Zoom' : 'Stretch');

    function save(key: string, value: string | number | boolean) {
        try {
            localStorage.setItem(key, String(value));
        } catch {}
    }

    function closeAfter(fn: () => void | Promise<void>) {
        void fn();
        onClose();
    }

    function back() {
        settingsPage = settingsPage.startsWith('sub-') ? 'substyle' : 'main';
    }

    function setSkip(value: number) {
        skipDuration = value;
        save('player-skip-duration', value);
    }

    function stepSkip(delta: -1 | 1) {
        const i = SKIP_STEPS.indexOf(skipDuration);
        const next = SKIP_STEPS[i + delta];
        if (next) setSkip(next);
    }

    function setSubBgValue(value: string) {
        subBg = value;
        save('player-sub-bg', value);
    }

    function setSubBgColor(value: string) {
        subBgColor = value;
        save('player-sub-bg-color', value);
    }

    function setSubBgOpacity(value: string) {
        subBgOpacity = value;
        save('player-sub-bg-opacity', value);
    }

    function actionClass(compact: boolean, border = false) {
        if (compact) {
            return 'w-full flex items-center gap-2 px-3 py-2.5 hover:bg-white/5 rounded-lg transition-colors';
        }
        return `w-full flex items-center gap-3 py-3.5 ${border ? 'border-b border-white/5' : ''}`;
    }

    function iconClass(compact: boolean, tone = 'text-gray-500') {
        return `${compact ? 'w-3.5 h-3.5' : 'w-5 h-5'} ${tone}`;
    }

    function rowText(compact: boolean, tone = 'text-white') {
        return `${tone} ${compact ? 'text-[12px]' : 'text-[14px]'} font-medium`;
    }

    function toggleAutoNext() {
        autoNextEpisode = !autoNextEpisode;
        save('player-auto-next', autoNextEpisode);
    }

    function toggleAutoSkip() {
        autoSkipIntro = !autoSkipIntro;
        save('player-auto-skip-intro', autoSkipIntro);
    }

    function toggleNativeFs() {
        useNativeIOSFs = !useNativeIOSFs;
        save('player-ios-native-fs', useNativeIOSFs);
    }

    function toggleJoinLines() {
        subBgJoin = !subBgJoin;
        save('player-sub-bg-join', subBgJoin);
    }
</script>

{#snippet actionRows(compact = false)}
    <button onclick={() => closeAfter(onClip)} class={actionClass(compact, true)}>
        <Icon name="close-bold" class={iconClass(compact)} />
        <p class={rowText(compact)}>{$t('player.create_clip')}</p>
    </button>
    {#if partyActive}
        <button
            onclick={() => closeAfter(onOpenChat)}
            class={compact
                ? 'w-full flex items-center justify-between px-3 py-2.5 hover:bg-white/5 rounded-lg transition-colors'
                : 'w-full flex items-center justify-between py-3.5 border-b border-white/5'}
        >
            <div class={compact ? 'flex items-center gap-2' : 'flex items-center gap-3'}>
                <Icon name="chat-alt" class={iconClass(compact, 'text-green-400')} />
                <p class={rowText(compact, 'text-green-400')}>{$t('player.chat')}</p>
            </div>
            {#if unreadCount > 0}
                <span
                    class={compact
                        ? 'bg-red-500 text-white text-[8px] px-1.5 rounded-full'
                        : 'bg-red-500 text-white text-[10px] px-2 py-0.5 rounded-full'}>{unreadCount}</span
                >
            {/if}
        </button>
        <button onclick={() => closeAfter(onLeaveParty)} class={actionClass(compact)}>
            <Icon name="logout-right" class={iconClass(compact, 'text-red-400')} />
            <p class={rowText(compact, 'text-red-400')}>{$t('player.leave_party')}</p>
        </button>
    {:else}
        <button onclick={() => closeAfter(onStartParty)} class={actionClass(compact)}>
            <Icon name="users" class={iconClass(compact)} />
            <p class={rowText(compact)}>{$t('player.watch_together')}</p>
        </button>
    {/if}
{/snippet}

{#snippet settingsRows(compact = false)}
    <NavRow
        icon="bolt"
        label={$t('player.playback_speed')}
        value={currentSpeed === 1 ? 'Normal' : `${currentSpeed}x`}
        onClick={() => {
            settingsPage = 'speed';
        }}
        {compact}
    />
    <NavRow
        icon="chevron-right-sm"
        label={$t('player.skip_duration')}
        value={`${skipDuration}s`}
        onClick={() => {
            settingsPage = 'skip';
        }}
        {compact}
    />
    <NavRow
        icon="drag-handle"
        label={$t('player.video_fit')}
        value={fitLabel}
        onClick={() => {
            settingsPage = 'fit';
        }}
        {compact}
    />
    <NavRow
        icon="sliders"
        label={compact ? $t('player.layout') : $t('player.player_layout')}
        value={layoutMode}
        onClick={() => {
            settingsPage = 'layout';
        }}
        {compact}
    />
    <NavRow
        icon="subs"
        label={$t('player.subtitles')}
        value={`${subSize}, ${subColor}`}
        onClick={() => {
            settingsPage = 'substyle';
        }}
        {compact}
    />
{/snippet}

{#snippet toggleRows(compact = false)}
    <ToggleRow
        icon="next-track"
        label={$t('player.auto_next_episode')}
        desc={$t('player.auto_next_episode_desc')}
        value={autoNextEpisode}
        onChange={toggleAutoNext}
        {compact}
    />
    <ToggleRow
        icon="skip-fast"
        label={$t('player.auto_skip_intro')}
        desc={$t('player.auto_skip_intro_desc')}
        value={autoSkipIntro}
        onChange={toggleAutoSkip}
        {compact}
    />
{/snippet}

{#snippet mainPage(compact = false)}
    {#if compact}
        {@render actionRows(true)}
        <div class="h-px bg-white/5 mx-2 my-1"></div>
        {@render settingsRows(true)}
        <div class="h-px bg-white/5 mx-2 my-1"></div>
        <div class="px-3">
            {@render toggleRows(true)}
        </div>
    {:else}
        <div class="bg-white/5 rounded-2xl px-4 mb-2">
            {@render actionRows()}
        </div>
        <div class="bg-white/5 rounded-2xl px-4 mb-2">
            {@render settingsRows()}
        </div>
        <div class="bg-white/5 rounded-2xl px-4">
            <div class="border-b border-white/5">
                <ToggleRow
                    icon="next-track"
                    label={$t('player.auto_next_episode')}
                    desc={$t('player.auto_next_episode_desc')}
                    value={autoNextEpisode}
                    onChange={toggleAutoNext}
                />
            </div>
            <ToggleRow
                icon="skip-fast"
                label={$t('player.auto_skip_intro')}
                desc={$t('player.auto_skip_intro_desc')}
                value={autoSkipIntro}
                onChange={toggleAutoSkip}
            />
            {#if isIPhone}
                <div class="flex items-center justify-between py-3.5">
                    <div class="flex items-center gap-3">
                        <Icon name="screen-text" class="w-5 h-5 text-gray-500" />
                        <div>
                            <p class="text-white text-[14px] font-medium">native ios fullscreen</p>
                            <p class="text-gray-500 text-[12px]">
                                use safari's player when fullscreening, no chat overlay
                            </p>
                        </div>
                    </div>
                    <button
                        onclick={toggleNativeFs}
                        aria-label="toggle native ios fullscreen"
                        class="w-11 h-6 rounded-full transition-all duration-200 relative flex-shrink-0 {useNativeIOSFs
                            ? 'bg-primary-500'
                            : 'bg-white/15'}"
                    >
                        <div
                            class="absolute top-0.5 w-5 h-5 rounded-full bg-white shadow-md transition-all duration-200 {useNativeIOSFs
                                ? 'left-[22px]'
                                : 'left-0.5'}"
                        ></div>
                    </button>
                </div>
            {/if}
        </div>
    {/if}
{/snippet}

{#snippet speedPage(compact = false)}
    <SliderPage
        value={currentSpeed}
        min={0.25}
        max={3}
        step={0.25}
        format={(v) => `${v.toFixed(2)}x`}
        onChange={(v) => onSetSpeed(Math.round(v * 100) / 100)}
        presets={compact ? SPEED_PRESETS.map(({ sublabel, ...p }) => p) : SPEED_PRESETS}
        {compact}
    />
{/snippet}

{#snippet skipPage(compact = false)}
    <div class={compact ? 'flex flex-col items-center py-4 px-3' : 'flex flex-col items-center py-6'}>
        <div class={compact ? 'flex items-center gap-4 mb-4' : 'flex items-center gap-6 mb-6'}>
            <button
                onclick={() => stepSkip(-1)}
                class={compact
                    ? 'w-7 h-7 rounded-full bg-white/10 flex items-center justify-center text-white hover:bg-white/20'
                    : 'w-9 h-9 rounded-full bg-white/10 flex items-center justify-center text-white text-lg font-bold active:bg-white/20'}
            >
                <Icon name="chevron-left" class={compact ? 'w-4 h-4' : 'w-5 h-5'} />
            </button>
            <p
                class={compact
                    ? 'text-white text-2xl font-bold w-12 text-center'
                    : 'text-white text-4xl font-bold w-16 text-center'}
            >
                {skipDuration}s
            </p>
            <button
                onclick={() => stepSkip(1)}
                class={compact
                    ? 'w-7 h-7 rounded-full bg-white/10 flex items-center justify-center text-white hover:bg-white/20'
                    : 'w-9 h-9 rounded-full bg-white/10 flex items-center justify-center text-white text-lg font-bold active:bg-white/20'}
            >
                <Icon name="chevron-right" class={compact ? 'w-4 h-4' : 'w-5 h-5'} />
            </button>
        </div>
        <div class={compact ? 'flex gap-1.5 w-full' : 'flex gap-2 w-full'}>
            {#each SKIP_STEPS as dur}
                <button
                    onclick={() => setSkip(dur)}
                    class="{compact
                        ? 'flex-1 py-1.5 rounded-lg text-[11px]'
                        : 'flex-1 py-2.5 rounded-xl text-[13px]'} font-medium transition-all {skipDuration === dur
                        ? 'bg-white text-black'
                        : compact
                          ? 'bg-white/10 text-gray-400 hover:bg-white/15'
                          : 'bg-white/10 text-gray-300 active:bg-white/20'}"
                >
                    {dur}s
                </button>
            {/each}
        </div>
    </div>
{/snippet}

{#snippet subStylePage(compact = false)}
    <NavRow
        icon={compact ? undefined : 'text-size'}
        label={$t('player.size')}
        value={subSize}
        onClick={() => {
            settingsPage = 'sub-size';
        }}
        {compact}
    />
    <NavRow
        icon={compact ? undefined : 'color-bucket'}
        label={$t('player.font_color')}
        value={subColor}
        onClick={() => {
            settingsPage = 'sub-color';
        }}
        {compact}
    />
    <NavRow
        icon={compact ? undefined : 'square'}
        label={$t('player.background')}
        value={subBg === 'transparent' ? 'None' : subBg}
        onClick={() => {
            settingsPage = 'sub-bg';
        }}
        {compact}
    />
    <NavRow
        icon={compact ? undefined : 'clock'}
        label={$t('player.sync_offset')}
        value={`${subSync > 0 ? '+' : ''}${subSync.toFixed(1)}s`}
        onClick={() => {
            settingsPage = 'sub-sync';
        }}
        {compact}
    />
    <NavRow
        icon={compact ? undefined : 'arrows-vertical'}
        label={$t('player.position')}
        value={`${subYOffset}px`}
        onClick={() => {
            settingsPage = 'sub-pos';
        }}
        {compact}
    />
{/snippet}

{#snippet bgControls(compact = false)}
    {#if subBg === 'box'}
        <div class={compact ? 'px-4 py-2' : 'py-3 border-b border-white/5'}>
            <p
                class={compact
                    ? 'text-[10px] text-gray-500 font-medium uppercase tracking-wider mb-1.5'
                    : 'text-[11px] text-gray-500 font-medium uppercase tracking-wider mb-2'}
            >
                {$t('player.box_color')}
            </p>
            <div class={compact ? 'flex gap-1' : 'flex gap-1.5'}>
                {#each BG_COLORS as opt}
                    <button
                        onclick={() => setSubBgColor(opt.key)}
                        class="{compact
                            ? 'flex-1 h-7 rounded-md'
                            : 'flex-1 h-9 rounded-lg'} border-2 transition-all {subBgColor === opt.key
                            ? 'border-primary-400'
                            : 'border-white/10'}"
                        style="background: {opt.color};"
                        aria-label={opt.key}
                    ></button>
                {/each}
            </div>
        </div>
        <div class={compact ? 'px-4 py-2' : 'py-3 border-b border-white/5'}>
            <p
                class={compact
                    ? 'text-[10px] text-gray-500 font-medium uppercase tracking-wider mb-1.5'
                    : 'text-[11px] text-gray-500 font-medium uppercase tracking-wider mb-2'}
            >
                {$t('player.opacity')}
            </p>
            <div class={compact ? 'flex gap-1' : 'flex gap-1.5'}>
                {#each BG_OPACITY as opt}
                    <button
                        onclick={() => setSubBgOpacity(opt.key)}
                        class="{compact
                            ? 'flex-1 py-1.5 rounded-md text-[11px]'
                            : 'flex-1 py-2 rounded-lg text-[13px]'} font-medium transition-all {subBgOpacity === opt.key
                            ? 'bg-white text-black'
                            : compact
                              ? 'bg-white/5 text-gray-400 hover:bg-white/10'
                              : 'bg-white/5 text-gray-400'}">{opt.label}</button
                    >
                {/each}
            </div>
        </div>
    {/if}
    <button
        onclick={toggleJoinLines}
        class={compact
            ? 'w-full flex items-center justify-between px-4 py-2.5 hover:bg-white/5 transition-colors'
            : 'w-full flex items-center justify-between py-4 text-left'}
    >
        <div class={compact ? 'text-left' : ''}>
            <p class={compact ? 'text-white text-[12px] font-medium' : 'text-white text-[14px] font-medium'}>
                {$t('player.join_lines')}
            </p>
            <p class={compact ? 'text-gray-500 text-[10px] leading-tight mt-0.5' : 'text-gray-500 text-[11px] mt-0.5'}>
                {$t('player.join_lines_desc')}
            </p>
        </div>
        <div
            class="{compact ? 'w-9 h-5' : 'w-11 h-6'} rounded-full transition-colors {subBgJoin
                ? 'bg-primary-500'
                : 'bg-white/10'} relative flex-shrink-0"
        >
            <div
                class="absolute top-0.5 rounded-full bg-white transition-all {compact
                    ? 'w-4 h-4'
                    : 'w-5 h-5'} {subBgJoin ? (compact ? 'left-[18px]' : 'left-[22px]') : 'left-0.5'}"
            ></div>
        </div>
    </button>
{/snippet}

{#snippet pageBody(compact = false)}
    {#if settingsPage === 'main'}
        {@render mainPage(compact)}
    {:else if settingsPage === 'speed'}
        {@render speedPage(compact)}
    {:else if settingsPage === 'skip'}
        {@render skipPage(compact)}
    {:else if settingsPage === 'fit'}
        <SelectRow options={FIT_OPTIONS} selected={videoFit} onSelect={(k) => onSetVideoFit(k as FitMode)} {compact} />
    {:else if settingsPage === 'layout'}
        <SelectRow
            options={LAYOUT_OPTIONS}
            selected={layoutMode}
            onSelect={(k) => onSetLayoutMode(k as LayoutMode)}
            {compact}
        />
    {:else if settingsPage === 'substyle'}
        {@render subStylePage(compact)}
    {:else if settingsPage === 'sub-size'}
        <SelectRow
            options={[
                { key: 'small', label: $t('player.small') },
                { key: 'medium', label: $t('player.medium') },
                { key: 'large', label: $t('player.large') }
            ]}
            selected={subSize}
            onSelect={(k) => {
                subSize = k;
                save('player-sub-size', k);
            }}
            {compact}
        />
    {:else if settingsPage === 'sub-color'}
        <SelectRow
            options={[
                { key: 'white', label: $t('player.white'), swatch: '#fff' },
                { key: 'yellow', label: $t('player.yellow'), swatch: '#ffd700' },
                { key: 'cyan', label: $t('player.cyan'), swatch: compact ? '#0ff' : '#00ffff' },
                { key: 'green', label: $t('player.green'), swatch: compact ? '#0f0' : '#00ff00' }
            ]}
            selected={subColor}
            onSelect={(k) => {
                subColor = k;
                save('player-sub-color', k);
            }}
            {compact}
        />
    {:else if settingsPage === 'sub-bg'}
        <SelectRow
            options={[
                { key: 'transparent', label: $t('player.none') },
                { key: 'shadow', label: $t('player.shadow') },
                { key: 'box', label: $t('player.box') }
            ]}
            selected={subBg}
            onSelect={setSubBgValue}
            {compact}
        />
        {@render bgControls(compact)}
    {:else if settingsPage === 'sub-sync'}
        <SliderPage
            value={subSync}
            min={-10}
            max={10}
            step={0.5}
            format={(v) => `${v > 0 ? '+' : ''}${v.toFixed(1)}s`}
            onChange={(v) => {
                subSync = v;
                save('player-sub-sync', v);
            }}
            onReset={() => {
                subSync = 0;
                save('player-sub-sync', '0');
            }}
            resetLabel={compact ? 'Reset' : $t('player.reset_to_zero')}
            {compact}
        />
    {:else if settingsPage === 'sub-pos'}
        <SliderPage
            value={subYOffset}
            min={-100}
            max={100}
            step={10}
            format={(v) => `${v}px`}
            onChange={(v) => {
                subYOffset = v;
                save('player-sub-y', v);
            }}
            onReset={() => {
                subYOffset = 0;
                save('player-sub-y', '0');
            }}
            resetLabel={compact ? 'Reset' : $t('player.reset_to_zero')}
            {compact}
        />
    {/if}
{/snippet}

{#if isMobile}
    <div class="fixed inset-0 z-50">
        <button
            type="button"
            aria-label="close settings"
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
            {#if settingsPage !== 'main'}
                <div class="flex items-center gap-3 px-5 pt-2 pb-3">
                    <button onclick={back} class="text-white">
                        <Icon name="chevron-left" class="w-5 h-5" />
                    </button>
                    <h3 class="text-white text-lg font-bold">{settingsTitle}</h3>
                </div>
            {/if}
            <div class="overflow-y-auto scrollbar-hide pb-10 px-4" style="max-height: calc(70vh - 80px);">
                {@render pageBody()}
            </div>
        </div>
    </div>
{:else}
    <button
        type="button"
        aria-label="close settings"
        class="absolute inset-0 z-30 bg-transparent border-0 p-0"
        onclick={onClose}
    ></button>
    <div in:fly={{ y: 8, duration: 200, easing: cubicOut }} class="absolute z-40 bottom-24 right-10 w-72">
        <div
            class="bg-black/60 backdrop-blur-2xl rounded-2xl shadow-[0_8px_32px_rgba(0,0,0,0.6)] ring-1 ring-white/10 overflow-hidden max-h-[55vh] overflow-y-auto scrollbar-hide"
        >
            {#if settingsPage !== 'main'}
                <div class="bg-white/5 flex items-center gap-2 px-3 pt-3 pb-2.5">
                    <button onclick={back} class="text-white hover:text-gray-300 transition-colors">
                        <Icon name="chevron-left" class="w-4 h-4" />
                    </button>
                    <h3 class="text-white text-[13px] font-semibold">{settingsTitle}</h3>
                </div>
            {:else}
                <div class="bg-white/5 flex items-center justify-between px-4 pt-3 pb-2.5">
                    <div class="flex items-center gap-2">
                        <Icon name="gear" class="w-4 h-4 text-gray-400" />
                        <h3 class="text-white text-[13px] font-semibold">Settings</h3>
                    </div>
                    <button onclick={onClose} class="text-gray-500 hover:text-white transition-colors">
                        <Icon name="close" class="w-4 h-4" />
                    </button>
                </div>
            {/if}
            <div class="px-1 py-1">
                {@render pageBody(true)}
            </div>
        </div>
    </div>
{/if}
