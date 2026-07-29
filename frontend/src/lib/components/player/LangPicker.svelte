<script lang="ts">
    type Option = { code: string; name: string };

    let {
        value = $bindable(),
        options,
        priorityCodes = ['en', 'pl', 'de'],
        onChange = (_c: string) => {},
        size = 'lg'
    }: {
        value: string;
        options: Option[];
        priorityCodes?: string[];
        onChange?: (code: string) => void;
        size?: 'sm' | 'lg';
    } = $props();

    let open = $state(false);
    let btnEl: HTMLButtonElement | undefined = $state();
    let panelEl: HTMLDivElement | undefined = $state();
    let pos = $state({ top: 0, left: 0, width: 0, openUp: false });

    const ordered = $derived.by(() => {
        const priority = priorityCodes.map((c) => options.find((o) => o.code === c)).filter((o): o is Option => !!o);
        const priorityCodesSet = new Set(priorityCodes);
        const rest = options.filter((o) => !priorityCodesSet.has(o.code));
        return { priority, rest };
    });

    const current = $derived(options.find((o) => o.code === value));

    function pick(code: string) {
        value = code;
        open = false;
        onChange(code);
    }

    function recalcPos() {
        if (!btnEl) return;
        const r = btnEl.getBoundingClientRect();
        const PANEL_H_EST = Math.min(288, (options.length + 1) * 38);
        const spaceBelow = window.innerHeight - r.bottom - 8;
        const openUp = spaceBelow < PANEL_H_EST && r.top > spaceBelow;
        pos = {
            top: openUp ? r.top - 4 : r.bottom + 4,
            left: r.left,
            width: r.width,
            openUp
        };
    }

    function onDocClick(e: MouseEvent) {
        const t = e.target as Node;
        if (btnEl?.contains(t)) return;
        if (panelEl?.contains(t)) return;
        open = false;
    }

    $effect(() => {
        if (!open) return;
        recalcPos();
        document.addEventListener('click', onDocClick, { capture: true });
        window.addEventListener('resize', recalcPos);
        window.addEventListener('scroll', recalcPos, { capture: true });
        return () => {
            document.removeEventListener('click', onDocClick, { capture: true });
            window.removeEventListener('resize', recalcPos);
            window.removeEventListener('scroll', recalcPos, { capture: true });
        };
    });

    function portal(node: HTMLElement) {
        const fullscreenRoot =
            document.fullscreenElement ?? (document as Document & { webkitFullscreenElement?: Element }).webkitFullscreenElement;
        const target = fullscreenRoot instanceof HTMLElement ? fullscreenRoot : document.body;

        target.appendChild(node);
        return {
            destroy() {
                if (node.parentElement) node.parentElement.removeChild(node);
            }
        };
    }
</script>

<button
    bind:this={btnEl}
    type="button"
    onclick={() => {
        if (!open) recalcPos();
        open = !open;
    }}
    class="w-full flex items-center justify-between gap-2 bg-[#2c2c2e] hover:bg-[#3a3a3c] border border-white/10 {size ===
    'sm'
        ? 'rounded-md px-2.5 py-1.5 text-[11.5px]'
        : 'rounded-xl px-3 py-2.5 text-sm'} text-white transition-colors"
    aria-haspopup="listbox"
    aria-expanded={open}
>
    <span class="truncate">{current?.name ?? value}</span>
    <svg
        width={size === 'sm' ? 11 : 13}
        height={size === 'sm' ? 11 : 13}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="opacity-70 flex-shrink-0 transition-transform {open ? 'rotate-180' : ''}"
    >
        <polyline points="6 9 12 15 18 9" />
    </svg>
</button>

{#if open && pos.width > 0}
    <div
        bind:this={panelEl}
        use:portal
        class="fixed z-[200] bg-[#1c1c1e] border border-white/10 rounded-xl shadow-2xl overflow-y-auto max-h-72 text-sm"
        style="top: {pos.openUp ? 'auto' : pos.top + 'px'}; bottom: {pos.openUp
            ? Math.max(0, (typeof window !== 'undefined' ? window.innerHeight : 0) - pos.top) + 'px'
            : 'auto'}; left: {pos.left}px; width: {pos.width}px;"
        role="listbox"
    >
        {#each ordered.priority as opt (opt.code)}
            <button
                type="button"
                onclick={() => pick(opt.code)}
                class="w-full flex items-center justify-between px-3 py-2 text-left text-white hover:bg-white/[0.08] {value ===
                opt.code
                    ? 'bg-primary-500/15 text-primary-200'
                    : ''}"
                role="option"
                aria-selected={value === opt.code}
            >
                <span>{opt.name}</span>
                {#if value === opt.code}
                    <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                    >
                {/if}
            </button>
        {/each}
        {#if ordered.priority.length > 0 && ordered.rest.length > 0}
            <div class="h-px bg-white/[0.08] mx-3 my-1"></div>
        {/if}
        {#each ordered.rest as opt (opt.code)}
            <button
                type="button"
                onclick={() => pick(opt.code)}
                class="w-full flex items-center justify-between px-3 py-2 text-left text-gray-300 hover:bg-white/[0.08] {value ===
                opt.code
                    ? 'bg-primary-500/15 text-primary-200'
                    : ''}"
                role="option"
                aria-selected={value === opt.code}
            >
                <span>{opt.name}</span>
                {#if value === opt.code}
                    <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
                    >
                {/if}
            </button>
        {/each}
    </div>
{/if}
