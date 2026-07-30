<script lang="ts">
    import { onMount } from 'svelte';
    import { goto } from '$app/navigation';
    import { api, type AdminSettings, type User, type VpnStatus } from '$lib/api';

    type Step = 'start' | 'metadata' | 'downloads' | 'vpn' | 'done';

    const steps: { id: Step; label: string; note: string }[] = [
        { id: 'start', label: 'check', note: 'what is ready' },
        { id: 'metadata', label: 'metadata', note: 'tmdb key' },
        { id: 'downloads', label: 'services', note: 'download stack' },
        { id: 'vpn', label: 'vpn', note: 'optional tunnel' },
        { id: 'done', label: 'finish', note: 'open app' }
    ];

    let me = $state<User | null>(null);
    let settings = $state<AdminSettings | null>(null);
    let vpn = $state<VpnStatus | null>(null);
    let step = $state<Step>('start');
    let booting = $state(true);

    let tmdbKey = $state('');
    let wyzieKey = $state('');
    let omdbKey = $state('');
    let savingMetadata = $state(false);
    let metadataErr = $state('');

    let refreshing = $state(false);

    let vpnPaste = $state('');
    let vpnProvider = $state('mullvad');
    let vpnCountry = $state('');
    let savingVpn = $state(false);
    let vpnErr = $state('');

    let tmdbOk = $derived(settings?.tmdb_ready ?? false);
    let subOk = $derived(settings?.wyzie_ready ?? false);
    let animeOk = $derived(settings?.omdb_ready ?? false);
    let qbitOk = $derived(settings?.qbit_ready ?? false);
    let jackettOk = $derived(settings?.jackett_ready ?? false);
    let prowlarrOk = $derived(settings?.prowlarr_ready ?? false);
    let vpnOk = $derived(vpn?.enabled && vpn.has_key);

    let requiredRows = $derived([
        { name: 'admin account', ok: true, state: 'ready', detail: 'this user owns the server' },
        { name: 'tmdb', ok: tmdbOk, state: tmdbOk ? 'ready' : 'required', detail: 'search, posters, cast, seasons' }
    ]);

    let serviceRows = $derived([
        { name: 'qBittorrent', ok: qbitOk, state: qbitOk ? 'connected' : 'optional', detail: 'download client' },
        { name: 'Jackett', ok: jackettOk, state: jackettOk ? 'connected' : 'optional', detail: 'indexer bridge' },
        {
            name: 'Prowlarr',
            ok: prowlarrOk,
            state: prowlarrOk ? 'connected' : 'optional',
            detail: 'second indexer bridge'
        }
    ]);

    onMount(async () => {
        try {
            me = await api.me();
        } catch {
            goto('/login', { replaceState: true });
            return;
        }
        if (me.role !== 'admin') {
            goto('/', { replaceState: true });
            return;
        }
        await refreshSetup();
        booting = false;
    });

    async function refreshSetup() {
        refreshing = true;
        try {
            settings = await api.getSettings();
            vpn = await api.vpnStatus().catch(() => null);
        } finally {
            refreshing = false;
        }
    }

    function nextFromStart() {
        step = tmdbOk ? 'downloads' : 'metadata';
    }

    async function saveMetadata() {
        if (!tmdbOk && !tmdbKey.trim()) {
            metadataErr = 'tmdb key is required for search and metadata';
            return;
        }

        savingMetadata = true;
        metadataErr = '';
        try {
            const body: Record<string, string> = {};
            if (tmdbKey.trim()) body.tmdb_api_key = tmdbKey.trim();
            if (wyzieKey.trim()) body.wyzie_api_key = wyzieKey.trim();
            if (omdbKey.trim()) body.omdb_api_key = omdbKey.trim();

            settings = await api.updateSettings(body);
            if (!settings.tmdb_ready) {
                metadataErr = 'tmdb saved, but the key did not validate';
                return;
            }

            tmdbKey = '';
            wyzieKey = '';
            omdbKey = '';
            step = 'downloads';
        } catch (e) {
            metadataErr = e instanceof Error ? e.message : 'save failed';
        } finally {
            savingMetadata = false;
        }
    }

    function parseWg(text: string): { key: string; addresses: string } {
        let key = '';
        let addresses = '';
        for (const raw of text.split(/\r?\n/)) {
            const line = raw.trim();
            if (!line || line.startsWith('#') || line.startsWith('[')) continue;
            const idx = line.indexOf('=');
            if (idx < 0) continue;
            const k = line.slice(0, idx).trim().toLowerCase();
            const v = line.slice(idx + 1).trim();
            if (k === 'privatekey') key = v;
            if (k === 'address') addresses = v;
        }
        addresses = addresses
            .split(',')
            .map((a) => a.trim())
            .filter((a) => a && !a.includes(':'))
            .join(',');
        return { key, addresses };
    }

    async function saveVpn() {
        savingVpn = true;
        vpnErr = '';
        try {
            const parsed = parseWg(vpnPaste);
            if (!parsed.key || !parsed.addresses) {
                vpnErr = 'paste a wireguard config with PrivateKey and Address';
                return;
            }
            await api.vpnSave({
                provider: vpnProvider,
                wireguard_private_key: parsed.key,
                wireguard_addresses: parsed.addresses,
                countries: vpnCountry.trim() || undefined
            });
            vpnPaste = '';
            await refreshSetup();
            step = 'done';
        } catch (e) {
            vpnErr = e instanceof Error ? e.message : 'vpn setup failed';
        } finally {
            savingVpn = false;
        }
    }

    function finish() {
        goto('/', { replaceState: true });
    }

    function adminSettings() {
        goto('/admin?tab=settings', { replaceState: true });
    }
