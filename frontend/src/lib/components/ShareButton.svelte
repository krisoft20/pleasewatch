<script lang="ts">
    import type { ShareKind } from '$lib/api';
    import { t } from '$lib/i18n.svelte';
    import ShareSheet from '$lib/components/ShareSheet.svelte';

    let {
        kind,
        key,
        title,
        cover = null
    }: { kind: ShareKind; key: string; title: string; cover?: string | null } = $props();

    let open = $state(false);
</script>

<button class="pw-v1-btn-lib" onclick={() => (open = true)}>
    <svg
        width="13"
        height="13"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        ><circle cx="18" cy="5" r="3" /><circle cx="6" cy="12" r="3" /><circle cx="18" cy="19" r="3" /><line
            x1="8.59"
            y1="13.51"
            x2="15.42"
            y2="17.49"
        /><line x1="15.41" y1="6.51" x2="8.59" y2="10.49" /></svg
    >
    {t('share.button')}
</button>

{#if open}
    <ShareSheet {kind} {key} {title} {cover} onclose={() => (open = false)} />
{/if}
