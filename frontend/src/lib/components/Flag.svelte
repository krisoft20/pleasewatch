<script lang="ts">
    let { code }: { code: string } = $props();
</script>

{#if code === 'GB'}
    <svg class="pw-flag" viewBox="0 0 60 30" preserveAspectRatio="none" aria-hidden="true">
        <rect width="60" height="30" fill="#012169" />
        <path d="M0,0 L60,30 M60,0 L0,30" stroke="#fff" stroke-width="6" />
        <path d="M0,0 L60,30 M60,0 L0,30" stroke="#C8102E" stroke-width="2" />
        <path d="M30,0 v30 M0,15 h60" stroke="#fff" stroke-width="10" />
        <path d="M30,0 v30 M0,15 h60" stroke="#C8102E" stroke-width="6" />
    </svg>
{:else if code === 'PL'}
    <svg class="pw-flag" viewBox="0 0 5 3" preserveAspectRatio="none" aria-hidden="true">
        <rect width="5" height="1.5" fill="#fff" />
        <rect y="1.5" width="5" height="1.5" fill="#dc143c" />
    </svg>
{:else if code === 'DE'}
    <svg class="pw-flag" viewBox="0 0 5 3" preserveAspectRatio="none" aria-hidden="true">
        <rect width="5" height="1" fill="#000" />
        <rect y="1" width="5" height="1" fill="#dd0000" />
        <rect y="2" width="5" height="1" fill="#ffce00" />
    </svg>
{/if}
