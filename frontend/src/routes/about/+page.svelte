<script lang="ts">
    import Shell from '$lib/components/Shell.svelte';
    import Wordmark from '$lib/components/Wordmark.svelte';
</script>

<svelte:head><title>about - pleasewatch</title></svelte:head>

<div class="pw-shell">
    <Shell label="about" />

    <main class="pw-about-stage">
        <Wordmark />

        <section class="pw-about-card">
            <div>
                <div class="pw-eyebrow"><span>&gt;</span> about</div>
                <p>pleasewatch is a self-hosted media library.</p>
            </div>

            <div class="pw-about-source">
                <span>source</span>
                <a href="https://github.com/krisoft20/pleasewatch" target="_blank" rel="noreferrer">
                    github.com/krisoft20/pleasewatch
                </a>
                <small>licensed under AGPL-3.0-only</small>
            </div>

            <div class="pw-about-tmdb">
                <a href="https://www.themoviedb.org" target="_blank" rel="noreferrer">
                    <img src="/tmdb-logo.svg" alt="The Movie Database" />
                </a>
                <p>This product uses the TMDB API but is not endorsed or certified by TMDB.</p>
            </div>

            <a class="pw-about-back" href="/">back to library</a>
        </section>
    </main>
</div>
