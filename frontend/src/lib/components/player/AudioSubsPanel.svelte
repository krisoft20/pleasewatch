<script lang="ts">
    import { fly, fade } from 'svelte/transition';
    import { cubicOut } from 'svelte/easing';
    import type { MediaSubtitle, SubSearchResult } from '$lib/types';
    import { t } from '$lib/i18n';
    import Icon from '../Icon.svelte';
    import LangPicker from './LangPicker.svelte';

    type AudioTrack = { id: string; label: string; language: string; codec?: string };
    type SyncMethod = 'ffsubsync' | 'alass' | 'whisper';
    type LangOption = { code: string; name: string };

    const SUB_LANG_OPTIONS: LangOption[] = [
        { code: 'en', name: 'English' },
        { code: 'pl', name: 'Polski' },
        { code: 'de', name: 'Deutsch' },
        { code: 'ar', name: 'Arabic' },
        { code: 'es', name: 'Spanish' },
        { code: 'fr', name: 'French' },
        { code: 'ja', name: 'Japanese' },
        { code: 'ko', name: 'Korean' },
        { code: 'pt', name: 'Portuguese' },
        { code: 'ru', name: 'Russian' },
        { code: 'zh', name: 'Chinese' },
        { code: 'tr', name: 'Turkish' },
        { code: 'it', name: 'Italiano' },
        { code: 'nl', name: 'Nederlands' },
        { code: 'sv', name: 'Svenska' }
    ];

    let {
        isMobile,
        audioTracks,
        selectedAudioIndex,
        selectedSubIndex,
        subtitles,
        partyMode,
        syncErr,
        syncing,
        syncMethod,
        subSearchMode = $bindable(false),
        subSearchLang = $bindable('en'),
        subSearchResults = $bindable([]),
        subSearching,
        subSearched = $bindable(false),
        subDownloading,
        subUploadFile = $bindable(null),
        subUploadLabel = $bindable(''),
        subUploadLang = $bindable('en'),
        subUploading,
        aiTranslating,
        aiTranslateErr,
        onClose,
        onSelectAudio,
        onSelectSubtitle,
        onRunSync,
        onDeleteSubtitle,
        onSearch,
        onDownload,
        onAiTranslate,
        onUpload,
        onOpenFile
    }: {
        isMobile: boolean;
        audioTracks: AudioTrack[];
        selectedAudioIndex: number;
        selectedSubIndex: number;
        subtitles: MediaSubtitle[];
        partyMode: boolean;
        syncErr: string | null;
        syncing: string | null;
        syncMethod: SyncMethod;
        subSearchMode: boolean;
        subSearchLang: string;
        subSearchResults: SubSearchResult[];
        subSearching: boolean;
        subSearched: boolean;
        subDownloading: string | null;
        subUploadFile: File | null;
        subUploadLabel: string;
        subUploadLang: string;
        subUploading: boolean;
        aiTranslating: boolean;
        aiTranslateErr: string | null;
        onClose: () => void;
        onSelectAudio: (idx: number) => void;
        onSelectSubtitle: (idx: number) => void;
        onRunSync: (subId: string) => void;
        onDeleteSubtitle: (subId: string, idx: number) => void;
        onSearch: () => void;
        onDownload: (result: SubSearchResult) => void;
        onAiTranslate: () => void;
        onUpload: () => void;
        onOpenFile: () => void;
    } = $props();

    function langRank(lang: string | null | undefined): number {
        const raw = (lang ?? '').toLowerCase().trim();
        const l = raw.split(/[-_]/)[0];
        if (['en', 'eng', 'english'].includes(l)) return 0;
        if (['pl', 'pol', 'polish', 'polski'].includes(l)) return 1;
        if (['de', 'ger', 'deu', 'german', 'deutsch'].includes(l)) return 2;
        return 99;
    }

    function partitionByLang<T extends { language: string }>(items: T[]) {
        const priority: { item: T; idx: number }[] = [];
        const rest: { item: T; idx: number }[] = [];

        items.forEach((item, idx) => {
            if (langRank(item.language) < 99) priority.push({ item, idx });
            else rest.push({ item, idx });
        });

        priority.sort((a, b) => langRank(a.item.language) - langRank(b.item.language));
        return { priority, rest };
    }

    function langName(raw: string | null | undefined): string | null {
        const l = (raw ?? '').toLowerCase().split(/[-_]/)[0];
        const names: Record<string, string> = {
            en: 'English',
            eng: 'English',
            pl: 'Polish',
            pol: 'Polish',
            de: 'German',
            deu: 'German',
            ger: 'German',
            es: 'Spanish',
            spa: 'Spanish',
            fr: 'French',
            fre: 'French',
            fra: 'French',
            ja: 'Japanese',
            jpn: 'Japanese',
            ko: 'Korean',
            kor: 'Korean',
            zh: 'Chinese',
            chi: 'Chinese',
            zho: 'Chinese',
            it: 'Italian',
            ita: 'Italian',
            pt: 'Portuguese',
            por: 'Portuguese',
            ru: 'Russian',
            rus: 'Russian'
        };
        return names[l] ?? null;
    }

    function bareLabel(raw: string | null | undefined): string {
        return (raw ?? '').replace(/\([^)]*\)/g, '').trim().toLowerCase();
    }

    function isUnknownLabel(raw: string | null | undefined): boolean {
        const s = bareLabel(raw);
        return !s || s === 'unknown' || s === 'und' || s === 'undefined';
    }

    function cleanCodec(raw: string | null | undefined): string {
        const c = (raw ?? '').trim().toLowerCase();
        if (!c) return '';
        if (c === 'eac3') return 'eac3';
        if (c === 'ac3') return 'ac3';
        if (c === 'aac') return 'aac';
        if (c === 'opus') return 'opus';
        if (c === 'dts') return 'dts';
        if (c === 'truehd') return 'truehd';
        return c.replace(/^codec:/, '').trim();
    }

    function codecFromLabel(label: string): string {
        const m = label.match(/\(([^)]+)\)\s*$/);
        return cleanCodec(m?.[1]);
    }

    function audioLabel(track: AudioTrack, idx: number): string {
        const raw = (track.label ?? '').trim();
        if (!isUnknownLabel(raw)) return raw;

        const lang = langName(track.language);
        const codec = cleanCodec(track.codec) || codecFromLabel(raw);
        return `${lang ?? `audio ${idx + 1}`}${codec ? ` (${codec})` : ''}`;
    }

    function subFlavor(raw: string): string {
        const s = raw.toLowerCase();
        if (s.includes('forced')) return 'forced';
        if (s.includes('sdh') || s.includes('hearing')) return 'sdh';
        if (s === 'cc' || s.includes('(cc)')) return 'cc';
        return '';
    }

    function subLabel(sub: MediaSubtitle, idx: number): string {
        const raw = (sub.label ?? '').trim();
        if (!isUnknownLabel(raw)) {
            return raw.replace(/\(Forced\)/g, '(forced)').replace(/\(SDH\)/g, '(sdh)');
        }

        const base = langName(sub.language) ?? `subtitle ${idx + 1}`;
        const flavor = subFlavor(raw);
        return flavor ? `${base} (${flavor})` : base;
    }

    function sourceName(src: string | undefined | null): string {
        switch ((src ?? '').toLowerCase()) {
            case 'charlie':
                return 'OpenSubtitles';
            case 'bravo':
                return 'Subf2m';
            case 'india':
                return 'YIFY';
            case 'foxtrot':
                return 'Jimaku';
            case 'juliet':
                return 'Ajatt-Tools';
            case 'ai':
                return 'AI Generated';
            case '':
                return 'unknown';
            default:
                return src as string;
        }
    }

    function isAiSource(src: string | undefined | null): boolean {
        return (src ?? '').toLowerCase() === 'ai';
    }

    function fmtDownloads(n: number | undefined): string {
        if (!n || n <= 0) return '';
        if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
        if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 10_000 ? 0 : 1)}K`;
        return String(n);
    }

    function startSearch() {
        subSearchMode = true;
        subSearched = false;
        subSearchResults = [];
    }

    function clearUpload() {
        subUploadFile = null;
    }
</script>

{#snippet searchResultPills(result: SubSearchResult, dense = false)}
    <div
        class={dense
            ? 'flex items-center gap-1 text-[9.5px] mt-1 flex-wrap'
            : 'flex items-center gap-1.5 text-[10.5px] mt-1.5 flex-wrap'}
    >
        {#if result.source}
            <span
                class="{dense ? 'px-1' : 'px-1.5 tracking-wide'} py-0.5 rounded font-semibold {isAiSource(result.source)
                    ? 'bg-gradient-to-r from-violet-500/30 to-fuchsia-500/30 text-violet-100 border border-violet-400/40'
                    : 'bg-white/10 text-gray-300'}">{sourceName(result.source)}</span
            >
        {/if}
        {#if result.format}<span
                class="{dense
                    ? 'px-1'
                    : 'px-1.5 tracking-wide'} py-0.5 rounded bg-gray-800/70 text-gray-400 uppercase font-mono"
                >{result.format}</span
            >{/if}
        {#if result.encoding && result.encoding.toLowerCase() !== 'utf-8'}<span
                class="{dense ? 'px-1' : 'px-1.5'} py-0.5 rounded bg-amber-500/15 text-amber-300 font-mono"
                >{result.encoding}</span
            >{/if}
        {#if result.origin}<span
                class="{dense ? 'px-1' : 'px-1.5 tracking-wide'} py-0.5 rounded bg-gray-800/70 text-gray-400 uppercase"
                >{result.origin}</span
            >{/if}
        {#if result.hearing_impaired}<span
                class="{dense
                    ? 'px-1'
                    : 'px-1.5 tracking-wide'} py-0.5 rounded bg-yellow-500/15 text-yellow-300 font-semibold">HI</span
            >{/if}
        {#if fmtDownloads(result.downloads)}<span class="text-gray-500 ml-auto font-mono"
                >{fmtDownloads(result.downloads)} dl</span
            >{/if}
        {#if subDownloading === result.url}<span class="text-primary-400">{dense ? '...' : 'downloading...'}</span>{/if}
    </div>
{/snippet}

{#snippet mobileSearch()}
    <div class="px-2 space-y-3">
        <div class="flex items-center gap-2">
            <div class="flex-1">
                <LangPicker bind:value={subSearchLang} options={SUB_LANG_OPTIONS} size="lg" />
            </div>
            <button
                onclick={onSearch}
                disabled={subSearching}
                class="bg-primary-600 text-white px-4 py-2.5 rounded-xl text-sm font-medium disabled:opacity-50"
            >
                {subSearching ? '...' : $t('buttons.search')}
            </button>
        </div>
        {#if subSearchResults.length > 0}
            <button
                onclick={() => onDownload(subSearchResults[0])}
                disabled={!!subDownloading}
                class="w-full bg-primary-600/90 active:bg-primary-700 text-white px-4 py-2.5 rounded-2xl text-sm font-medium disabled:opacity-50 flex items-center justify-center gap-2"
            >
                <Icon name="check-alt" class="w-4 h-4" />
                use best match (score {subSearchResults[0].score})
            </button>
            <div class="space-y-1.5 max-h-[40vh] overflow-y-auto">
                {#each subSearchResults as result}
                    <button
                        onclick={() => onDownload(result)}
                        disabled={subDownloading === result.url}
                        class="w-full text-left px-3 py-3 rounded-2xl bg-[#2c2c2e] active:bg-[#3a3a3c] transition-all disabled:opacity-50"
                    >
                        <div class="flex items-start gap-2">
                            <div
                                class="w-2 h-2 rounded-full mt-1.5 flex-shrink-0 {result.score >= 50
                                    ? 'bg-green-400'
                                    : result.score >= 20
                                      ? 'bg-yellow-400'
                                      : 'bg-gray-600'}"
                                title="match score {result.score}"
                            ></div>
                            <div class="flex-1 min-w-0">
                                <p
                                    class="text-white text-[13px] font-medium leading-snug"
                                    style="word-break: break-word;"
                                >
                                    {result.release || 'Unknown release'}
                                </p>
                                {@render searchResultPills(result)}
                            </div>
                        </div>
                    </button>
                {/each}
            </div>
        {:else if subSearching}
            <div class="flex justify-center py-6">
                <div class="animate-spin rounded-full h-6 w-6 border-t-2 border-b-2 border-primary-500"></div>
            </div>
        {:else if subSearched}
            <div class="text-center py-6 px-4">
                <p class="text-gray-400 text-sm">No {subSearchLang.toUpperCase()} subtitles found</p>
                <button
                    onclick={onAiTranslate}
                    disabled={aiTranslating}
                    class="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-gradient-to-r from-primary-500/20 to-purple-500/20 hover:from-primary-500/30 hover:to-purple-500/30 border border-primary-500/40 text-primary-200 text-[12px] font-semibold tracking-wide transition-all disabled:opacity-60 disabled:cursor-wait"
                >
                    {#if aiTranslating}
                        <span
                            class="inline-block w-3 h-3 rounded-full border-2 border-primary-300 border-t-transparent animate-spin"
                        ></span>
                        translating...
                    {:else}
                        Translate with AI
                    {/if}
                </button>
                {#if aiTranslateErr}
                    <p class="text-red-400/80 text-[11px] mt-2">{aiTranslateErr}</p>
                {/if}
                <p class="text-gray-600 text-[10px] mt-3">{$t('player.try_different_language')}</p>
            </div>
        {:else}
            <p class="text-gray-500 text-sm text-center py-4">{$t('player.select_language_and_search')}</p>
        {/if}
    </div>
{/snippet}

{#snippet desktopSearch()}
    <div class="px-3 py-2 space-y-2">
        <div class="flex items-center gap-1.5">
            <div class="flex-1">
                <LangPicker
                    bind:value={subSearchLang}
                    options={SUB_LANG_OPTIONS}
                    size="sm"
                    onChange={() => onSearch()}
                />
            </div>
            <button
                onclick={onSearch}
                disabled={subSearching}
                class="bg-primary-600 text-white px-3 py-1.5 rounded-lg text-[12px] font-medium disabled:opacity-50 hover:bg-primary-500"
            >
                {subSearching ? '...' : $t('buttons.search')}
            </button>
        </div>
        {#if subSearchResults.length > 0}
            <button
                onclick={() => onDownload(subSearchResults[0])}
                disabled={!!subDownloading}
                class="w-full bg-primary-600/90 hover:bg-primary-600 text-white px-3 py-1.5 rounded-lg text-[12px] font-medium disabled:opacity-50 flex items-center justify-center gap-1.5 mb-1"
            >
                <Icon name="check-alt" class="w-3.5 h-3.5" />
                use best match (score {subSearchResults[0].score})
            </button>
            <div class="space-y-0.5 max-h-[250px] overflow-y-auto">
                {#each subSearchResults as result}
                    <button
                        onclick={() => onDownload(result)}
                        disabled={subDownloading === result.url}
                        class="w-full text-left px-2 py-2 rounded-lg hover:bg-white/5 transition-all disabled:opacity-50"
                    >
                        <div class="flex items-start gap-1.5">
                            <div
                                class="w-1.5 h-1.5 rounded-full mt-1.5 flex-shrink-0 {result.score >= 50
                                    ? 'bg-green-400'
                                    : result.score >= 20
                                      ? 'bg-yellow-400'
                                      : 'bg-gray-600'}"
                                title="match score {result.score}"
                            ></div>
                            <div class="flex-1 min-w-0">
                                <p class="text-white text-[12px] leading-snug" style="word-break: break-word;">
                                    {result.release || 'Unknown'}
                                </p>
                                {@render searchResultPills(result, true)}
                            </div>
                        </div>
                    </button>
                {/each}
            </div>
        {:else if subSearching}
            <div class="flex justify-center py-4">
                <div class="animate-spin rounded-full h-4 w-4 border-t-2 border-b-2 border-primary-500"></div>
            </div>
        {:else if subSearched}
            <div class="text-center py-4 px-2">
                <p class="text-gray-400 text-[11px]">No {subSearchLang.toUpperCase()} subtitles found</p>
                <button
                    onclick={onAiTranslate}
                    disabled={aiTranslating}
                    class="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-gradient-to-r from-primary-500/20 to-purple-500/20 hover:from-primary-500/30 hover:to-purple-500/30 border border-primary-500/40 text-primary-200 text-[10.5px] font-semibold tracking-wide transition-all disabled:opacity-60 disabled:cursor-wait"
                >
                    {#if aiTranslating}
                        <span
                            class="inline-block w-2.5 h-2.5 rounded-full border-2 border-primary-300 border-t-transparent animate-spin"
                        ></span>
                        translating...
                    {:else}
                        Translate with AI
                    {/if}
                </button>
                {#if aiTranslateErr}
                    <p class="text-red-400/80 text-[10px] mt-1.5">{aiTranslateErr}</p>
                {/if}
                <p class="text-gray-600 text-[10px] mt-2">{$t('player.try_a_different_language')}</p>
            </div>
        {/if}
    </div>
{/snippet}

{#snippet mobileAudioList()}
    {#if audioTracks.length > 1}
        <p class="text-[10.5px] text-gray-500 font-semibold uppercase tracking-wider px-4 mb-1 mt-1">
            {$t('player.audio')}
        </p>
        {#each audioTracks as track, idx (idx)}
            <button
                onclick={() => onSelectAudio(idx)}
                class="w-full flex items-center justify-between px-4 py-3.5 rounded-2xl transition-all {selectedAudioIndex ===
                idx
                    ? 'bg-white/10'
                    : 'active:bg-white/5'}"
            >
                <span class="text-[12px] {selectedAudioIndex === idx ? 'text-white font-semibold' : 'text-gray-300'}"
                    >{audioLabel(track, idx)}</span
                >
                {#if selectedAudioIndex === idx}<Icon name="check" class="w-5 h-5 text-primary-400" />{/if}
            </button>
        {/each}
        <p class="text-[10.5px] text-gray-500 font-semibold uppercase tracking-wider px-4 mb-1 mt-3">
            {$t('player.subtitles')}
        </p>
    {/if}
{/snippet}

{#snippet desktopAudioList()}
    {#if audioTracks.length > 1}
        <p class="text-[10px] text-gray-500 font-semibold uppercase tracking-wider px-4 pt-1 pb-1">
            {$t('player.audio')}
        </p>
        {#each audioTracks as track, idx (idx)}
            <button
                onclick={() => onSelectAudio(idx)}
                class="w-full flex items-center gap-3 px-4 py-1.5 text-[12px] transition-all {selectedAudioIndex === idx
                    ? 'text-white bg-white/10'
                    : 'text-gray-400 hover:text-white hover:bg-white/5'}"
            >
                <div
                    class="w-4 h-4 rounded-full border-2 flex items-center justify-center flex-shrink-0 {selectedAudioIndex ===
                    idx
                        ? 'border-primary-500 bg-primary-500'
                        : 'border-gray-600'}"
                >
                    {#if selectedAudioIndex === idx}<Icon name="check" class="w-2.5 h-2.5 text-white" />{/if}
                </div>
                {audioLabel(track, idx)}
            </button>
        {/each}
        <p class="text-[10px] text-gray-500 font-semibold uppercase tracking-wider px-4 pt-2 pb-1">
            {$t('player.subtitles')}
        </p>
    {/if}
{/snippet}

{#snippet mobileSubtitleList()}
    {@render mobileAudioList()}
    <button
        onclick={() => onSelectSubtitle(-1)}
        class="w-full flex items-center justify-between px-4 py-3 rounded-2xl transition-all {selectedSubIndex === -1
            ? 'bg-white/10'
            : 'active:bg-white/5'}"
    >
        <span class="text-[12px] {selectedSubIndex === -1 ? 'text-white font-semibold' : 'text-gray-300'}"
            >{$t('player.off')}</span
        >
        {#if selectedSubIndex === -1}<Icon name="check" class="w-5 h-5 text-primary-400" />{/if}
    </button>
    {#if subtitles.length > 0 && !partyMode && syncErr}
        <p class="px-4 pt-2 text-[10.5px] text-red-400/80 truncate">{syncErr}</p>
    {/if}
    {@const subParts = partitionByLang(subtitles)}
    {#snippet subRow(idx: number, sub: MediaSubtitle)}
        <div class="flex items-center rounded-2xl transition-all {selectedSubIndex === idx ? 'bg-white/10' : ''}">
            <button
                onclick={() => onSelectSubtitle(idx)}
                class="flex-1 flex items-center justify-between px-4 py-3 min-w-0"
            >
                <span
                    class="text-[12px] truncate {selectedSubIndex === idx
                        ? 'text-white font-semibold'
                        : 'text-gray-300'}">{subLabel(sub, idx)}</span
                >
                {#if selectedSubIndex === idx}<Icon name="check" class="w-5 h-5 text-primary-400 flex-shrink-0" />{/if}
            </button>
            {#if !partyMode}
                <button
                    onclick={() => onRunSync(sub.id)}
                    disabled={syncing === sub.id || !!syncing}
                    title="sync ({syncMethod})"
                    class="px-2.5 py-2 text-gray-600 active:text-primary-400 disabled:opacity-40 flex-shrink-0"
                >
                    {#if syncing === sub.id}
                        <Icon name="spinner" class="w-4 h-4 animate-spin text-primary-400" />
                    {:else}
                        <Icon name="sync" class="w-4 h-4" />
                    {/if}
                </button>
                <button
                    onclick={() => onDeleteSubtitle(sub.id, idx)}
                    class="px-3 py-2 text-gray-600 active:text-red-400 flex-shrink-0"
                >
                    <Icon name="trash" class="w-4 h-4" />
                </button>
            {/if}
        </div>
    {/snippet}
    {#each subParts.priority as p (p.idx)}{@render subRow(p.idx, p.item)}{/each}
    {#if subParts.priority.length > 0 && subParts.rest.length > 0}
        <div class="h-px bg-white/[0.08] mx-4 my-1.5"></div>
    {/if}
    {#each subParts.rest as p (p.idx)}{@render subRow(p.idx, p.item)}{/each}
    {#if subtitles.length === 0}
        <p class="text-[12px] text-gray-500 px-4 py-4 text-center">{$t('player.no_subtitles_available')}</p>
    {/if}
{/snippet}

{#snippet desktopSubtitleList()}
    {@render desktopAudioList()}
    <button
        onclick={() => onSelectSubtitle(-1)}
        class="w-full flex items-center gap-3 px-4 py-2 text-[12px] transition-all {selectedSubIndex === -1
            ? 'text-white bg-white/10'
            : 'text-gray-400 hover:text-white hover:bg-white/5'}"
    >
        <div
            class="w-4 h-4 rounded-full border-2 flex items-center justify-center flex-shrink-0 {selectedSubIndex === -1
                ? 'border-primary-500 bg-primary-500'
                : 'border-gray-600'}"
        >
            {#if selectedSubIndex === -1}<Icon name="check" class="w-2.5 h-2.5 text-white" />{/if}
        </div>
        {$t('player.off')}
    </button>
    {#if syncErr && !partyMode}
        <p class="px-4 pt-1 text-[10px] text-red-400/80 truncate">{syncErr}</p>
    {/if}
    {@const subParts = partitionByLang(subtitles)}
    {#snippet subRow(idx: number, sub: MediaSubtitle)}
        <div
            class="flex items-center group/sub transition-all {selectedSubIndex === idx
                ? 'bg-white/10'
                : 'hover:bg-white/5'}"
        >
            <button
                onclick={() => onSelectSubtitle(idx)}
                class="flex-1 flex items-center gap-3 px-4 py-2 text-[12px] min-w-0"
            >
                <div
                    class="w-4 h-4 rounded-full border-2 flex items-center justify-center flex-shrink-0 {selectedSubIndex ===
                    idx
                        ? 'border-primary-500 bg-primary-500'
                        : 'border-gray-600'}"
                >
                    {#if selectedSubIndex === idx}<Icon name="check" class="w-2.5 h-2.5 text-white" />{/if}
                </div>
                <span class="truncate {selectedSubIndex === idx ? 'text-white' : 'text-gray-400'}"
                    >{subLabel(sub, idx)}</span
                >
            </button>
            {#if !partyMode}
                <button
                    onclick={() => onRunSync(sub.id)}
                    disabled={syncing === sub.id || !!syncing}
                    title="sync ({syncMethod})"
                    class="px-2 py-1 text-transparent group-hover/sub:text-gray-600 hover:!text-primary-400 disabled:!text-primary-400 flex-shrink-0 transition-colors"
                >
                    {#if syncing === sub.id}
                        <Icon name="spinner" class="w-3.5 h-3.5 animate-spin" />
                    {:else}
                        <Icon name="sync" class="w-3.5 h-3.5" />
                    {/if}
                </button>
                <button
                    onclick={() => onDeleteSubtitle(sub.id, idx)}
                    class="px-2 py-1 text-transparent group-hover/sub:text-gray-600 hover:!text-red-400 flex-shrink-0 transition-colors"
                >
                    <Icon name="trash" class="w-3.5 h-3.5" />
                </button>
            {/if}
        </div>
    {/snippet}
    {#each subParts.priority as p (p.idx)}{@render subRow(p.idx, p.item)}{/each}
    {#if subParts.priority.length > 0 && subParts.rest.length > 0}
        <div class="h-px bg-white/[0.08] mx-4 my-1"></div>
    {/if}
    {#each subParts.rest as p (p.idx)}{@render subRow(p.idx, p.item)}{/each}
    {#if subtitles.length === 0}
        <p class="text-[12px] text-gray-600 px-4 py-3 text-center">{$t('player.no_subtitles_available')}</p>
    {/if}
{/snippet}

{#snippet mobileUpload()}
    {#if !partyMode}
        <div class="mt-2 px-2 space-y-2">
            {#if subUploadFile}
                <div class="bg-[#2c2c2e] rounded-2xl p-4 space-y-3">
                    <p class="text-white text-sm font-medium truncate">{subUploadFile.name}</p>
                    <input
                        type="text"
                        bind:value={subUploadLabel}
                        placeholder="Label (e.g. English)"
                        class="w-full bg-[#1c1c1e] rounded-xl px-3 py-2.5 text-white text-sm placeholder-gray-500 border border-gray-700 focus:border-primary-500 outline-none"
                    />
                    <select
                        bind:value={subUploadLang}
                        class="w-full bg-[#1c1c1e] rounded-xl px-3 py-2.5 text-white text-sm border border-gray-700 focus:border-primary-500 outline-none"
                    >
                        <option value="en">English</option><option value="ar">Arabic</option><option value="es"
                            >Spanish</option
                        >
                        <option value="fr">French</option><option value="de">German</option><option value="ja"
                            >Japanese</option
                        >
                        <option value="ko">Korean</option><option value="pt">Portuguese</option><option value="ru"
                            >Russian</option
                        >
                        <option value="zh">Chinese</option><option value="und">Other</option>
                    </select>
                    <div class="flex gap-2">
                        <button
                            onclick={clearUpload}
                            class="flex-1 py-2.5 rounded-xl bg-[#3a3a3c] text-gray-300 text-sm font-medium"
                            >{$t('buttons.cancel')}</button
                        >
                        <button
                            onclick={onUpload}
                            disabled={subUploading}
                            class="flex-1 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-medium disabled:opacity-50"
                        >
                            {subUploading ? 'Uploading...' : 'Upload'}
                        </button>
                    </div>
                </div>
            {:else}
                <div class="flex gap-2">
                    <button
                        onclick={onOpenFile}
                        class="flex-1 flex items-center justify-center gap-2 px-3 py-3 rounded-2xl bg-[#2c2c2e] active:bg-[#3a3a3c] transition-colors"
                    >
                        <Icon name="plus" class="w-4 h-4 text-gray-400" />
                        <span class="text-gray-300 text-[13px] font-medium">Upload</span>
                    </button>
                    <button
                        onclick={startSearch}
                        class="flex-1 flex items-center justify-center gap-2 px-3 py-3 rounded-2xl bg-[#2c2c2e] active:bg-[#3a3a3c] transition-colors"
                    >
                        <Icon name="search" class="w-4 h-4 text-gray-400" />
                        <span class="text-gray-300 text-[13px] font-medium">Search</span>
                    </button>
                </div>
            {/if}
        </div>
    {/if}
{/snippet}

{#snippet desktopUpload()}
    {#if !partyMode}
        <div class="border-t border-white/5 mt-1 pt-1">
            {#if subUploadFile}
                <div class="px-3 py-2 space-y-2">
                    <p class="text-white text-[12px] font-medium truncate">{subUploadFile.name}</p>
                    <input
                        type="text"
                        bind:value={subUploadLabel}
                        placeholder="Label"
                        class="w-full bg-white/5 rounded-lg px-2.5 py-1.5 text-white text-[12px] placeholder-gray-500 border border-white/10 focus:border-primary-500 outline-none"
                    />
                    <div class="flex gap-2">
                        <button
                            onclick={clearUpload}
                            class="flex-1 py-1.5 rounded-lg bg-white/5 text-gray-300 text-[11px] font-medium hover:bg-white/10"
                            >{$t('buttons.cancel')}</button
                        >
                        <button
                            onclick={onUpload}
                            disabled={subUploading}
                            class="flex-1 py-1.5 rounded-lg bg-primary-600 text-white text-[11px] font-medium disabled:opacity-50 hover:bg-primary-500"
                        >
                            {subUploading ? '...' : $t('buttons.upload')}
                        </button>
                    </div>
                </div>
            {:else}
                <button
                    onclick={onOpenFile}
                    class="w-full flex items-center gap-3 px-4 py-2 text-[12px] text-gray-400 hover:text-white hover:bg-white/5 transition-all"
                >
                    <Icon name="plus" class="w-4 h-4" />
                    {$t('player.upload_file')}
                </button>
                <button
                    onclick={startSearch}
                    class="w-full flex items-center gap-3 px-4 py-2 text-[12px] text-gray-400 hover:text-white hover:bg-white/5 transition-all"
                >
                    <Icon name="search" class="w-4 h-4" />
                    {$t('player.search_subtitles')}
                </button>
            {/if}
        </div>
    {/if}
{/snippet}

{#if isMobile}
    <div class="fixed inset-0 z-50">
        <button
            type="button"
            class="absolute inset-0 bg-black/60"
            transition:fade={{ duration: 200 }}
            onclick={onClose}
            aria-label="close audio and subtitles"
        ></button>
        <div
            class="absolute bottom-0 left-0 right-0 bg-black/70 backdrop-blur-2xl rounded-t-3xl shadow-[0_-10px_40px_rgba(0,0,0,0.5)] max-h-[65vh] overflow-hidden"
            in:fly={{ y: 500, duration: 400, easing: cubicOut }}
            out:fly={{ y: 500, duration: 300 }}
        >
            <div class="flex justify-center pt-3 pb-1">
                <div class="w-10 h-1.5 bg-gray-600 rounded-full"></div>
            </div>
            {#if subSearchMode}
                <div class="flex items-center gap-3 px-5 pt-2 pb-3">
                    <button
                        onclick={() => {
                            subSearchMode = false;
                        }}
                        class="text-white"
                    >
                        <Icon name="chevron-left" class="w-5 h-5" />
                    </button>
                    <h3 class="text-white text-lg font-bold">{$t('player.search_subtitles')}</h3>
                </div>
            {/if}
            <div class="overflow-y-auto scrollbar-hide pb-8 px-4" style="max-height: calc(65vh - 80px);">
                {#if subSearchMode}
                    {@render mobileSearch()}
                {:else}
                    {@render mobileSubtitleList()}
                    {@render mobileUpload()}
                {/if}
            </div>
        </div>
    </div>
{:else}
    <button type="button" class="absolute inset-0 z-30" onclick={onClose} aria-label="close audio and subtitles"
    ></button>
    <div in:fly={{ y: 8, duration: 200, easing: cubicOut }} class="absolute z-40 bottom-24 right-10 w-72">
        <div
            class="bg-black/60 backdrop-blur-2xl rounded-2xl shadow-[0_8px_32px_rgba(0,0,0,0.6)] ring-1 ring-white/10 overflow-hidden"
        >
            <div class="bg-white/5 flex items-center justify-between px-4 pt-3.5 pb-2.5">
                {#if subSearchMode}
                    <button
                        onclick={() => {
                            subSearchMode = false;
                        }}
                        class="text-primary-400 text-[12px] font-medium flex items-center gap-1"
                    >
                        <Icon name="chevron-left" class="w-3.5 h-3.5" />
                        {$t('buttons.back')}
                    </button>
                    <span class="text-white text-[12px] font-semibold">{$t('player.search_subtitles')}</span>
                {:else}
                    <h3 class="text-white text-[13px] font-semibold">{$t('player.audio_and_subs')}</h3>
                {/if}
                <button onclick={onClose} class="text-gray-500 hover:text-white transition-colors">
                    <Icon name="close" class="w-4 h-4" />
                </button>
            </div>
            <div class="max-h-[50vh] overflow-y-auto scrollbar-hide pb-2">
                {#if subSearchMode}
                    {@render desktopSearch()}
                {:else}
                    {@render desktopSubtitleList()}
                    {@render desktopUpload()}
                {/if}
            </div>
        </div>
    </div>
{/if}
