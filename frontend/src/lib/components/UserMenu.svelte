<script lang="ts">
    import { goto } from '$app/navigation';
    import { api, type User } from '$lib/api';
    import { i18n, t, LANGS } from '$lib/i18n.svelte';
    import Flag from '$lib/components/Flag.svelte';

    let { user, onclose }: { user: User; onclose: () => void } = $props();

    const isAdmin = $derived(user.role === 'admin');

    function nav(path: string) {
        onclose();
        goto(path);
    }

    async function logout() {
        try {
            await api.logout();
        } catch {}
        goto('/login', { replaceState: true });
    }
</script>

<div class="pw-menu-id">
    <div class="pw-menu-id-name">
        <span class="pw-menu-id-username">{user.username}</span>
        {#if isAdmin}<span class="pw-admin-badge">Admin</span>{/if}
    </div>
    <div class="pw-menu-id-email">{user.email}</div>
</div>

<div class="pw-menu-sep"></div>

<div class="pw-menu-rows">
    <button class="pw-menu-row" onclick={() => nav('/')}>
        <svg
            class="pw-menu-row-ico"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M3 11l9-8 9 8" /><path d="M5 10v10h14V10" />
        </svg>
        {t('menu.home')}
    </button>
    {#if isAdmin}
        <button class="pw-menu-row" onclick={() => nav('/admin')}>
            <svg
                class="pw-menu-row-ico"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <circle cx="12" cy="12" r="3" />
                <path
                    d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09A1.65 1.65 0 0 0 15 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.36.16.66.42.88.74.22.32.34.7.34 1.09 0 .39-.12.77-.34 1.09-.22.32-.52.58-.88.74z"
                />
            </svg>
            {t('menu.admin')}
        </button>
    {/if}
    <button class="pw-menu-row" onclick={() => nav('/about')}>
        <svg
            class="pw-menu-row-ico"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <circle cx="12" cy="12" r="9" />
            <path d="M12 11v6M12 7h.01" />
        </svg>
        {t('menu.about')}
    </button>
</div>

<div class="pw-menu-sep"></div>

<div class="pw-menu-lang">
    <span class="pw-menu-lang-label">{t('menu.language')}</span>
    <div class="pw-menu-lang-chips">
        {#each LANGS as l (l.code)}
            <button
                class="pw-menu-lang-chip"
                class:is-active={i18n.lang === l.code}
                onclick={() => {
                    i18n.set(l.code);
                    location.reload();
                }}
                aria-label={l.name}
                type="button"
            >
                <Flag code={l.country} />
                <span class="pw-menu-lang-code">{l.code}</span>
                <span class="pw-menu-lang-name">{l.name}</span>
            </button>
        {/each}
    </div>
</div>

<div class="pw-menu-sep"></div>

<div class="pw-menu-rows">
    <button class="pw-menu-row pw-menu-row-danger" onclick={logout}>
        <svg
            class="pw-menu-row-ico"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
            <polyline points="16 17 21 12 16 7" />
            <line x1="21" y1="12" x2="9" y2="12" />
        </svg>
        {t('menu.logout')}
    </button>
</div>
