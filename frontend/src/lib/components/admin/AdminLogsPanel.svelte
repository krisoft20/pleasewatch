<script lang="ts">
    import { onDestroy, onMount } from 'svelte';
    import { tagColor, type CountSetter } from './adminUtils';

    type Props = {
        onCount?: CountSetter;
    };

    type LogEntry = {
        seq: number;
        ts: string;
        level: string;
        tag: string;
        msg: string;
    };

    let { onCount }: Props = $props();

    let logs = $state<LogEntry[]>([]);
    let logsWs: WebSocket | null = null;
    let logsAutoscroll = $state(true);
    let logsPaused = $state(false);
    let logsFilter = $state('');
    let logsTagFilter = $state<string | null>(null);
    let logsViewport: HTMLDivElement | null = $state(null);
    let mounted = true;

    const LOGS_KEEP = 4000;

    onMount(() => {
        mounted = true;
        onCount?.(null);
        connectLogsWs();
    });

    onDestroy(() => {
        mounted = false;
        disconnectLogsWs();
    });

    function connectLogsWs() {
        if (logsWs && logsWs.readyState !== WebSocket.CLOSED) return;
        const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const sock = new WebSocket(`${proto}//${location.host}/ws/admin/logs`);
        logsWs = sock;
        const lastSeqOnConnect = logs.length > 0 ? logs[logs.length - 1].seq : 0;

        sock.onmessage = (ev) => {
            if (logsPaused) return;
            try {
                const batch: LogEntry[] = JSON.parse(ev.data);
                if (!Array.isArray(batch) || batch.length === 0) return;
                const fresh = batch.filter((e) => e.seq > lastSeqOnConnect || !logs.some((l) => l.seq === e.seq));
                if (fresh.length === 0) return;
                const next = logs.concat(fresh);
                if (next.length > LOGS_KEEP) next.splice(0, next.length - LOGS_KEEP);
                logs = next;
                if (logsAutoscroll) {
                    queueMicrotask(() => {
                        if (logsViewport) logsViewport.scrollTop = logsViewport.scrollHeight;
                    });
                }
            } catch {}
        };

        sock.onclose = () => {
            if (logsWs !== sock) return;
            logsWs = null;
            if (mounted) setTimeout(connectLogsWs, 2000);
        };

        sock.onerror = () => {
            try {
                sock.close();
            } catch {}
        };
    }

    function disconnectLogsWs() {
        const sock = logsWs;
        logsWs = null;
        try {
            sock?.close();
        } catch {}
    }

    function clearLogs() {
        logs = [];
    }

    const logTags = $derived.by(() => {
        const seen = new Set<string>();
        for (const l of logs) seen.add(l.tag);
        return Array.from(seen).sort();
    });

    const visibleLogs = $derived.by(() => {
        const q = logsFilter.trim().toLowerCase();
        return logs.filter((l) => {
            if (logsTagFilter && l.tag !== logsTagFilter) return false;
            if (q && !l.msg.toLowerCase().includes(q) && !l.tag.toLowerCase().includes(q)) return false;
            return true;
        });
    });

    $effect(() => {
        if (!logsViewport) return;
        const el = logsViewport;
        queueMicrotask(() => {
            el.scrollTop = el.scrollHeight;
        });
    });
</script>

<div class="space-y-3">
    <div class="flex items-center gap-2 flex-wrap">
        <input
            bind:value={logsFilter}
            placeholder="search msg or tag..."
            class="
                flex-1 min-w-[140px] bg-gray-900 border border-gray-700 rounded px-3 py-1.5 text-sm text-white
                outline-none focus:border-primary-500 font-mono
            "
            autocomplete="off"
        />
        <button
            onclick={() => (logsPaused = !logsPaused)}
            class="px-3 py-1.5 rounded text-xs font-medium {logsPaused
                ? 'bg-red-500/20 text-red-300 border border-red-500/40'
                : 'bg-gray-800 text-gray-300 border border-gray-700'}"
        >
            {logsPaused ? 'paused' : 'live'}
        </button>
        <button
            onclick={() => (logsAutoscroll = !logsAutoscroll)}
            class="px-3 py-1.5 rounded text-xs font-medium {logsAutoscroll
                ? 'bg-primary-500/20 text-primary-300 border border-primary-500/40'
                : 'bg-gray-800 text-gray-300 border border-gray-700'}"
        >
            autoscroll
        </button>
        <button
            onclick={clearLogs}
            class="px-3 py-1.5 rounded text-xs font-medium bg-gray-800 text-gray-300 border border-gray-700 hover:bg-gray-700"
        >
            clear
        </button>
    </div>

    <div class="flex items-center gap-1.5 flex-wrap text-xs">
        <button
            onclick={() => (logsTagFilter = null)}
            class="px-2 py-0.5 rounded {logsTagFilter === null
                ? 'bg-white/15 text-white'
                : 'bg-gray-800/60 text-gray-400 hover:text-gray-200'}"
        >
            all <span class="opacity-60">({logs.length})</span>
        </button>
        {#each logTags as tag}
            {@const n = logs.filter((l) => l.tag === tag).length}
            <button
                onclick={() => (logsTagFilter = tag === logsTagFilter ? null : tag)}
                class="px-2 py-0.5 rounded font-mono {logsTagFilter === tag
                    ? 'bg-white/15 text-white'
                    : 'bg-gray-800/60 hover:bg-gray-800'}"
                style:color={logsTagFilter === tag ? '' : tagColor(tag)}
            >
                {tag} <span class="opacity-60">({n})</span>
            </button>
        {/each}
    </div>

    <div
        bind:this={logsViewport}
        class="bg-black/60 border border-gray-800 rounded-lg overflow-auto font-mono text-[12px] leading-relaxed p-3"
        style="height: calc(100vh - 280px); min-height: 320px;"
    >
        {#if visibleLogs.length === 0}
            <div class="text-gray-600 italic">{logs.length === 0 ? 'connecting...' : 'no entries match filter'}</div>
        {:else}
            {#each visibleLogs as l (l.seq)}
                <div class="flex gap-2 hover:bg-white/[0.02] -mx-3 px-3">
                    <span class="text-gray-600 select-none flex-shrink-0">{l.ts}</span>
                    <span class="flex-shrink-0 select-none" style:color={tagColor(l.tag)}>[{l.tag}]</span>
                    <span class={l.level === 'error' ? 'text-red-300' : 'text-gray-200'}>{l.msg}</span>
                </div>
            {/each}
        {/if}
    </div>
</div>
