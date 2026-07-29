<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type AdminSettings, type AdminSettingsUpdate, type VpnStatus } from '$lib/api';
    import CopyCode from '$lib/components/CopyCode.svelte';
    import Icon from '$lib/components/Icon.svelte';
    import KeyList from './KeyList.svelte';

    type Shell = 'unix' | 'pwsh' | 'cmd';
    type VpnProvider = 'mullvad' | 'protonvpn' | 'nordvpn' | 'pia' | 'surfshark' | 'other';

    let settings = $state<AdminSettings | null>(null);
    let loading = $state(true);
    let loadErr = $state('');

    let formTmdbKey = $state('');
    let formWyzieKey = $state('');
    let formOmdbKey = $state('');
    let formJackettUrl = $state('');
    let formJackettKey = $state('');
    let formProwlarrUrl = $state('');
    let formProwlarrKey = $state('');
    let formQbitUrl = $state('');
    let formQbitUser = $state('');
    let formQbitPass = $state('');
    let savingSettings = $state(false);
    let settingsMsg = $state('');

    let vpnProvider = $state<VpnProvider>('mullvad');
    let vpnStatus = $state<VpnStatus | null>(null);
    let vpnPaste = $state('');
    let vpnCountries = $state('');
    let vpnSaving = $state(false);
    let vpnMsg = $state('');

    let platform = $state<Shell>(typeof navigator !== 'undefined' && /win/i.test(navigator.platform) ? 'pwsh' : 'unix');

    const jackettCmd = [
        'docker run -d --name jackett',
        '-p 9117:9117',
        '-v jackett-config:/config',
        '--restart unless-stopped',
        'lscr.io/linuxserver/jackett'
    ].join(' ');

    const qbitCmd = $derived.by(() => {
        const base = [
            'docker run -d --name qbittorrent',
            '-p 8080:8080',
            '-p 6881:6881',
            '-p 6881:6881/udp',
            '-e WEBUI_PORT=8080',
            '-v qbit-config:/config'
        ].join(' ');
        const tail = '--restart unless-stopped lscr.io/linuxserver/qbittorrent';
        if (platform === 'pwsh') return `${base} -v "\${PWD}\\media:/downloads" ${tail}`;
        if (platform === 'cmd') return `${base} -v "%cd%\\media:/downloads" ${tail}`;
        return `${base} -v "$(pwd)/media:/downloads" ${tail}`;
    });

    const qbitPwCmd = $derived.by(() => {
        if (platform === 'pwsh') return 'docker logs qbittorrent 2>&1 | Select-String password';
        if (platform === 'cmd') return 'docker logs qbittorrent 2>&1 | findstr /i password';
        return 'docker logs qbittorrent 2>&1 | grep -i password';
    });
    const wgPlaceholder = [
        '[Interface]',
        'PrivateKey = ...',
        'Address = 10.x.x.x/32',
        '',
        '[Peer]',
        'PublicKey = ...',
        'Endpoint = ...'
    ].join('\n');

    onMount(() => {
        loadSettings();
        loadVpn();
    });

    function fillForms(s: AdminSettings) {
        formJackettUrl = s.jackett_url || '';
        formProwlarrUrl = s.prowlarr_url || '';
        formQbitUrl = s.qbit_url || '';
        formQbitUser = s.qbit_user || '';
        formTmdbKey = '';
        formJackettKey = '';
        formProwlarrKey = '';
        formQbitPass = '';
    }

    async function loadSettings() {
        loading = true;
        loadErr = '';
        try {
            const s = await api.getSettings();
            settings = s;
            fillForms(s);
        } catch (e) {
            loadErr = e instanceof Error ? e.message : 'settings load failed';
        } finally {
            loading = false;
        }
    }

    async function loadVpn() {
        try {
            vpnStatus = await api.vpnStatus();
        } catch {}
    }

    function parseWgConf(text: string): { key: string; addresses: string } {
        let key = '';
        let addresses = '';
        for (const raw of text.split(/\r?\n/)) {
            const line = raw.trim();
            if (!line || line.startsWith('#') || line.startsWith('[')) continue;
            const eq = line.indexOf('=');
            if (eq < 0) continue;
            const name = line.slice(0, eq).trim().toLowerCase();
            const val = line.slice(eq + 1).trim();
            if (name === 'privatekey') key = val;
            else if (name === 'address') addresses = val;
        }
        addresses = addresses
            .split(',')
            .map((x) => x.trim())
            .filter((x) => x && !x.includes(':'))
            .join(',');
        return { key, addresses };
    }

    function addOnEnter(e: KeyboardEvent, add: () => void) {
        if (e.key !== 'Enter') return;
        e.preventDefault();
        add();
    }

    async function saveSettings() {
        savingSettings = true;
        settingsMsg = '';
        try {
            const body: Partial<AdminSettingsUpdate> = {};
            if (formTmdbKey.trim()) body.tmdb_api_key = formTmdbKey.trim();
            if (formOmdbKey.trim()) body.omdb_api_key = formOmdbKey.trim();
            if (formJackettUrl.trim()) body.jackett_url = formJackettUrl.trim();
            if (formJackettKey.trim()) body.jackett_api_key = formJackettKey.trim();
            if (formProwlarrUrl.trim()) body.prowlarr_url = formProwlarrUrl.trim();
            if (formProwlarrKey.trim()) body.prowlarr_api_key = formProwlarrKey.trim();
            if (formQbitUrl.trim()) body.qbit_url = formQbitUrl.trim();
            if (formQbitUser.trim()) body.qbit_user = formQbitUser.trim();
            if (formQbitPass.trim()) body.qbit_pass = formQbitPass.trim();
            settings = await api.updateSettings(body);
            settingsMsg = 'saved';
            formTmdbKey = '';
            formOmdbKey = '';
            formJackettKey = '';
            formProwlarrKey = '';
            formQbitPass = '';
            setTimeout(() => (settingsMsg = ''), 2200);
        } catch (e) {
            settingsMsg = e instanceof Error ? e.message : 'save failed';
        } finally {
            savingSettings = false;
        }
    }

    async function addWyzieKey() {
        const key = formWyzieKey.trim();
        if (!key) return;
        try {
            settings = await api.updateSettings({ wyzie_key_add: key });
            formWyzieKey = '';
        } catch (e) {
            settingsMsg = e instanceof Error ? e.message : 'add key failed';
        }
    }

    async function removeWyzieKey(masked: string) {
        try {
            settings = await api.updateSettings({ wyzie_key_remove_mask: masked });
        } catch (e) {
            settingsMsg = e instanceof Error ? e.message : 'remove key failed';
        }
    }

    async function addOmdbKey() {
        const key = formOmdbKey.trim();
        if (!key) return;
        try {
            settings = await api.updateSettings({ omdb_key_add: key });
            formOmdbKey = '';
        } catch (e) {
            settingsMsg = e instanceof Error ? e.message : 'add key failed';
        }
    }

    async function removeOmdbKey(masked: string) {
        try {
            settings = await api.updateSettings({ omdb_key_remove_mask: masked });
        } catch (e) {
            settingsMsg = e instanceof Error ? e.message : 'remove key failed';
        }
    }

    async function saveVpn() {
        vpnMsg = '';
        vpnSaving = true;
        try {
            const { key, addresses } = parseWgConf(vpnPaste);
            if (!key || !addresses) {
                vpnMsg = 'wireguard config needs PrivateKey and Address lines.';
                return;
            }
            await api.vpnSave({
                provider: vpnProvider,
                wireguard_private_key: key,
                wireguard_addresses: addresses,
                countries: vpnCountries.trim() || undefined
            });
            vpnMsg = 'saved. gluetun is starting.';
            vpnPaste = '';
            setTimeout(loadVpn, 4000);
        } catch (e) {
            vpnMsg = e instanceof Error ? e.message : 'save failed';
        } finally {
            vpnSaving = false;
        }
    }

    async function disableVpn() {
        if (!confirm('disable vpn? qbit goes back to your bare ip.')) return;
        try {
            await api.vpnDisable();
            vpnMsg = 'vpn off. qbit is back on the regular network.';
            setTimeout(loadVpn, 2000);
        } catch (e) {
            vpnMsg = e instanceof Error ? e.message : 'disable failed';
        }
    }
