import { api, type Share, type ShareKind } from '$lib/api';

const POLL_MS = 45_000;

export function sharePath(kind: ShareKind, key: string) {
    return `/${kind}/${encodeURIComponent(key)}`;
}

class Inbox {
    items = $state<Share[]>([]);
    unread = $state(0);
    fresh = $state<Set<string>>(new Set());
    private timer: ReturnType<typeof setInterval> | undefined;
    private users = 0;

    async refresh() {
        try {
            const r = await api.shares();
            this.items = r.items;
            this.unread = r.unread;
        } catch {}
    }

    async markSeen() {
        this.fresh = new Set(this.items.filter((s) => !s.seen).map((s) => s.id));
        if (this.unread === 0) return;
        this.unread = 0;
        await api.sharesSeen().catch(() => {});
    }

    start() {
        this.users++;
        if (this.timer) return () => this.stop();
        this.refresh();
        this.timer = setInterval(() => {
            if (document.visibilityState === 'visible') this.refresh();
        }, POLL_MS);
        return () => this.stop();
    }

    private stop() {
        this.users--;
        if (this.users > 0 || !this.timer) return;
        clearInterval(this.timer);
        this.timer = undefined;
    }
}

export const inbox = new Inbox();
