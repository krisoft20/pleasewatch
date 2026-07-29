export type CountSetter = (count: number | null) => void;

export function fmtUptime(secs: number): string {
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
}

export function fmtWatchTime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    const rm = mins % 60;
    if (hours < 24) return `${hours}h ${rm}m`;
    const days = Math.floor(hours / 24);
    const rh = hours % 24;
    return `${days}d ${rh}h`;
}

export function fmtSize(bytes: number | null): string {
    if (!bytes || bytes === 0) return '-';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(0) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}

export function fmtBytes(bytes: number | null): string {
    if (bytes === null) return 'unavailable';
    if (bytes === 0) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes < 1024 * 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    return `${(bytes / (1024 * 1024 * 1024 * 1024)).toFixed(2)} TB`;
}

export function fmtPct(p: number): string {
    return `${(p * 100).toFixed(1)}%`;
}

export function fmtDate(d: string): string {
    return new Date(d).toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
    });
}

export function storageColor(key: string): string {
    const colors: Record<string, string> = {
        movies: '#5b8def',
        series: '#8b7cf6',
        anime: '#e66fa7',
        _dl: '#f59e0b',
        clips: '#39b98a',
        thumbnails: '#45a9c9',
        books: '#7c91c7',
        manga: '#d66d6d',
        video: '#5b8def',
        audio: '#8b7cf6',
        subtitles: '#39b98a',
        artwork: '#45a9c9',
        other: '#6b7280'
    };
    return colors[key] ?? colors.other;
}

export function tagColor(tag: string): string {
    let h = 0;
    for (const c of tag) h = ((h << 5) - h + c.charCodeAt(0)) | 0;
    const hue = Math.abs(h) % 360;
    return `oklch(0.78 0.18 ${hue})`;
}
