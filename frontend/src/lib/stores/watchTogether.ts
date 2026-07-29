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
};

export const watchTogether = writable<WatchTogetherState>({
    active: false,
    code: '',
    participants: 0,
    isHost: false,
    synced: false,
    syncedTime: 0,
    messages: []
});

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
        messages: []
    }));

    sock.onopen = () => {
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
                    syncedTime: typeof msg.time === 'number' ? msg.time : s.syncedTime
                }));
                break;
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
        messages: []
    });
}

function sendRaw(payload: any) {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    try {
        ws.send(JSON.stringify(payload));
    } catch {}
}

export const sendPlay = (t: number) => sendRaw({ type: 'Play', time: t });
export const sendPause = (t: number) => sendRaw({ type: 'Pause', time: t });
export const sendSeek = (t: number) => sendRaw({ type: 'Seek', time: t });
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