</script>

<svelte:head><title>first run - pleasewatch</title></svelte:head>

{#if booting}
    <main class="pw-onboarding">
        <section class="pw-boot">
            <div class="pw-spinner"></div>
            <p>checking setup...</p>
        </section>
    </main>
{:else if me}
    <main class="pw-onboarding">
        <section class="pw-shell">
            <aside class="pw-rail">
                <div class="pw-brand">
                    <span>pleasewatch</span>
                    <strong>first run</strong>
                </div>

                <nav class="pw-steps" aria-label="setup steps">
                    {#each steps as item}
                        <button type="button" class:active={step === item.id} onclick={() => (step = item.id)}>
                            <span></span>
                            <div>
                                <strong>{item.label}</strong>
                                <small>{item.note}</small>
                            </div>
                        </button>
                    {/each}
                </nav>

                <div class="pw-mini-status">
                    <div><span class:ok={tmdbOk}></span>tmdb {tmdbOk ? 'ready' : 'missing'}</div>
                    <div><span class:ok={qbitOk}></span>qbit {qbitOk ? 'ready' : 'optional'}</div>
                    <div><span class:ok={vpnOk}></span>vpn {vpnOk ? 'on' : 'optional'}</div>
                </div>
            </aside>

            <section class="pw-main">
                {#if step === 'start'}
                    <div class="pw-page-head">
                        <p>fresh install</p>
                        <h1>check the basics.</h1>
                        <span>the server is running. add tmdb first, then keep or skip the download stack.</span>
                    </div>

                    <div class="pw-list">
                        {#each requiredRows as row}
                            <article class:ready={row.ok}>
                                <span></span>
                                <div>
                                    <strong>{row.name}</strong>
                                    <small>{row.detail}</small>
                                </div>
                                <b>{row.state}</b>
                            </article>
                        {/each}
                    </div>

                    <div class="pw-actions">
                        <button class="pw-primary" type="button" onclick={nextFromStart}> continue setup </button>
                        {#if tmdbOk}
                            <button class="pw-ghost" type="button" onclick={finish}>open library</button>
                        {/if}
                    </div>
                {:else if step === 'metadata'}
                    <div class="pw-page-head">
                        <p>metadata</p>
                        <h1>connect tmdb.</h1>
                        <span>tmdb is the only required key. omdb and wyzie can be added now or later.</span>
                    </div>

                    <form
                        class="pw-form"
                        onsubmit={(e) => {
                            e.preventDefault();
                            saveMetadata();
                        }}
                    >
                        <label>
                            <span>tmdb api key {tmdbOk ? '(already set)' : ''}</span>
                            <input
                                bind:value={tmdbKey}
                                placeholder={tmdbOk ? 'leave blank to keep current key' : 'paste v3 api key'}
                                spellcheck="false"
                                autocomplete="off"
                            />
                        </label>
                        <a
                            class="pw-inline-link"
                            href="https://www.themoviedb.org/settings/api"
                            target="_blank"
                            rel="noreferrer"
                        >
                            get a free tmdb v3 key
                        </a>

                        <div class="pw-tmdb-help">
                            <b>tmdb form example</b>
                            <div><span>app name</span><strong>pleasewatch</strong></div>
                            <div><span>app url</span><strong>http://localhost</strong></div>
                            <div><span>type</span><strong>personal or desktop app</strong></div>
                            <div>
                                <span>summary</span>
                                <strong>keep track of what i watch</strong>
                            </div>
                            <p>phone is a contact field. it usually does not ask for an sms code.</p>
                        </div>

                        <div class="pw-two">
                            <label>
                                <span>wyzie key {subOk ? '(set)' : '(optional)'}</span>
                                <input
                                    bind:value={wyzieKey}
                                    placeholder="subtitle search key"
                                    spellcheck="false"
                                    autocomplete="off"
                                />
                            </label>
                            <label>
                                <span>omdb key {animeOk ? '(set)' : '(optional)'}</span>
                                <input
                                    bind:value={omdbKey}
                                    placeholder="anime season helper"
                                    spellcheck="false"
                                    autocomplete="off"
                                />
                            </label>
                        </div>

                        {#if metadataErr}<p class="pw-error">{metadataErr}</p>{/if}

                        <div class="pw-actions">
                            <button class="pw-primary" type="submit" disabled={savingMetadata}>
                                {savingMetadata ? 'saving...' : 'save metadata'}
                            </button>
                            <button class="pw-ghost" type="button" onclick={() => (step = 'downloads')}>
                                skip optional keys
                            </button>
                        </div>
                    </form>
                {:else if step === 'downloads'}
                    <div class="pw-page-head">
                        <p>services</p>
                        <h1>downloads are optional.</h1>
                        <span>docker can start the containers before the app has working credentials and api keys.</span
                        >
                    </div>

                    <div class="pw-list">
                        {#each serviceRows as row}
                            <article class:ready={row.ok}>
                                <span></span>
                                <div>
                                    <strong>{row.name}</strong>
                                    <small>{row.detail}</small>
                                </div>
                                <b>{row.state}</b>
                            </article>
                        {/each}
                    </div>

                    <div class="pw-note">
                        use admin settings for qbit credentials, indexer keys, and default indexers. a catalog-only
                        server can leave this alone.
                    </div>

                    <div class="pw-actions">
                        <button class="pw-primary" type="button" onclick={() => (step = 'vpn')}> continue </button>
                        <button class="pw-ghost" type="button" onclick={refreshSetup} disabled={refreshing}>
                            {refreshing ? 'checking...' : 'recheck services'}
                        </button>
                        <button class="pw-ghost" type="button" onclick={adminSettings}>admin settings</button>
                    </div>
                {:else if step === 'vpn'}
                    <div class="pw-page-head">
                        <p>vpn</p>
                        <h1>{vpnOk ? 'vpn is enabled.' : 'vpn can wait.'}</h1>
                        <span>paste a wireguard config if qbit should use gluetun. otherwise skip this.</span>
                    </div>

                    <div class="pw-vpn-meta">
                        <label>
                            <span>provider</span>
                            <select bind:value={vpnProvider}>
                                <option value="mullvad">mullvad</option>
                                <option value="protonvpn">proton vpn</option>
                                <option value="surfshark">surfshark</option>
                                <option value="other">other</option>
                            </select>
                        </label>
                        <label>
                            <span>country</span>
                            <input bind:value={vpnCountry} placeholder="optional" autocomplete="off" />
                        </label>
                    </div>

                    <label class="pw-textarea">
                        <span>wireguard config</span>
                        <textarea
                            bind:value={vpnPaste}
                            rows="9"
                            placeholder="[Interface]
PrivateKey = ...
Address = 10.x.x.x/32"
                            spellcheck="false"></textarea>
                    </label>

                    {#if vpnErr}<p class="pw-error">{vpnErr}</p>{/if}

                    <div class="pw-actions">
                        <button
                            class="pw-primary"
                            type="button"
                            onclick={saveVpn}
                            disabled={savingVpn || !vpnPaste.trim()}
                        >
                            {savingVpn ? 'starting...' : 'enable vpn'}
                        </button>
                        <button class="pw-ghost" type="button" onclick={() => (step = 'done')}> skip vpn </button>
                    </div>
                {:else}
                    <div class="pw-page-head">
                        <p>ready</p>
                        <h1>{tmdbOk ? 'setup is done.' : 'tmdb is still missing.'}</h1>
                        <span
                            >{tmdbOk
                                ? 'open the app, add media, or tune services later.'
                                : 'add tmdb before search.'}</span
                        >
                    </div>

                    <div class="pw-grid">
                        <article class="pw-card ready">
                            <b>metadata</b>
                            <strong>{tmdbOk ? 'ready' : 'missing'}</strong>
                            <span>{tmdbOk ? 'search can run now.' : 'add tmdb before search.'}</span>
                        </article>
                        <article class="pw-card" class:ready={qbitOk || jackettOk || prowlarrOk}>
                            <b>downloads</b>
                            <strong>{qbitOk || jackettOk || prowlarrOk ? 'detected' : 'skipped'}</strong>
                            <span>admin settings keeps the full setup.</span>
                        </article>
                        <article class="pw-card" class:ready={vpnOk}>
                            <b>vpn</b>
                            <strong>{vpnOk ? 'enabled' : 'skipped'}</strong>
                            <span>can be enabled later.</span>
                        </article>
                    </div>

                    <div class="pw-actions">
                        {#if tmdbOk}
                            <button class="pw-primary" type="button" onclick={finish}>open app</button>
                        {:else}
                            <button class="pw-primary" type="button" onclick={() => (step = 'metadata')}>
                                add tmdb
                            </button>
                        {/if}
                        <button class="pw-ghost" type="button" onclick={adminSettings}>admin settings</button>
                    </div>
                {/if}
            </section>
        </section>
    </main>
{/if}

<style>
    .pw-onboarding {
        min-height: 100vh;
        background: #07090d;
        color: #eef4f8;
        display: grid;
        place-items: center;
        padding: 24px;
    }

    .pw-shell {
        width: min(1120px, 100%);
        min-height: 680px;
        display: grid;
        grid-template-columns: 300px minmax(0, 1fr);
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: #0a0d12;
        border-radius: 8px;
        overflow: hidden;
        box-shadow: 0 28px 80px rgba(0, 0, 0, 0.38);
    }

    .pw-rail {
        border-right: 1px solid rgba(255, 255, 255, 0.08);
        background: #080b10;
        padding: 22px;
        display: flex;
        flex-direction: column;
        gap: 22px;
    }

    .pw-brand span,
    .pw-page-head p,
    .pw-card b {
        color: #83d8ff;
        font:
            700 11px/1.2 ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
        text-transform: uppercase;
    }

    .pw-brand strong {
        display: block;
        margin-top: 6px;
        font-size: 26px;
        line-height: 1;
        font-weight: 750;
    }

    .pw-steps {
        display: grid;
        gap: 8px;
    }

    .pw-steps button {
        width: 100%;
        border: 1px solid transparent;
        border-radius: 8px;
        background: transparent;
        color: #a5b0bb;
        display: grid;
        grid-template-columns: 14px minmax(0, 1fr);
        gap: 11px;
        align-items: center;
        padding: 10px;
        text-align: left;
        cursor: pointer;
    }

    .pw-steps button:hover,
    .pw-steps button.active {
        background: rgba(255, 255, 255, 0.045);
        border-color: rgba(255, 255, 255, 0.08);
        color: #fff;
    }

    .pw-steps button > span,
    .pw-mini-status span,
    .pw-list article > span {
        width: 9px;
        height: 9px;
        border-radius: 999px;
        background: #48515c;
    }

    .pw-steps button.active > span,
    .pw-mini-status span.ok,
    .pw-list article.ready > span {
        background: #94e66d;
        box-shadow: 0 0 18px rgba(148, 230, 109, 0.36);
    }

    .pw-steps strong,
    .pw-steps small {
        display: block;
    }

    .pw-steps strong {
        font-size: 13px;
    }

    .pw-steps small {
        color: #687581;
        margin-top: 2px;
        font-size: 11px;
    }

    .pw-mini-status {
        margin-top: auto;
        display: grid;
        gap: 8px;
        color: #8b98a5;
        font:
            12px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    .pw-mini-status div {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .pw-main {
        padding: 42px;
        display: flex;
        flex-direction: column;
        justify-content: center;
    }

    .pw-page-head {
        max-width: 720px;
        margin-bottom: 28px;
    }

    .pw-page-head h1 {
        margin: 7px 0 0;
        font-size: 42px;
        line-height: 1.05;
        font-weight: 760;
    }

    .pw-page-head span {
        display: block;
        max-width: 680px;
        margin-top: 13px;
        color: #8793a0;
        line-height: 1.55;
        font-size: 15px;
    }

    .pw-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: 12px;
    }

    .pw-card,
    .pw-list article,
    .pw-note,
    .pw-form,
    .pw-boot {
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: #0d1219;
        border-radius: 8px;
    }

    .pw-card {
        min-height: 142px;
        padding: 16px;
        display: flex;
        flex-direction: column;
    }

    .pw-card.ready {
        border-color: rgba(148, 230, 109, 0.24);
        background: #0e1616;
    }

    .pw-card strong {
        margin-top: 20px;
        font-size: 24px;
        line-height: 1;
    }

    .pw-card span {
        margin-top: auto;
        color: #7f8b96;
        line-height: 1.4;
        font-size: 13px;
    }

    .pw-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-top: 22px;
    }

    .pw-primary,
    .pw-ghost {
        min-height: 40px;
        border-radius: 8px;
        padding: 0 15px;
        font-weight: 700;
        cursor: pointer;
    }

    .pw-primary {
        border: 0;
        background: #d9f2ff;
        color: #081018;
    }

    .pw-ghost {
        border: 1px solid rgba(255, 255, 255, 0.1);
        background: rgba(255, 255, 255, 0.035);
        color: #c7d0d8;
    }

    .pw-primary:disabled,
    .pw-ghost:disabled {
        opacity: 0.55;
        cursor: default;
    }

    .pw-form {
        padding: 18px;
        display: grid;
        gap: 14px;
    }

    .pw-two,
    .pw-vpn-meta {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 12px;
    }

    label span {
        display: block;
        color: #98a5b0;
        margin-bottom: 7px;
        font-size: 12px;
        font-weight: 700;
    }

    input,
    select,
    textarea {
        width: 100%;
        border: 1px solid rgba(255, 255, 255, 0.11);
        background: #070a0f;
        color: #eef4f8;
        border-radius: 8px;
        outline: none;
        padding: 11px 12px;
        font:
            13px ui-monospace,
            SFMono-Regular,
            Menlo,
            Consolas,
            monospace;
    }

    textarea {
        resize: vertical;
        line-height: 1.45;
    }

    input:focus,
    select:focus,
    textarea:focus {
        border-color: #83d8ff;
    }

    .pw-error {
        margin: 0;
        color: #ff8d8d;
        font-size: 13px;
    }

    .pw-inline-link {
        width: fit-content;
        color: #83d8ff;
        font-size: 13px;
        font-weight: 700;
        text-decoration: none;
    }

    .pw-inline-link:hover {
        text-decoration: underline;
    }

    .pw-tmdb-help {
        border: 1px solid rgba(131, 216, 255, 0.16);
        background: rgba(131, 216, 255, 0.055);
        border-radius: 8px;
        padding: 13px;
        display: grid;
        gap: 8px;
    }

    .pw-tmdb-help b {
        color: #83d8ff;
        font-size: 12px;
    }

    .pw-tmdb-help div {
        display: grid;
        grid-template-columns: 110px minmax(0, 1fr);
        gap: 10px;
        align-items: start;
        color: #7f8b96;
        font-size: 13px;
    }

    .pw-tmdb-help strong {
        color: #dce8ef;
        font-weight: 650;
    }

    .pw-tmdb-help p {
        margin: 2px 0 0;
        color: #94a4af;
        font-size: 13px;
        line-height: 1.45;
    }

    .pw-list {
        display: grid;
        gap: 10px;
    }

    .pw-list article {
        display: grid;
        grid-template-columns: 12px minmax(0, 1fr) auto;
        align-items: center;
        gap: 14px;
        padding: 14px;
    }

    .pw-list strong,
    .pw-list small {
        display: block;
    }

    .pw-list small {
        color: #7c8995;
        margin-top: 2px;
    }

    .pw-list b {
        color: #81909d;
        font-size: 12px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    }

    .pw-list article.ready b {
        color: #a9f28a;
    }

    .pw-note {
        margin-top: 12px;
        padding: 14px;
        color: #8d9aa6;
        line-height: 1.5;
        font-size: 13px;
    }

    .pw-vpn-meta {
        margin-bottom: 13px;
    }

    .pw-textarea {
        display: block;
    }

    .pw-boot {
        min-width: 280px;
        padding: 26px;
        display: grid;
        justify-items: center;
        gap: 12px;
        color: #9ba7b2;
    }

    .pw-spinner {
        width: 26px;
        height: 26px;
        border-radius: 999px;
        border: 2px solid rgba(255, 255, 255, 0.14);
        border-top-color: #83d8ff;
        animation: pw-spin 0.8s linear infinite;
    }

    @keyframes pw-spin {
        to {
            transform: rotate(360deg);
        }
    }

    @media (max-width: 860px) {
        .pw-onboarding {
            padding: 12px;
            place-items: stretch;
        }
        .pw-shell {
            min-height: 0;
            grid-template-columns: 1fr;
        }
        .pw-rail {
            border-right: 0;
            border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        }
        .pw-steps {
            grid-template-columns: repeat(5, minmax(0, 1fr));
        }
        .pw-steps button {
            grid-template-columns: 1fr;
            gap: 6px;
            justify-items: start;
        }
        .pw-steps small {
            display: none;
        }
        .pw-main {
            padding: 24px;
        }
        .pw-page-head h1 {
            font-size: 31px;
        }
        .pw-grid,
        .pw-two,
        .pw-vpn-meta {
            grid-template-columns: 1fr;
        }
    }
</style>
