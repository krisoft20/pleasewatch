<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type User } from '$lib/api';
    import { fmtDate, type CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    let { onCount }: Props = $props();

    let allUsers = $state<User[]>([]);
    let pendingUsers = $state<User[]>([]);

    onMount(() => {
        loadUsers().catch(() => {});
    });

    async function loadUsers() {
        allUsers = await api.listUsers();
        pendingUsers = allUsers.filter((u) => u.role === 'pending');
        onCount?.(pendingUsers.length || null);
    }

    async function approve(id: string) {
        await api.approveUser(id);
        await loadUsers();
    }

    async function reject(id: string, name: string) {
        if (!confirm(`reject ${name}? account gets deleted.`)) return;
        await api.deleteUser(id);
        await loadUsers();
    }
</script>

{#if pendingUsers.length === 0}
    <p class="text-gray-400 text-center py-10">no users waiting for approval</p>
{:else}
    <div class="space-y-2">
        {#each pendingUsers as u}
            <div class="bg-gray-800/40 rounded-lg p-4 flex items-center gap-3">
                <div class="flex-1 min-w-0">
                    <p class="text-white font-medium">{u.username}</p>
                    <p class="text-xs text-gray-400">{u.email}, joined {fmtDate(u.created_at)}</p>
                </div>
                <button
                    onclick={() => approve(u.id)}
                    class="text-xs bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded font-medium"
                >
                    approve
                </button>
                <button
                    onclick={() => reject(u.id, u.username)}
                    class="text-xs text-red-400 hover:text-red-300 px-3 py-2"
                >
                    reject
                </button>
            </div>
        {/each}
    </div>
{/if}
