<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type ClipInfo } from '$lib/api';
    import Icon from '$lib/components/Icon.svelte';
    import type { CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    let { onCount }: Props = $props();

    let clips = $state<ClipInfo[]>([]);

    onMount(() => {
        loadClips().catch(() => {});
    });

    async function loadClips() {
        clips = await api.listClips();
        onCount?.(null);
    }

    async function deleteClip(id: string) {
        if (!confirm('delete this clip?')) return;
        await api.deleteClip(id);
        clips = clips.filter((x) => x.id !== id);
    }

    function copyClip(id: string) {
        navigator.clipboard.writeText(`${location.origin}/clip/${id}`);
    }
</script>

{#if clips.length === 0}
    <div class="bg-gray-900 rounded-xl p-8 text-center">
        <div class="w-16 h-16 rounded-full bg-gray-800 mx-auto mb-3 flex items-center justify-center">
            <Icon name="play" class="w-7 h-7 text-gray-600" />
        </div>
        <p class="text-white font-medium">no clips yet</p>
        <p class="text-gray-500 text-sm mt-1">cut a clip from the player to see it here.</p>
    </div>
{:else}
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        {#each clips as c (c.id)}
            {@const dur = c.end_sec - c.start_sec}
            {@const mb = c.file_size ? (c.file_size / 1024 / 1024).toFixed(1) : '?'}
            <div
                class="bg-gray-900 rounded-lg overflow-hidden border border-white/5 hover:border-white/15 transition-colors"
            >
                <video src={`/api/clips/${c.id}`} controls preload="metadata" class="w-full aspect-video bg-black">
                    <track kind="captions" />
                </video>
                <div class="p-3 flex items-center justify-between gap-2">
                    <div class="min-w-0">
                        <div class="text-xs text-gray-400 font-mono">
                            {dur.toFixed(1)}s, {mb} MB
                            {#if c.subtitle_id}<span class="ml-1 text-primary-400">, subs</span>{/if}
                        </div>
                        <div class="text-xs text-gray-500 truncate mt-0.5">
                            {c.created_at}
                        </div>
                    </div>
                    <div class="flex gap-1.5 flex-shrink-0">
                        <button
                            class="w-8 h-8 rounded bg-white/5 hover:bg-white/10 text-gray-300 inline-flex items-center justify-center"
                            onclick={() => copyClip(c.id)}
                            title="copy share link"
                            aria-label="copy share link"
                        >
                            <Icon name="copy" class="w-4 h-4" />
                        </button>
                        <button
                            class="w-8 h-8 rounded bg-red-600/80 hover:bg-red-600 text-white inline-flex items-center justify-center"
                            onclick={() => deleteClip(c.id)}
                            title="delete clip"
                            aria-label="delete clip"
                        >
                            <Icon name="trash" class="w-4 h-4" />
                        </button>
                    </div>
                </div>
            </div>
        {/each}
    </div>
{/if}
