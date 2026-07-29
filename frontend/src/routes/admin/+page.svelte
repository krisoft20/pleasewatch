<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api, type User } from '$lib/api';
    import Icon from '$lib/components/Icon.svelte';
    import TopBar from '$lib/components/TopBar.svelte';
    import AdminClipsPanel from '$lib/components/admin/AdminClipsPanel.svelte';
    import AdminDashboardPanel from '$lib/components/admin/AdminDashboardPanel.svelte';
    import AdminDownloadsPanel from '$lib/components/admin/AdminDownloadsPanel.svelte';
    import AdminLeaderboardPanel from '$lib/components/admin/AdminLeaderboardPanel.svelte';
    import AdminLogsPanel from '$lib/components/admin/AdminLogsPanel.svelte';
    import AdminPendingUsersPanel from '$lib/components/admin/AdminPendingUsersPanel.svelte';
    import AdminSettingsPanel from '$lib/components/admin/AdminSettingsPanel.svelte';
    import AdminStoragePanel from '$lib/components/admin/AdminStoragePanel.svelte';
    import AdminUsersPanel from '$lib/components/admin/AdminUsersPanel.svelte';

    type Tab =
        | 'dashboard'
        | 'storage'
        | 'leaderboard'
        | 'pending'
        | 'users'
        | 'downloads'
        | 'clips'
        | 'logs'
        | 'settings';

    let me = $state<User | null>(null);
    let activeTab = $state<Tab>('dashboard');
    let loading = $state(true);
    let tabCounts = $state<Record<Tab, number | null>>({
        dashboard: null,
        storage: null,
        leaderboard: null,
        pending: null,
        users: null,
        downloads: null,
        clips: null,
        logs: null,
        settings: null
    });

    const tabsList: Array<{ key: Tab; label: string }> = [
        { key: 'dashboard', label: 'Dashboard' },
        { key: 'storage', label: 'Storage' },
        { key: 'leaderboard', label: 'Leaderboard' },
        { key: 'pending', label: 'Pending Users' },
        { key: 'users', label: 'All Users' },
        { key: 'downloads', label: 'Downloads' },
        { key: 'clips', label: 'Clips' },
        { key: 'logs', label: 'Logs' },
        { key: 'settings', label: 'Settings' }
    ];

    onMount(async () => {
        try {
            me = await api.me();
        } catch {
            goto('/login');
            return;
        }
        if (me.role !== 'admin') {
            goto('/');
            return;
        }
        const requested = new URLSearchParams(window.location.search).get('tab') as Tab | null;
        if (requested && tabsList.some((t) => t.key === requested)) {
            activeTab = requested;
        }
        loading = false;
    });

    function selectTab(tab: Tab) {
        activeTab = tab;
        const url = new URL(window.location.href);
        url.searchParams.set('tab', tab);
        history.replaceState(history.state, '', url.pathname + url.search);
    }

    function setTabCount(tab: Tab, count: number | null) {
        tabCounts = { ...tabCounts, [tab]: count };
    }

    function countFor(tab: Tab): number | null {
        return tabCounts[tab];
    }
</script>

<svelte:head><title>{activeTab} - admin - pleasewatch</title></svelte:head>

{#if me}
    <TopBar user={me} back={true} />

    <div class="max-w-6xl mx-auto px-4 sm:px-8 py-6 text-gray-100">
        <div class="pw-admin-tabs-mobile">
            <select
                class="pw-admin-tab-select"
                value={activeTab}
                onchange={(e) => selectTab((e.currentTarget as HTMLSelectElement).value as Tab)}
                aria-label="section"
            >
                {#each tabsList as t}
                    {@const c = countFor(t.key)}
                    <option value={t.key}>
                        {t.label}{c !== null && c > 0 ? ` (${c})` : ''}
                    </option>
                {/each}
            </select>
            <Icon name="chevron-down" class="pw-admin-tab-caret" strokeWidth={2} />
        </div>

        <div class="pw-admin-tabs-desktop">
            {#each tabsList as t}
                {@const c = countFor(t.key)}
                <button
                    class="pw-admin-tab"
                    class:is-active={activeTab === t.key}
                    onclick={() => selectTab(t.key)}
                    type="button"
                >
                    {t.label}
                    {#if c !== null && c > 0}
                        <span class="pw-admin-tab-count">{c}</span>
                    {/if}
                </button>
            {/each}
        </div>

        {#if loading}
            <div class="flex items-center justify-center py-20">
                <div class="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-primary-500"></div>
            </div>
        {:else if activeTab === 'dashboard'}
            <AdminDashboardPanel onCount={(count) => setTabCount('dashboard', count)} />
        {:else if activeTab === 'storage'}
            <AdminStoragePanel onCount={(count) => setTabCount('storage', count)} />
        {:else if activeTab === 'leaderboard'}
            <AdminLeaderboardPanel onCount={(count) => setTabCount('leaderboard', count)} />
        {:else if activeTab === 'pending'}
            <AdminPendingUsersPanel onCount={(count) => setTabCount('pending', count)} />
        {:else if activeTab === 'users'}
            <AdminUsersPanel {me} onCount={(count) => setTabCount('users', count)} />
        {:else if activeTab === 'downloads'}
            <AdminDownloadsPanel onCount={(count) => setTabCount('downloads', count)} />
        {:else if activeTab === 'clips'}
            <AdminClipsPanel onCount={(count) => setTabCount('clips', count)} />
        {:else if activeTab === 'logs'}
            <AdminLogsPanel onCount={(count) => setTabCount('logs', count)} />
        {:else if activeTab === 'settings'}
            <AdminSettingsPanel />
        {/if}
    </div>
{/if}
