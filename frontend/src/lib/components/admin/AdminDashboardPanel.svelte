<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type AppMetrics, type HealthCheck, type Insight, type SystemMetrics } from '$lib/api';
    import { fmtSize, fmtUptime, type CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    let { onCount }: Props = $props();

    let systemMetrics = $state<SystemMetrics | null>(null);
    let appMetrics = $state<AppMetrics | null>(null);
    let serviceHealth = $state<HealthCheck[]>([]);
    let insights = $state<Insight[]>([]);
    let healthCheckedAt = $state(0);
    let healthLoading = $state(false);
    let dashTimer: ReturnType<typeof setInterval> | null = null;

    let diskPct = $derived(
        systemMetrics && systemMetrics.disk_total_bytes > 0
            ? (systemMetrics.disk_used_bytes / systemMetrics.disk_total_bytes) * 100
            : 0
    );
    let langMax = $derived(appMetrics?.subtitles.by_language[0]?.count || 1);
    let srcMax = $derived(appMetrics?.subtitles.by_source[0]?.count || 1);

    onMount(() => {
        refreshDashboard();
        dashTimer = setInterval(refreshDashboard, 30000);
        return () => {
            if (dashTimer) clearInterval(dashTimer);
        };
    });

    function setInsights(next: Insight[]) {
        insights = next;
        onCount?.(next.filter((i) => i.severity !== 'info').length || null);
    }

    async function refreshDashboard() {
        api.adminSystem()
            .then((d) => {
                systemMetrics = d;
            })
            .catch(() => {});
        api.adminMetrics()
            .then((d) => {
                appMetrics = d;
            })
            .catch(() => {});
        api.adminInsights()
            .then((d) => setInsights(d.insights || []))
            .catch(() => {});
        healthLoading = true;
        api.adminHealth()
            .then((d) => {
                serviceHealth = d;
                healthCheckedAt = Math.floor(Date.now() / 1000);
            })
            .catch(() => {})
            .finally(() => {
                healthLoading = false;
            });
    }
</script>

<div class="space-y-6">
    {#if insights.length > 0}
        <div class="space-y-2">
            {#each insights as ins}
                <div
                    class="rounded-lg p-3 flex items-start gap-3 {ins.severity === 'critical'
                        ? 'bg-red-950 border border-red-800'
                        : ins.severity === 'warning'
                          ? 'bg-yellow-950 border border-yellow-800'
                          : 'bg-blue-950 border border-blue-800'}"
                >
                    <span
                        class="w-2 h-2 mt-1.5 rounded-full flex-shrink-0 {ins.severity === 'critical'
                            ? 'bg-red-400'
                            : ins.severity === 'warning'
                              ? 'bg-yellow-400'
                              : 'bg-blue-400'}"
                    ></span>
                    <div class="flex-1">
                        <p class="text-white text-sm font-medium">{ins.title}</p>
                        <p class="text-gray-400 text-xs">{ins.detail}</p>
                    </div>
                </div>
            {/each}
        </div>
    {/if}

    {#if systemMetrics}
        <div>
            <h2 class="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-3">System</h2>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">CPU</p>
                    <p class="text-2xl font-bold text-white">
                        {systemMetrics.cpu_percent.toFixed(1)}%
                    </p>
                    <div class="mt-2 h-1 bg-gray-700 rounded-full overflow-hidden">
                        <div
                            class="h-full {systemMetrics.cpu_percent > 80
                                ? 'bg-red-500'
                                : systemMetrics.cpu_percent > 50
                                  ? 'bg-yellow-500'
                                  : 'bg-green-500'}"
                            style="width: {systemMetrics.cpu_percent}%"
                        ></div>
                    </div>
                    <p class="text-xs text-gray-500 mt-2">
                        load {systemMetrics.load_avg[0].toFixed(2)}
                        {systemMetrics.load_avg[1].toFixed(2)}
                        {systemMetrics.load_avg[2].toFixed(2)}
                    </p>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Memory</p>
                    <p class="text-2xl font-bold text-white">
                        {systemMetrics.memory_percent.toFixed(0)}%
                    </p>
                    <div class="mt-2 h-1 bg-gray-700 rounded-full overflow-hidden">
                        <div
                            class="h-full {systemMetrics.memory_percent > 80
                                ? 'bg-red-500'
                                : systemMetrics.memory_percent > 50
                                  ? 'bg-yellow-500'
                                  : 'bg-green-500'}"
                            style="width: {systemMetrics.memory_percent}%"
                        ></div>
                    </div>
                    <p class="text-xs text-gray-500 mt-2">
                        {fmtSize(systemMetrics.memory_used_bytes)} / {fmtSize(systemMetrics.memory_total_bytes)}
                    </p>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Disk</p>
                    <p class="text-2xl font-bold text-white">{diskPct.toFixed(0)}%</p>
                    <div class="mt-2 h-1 bg-gray-700 rounded-full overflow-hidden">
                        <div
                            class="h-full {diskPct > 90
                                ? 'bg-red-500'
                                : diskPct > 75
                                  ? 'bg-yellow-500'
                                  : 'bg-green-500'}"
                            style="width: {diskPct}%"
                        ></div>
                    </div>
                    <p class="text-xs text-gray-500 mt-2">
                        {fmtSize(systemMetrics.disk_used_bytes)} / {fmtSize(systemMetrics.disk_total_bytes)}
                    </p>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Uptime</p>
                    <p class="text-2xl font-bold text-white">
                        {fmtUptime(systemMetrics.uptime_seconds)}
                    </p>
                    <p class="text-xs text-gray-500 mt-7">host system</p>
                </div>
            </div>
        </div>
    {/if}

    {#if appMetrics}
        <div>
            <h2 class="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-3">App</h2>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Media</p>
                    <p class="text-2xl font-bold text-white">
                        {appMetrics.media.ready}<span class="text-sm text-gray-500"> / {appMetrics.media.total}</span>
                    </p>
                    <p class="text-xs text-gray-500 mt-2">
                        {appMetrics.media.error} errored
                    </p>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Episodes</p>
                    <p class="text-2xl font-bold text-white">
                        {appMetrics.episodes.ready}<span class="text-sm text-gray-500">
                            / {appMetrics.episodes.total}</span
                        >
                    </p>
                    <p class="text-xs text-gray-500 mt-2">ready / total</p>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Downloads</p>
                    <p class="text-2xl font-bold text-white">
                        {appMetrics.downloads.active}
                    </p>
                    <p class="text-xs text-gray-500 mt-2">
                        active, {appMetrics.downloads.errored} errored
                    </p>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-1">Watch (24h)</p>
                    <p class="text-2xl font-bold text-white">
                        {appMetrics.watch.active_last_24h}
                    </p>
                    <p class="text-xs text-gray-500 mt-2">
                        {appMetrics.watch.completed_records} completed total
                    </p>
                </div>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-3 mt-3">
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-3">
                        Subtitles by language ({appMetrics.subtitles.total} total)
                    </p>
                    <div class="space-y-1.5">
                        {#if appMetrics.subtitles.by_language.length === 0}
                            <p class="text-gray-600 text-xs italic">no subtitles indexed yet</p>
                        {/if}
                        {#each appMetrics.subtitles.by_language.slice(0, 6) as lang}
                            <div class="flex items-center gap-2 text-xs">
                                <span class="text-gray-400 w-12 uppercase">{lang.language}</span>
                                <div class="flex-1 h-2 bg-gray-700 rounded-full overflow-hidden">
                                    <div
                                        class="h-full bg-primary-500"
                                        style="width: {(lang.count / langMax) * 100}%"
                                    ></div>
                                </div>
                                <span class="text-white tabular-nums w-10 text-right">{lang.count}</span>
                            </div>
                        {/each}
                    </div>
                </div>
                <div class="bg-gray-800 rounded-lg p-4">
                    <p class="text-xs text-gray-500 mb-3">Subtitles by source</p>
                    <div class="space-y-1.5">
                        {#if appMetrics.subtitles.by_source.length === 0}
                            <p class="text-gray-600 text-xs italic">no sources tracked yet</p>
                        {/if}
                        {#each appMetrics.subtitles.by_source.slice(0, 6) as src}
                            <div class="flex items-center gap-2 text-xs">
                                <span class="text-gray-400 w-28 truncate">{src.source}</span>
                                <div class="flex-1 h-2 bg-gray-700 rounded-full overflow-hidden">
                                    <div
                                        class="h-full bg-primary-500"
                                        style="width: {(src.count / srcMax) * 100}%"
                                    ></div>
                                </div>
                                <span class="text-white tabular-nums w-10 text-right">{src.count}</span>
                            </div>
                        {/each}
                    </div>
                </div>
            </div>
        </div>
    {/if}

    <div>
        <div class="flex items-center justify-between mb-3">
            <h2 class="text-sm font-semibold text-gray-400 uppercase tracking-wider">External Services</h2>
            {#if healthLoading}
                <span class="text-xs text-gray-500">checking...</span>
            {:else if healthCheckedAt > 0}
                <span class="text-xs text-gray-500">
                    checked {Math.round(Date.now() / 1000 - healthCheckedAt)}s ago
                </span>
            {/if}
        </div>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
            {#each serviceHealth as svc}
                <div class="bg-gray-800 rounded-lg p-3 flex items-center justify-between">
                    <div class="flex items-center gap-2">
                        <span class="w-2 h-2 rounded-full {svc.ok ? 'bg-green-400' : 'bg-red-400'}"></span>
                        <span class="text-white text-sm">{svc.name}</span>
                    </div>
                    <div class="text-xs text-gray-400">
                        {#if !svc.ok}{svc.detail ?? 'down'}
                        {:else if svc.latency_ms !== null}{svc.latency_ms}ms
                        {/if}
                    </div>
                </div>
            {/each}
        </div>
    </div>

    <button onclick={refreshDashboard} class="text-xs text-primary-400 hover:text-primary-300"> refresh now </button>
</div>