</script>

{#if loading}
    <div class="pw-set-card">loading settings...</div>
{:else if loadErr}
    <div class="pw-set-card pw-set-error">{loadErr}</div>
{:else if settings}
    <div class="pw-settings">
        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>TMDB</h3>
                <span class:ok={settings.tmdb_ready}>{settings.tmdb_ready ? 'connected' : 'not configured'}</span>
            </div>
            <p class="pw-set-copy">
                powers search and metadata. free key from
                <a href="https://www.themoviedb.org/settings/api" target="_blank" rel="noreferrer">
                    themoviedb.org/settings/api
                </a>.
            </p>
            <label class="pw-field">
                <span>api key {settings.tmdb_api_key_set ? '(set, leave blank to keep)' : ''}</span>
                <input bind:value={formTmdbKey} placeholder="paste tmdb here" autocomplete="off" />
            </label>
        </div>

        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>OMDB</h3>
                <div class="pw-set-statuses">
                    <span class:ok={settings.omdb_ready}>{settings.omdb_ready ? 'connected' : 'not configured'}</span>
                    {#if settings.omdb_key_count > 0}
                        <span>{settings.omdb_key_count} {settings.omdb_key_count === 1 ? 'key' : 'keys'}</span>
                    {/if}
                </div>
            </div>
            <p class="pw-set-copy">
                imdb source for anime season splits. 1k reqs/day per key, round-robin on quota errors.
                <a href="https://www.omdbapi.com/apikey.aspx" target="_blank" rel="noreferrer"
                    >omdbapi.com/apikey.aspx</a
                >
            </p>
            <KeyList
                masked={settings.omdb_keys_masked}
                full={settings.omdb_keys_full ?? []}
                kind="omdb"
                onRemove={removeOmdbKey}
            />
            <label class="pw-field">
                <span>add new key</span>
                <div class="pw-inline-input">
                    <input
                        bind:value={formOmdbKey}
                        placeholder="paste omdb key"
                        autocomplete="off"
                        onkeydown={(e) => addOnEnter(e, addOmdbKey)}
                    />
                    <button onclick={addOmdbKey} disabled={!formOmdbKey.trim()}>add</button>
                </div>
            </label>
        </div>

        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>Wyzie</h3>
                <div class="pw-set-statuses">
                    <span class:ok={settings.wyzie_ready}>{settings.wyzie_ready ? 'connected' : 'not configured'}</span>
                    {#if settings.wyzie_key_count > 0}
                        <span>{settings.wyzie_key_count} {settings.wyzie_key_count === 1 ? 'key' : 'keys'}</span>
                    {/if}
                </div>
            </div>
            <p class="pw-set-copy">
                subtitle search. add more keys and we round-robin them.
                <a href="https://sub.wyzie.io/redeem" target="_blank" rel="noreferrer">sub.wyzie.io/redeem</a>
            </p>
            <KeyList
                masked={settings.wyzie_keys_masked}
                full={settings.wyzie_keys_full ?? []}
                kind="wyzie"
                onRemove={removeWyzieKey}
            />
            <label class="pw-field">
                <span>add new key</span>
                <div class="pw-inline-input">
                    <input
                        bind:value={formWyzieKey}
                        placeholder="wyzie-..."
                        autocomplete="off"
                        onkeydown={(e) => addOnEnter(e, addWyzieKey)}
                    />
                    <button onclick={addWyzieKey} disabled={!formWyzieKey.trim()}>add</button>
                </div>
            </label>
        </div>

        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>Jackett</h3>
                <span class:ok={settings.jackett_ready}>{settings.jackett_ready ? 'connected' : 'not connected'}</span>
            </div>
            <p class="pw-set-copy">torrent indexers behind one api key.</p>
            {#if settings.jackett_ready}
                <a class="pw-open-link" href="/jackett/UI/Dashboard" target="_blank" rel="noreferrer">
                    <Icon name="external-link" class="w-4 h-4" />
                    open Jackett UI
                    <span>(uses your admin session)</span>
                </a>
            {/if}
            <details class="pw-guide" open={!settings.jackett_ready}>
                <summary>setup guide</summary>
                <div class="pw-guide-body">
                    {#if platform !== 'unix'}
                        <p>
                            <b>windows quick path:</b> download Jackett.Binaries.Windows.zip, unzip, run
                            <code>JackettConsole.exe</code>. opens at
                            <code>http://localhost:9117</code>.
                        </p>
                    {/if}
                    <p>docker compose already ships jackett. for standalone docker:</p>
                    <CopyCode code={jackettCmd} multiline />
                    <ol>
                        <li>open the dashboard and add the indexers you use</li>
                        <li>copy the api key shown top-right</li>
                        <li>paste below + save</li>
                    </ol>
                </div>
            </details>
            <label class="pw-field">
                <span>api key {settings.jackett_api_key_set ? '(set, leave blank to keep)' : ''}</span>
                <input bind:value={formJackettKey} placeholder="paste here" autocomplete="off" />
            </label>
            <details class="pw-advanced">
                <summary>advanced: url</summary>
                <input bind:value={formJackettUrl} placeholder="http://jackett:9117" autocomplete="off" />
                <p>preset by docker compose. change for local dev only.</p>
            </details>
        </div>

        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>Prowlarr</h3>
                <span class:ok={settings.prowlarr_ready}>{settings.prowlarr_ready ? 'connected' : 'not connected'}</span
                >
            </div>
            <p class="pw-set-copy">second indexer aggregator, runs in parallel with jackett.</p>
            {#if settings.prowlarr_ready}
                <a class="pw-open-link" href="/prowlarr/" target="_blank" rel="noreferrer">
                    <Icon name="external-link" class="w-4 h-4" />
                    open Prowlarr UI
                    <span>(uses your admin session)</span>
                </a>
            {/if}
            <details class="pw-guide" open={!settings.prowlarr_ready}>
                <summary>setup guide</summary>
                <div class="pw-guide-body">
                    <p>docker compose ships prowlarr. open the UI, set login, then add indexers.</p>
                    <p>
                        for vpn-routed scraping: settings, general, proxy. type HTTP, host <code>gluetun</code>, port
                        <code>8888</code>.
                    </p>
                    <ol>
                        <li>open the UI and set login</li>
                        <li>add the indexers you use</li>
                        <li>api key auto-imports from config on container start</li>
                    </ol>
                </div>
            </details>
            <label class="pw-field">
                <span>api key {settings.prowlarr_api_key_set ? '(set, leave blank to keep)' : ''}</span>
                <input bind:value={formProwlarrKey} placeholder="paste here" autocomplete="off" />
            </label>
            <details class="pw-advanced">
                <summary>advanced: url</summary>
                <input bind:value={formProwlarrUrl} placeholder="http://prowlarr:9696" autocomplete="off" />
                <p>preset by docker compose. change for local dev only.</p>
            </details>
        </div>

        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>qBittorrent</h3>
                <span class:ok={settings.qbit_ready}>{settings.qbit_ready ? 'connected' : 'not connected'}</span>
            </div>
            <p class="pw-set-copy">runs torrent downloads. pleasewatch talks to its web ui api.</p>
            <details class="pw-guide" open={!settings.qbit_ready}>
                <summary class="pw-guide-summary">
                    setup guide
                    <span class="pw-shells">
                        <button
                            class:on={platform === 'unix'}
                            onclick={(e) => {
                                e.preventDefault();
                                platform = 'unix';
                            }}
                        >
                            linux/mac
                        </button>
                        <button
                            class:on={platform === 'pwsh'}
                            onclick={(e) => {
                                e.preventDefault();
                                platform = 'pwsh';
                            }}
                        >
                            powershell
                        </button>
                        <button
                            class:on={platform === 'cmd'}
                            onclick={(e) => {
                                e.preventDefault();
                                platform = 'cmd';
                            }}
                        >
                            cmd
                        </button>
                    </span>
                </summary>
                <div class="pw-guide-body">
                    {#if platform !== 'unix'}
                        <p><b>windows quick path:</b> install qBittorrent, then enable web ui and set a password.</p>
                    {/if}
                    <p>docker compose already ships qbit. for standalone docker:</p>
                    <CopyCode code={qbitCmd} multiline />
                    <p>first boot temp password is in container logs:</p>
                    <CopyCode code={qbitPwCmd} multiline />
                    <ol>
                        <li>log into <code>http://localhost:8080</code></li>
                        <li>change the permanent web ui password</li>
                        <li>set default save path to your media root</li>
                    </ol>
                </div>
            </details>
            <label class="pw-field">
                <span>user</span>
                <input bind:value={formQbitUser} placeholder="admin" autocomplete="off" />
            </label>
            <label class="pw-field">
                <span>pass {settings.qbit_pass_set ? '(set, leave blank to keep)' : ''}</span>
                <input type="password" bind:value={formQbitPass} placeholder="paste here" autocomplete="off" />
            </label>
            <details class="pw-advanced">
                <summary>advanced: url</summary>
                <input bind:value={formQbitUrl} placeholder="http://qbittorrent:8080" autocomplete="off" />
                <p>preset by docker compose. change only if qbit runs somewhere else.</p>
            </details>
        </div>

        <div class="pw-set-card">
            <div class="pw-set-head">
                <h3>VPN</h3>
                <span class:ok={vpnStatus?.enabled}>{vpnStatus?.enabled ? 'active' : 'off'}</span>
            </div>
            <p class="pw-set-copy">routes qBittorrent through gluetun. killswitch pauses qbit if the tunnel drops.</p>
            {#if vpnStatus?.enabled}
                <div class="pw-vpn-grid">
                    <span>provider</span><b>{vpnStatus.provider || '(unset)'}</b>
                    <span>country</span><b>{vpnStatus.countries || 'auto'}</b>
                    <span>tunnel</span><b>{vpnStatus.container_state ?? '?'}</b>
                    <span>public ip</span><b class="mono">{vpnStatus.public_ip ?? 'detecting...'}</b>
                </div>
                <button class="pw-danger" onclick={disableVpn}>turn off VPN</button>
            {:else}
                <div class="pw-vpn-row">
                    <label>
                        <span>provider</span>
                        <select bind:value={vpnProvider}>
                            <option value="mullvad">Mullvad</option>
                            <option value="protonvpn">Proton VPN</option>
                            <option value="surfshark">Surfshark</option>
                            <option value="nordvpn">NordVPN (openvpn)</option>
                            <option value="pia">Private Internet Access (openvpn)</option>
                            <option value="other">other (gluetun list)</option>
                        </select>
                    </label>
                    <label>
                        <span>country</span>
                        <input bind:value={vpnCountries} placeholder="Sweden" autocomplete="off" />
                    </label>
                </div>
                <label class="pw-field">
                    <span>paste wireguard config</span>
                    <textarea bind:value={vpnPaste} rows="7" placeholder={wgPlaceholder} spellcheck="false"></textarea>
                </label>
                <p class="pw-set-copy">only PrivateKey and Address are read.</p>
                <button class="pw-main-btn" onclick={saveVpn} disabled={vpnSaving || !vpnPaste.trim()}>
                    {vpnSaving ? 'starting tunnel...' : 'enable VPN'}
                </button>
            {/if}
            {#if vpnMsg}
                <p class:ok-msg={vpnMsg.startsWith('saved') || vpnMsg.startsWith('vpn off')} class="pw-msg">{vpnMsg}</p>
            {/if}
        </div>

        <div class="pw-actions">
            <button class="pw-main-btn" onclick={saveSettings} disabled={savingSettings}>
                {savingSettings ? 'saving...' : 'save'}
            </button>
            {#if settingsMsg}
                <span class:ok-msg={settingsMsg === 'saved'}>{settingsMsg}</span>
            {/if}
        </div>
    </div>
{/if}

<style>
    .pw-settings {
        max-width: 768px;
        display: grid;
        gap: 18px;
    }
    .pw-set-card {
        padding: 20px;
        border-radius: 12px;
        background: rgba(31, 41, 55, 0.42);
        color: #e5e7eb;
    }
    .pw-set-error {
        color: #fca5a5;
    }
    .pw-set-head {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 12px;
        margin: 0 0 6px;
    }
    .pw-set-head h3 {
        margin: 0;
        color: #fff;
        font-size: 16px;
        font-weight: 650;
    }
    .pw-set-head span,
    .pw-set-statuses span {
        padding: 2px 8px;
        border-radius: 5px;
        background: #374151;
        color: #9ca3af;
        font-size: 12px;
    }
    .pw-set-head span.ok,
    .pw-set-statuses span.ok {
        background: rgba(34, 197, 94, 0.16);
        color: #4ade80;
    }
    .pw-set-statuses {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        justify-content: flex-end;
    }
    .pw-set-copy {
        margin: 0 0 14px;
        color: #9ca3af;
        font-size: 14px;
        line-height: 1.45;
    }
    .pw-set-copy a {
        color: #60a5fa;
        text-decoration: underline;
    }
    .pw-field {
        display: block;
        margin-top: 12px;
    }
    .pw-field span,
    .pw-vpn-row span {
        display: block;
        margin: 0 0 6px;
        color: #9ca3af;
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }
    .pw-field input,
    .pw-field textarea,
    .pw-advanced input,
    .pw-inline-input input,
    .pw-vpn-row input,
    .pw-vpn-row select {
        width: 100%;
        border: 1px solid #374151;
        border-radius: 6px;
        background: #111827;
        color: #fff;
        outline: none;
        font-size: 14px;
    }
    .pw-field input,
    .pw-advanced input,
    .pw-inline-input input,
    .pw-vpn-row input,
    .pw-vpn-row select {
        padding: 8px 10px;
    }
    .pw-field textarea {
        padding: 10px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 12px;
        resize: vertical;
    }
    .pw-inline-input {
        display: flex;
        gap: 8px;
    }
    .pw-inline-input button,
    .pw-main-btn {
        border: 0;
        border-radius: 6px;
        background: #2563eb;
        color: #fff;
        cursor: pointer;
        font-size: 14px;
        font-weight: 600;
    }
    .pw-inline-input button {
        padding: 0 16px;
    }
    .pw-main-btn {
        padding: 9px 20px;
    }
    .pw-inline-input button:disabled,
    .pw-main-btn:disabled {
        opacity: 0.55;
        cursor: default;
    }
    .pw-open-link {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        margin: 0 0 14px;
        padding: 8px 10px;
        border-radius: 6px;
        background: rgba(37, 99, 235, 0.18);
        color: #bfdbfe;
        font-size: 14px;
        text-decoration: none;
    }
    .pw-open-link span {
        color: rgba(191, 219, 254, 0.65);
        font-size: 12px;
    }
    .pw-guide,
    .pw-advanced {
        border-radius: 7px;
        background: rgba(17, 24, 39, 0.48);
    }
    .pw-guide {
        margin: 0 0 14px;
        padding: 12px;
        color: #d1d5db;
        font-size: 13px;
    }
    .pw-advanced {
        margin-top: 12px;
        padding: 8px 10px;
    }
    .pw-guide summary,
    .pw-advanced summary {
        cursor: pointer;
        color: #9ca3af;
        font-size: 12px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }
    .pw-guide-body {
        display: grid;
        gap: 10px;
        margin-top: 12px;
    }
    .pw-guide p,
    .pw-guide ol,
    .pw-advanced p {
        margin: 0;
        color: #9ca3af;
    }
    .pw-guide ol {
        padding-left: 18px;
    }
    .pw-guide code {
        padding: 1px 5px;
        border-radius: 4px;
        background: rgba(0, 0, 0, 0.38);
        color: #e5e7eb;
    }
    .pw-guide-summary {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
    }
    .pw-shells {
        display: inline-flex;
        gap: 2px;
        padding: 2px;
        border: 1px solid #374151;
        border-radius: 5px;
        background: #111827;
    }
    .pw-shells button {
        border: 0;
        border-radius: 4px;
        background: transparent;
        color: #9ca3af;
        font-size: 10px;
        cursor: pointer;
    }
    .pw-shells button.on {
        background: #374151;
        color: #fff;
    }
    .pw-vpn-grid {
        display: grid;
        grid-template-columns: max-content 1fr;
        gap: 8px 14px;
        margin: 0 0 14px;
        padding: 12px;
        border-radius: 7px;
        background: rgba(17, 24, 39, 0.48);
        font-size: 14px;
    }
    .pw-vpn-grid span {
        color: #9ca3af;
    }
    .mono {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    }
    .pw-vpn-row {
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-bottom: 12px;
    }
    .pw-vpn-row label {
        min-width: 150px;
    }
    .pw-danger {
        border: 1px solid rgba(248, 113, 113, 0.45);
        border-radius: 6px;
        background: transparent;
        color: #f87171;
        padding: 7px 12px;
        cursor: pointer;
    }
    .pw-actions {
        display: flex;
        align-items: center;
        gap: 14px;
        padding: 4px 0 0;
    }
    .pw-actions span,
    .pw-msg {
        color: #f87171;
        font-size: 14px;
    }
    .pw-actions span.ok-msg,
    .pw-msg.ok-msg {
        color: #4ade80;
    }
</style>
