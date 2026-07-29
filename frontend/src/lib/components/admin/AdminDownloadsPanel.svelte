<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type DownloadStatus } from '$lib/api';
    import { fmtDate, fmtPct, type CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    let { onCount }: Props = $props();

    let downloads = $state<DownloadStatus[]>([]);

    onMount(() => {
        loadDownloads().catch(() => {});
    });

    async function loadDownloads() {
        downloads = await api.listDownloads();
        onCount?.(downloads.length || null);
    }

    async function cancelDl(id: string) {
        if (!confirm('cancel this download?')) return;
        await api.cancelDownload(id);
        downloads = downloads.filter((d) => d.id !== id);
        onCount?.(downloads.length || null);
    }

    async function cleanupErrors() {
        const ok = confirm('delete every cancelled or errored download record? media files are NOT touched.');
        if (!ok) return;
        const r = await api.cleanupErrored();
        await loadDownloads();
        alert(`removed ${r.removed} dead downloads.`);
    }
</script>

<div class="flex items-center justify-between mb-4">
    <p class="text-sm text-gray-400">{downloads.length} downloads tracked</p>
    <button onclick={cleanupErrors} class="text-xs bg-red-600/20 hover:bg-red-600/40 text-red-400 px-3 py-1.5 rounded">
        cleanup errored / cancelled
    </button>
</div>
{#if downloads.length === 0}
    <p class="text-gray-400 text-center py-10">no downloads</p>
{:else}
    <div class="space-y-2">
        {#each downloads as d}
            <div class="bg-gray-800/40 rounded-lg p-4">
                <div class="flex items-center justify-between gap-3">
                    <div class="flex-1 min-w-0">
                        <p class="text-white font-medium truncate">{d.title ?? d.id}</p>
                        <p class="text-xs text-gray-400">{fmtDate(d.created_at)}</p>
                    </div>
                    <span
                        class="text-xs uppercase tracking-wider {d.status === 'complete'
                            ? 'text-green-400'
                            : d.status === 'cancelled'
                              ? 'text-gray-500'
                              : d.status === 'error'
                                ? 'text-red-400'
                                : 'text-primary-400'}"
                    >
                        {d.status === 'complete' ? 'done' : (d.state ?? d.status)}
                    </span>
                    {#if d.status !== 'complete' && d.status !== 'cancelled'}
                        <button
                            onclick={() => cancelDl(d.id)}
                            class="text-xs text-red-400 hover:text-red-300 px-2 py-1"
                        >
                            cancel
                        </button>
                    {/if}
                </div>
                <div class="mt-3 h-1.5 bg-gray-800 rounded-full overflow-hidden">
                    <div class="h-full bg-primary-500" style:width={fmtPct(d.progress)}></div>
                </div>
            </div>
        {/each}
    </div>
{/if}
