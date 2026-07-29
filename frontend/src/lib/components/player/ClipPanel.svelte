<script lang="ts">
    import Icon from '../Icon.svelte';

    let {
        clipMode,
        clipStart = $bindable(0),
        clipEnd = $bindable(0),
        currentTime,
        duration,
        clipCreating,
        clipShowResult,
        fmt,
        onCopyLink,
        onExit,
        onCreate
    }: {
        clipMode: boolean;
        clipStart: number;
        clipEnd: number;
        currentTime: number;
        duration: number;
        clipCreating: boolean;
        clipShowResult: boolean;
        fmt: (time: number) => string;
        onCopyLink: () => void;
        onExit: () => void;
        onCreate: () => void;
    } = $props();

    const dur = $derived(Math.max(0, clipEnd - clipStart));
</script>

{#if clipMode}
    <div class="absolute top-20 left-0 right-0 z-[55] flex justify-center px-4">
        {#if clipShowResult}
            <div class="pw-clip-card">
                <button class="pw-clip-btn pw-clip-btn-primary" onclick={onCopyLink}>copy link</button>
                <button class="pw-clip-btn pw-clip-btn-ghost" onclick={onExit}>close</button>
            </div>
        {:else}
            <div class="pw-clip-card pw-clip-trim">
                <div class="pw-clip-group">
                    <span class="pw-clip-label">from</span>
                    <div class="pw-clip-stepper pw-clip-stepper-start">
                        <button
                            class="pw-clip-step"
                            onclick={() => {
                                clipStart = Math.max(0, clipStart - 1);
                            }}>-</button
                        >
                        <button
                            class="pw-clip-time pw-clip-time-start"
                            onclick={() => {
                                clipStart = currentTime;
                            }}
                            title="set from current position">{fmt(clipStart)}</button
                        >
                        <button
                            class="pw-clip-step"
                            onclick={() => {
                                clipStart = Math.min(clipEnd - 1, clipStart + 1);
                            }}>+</button
                        >
                    </div>
                </div>
                <div class="pw-clip-arrow">
                    <span>-></span>
                    <span class="pw-clip-dur">{dur.toFixed(1)}s</span>
                </div>
                <div class="pw-clip-group">
                    <span class="pw-clip-label">to</span>
                    <div class="pw-clip-stepper pw-clip-stepper-end">
                        <button
                            class="pw-clip-step"
                            onclick={() => {
                                clipEnd = Math.max(clipStart + 1, clipEnd - 1);
                            }}>-</button
                        >
                        <button
                            class="pw-clip-time pw-clip-time-end"
                            onclick={() => {
                                clipEnd = currentTime;
                            }}
                            title="set from current position">{fmt(clipEnd)}</button
                        >
                        <button
                            class="pw-clip-step"
                            onclick={() => {
                                clipEnd = Math.min(duration, clipEnd + 1);
                            }}>+</button
                        >
                    </div>
                </div>
                <div class="pw-clip-divider"></div>
                <button class="pw-clip-action pw-clip-cancel" onclick={onExit} aria-label="cancel">
                    <Icon name="close" class="w-5 h-5" />
                </button>
                <button
                    class="pw-clip-action pw-clip-create"
                    onclick={onCreate}
                    disabled={clipCreating || clipEnd <= clipStart}
                    aria-label="create clip"
                >
                    {#if clipCreating}
                        <div class="pw-clip-spin"></div>
                    {:else}
                        <Icon name="check-alt" class="w-4 h-4" strokeWidth={2.6} />
                    {/if}
                </button>
            </div>
        {/if}
    </div>
{/if}
