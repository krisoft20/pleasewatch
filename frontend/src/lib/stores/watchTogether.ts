import { writable, get } from 'svelte/store';

export type ChatMessage = {
    user: string;
    name: string;
    text: string;
    message: string;
    at: number;
    time: number;
};

export type WatchTogetherState = {
    active: boolean;
    code: string;
    participants: number;
    isHost: boolean;
    synced: boolean;
    syncedTime: number;
    messages: ChatMessage[];
    wait: boolean;
    waiting: string[];
};

export const watchTogether = writable<WatchTogetherState>({
    active: false,
    code: '',
    participants: 0,
    isHost: false,
    synced: false,
    syncedTime: 0,
    messages: [],
    wait: false,
    waiting: []
});

let offset = 0;
let samples: { rtt: number; off: number }[] = [];
let server: { playing: boolean; time: number; at: number } | null = null;
let quietUntil = 0;
let pinger: ReturnType<typeof setInterval> | null = null;

function onPong(t: number, srv: number) {
    const now = Date.now();
    const rtt = now - t;
    if (rtt < 0 || rtt > 5000) return;
    samples = [...samples, { rtt, off: srv + rtt / 2 - now }].slice(-10);
    offset = samples.reduce((a, b) => (b.rtt < a.rtt ? b : a)).off;
}

export function expectedPosition(): { time: number; playing: boolean } | null {
    if (!server || samples.length === 0 || Date.now() < quietUntil) return null;
    if (!server.playing) return { time: server.time, playing: false };
    const elapsed = (Date.now() + offset - server.at) / 1000;
    return { time: server.time + Math.max(0, elapsed), playing: true };
}

type RemoteCallbacks = {
    onPlay?: (t: number) => void;
    onPause?: (t: number) => void;
    onSeek?: (t: number) => void;
    onEpisodeSwitch?: (episodeId: string) => void;
};

let cbs: RemoteCallbacks = {};
let onChat: ((m: ChatMessage) => void) | null = null;
let ws: WebSocket | null = null;
let myName = '';

export const setRemoteCallbacks = (c: RemoteCallbacks) => {
    cbs = { ...cbs, ...c };
};
export const setOnChatMessage = (fn: (m: ChatMessage) => void) => {
    onChat = fn;
};

export function joinSession(code: string, name: string, isHost = false, initialTime = 0) {
    if (ws) {
        try {
            ws.close();
        } catch {}
        ws = null;
    }
    myName = name || 'Guest';
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const url = `${proto}//${location.host}/api/ws/party/${code}?name=${encodeURIComponent(myName)}`;
    const sock = new WebSocket(url);
    ws = sock;

    watchTogether.update((s) => ({
        ...s,
        active: true,
        code,
        isHost,
        participants: 0,
        synced: isHost,
        syncedTime: isHost ? initialTime : 0,
        messages: [],
        wait: false,
        waiting: []
    }));
    samples = [];
    server = null;

    sock.onopen = () => {
        const ping = () => sendRaw({ type: 'Ping', t: Date.now() });
        for (let i = 0; i < 5; i++) setTimeout(ping, i * 300);
        if (pinger) clearInterval(pinger);
        pinger = setInterval(ping, 10000);
        if (isHost && initialTime > 0) {
            try {
                sock.send(JSON.stringify({ type: 'Seek', time: initialTime }));
            } catch {}
        }
    };

    sock.onmessage = (ev) => {
        let msg: any;
        try {
            msg = JSON.parse(ev.data);
        } catch {
            return;
        }
        switch (msg.type) {
            case 'Play':
                cbs.onPlay?.(msg.time);
                break;
            case 'Pause':
                cbs.onPause?.(msg.time);
                break;
            case 'Seek':
                cbs.onSeek?.(msg.time);
                break;
            case 'Chat': {
                const m: ChatMessage = {
                    user: msg.name,
                    name: msg.name,
                    text: msg.message,
                    message: msg.message,
                    at: Date.now(),
                    time: Date.now()
                };
                watchTogether.update((s) => ({ ...s, messages: [...s.messages, m].slice(-200) }));
                onChat?.(m);
                break;
            }
            case 'StateSync':
                watchTogether.update((s) => ({
                    ...s,
                    participants: msg.participants ?? s.participants,
                    synced: true,
                    syncedTime: typeof msg.time === 'number' ? msg.time : s.syncedTime,
                    wait: !!msg.wait
                }));
                break;
            case 'Pong':
                onPong(msg.t, msg.server);
                break;
            case 'Tick': {
                server = { playing: msg.playing, time: msg.time, at: msg.at };
                const names: string[] = msg.waiting ?? [];
                watchTogether.update((s) =>
                    s.wait === msg.wait && s.waiting.join() === names.join()
                        ? s
                        : { ...s, wait: msg.wait, waiting: names }
                );
                break;
            }
            case 'UserJoined':
            case 'UserLeft':
                watchTogether.update((s) => ({ ...s, participants: msg.participants ?? s.participants }));
                break;
            case 'EpisodeSwitch':
                cbs.onEpisodeSwitch?.(msg.episode_id);
                break;
        }
    };

    sock.onclose = () => {
        if (ws === sock) ws = null;
        if (pinger) clearInterval(pinger);
        pinger = null;
        server = null;
        watchTogether.update((s) => ({ ...s, active: false }));
    };
}

export function leaveSession() {
    if (ws) {
        try {
            ws.close();
        } catch {}
        ws = null;
    }
    watchTogether.set({
        active: false,
        code: '',
        participants: 0,
        isHost: false,
        synced: false,
        syncedTime: 0,
        messages: [],
        wait: false,
        waiting: []
    });
}

function sendRaw(payload: any) {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    try {
        ws.send(JSON.stringify(payload));
    } catch {}
}

function local(payload: any) {
    quietUntil = Date.now() + 1500;
    sendRaw(payload);
}

export const sendPlay = (t: number) => local({ type: 'Play', time: t });
export const sendPause = (t: number) => local({ type: 'Pause', time: t });
export const sendSeek = (t: number) => local({ type: 'Seek', time: t });
export const sendBuffering = (on: boolean) => sendRaw({ type: 'Buffering', on });
export const sendWaitMode = (on: boolean) => sendRaw({ type: 'WaitMode', on });
export const sendChat = (msg: string) => {
    if (!msg.trim()) return;
    sendRaw({ type: 'Chat', name: myName, message: msg.trim() });
};

export async function createSession(mediaId: string, episodeId?: string): Promise<string> {
    const r = await fetch('/api/party', {
        method: 'POST',
        credentials: 'include',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ media_id: mediaId, episode_id: episodeId ?? null })
    });
    if (!r.ok) throw new Error('party create failed');
    const j = await r.json();
    void get(watchTogether);
    return j.code as string;
}
