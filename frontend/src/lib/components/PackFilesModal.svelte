<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type PackFile } from '$lib/api';
    import { t } from '$lib/i18n.svelte';

    let { downloadId, title, onClose }: { downloadId: string; title: string; onClose: () => void } =
        $props();

    let files = $state<PackFile[]>([]);
    let loading = $state(true);
    let err = $state('');
    let lastIndex = $state(-1);

    onMount(() => {
        loadFiles().catch(() => {});
    });

    $effect(() => {
        const onKey = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        document.addEventListener('keydown', onKey);
        return () => document.removeEventListener('keydown', onKey);
    });

    async function loadFiles() {
        try {
            files = await api.packFiles(downloadId);
        } catch (e) {
            err = e instanceof Error ? e.message : 'failed to load files';
        } finally {
            loading = false;
        }
    }

    const seasons = $derived.by(() => {
        const out = new Map<number, PackFile[]>();
        for (const f of files) {
            const list = out.get(f.season);
            if (list) list.push(f);
            else out.set(f.season, [f]);
        }
        return [...out.entries()];
    });

    const wantedCount = $derived(files.filter((f) => f.wanted).length);

    function setLocal(index: number, wanted: boolean) {
        files = files.map((f) => (f.index === index ? { ...f, wanted } : f));
    }

    async function toggle(file: PackFile, range: boolean) {
        const want = !file.wanted;
        let targets = [file];

        if (range && lastIndex >= 0) {
            const a = files.findIndex((f) => f.index === lastIndex);
            const b = files.findIndex((f) => f.index === file.index);
            if (a >= 0 && b >= 0 && a !== b) {
                targets = files.slice(Math.min(a, b), Math.max(a, b) + 1);
            }
        }
        lastIndex = file.index;

        for (const f of targets) {
            if (f.wanted === want) continue;
            setLocal(f.index, want);
            try {
                await api.setPackFile(downloadId, f.index, want);
            } catch (e) {
                setLocal(f.index, !want);
                err = e instanceof Error ? e.message : 'save failed';
                return;
            }
        }
    }

    function onBackdrop(e: MouseEvent) {
        if (e.target === e.currentTarget) onClose();
    }
</script>

<div
    class="pw-pack-bg"
    onclick={onBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
>
    <div class="pw-pack-shell">
        <header class="pw-pack-head">
            <div class="pw-pack-head-txt">
                <p class="pw-pack-title">{title}</p>
                <p class="pw-pack-meta">
                    {#if loading}
                        ...
                    {:else}
                        {wantedCount}/{files.length} {t('media.included')}
                    {/if}
                </p>
            </div>
            <button class="pw-pack-close" onclick={onClose} aria-label="close">
                <svg
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
            </button>
        </header>

        {#if loading}
            <p class="pw-pack-note">// loading files...</p>
        {:else if files.length === 0}
            <p class="pw-pack-note">// no episode files in this download</p>
        {:else}
            <p class="pw-pack-hint">
                {t('media.skip_episodes_hint')}
                <span class="pw-pack-hint-key">{t('media.shift_hint')}</span>
            </p>
            <div class="pw-pack-body">
                {#each seasons as [season, list] (season)}
                    <div class="pw-pack-season">
                        {#if seasons.length > 1}
                            <span class="pw-pack-season-tag">S{String(season).padStart(2, '0')}</span>
                        {/if}
                        <div class="pw-pack-chips">
                            {#each list as f (f.index)}
                                <button
                                    class="pw-pack-chip"
                                    class:off={!f.wanted}
                                    title={f.name}
                                    onclick={(e) => toggle(f, e.shiftKey)}
                                >
                                    E{String(f.episode).padStart(2, '0')}
                                </button>
                            {/each}
                        </div>
                    </div>
                {/each}
            </div>
        {/if}

        {#if err}<p class="pw-pack-err">{err}</p>{/if}
    </div>
</div>

<style>
    .pw-pack-bg {
        position: fixed;
        inset: 0;
        z-index: 200;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        background: rgba(0, 0, 0, 0.72);
        backdrop-filter: blur(10px);
        -webkit-backdrop-filter: blur(10px);
    }
    .pw-pack-shell {
        width: 100%;
        max-width: 720px;
        max-height: 82vh;
        display: flex;
        flex-direction: column;
        padding: 18px 20px 20px;
        border-radius: 14px;
        background: #0c0c0f;
        border: 1px solid rgba(255, 255, 255, 0.08);
        box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.9);
    }
    .pw-pack-head {
        display: flex;
        align-items: flex-start;
        gap: 12px;
    }
    .pw-pack-head-txt {
        min-width: 0;
        flex: 1;
    }
    .pw-pack-title {
        margin: 0;
        font-size: 14px;
        font-weight: 600;
        color: #ececef;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pw-pack-meta {
        margin: 3px 0 0;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        color: rgba(232, 232, 234, 0.5);
    }
    .pw-pack-close {
        width: 26px;
        height: 26px;
        display: grid;
        place-items: center;
        border-radius: 999px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        background: rgba(255, 255, 255, 0.04);
        color: rgba(232, 232, 234, 0.65);
        cursor: pointer;
        transition: background 0.14s ease;
    }
    .pw-pack-close:hover {
        background: rgba(255, 255, 255, 0.12);
    }
    .pw-pack-hint {
        margin: 14px 0 10px;
        font-size: 12px;
        line-height: 1.5;
        color: rgba(220, 220, 225, 0.55);
    }
    .pw-pack-hint-key {
        display: block;
        margin-top: 3px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10.5px;
        color: rgba(220, 220, 225, 0.35);
    }
    .pw-pack-note {
        margin: 18px 0;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 12px;
        color: rgba(220, 220, 225, 0.45);
    }
    .pw-pack-body {
        overflow-y: auto;
        padding-right: 4px;
    }
    .pw-pack-season + .pw-pack-season {
        margin-top: 14px;
    }
    .pw-pack-season-tag {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        letter-spacing: 0.08em;
        color: rgba(232, 232, 234, 0.4);
    }
    .pw-pack-chips {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        margin-top: 6px;
    }
    .pw-pack-chip {
        min-width: 52px;
        padding: 7px 9px;
        border-radius: 8px;
        border: 1px solid color-mix(in oklch, var(--pw-accent) 35%, transparent);
        background: color-mix(in oklch, var(--pw-accent) 16%, transparent);
        color: color-mix(in oklch, var(--pw-accent) 78%, white 22%);
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.02em;
        cursor: pointer;
        transition:
            background 0.14s ease,
            border-color 0.14s ease,
            color 0.14s ease;
    }
    .pw-pack-chip:hover {
        background: color-mix(in oklch, var(--pw-accent) 26%, transparent);
    }
    .pw-pack-chip.off {
        border-color: rgba(255, 255, 255, 0.08);
        border-style: dashed;
        background: transparent;
        color: rgba(232, 232, 234, 0.3);
        text-decoration: line-through;
    }
    .pw-pack-chip.off:hover {
        border-color: rgba(255, 255, 255, 0.2);
        color: rgba(232, 232, 234, 0.55);
    }
    .pw-pack-err {
        margin: 12px 0 0;
        font-size: 12px;
        color: oklch(0.7 0.2 25);
    }
</style>
