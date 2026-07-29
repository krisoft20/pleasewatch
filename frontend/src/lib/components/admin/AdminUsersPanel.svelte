<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type User } from '$lib/api';
    import { fmtDate, type CountSetter } from './adminUtils';

    type Props = {
        me: User | null;
        onCount?: CountSetter;
    };

    let { me, onCount }: Props = $props();

    let allUsers = $state<User[]>([]);
    let visibleUsers = $derived(allUsers.filter((u) => u.role !== 'pending'));

    onMount(() => {
        loadUsers().catch(() => {});
    });

    async function loadUsers() {
        allUsers = await api.listUsers();
        onCount?.(allUsers.filter((u) => u.role !== 'pending').length || null);
    }

    async function setRole(id: string, role: string) {
        await api.setUserRole(id, role);
        await loadUsers();
    }

    async function removeUser(id: string, name: string) {
        if (!confirm(`delete ${name}?`)) return;
        await api.deleteUser(id);
        await loadUsers();
    }
</script>

<div class="space-y-2">
    {#each visibleUsers as u}
        <div class="bg-gray-800/40 rounded-lg p-3 flex items-center gap-3">
            <div
                class="w-9 h-9 rounded-full bg-primary-600 flex items-center justify-center text-white text-sm font-bold flex-shrink-0"
            >
                {u.username.charAt(0).toUpperCase()}
            </div>
            <div class="flex-1 min-w-0">
                <p class="text-white font-medium truncate">{u.username}</p>
                <p class="text-xs text-gray-400 truncate">{u.email}, joined {fmtDate(u.created_at)}</p>
            </div>
            <span
                class="text-[10px] uppercase tracking-wider px-2 py-0.5 rounded {u.role === 'admin'
                    ? 'bg-primary-600/30 text-primary-300'
                    : 'bg-gray-700 text-gray-300'}"
            >
                {u.role}
            </span>
            {#if me && u.id !== me.id}
                {#if u.role === 'admin'}
                    <button
                        onclick={() => setRole(u.id, 'user')}
                        class="text-xs text-gray-400 hover:text-white px-2 py-1.5"
                    >
                        demote
                    </button>
                {:else}
                    <button
                        onclick={() => setRole(u.id, 'admin')}
                        class="text-xs text-gray-400 hover:text-white px-2 py-1.5"
                    >
                        promote
                    </button>
                {/if}
                <button
                    onclick={() => removeUser(u.id, u.username)}
                    class="text-xs text-red-400 hover:text-red-300 px-2 py-1.5"
                >
                    delete
                </button>
            {/if}
        </div>
    {/each}
</div>
