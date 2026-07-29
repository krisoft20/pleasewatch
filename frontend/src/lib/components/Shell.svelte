<script lang="ts">
    let { label = 'login' }: { label?: string } = $props();
    let now = $state(new Date());

    $effect(() => {
        const id = setInterval(() => (now = new Date()), 1000);
        return () => clearInterval(id);
    });

    const t = $derived(now.toTimeString().slice(0, 8));
</script>

<div class="pw-mesh"></div>
<div class="pw-grain"></div>
<div class="pw-vignette"></div>

<div class="pw-corner"><span>///</span> {label}</div>

<div class="pw-meta">
    <span class="pw-pulse"></span>
    <em>srv</em><s>·</s><em>v1.0.1</em><s>·</s><em>{t}</em>
</div>
