<script lang="ts">
    import { page } from '$app/state';

    const HIDDEN = ['/login', '/register', '/onboarding', '/watch', '/party', '/read', '/admin'];
    const hidden = $derived(HIDDEN.some((path) => page.url.pathname.startsWith(path)));
</script>

{#if !hidden}
    <footer class="pw-footer">
        <div class="pw-footer-inner">
            <span class="pw-footer-name"><span aria-hidden="true">///</span> pleasewatch</span>

            <nav aria-label="project links">
                <a href="/about">about</a>
                <a href="https://github.com/krisoft20/pleasewatch" target="_blank" rel="noreferrer">github</a>
                <span>AGPL-3.0-only</span>
            </nav>
        </div>
    </footer>
{/if}

<style>
    .pw-footer {
        position: relative;
        z-index: 2;
        padding: 0 28px;
        background: var(--pw-bg);
    }
    .pw-footer-inner {
        width: 100%;
        max-width: 1200px;
        min-height: 68px;
        margin: 0 auto;
        border-top: 1px solid var(--pw-line-soft);
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 24px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 10px;
        letter-spacing: 0.04em;
    }
    .pw-footer-name {
        color: var(--pw-fg-dim);
    }
    .pw-footer-name span {
        margin-right: 6px;
        color: var(--pw-accent);
    }
    nav {
        display: flex;
        align-items: center;
        gap: 18px;
        color: var(--pw-fg-faint);
    }
    a {
        color: inherit;
        text-decoration: none;
        transition: color 0.15s ease;
    }
    a:hover {
        color: var(--pw-fg-dim);
    }
    a:focus-visible {
        border-radius: 2px;
        outline: 1px solid var(--pw-accent);
        outline-offset: 4px;
    }

    @media (max-width: 640px) {
        .pw-footer {
            padding: 0 18px;
        }
        .pw-footer-inner {
            min-height: 0;
            padding: 18px 0 calc(24px + env(safe-area-inset-bottom));
            flex-direction: column;
            align-items: flex-start;
            gap: 10px;
        }
        :global(body:has(.pw-bb)) .pw-footer-inner {
            padding-bottom: calc(86px + env(safe-area-inset-bottom));
        }
        nav {
            flex-wrap: wrap;
            gap: 8px 14px;
        }
    }
</style>
