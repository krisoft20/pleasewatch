<script lang="ts" module>
    export type IconName =
        | 'check'
        | 'check-alt'
        | 'chevron-right'
        | 'chevron-right-sm'
        | 'chevron-right-bold'
        | 'chevron-left'
        | 'chevron-down'
        | 'close'
        | 'close-bold'
        | 'play'
        | 'pause'
        | 'next-track'
        | 'skip-fast'
        | 'volume-off'
        | 'volume-low'
        | 'volume-high'
        | 'subs'
        | 'chat-square'
        | 'chat-round'
        | 'chat-alt'
        | 'gear'
        | 'sync'
        | 'spinner'
        | 'bolt'
        | 'search'
        | 'send'
        | 'trash'
        | 'plus'
        | 'fullscreen-enter'
        | 'fullscreen-exit'
        | 'sliders'
        | 'users'
        | 'drag-handle'
        | 'logout-right'
        | 'text-size'
        | 'color-bucket'
        | 'square'
        | 'clock'
        | 'arrows-vertical'
        | 'external-link'
        | 'copy'
        | 'eye'
        | 'eye-off'
        | 'trophy'
        | 'episodes'
        | 'screen-text'
        | 'list-lines';
</script>

<script lang="ts">
    type Props = {
        name: IconName;
        class?: string;
        strokeWidth?: number;
    };
    let { name, class: klass = 'w-5 h-5', strokeWidth = 1.5 }: Props = $props();

    const FILL: Partial<Record<IconName, string>> = {
        check: 'M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z',
        play: 'M8 5v14l11-7z',
        pause: 'M6 19h4V5H6v14zm8-14v14h4V5h-4z',
        'next-track': 'M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z',
        'skip-fast':
            'M11.933 12.8a1 1 0 000-1.6L6.6 7.2A1 1 0 005 8v8a1 1 0 001.6.8l5.333-4zM19.933 12.8a1 1 0 000-1.6l-5.333-4A1 1 0 0013 8v8a1 1 0 001.6.8l5.333-4z',
        bolt: 'M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z',
        spinner: 'M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z',
        'volume-off':
            'M16.5 12c0-1.77-1.02-3.29-2.5-4.03v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51C20.63 14.91 21 13.5 21 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3L3 4.27 7.73 9H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06c1.38-.31 2.63-.95 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4L9.91 6.09 12 8.18V4z',
        'volume-low': 'M18.5 12c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM5 9v6h4l5 5V4L9 9H5z',
        'volume-high':
            'M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z'
    };
    const STROKE: Partial<Record<IconName, string>> = {
        'check-alt': 'M5 13l4 4L19 7',
        'chevron-right': 'M9 5l7 7-7 7',
        'chevron-right-sm': 'M9 18l6-6-6-6',
        'chevron-right-bold': 'M15 18l6-6-6-6',
        'chevron-left': 'M15 19l-7-7 7-7',
        'chevron-down': 'M19 9l-7 7-7-7',
        close: 'M6 18L18 6M6 6l12 12',
        'close-bold':
            'M14.121 14.121L7.05 21.192m0-14.142l7.071 7.07m0 0L21.192 7.05M14.121 14.121L7.05 7.05M9.879 9.879a3 3 0 11-4.243-4.243 3 3 0 014.243 4.243zm0 4.242a3 3 0 11-4.243 4.243 3 3 0 014.243-4.243z',
        'chat-round':
            'M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z',
        'chat-alt': 'M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z',
        sync: 'M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15',
        search: 'M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z',
        send: 'M2.01 21L23 12 2.01 3 2 10l15 2-15 2z',
        trash: 'M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16',
        plus: 'M12 4v16m8-8H4',
        'fullscreen-enter': 'M8 3v3a2 2 0 01-2 2H3m18 0h-3a2 2 0 01-2-2V3m0 18v-3a2 2 0 012-2h3M3 16h3a2 2 0 012 2v3',
        'fullscreen-exit': 'M8 3H5a2 2 0 00-2 2v3m18 0V5a2 2 0 00-2-2h-3m0 18h3a2 2 0 002-2v-3M3 16v3a2 2 0 002 2h3',
        sliders:
            'M10.5 6h9.75M10.5 6a1.5 1.5 0 11-3 0m3 0a1.5 1.5 0 10-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 01-3 0m3 0a1.5 1.5 0 00-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 01-3 0m3 0a1.5 1.5 0 00-3 0m-9.75 0h9.75',
        users: 'M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z',
        'drag-handle': 'M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4',
        'logout-right': 'M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1',
        'text-size': 'M4 7V4h16v3M9 20h6M12 4v16',
        'color-bucket': 'M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4z',
        square: 'M4 4h16v16H4z',
        clock: 'M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z',
        'arrows-vertical': 'M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4',
        'external-link': 'M14 5h5v5M19 5l-9 9M9 5H5v14h14v-4',
        copy: 'M9 9h11v11H9zM5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1',
        eye: 'M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z M12 9a3 3 0 110 6 3 3 0 010-6z',
        'eye-off':
            'M9.88 9.88a3 3 0 104.24 4.24 M10.73 5.08A10.43 10.43 0 0112 5c7 0 10 7 10 7a13.16 13.16 0 01-1.67 2.68 M6.61 6.61A13.53 13.53 0 002 12s3 7 10 7a9.74 9.74 0 005.39-1.61 M2 2l20 20',
        trophy: 'M8 21h8M12 17v4M7 4h10v5c0 4.2-2 6.8-5 7.7C9 15.8 7 13.2 7 9V4zM7 7H4c.1 3.2 1.5 5.1 3.5 5.7M17 7h3c-.1 3.2-1.5 5.1-3.5 5.7',
        episodes:
            'M3 5a1 1 0 011-1h5a1 1 0 011 1v3a1 1 0 01-1 1H4a1 1 0 01-1-1V5zm11 0a1 1 0 011-1h5a1 1 0 011 1v3a1 1 0 01-1 1h-5a1 1 0 01-1-1V5zM3 14a1 1 0 011-1h5a1 1 0 011 1v3a1 1 0 01-1 1H4a1 1 0 01-1-1v-3zm11 0a1 1 0 011-1h5a1 1 0 011 1v3a1 1 0 01-1 1h-5a1 1 0 01-1-1v-3z',
        'screen-text': 'M5 5h14a2 2 0 012 2v10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2zM8 10h8M8 14h5',
        'list-lines': 'M3 6h18M3 12h18M3 18h18'
    };
    const fill = $derived(FILL[name]);
    const stroke = $derived(STROKE[name]);
