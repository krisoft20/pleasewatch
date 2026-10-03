export default async function ({ page, context }) {
    const { url, waitMs = 25000 } = context;

    const PLAYLIST = /\.m3u8|%2findex\.m3u8|mpegurl/i;
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

    const hits = [];
    const seenUrl = new Set();
    const targets = [];

    function record(u, from) {
        if (!u || !PLAYLIST.test(u) || seenUrl.has(u)) return;
        seenUrl.add(u);
        hits.push({ url: u, from: from || null });
    }

    const client = await page.target().createCDPSession();
    const conn = client.connection();
    const watched = new Set();

    async function watch(sess, label) {
        try {
            await sess.send('Network.enable');
        } catch (e) {}
        sess.on('Network.requestWillBeSent', (e) => {
            try {
                record(e.request.url, label);
            } catch (err) {}
        });
        sess.on('Target.attachedToTarget', async (e) => {
            const id = e.sessionId;
            if (watched.has(id)) return;
            watched.add(id);
            targets.push((e.targetInfo && e.targetInfo.url ? e.targetInfo.url : '?').slice(0, 90));
            let child = null;
            try {
                child = conn.session(id);
            } catch (err) {}
            if (child) {
                await watch(child, (e.targetInfo && e.targetInfo.url) || 'child');
                try {
                    await child.send('Runtime.runIfWaitingForDebugger');
                } catch (err) {}
            }
        });
        try {
            await sess.send('Target.setAutoAttach', {
                autoAttach: true,
                waitForDebuggerOnStart: true,
                flatten: true
            });
        } catch (e) {}
    }

    await watch(client, 'top');

    let navErr = null;
    try {
        await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 25000 });
    } catch (e) {
        navErr = String((e && e.message) || e).slice(0, 140);
    }

    const deadline = Date.now() + waitMs;
    let poked = false;
    while (Date.now() < deadline && hits.length === 0) {
        await sleep(400);
        if (!poked && deadline - Date.now() < waitMs - 4000) {
            poked = true;
            try {
                const vp = page.viewport() || { width: 1280, height: 720 };
                await page.mouse.click(vp.width / 2, vp.height / 2);
            } catch (e) {}
            for (const f of page.frames()) {
                try {
                    await f.evaluate(() => {
                        document.querySelectorAll('video').forEach((v) => {
                            const p = v.play();
                            if (p && p.catch) p.catch(() => {});
                        });
                        const b = document.querySelector('.playBtn,.play,#play,.jw-icon-display,button');
                        if (b) b.click();
                    });
                } catch (e) {}
            }
        }
    }

    let frames = [];
    try {
        frames = page.frames().map((f) => f.url().slice(0, 90));
    } catch (e) {}

    return {
        data: {
            found: hits.length > 0,
            hits: hits.slice(0, 4),
            navErr: navErr,
            frames: frames,
            targets: targets.slice(0, 12)
        },
        type: 'application/json'
    };
}