</script>

{#if name === 'subs'}
    <svg
        class={klass}
        fill="none"
        stroke="currentColor"
        stroke-width={strokeWidth}
        stroke-linecap="round"
        stroke-linejoin="round"
        viewBox="0 0 24 24"
    >
        <path d="M4 5h16a1 1 0 011 1v10a1 1 0 01-1 1h-3l-3 3-3-3H4a1 1 0 01-1-1V6a1 1 0 011-1z" />
        <path d="M7 9.5h10M7 13h6" />
    </svg>
{:else if name === 'chat-square'}
    <svg
        class={klass}
        fill="none"
        stroke="currentColor"
        stroke-width={strokeWidth}
        stroke-linecap="round"
        stroke-linejoin="round"
        viewBox="0 0 24 24"
    >
        <path d="M4 5h16a1 1 0 011 1v10a1 1 0 01-1 1h-3l-3 3-3-3H4a1 1 0 01-1-1V6a1 1 0 011-1z" />
    </svg>
{:else if name === 'gear'}
    <svg
        class={klass}
        fill="none"
        stroke="currentColor"
        stroke-width={strokeWidth}
        stroke-linecap="round"
        stroke-linejoin="round"
        viewBox="0 0 24 24"
    >
        <path
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
        />
        <circle cx="12" cy="12" r="3" />
    </svg>
{:else if fill}
    <svg class={klass} fill="currentColor" viewBox="0 0 24 24"><path d={fill} /></svg>
{:else if stroke}
    <svg
        class={klass}
        fill="none"
        stroke="currentColor"
        stroke-width={strokeWidth}
        stroke-linecap="round"
        stroke-linejoin="round"
        viewBox="0 0 24 24"><path d={stroke} /></svg
    >
{/if}
