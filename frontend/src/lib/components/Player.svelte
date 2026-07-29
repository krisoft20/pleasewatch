<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { fly, fade } from 'svelte/transition';
    import type { MediaSubtitle, EpisodeRecord, TmdbEpisode } from '$lib/types';
    import { api } from '$lib/api';
    import EpisodeDrawer from './player/EpisodeDrawer.svelte';
    import AudioSubsPanel from './player/AudioSubsPanel.svelte';
    import PlayerSettingsPanel from './player/PlayerSettingsPanel.svelte';
    import ClipPanel from './player/ClipPanel.svelte';
    import WatchTogetherPanel from './player/WatchTogetherPanel.svelte';
    import NextEpisodeOverlay from './player/NextEpisodeOverlay.svelte';
    import CreditsOverlay from './player/CreditsOverlay.svelte';

    import Icon from './Icon.svelte';

    import {
        watchTogether,
        setRemoteCallbacks,
        setOnChatMessage,
        joinSession,
        createSession,
        leaveSession,
        sendPlay,
        sendPause,
        sendSeek,
        sendChat
    } from '$lib/stores/watchTogether';
    import type { ChatMessage } from '$lib/stores/watchTogether';
    import { t } from '$lib/i18n';

    let {
        src,
        mediaId = '',
        showMediaId = '',
        currentSeason = 0,
        currentEpisodeNum = 0,
        subtitles = [],
        title = '',
        episodeLabel = null,
        releaseName = null,
        resumePosition = 0,
        episodes = [],
        tmdbEpisodes = [],
        currentEpisodeId = null,
        onProgress,
        onEpisodeSelect,
        onBack,
        onNextEpisode = null,
        subUrlBuilder = null,
        partyMode = false,
        episodeProgress = new Map(),
        introStart = null,
        introEnd = null,
        creditsStart = null,
        tmdbId = null,
        posterUrl = null
    }: {
        src: string;
        mediaId?: string;
        showMediaId?: string;
        currentSeason?: number;
        currentEpisodeNum?: number;
        subtitles?: MediaSubtitle[];
        title?: string;
        episodeLabel?: string | null;
        releaseName?: string | null;
        resumePosition?: number;
        episodes?: EpisodeRecord[];
        tmdbEpisodes?: TmdbEpisode[];
        currentEpisodeId?: string | null;
        onProgress?: (position: number, duration: number) => void;
        onEpisodeSelect?: (ep: EpisodeRecord) => void;
        onBack?: () => void;
        onNextEpisode?: (() => void) | null;
        subUrlBuilder?: ((sub: MediaSubtitle) => string) | null;
        partyMode?: boolean;
        episodeProgress?: Map<string, import('$lib/types').WatchProgress>;
        introStart?: number | null;
        introEnd?: number | null;
        creditsStart?: number | null;
        tmdbId?: number | null;
        posterUrl?: string | null;
    } = $props();
    let videoEl = $state<HTMLVideoElement | null>(null);
    let containerEl = $state<HTMLDivElement | null>(null);
    let seekBarEl = $state<HTMLDivElement | null>(null);
    let seekTrackEl = $state<HTMLDivElement | null>(null);
    let volumeBarEl = $state<HTMLDivElement | null>(null);
    let volumeDragging = $state(false);
    let playing = $state(false);
    let currentTime = $state(0);
    let duration = $state(0);
    let buffered = $state(0);
    let volume = $state(1);
    let muted = $state(false);
    let waiting = $state(false);
    let controlsVisible = $state(true);
    let controlsTimer: ReturnType<typeof setTimeout>;
    let isFullscreen = $state(false);
    const isIPhone = typeof navigator !== 'undefined' && /iPhone|iPod/.test(navigator.userAgent);
    let showEpisodeDrawer = $state(false);
    let showSubtitlePicker = $state(false);
    let showSettings = $state(false);
    let showVolume = $state(false);
    let selectedSubIndex = $state(-1);
    let audioTracks = $state<{ id: string; label: string; language: string; codec?: string }[]>([]);
    let selectedAudioIndex = $state(0);
    let audioTracksLoaded = false;
    let videoFit = $state<'contain' | 'cover' | 'fill'>(
        (localStorage.getItem('player-fit') as any) || ('ontouchend' in window ? 'cover' : 'contain')
    );
    let subSize = $state(localStorage.getItem('player-sub-size') || 'medium');
    let subColor = $state(localStorage.getItem('player-sub-color') || 'white');
    let subBg = $state(localStorage.getItem('player-sub-bg') || 'transparent');
    let subBgColor = $state(localStorage.getItem('player-sub-bg-color') || 'black');
    let subBgOpacity = $state(localStorage.getItem('player-sub-bg-opacity') || '75');
    let subBgJoin = $state(localStorage.getItem('player-sub-bg-join') === 'true');
    let subSync = $state(parseFloat(localStorage.getItem('player-sub-sync') || '0'));
    let subYOffset = $state(parseInt(localStorage.getItem('player-sub-y') || ('ontouchend' in window ? '-30' : '0')));
    let subCacheBust = $state(Date.now());
    let showShareModal = $state(false);
    let shareCode = $state('');
    let clipMode = $state(false);
    let clipStart = $state(0);
    let clipEnd = $state(0);
    let clipCreating = $state(false);
    let clipUrl = $state('');
    let clipShowResult = $state(false);
    let showChat = $state(false);
    let chatInput = $state('');
    let unreadCount = $state(0);
    let chatToasts = $state<ChatMessage[]>([]);
    let chatScrollEl: HTMLDivElement | undefined = $state();
    let chatInputEl: HTMLInputElement | undefined = $state();
    let kbOffset = $state(0);
    let subUploadFile = $state<File | null>(null);
    let subUploadLabel = $state('');
    let subUploadLang = $state('en');
    let subUploading = $state(false);
    let subFileInput: HTMLInputElement;
    let subSearchMode = $state(false);
    let subSearchLang = $state(
        typeof localStorage !== 'undefined' ? localStorage.getItem('player-sub-search-lang') || 'en' : 'en'
    );
    $effect(() => {
        try {
            localStorage.setItem('player-sub-search-lang', subSearchLang);
        } catch {}
    });
    let subSearchResults = $state<import('$lib/types').SubSearchResult[]>([]);
    let subSearching = $state(false);
    let subSearched = $state(false);
    let subDownloading = $state<string | null>(null);
    let subJobLabel = $state<string | null>(null);
    let aiTranslating = $state(false);
    let aiTranslateErr = $state<string | null>(null);

    async function runAiTranslate() {
        if (!mediaId || aiTranslating) return;
        aiTranslating = true;
        aiTranslateErr = null;
        subJobLabel = `Translating to ${subSearchLang.toUpperCase()} with AI`;
        try {
            const sub = await api.aiTranslateSub(mediaId, subSearchLang);
            subtitles = await api.listSubtitles(mediaId);
            const idx = subtitles.findIndex((s) => s.id === sub.id);
            if (idx >= 0) selectSubtitle(idx);
            subSearched = false;
            subSearchResults = [];
        } catch (e: any) {
            console.error('[subs] ai translate failed:', e);
            aiTranslateErr = e?.message || 'translate failed';
        } finally {
            aiTranslating = false;
            subJobLabel = null;
        }
    }
    let syncMethod = $state<'ffsubsync' | 'alass' | 'whisper'>(
        typeof localStorage !== 'undefined'
            ? ((localStorage.getItem('player-sync-method') as 'ffsubsync' | 'alass' | 'whisper') ?? 'ffsubsync')
            : 'ffsubsync'
    );
    $effect(() => {
        try {
            localStorage.setItem('player-sync-method', syncMethod);
        } catch {}
    });
    let syncing = $state<string | null>(null);
    let syncErr = $state<string | null>(null);
    let seeking = $state(false);
    let seekPreviewTime = $state(0);
    let seekPreviewX = $state(0);
    let seekHovering = $state(false);
    let showPlayFlash = $state<'play' | 'pause' | null>(null);
    let layoutMode = $state<'auto' | 'mobile' | 'desktop'>(
        (localStorage.getItem('player-layout-mode') as 'auto' | 'mobile' | 'desktop') || 'auto'
    );
    let autoNextEpisode = $state(localStorage.getItem('player-auto-next') !== 'false');
    let autoSkipIntro = $state(localStorage.getItem('player-auto-skip-intro') === 'true');
    let useNativeIOSFs = $state(
        localStorage.getItem('player-ios-native-fs') === null
            ? isIPhone
            : localStorage.getItem('player-ios-native-fs') === 'true'
    );
    let currentSpeed = $state(1);
    let skipDuration = $state(parseInt(localStorage.getItem('player-skip-duration') || '5'));

    function setSpeed(s: number) {
        if (videoEl) {
            videoEl.playbackRate = s;
            currentSpeed = s;
        }
    }
    let windowWidth = $state(typeof window !== 'undefined' ? window.innerWidth : 1024);
    let windowHeight = $state(typeof window !== 'undefined' ? window.innerHeight : 768);
    let isTouch = $state(typeof window !== 'undefined' && (navigator.maxTouchPoints ?? 0) > 0);
    let isMobile = $derived(
        layoutMode === 'mobile'
            ? true
            : layoutMode === 'desktop'
              ? false
              : windowWidth < 640 || (isTouch && Math.min(windowWidth, windowHeight) < 500)
    );
    let compactHeader = $derived(isMobile || isIPhone || (isTouch && Math.min(windowWidth, windowHeight) < 500));
    let introSkipped = $state(false);
    let showSkipIntro = $derived(
        !introSkipped &&
            introStart != null &&
            introEnd != null &&
            currentTime >= introStart - 1 &&
            currentTime < introEnd &&
            introEnd - introStart >= 10
    );

    let nextEpisode = $derived(() => {
        if (!currentEpisodeId || episodes.length === 0) return null;
        const current = episodes.find((e) => e.id === currentEpisodeId);
        if (!current) return null;
        const next = episodes.find(
            (e) => e.season === current.season && e.episode === current.episode + 1 && e.status === 'ready'
        );
        if (next) return next;
        return episodes.find((e) => e.season === current.season + 1 && e.episode === 1 && e.status === 'ready') || null;
    });

    let creditsDismissed = $state(false);
    const showCreditsOverlay = $derived(
        !creditsDismissed &&
            creditsStart != null &&
            duration > 0 &&
            currentTime >= creditsStart &&
            currentTime < duration - 2
    );
    const creditsHasNext = $derived(!!onNextEpisode && !!nextEpisode());

    function skipCredits() {
        if (videoEl && duration > 0) {
            const target = Math.max(0, duration - 1.5);
            videoEl.currentTime = target;
            currentTime = target;
            sendSeek(target);
        }
        creditsDismissed = true;
    }

    function creditsNextEpisode() {
        creditsDismissed = true;
        if (onNextEpisode) onNextEpisode();
    }

    $effect(() => {
        void creditsStart;
        creditsDismissed = false;
    });

    function skipIntro() {
        if (videoEl && introEnd != null) {
            const target = Math.max(0, introEnd - 2);
            videoEl.currentTime = target;
            currentTime = target;
            sendSeek(target);
            autoPlayCount = 0;
            introSkipped = true;
        }
    }
    $effect(() => {
        if (autoSkipIntro && showSkipIntro) {
            skipIntro();
        }
    });

    function setLayoutMode(mode: 'auto' | 'mobile' | 'desktop') {
        layoutMode = mode;
        localStorage.setItem('player-layout-mode', mode);
    }
    function toggleEpisodeDrawer() {
        showEpisodeDrawer = !showEpisodeDrawer;
        showSettings = false;
        showSubtitlePicker = false;
        resetControlsTimer();
    }
    let showNextOverlay = $state(false);
    let nextEpisodeDismissed = $state(false);
    let nextCountdown = $state(5);
    let nextCountdownTimer: ReturnType<typeof setInterval>;
    let srcSwitching = false;
    let autoDownloadTriggered = false;
    let autoPlayCount = 0;
    let showStillWatching = $state(false);
    let stillWatchingTimeout: ReturnType<typeof setTimeout>;
    let progressInterval: ReturnType<typeof setInterval>;
    let resizeHandler: () => void;
    let orientationHandler: () => void;
    let iosViewportTimers: ReturnType<typeof setTimeout>[] = [];
    let viewportMeta: HTMLMetaElement | null = null;
    let viewportContent = '';
    let prankKnockActive = false;
    let prankKnockTimer: ReturnType<typeof setTimeout> | null = null;
    let prankAudioCtx: AudioContext | null = null;
    let prankKnockBuffer: AudioBuffer | null = null;

    async function initPrankKnock() {
        try {
            const me = await (await fetch('/api/auth/me', { credentials: 'include' })).json();
            if (!me?.prank_knock) return;
            prankKnockActive = true;
            const Ctx = window.AudioContext || (window as any).webkitAudioContext;
            prankAudioCtx = new Ctx();
            const resp = await fetch('/knock.mp3');
            const bytes = await resp.arrayBuffer();
            prankKnockBuffer = await prankAudioCtx.decodeAudioData(bytes);
            scheduleNextKnock();
        } catch {}
    }

    function scheduleNextKnock() {
        if (!prankKnockActive) return;
        const ms = 20000 + Math.random() * 60000;
        prankKnockTimer = setTimeout(() => {
            playKnock();
            scheduleNextKnock();
        }, ms);
    }

    function playKnock() {
        if (!prankAudioCtx || !prankKnockBuffer) return;
        try {
            const src = prankAudioCtx.createBufferSource();
            src.buffer = prankKnockBuffer;
            const panner = prankAudioCtx.createStereoPanner();
            panner.pan.value = Math.random() * 2 - 1;
            const gain = prankAudioCtx.createGain();
            gain.gain.value = 0.8;
            src.connect(panner).connect(gain).connect(prankAudioCtx.destination);
            src.start();
        } catch {}
    }
    let prevHtmlOverflow = '';
    let prevBodyOverflow = '';
    let prevHtmlGutter = '';

    function syncViewportSize() {
        windowWidth = window.innerWidth;
        windowHeight = window.innerHeight;
    }

    function resetIOSViewport() {
        if (!viewportMeta) return;
        viewportMeta.content = 'width=device-width, initial-scale=1, maximum-scale=1, viewport-fit=cover';
        window.scrollTo(0, 0);
        requestAnimationFrame(() => {
            if (!viewportMeta) return;
            syncViewportSize();
            requestAnimationFrame(() => {
                if (!viewportMeta) return;
                viewportMeta.content = viewportContent;
                syncViewportSize();
                window.scrollTo(0, 0);
            });
        });
    }

    function settleIOSViewport() {
        if (!isIPhone) return;
        for (const timer of iosViewportTimers) clearTimeout(timer);
        resetIOSViewport();
        iosViewportTimers = [setTimeout(resetIOSViewport, 180), setTimeout(resetIOSViewport, 650)];
    }

    onMount(() => {
        prevHtmlOverflow = document.documentElement.style.overflow;
        prevBodyOverflow = document.body.style.overflow;
        prevHtmlGutter = document.documentElement.style.scrollbarGutter;
        document.documentElement.style.overflow = 'hidden';
        document.body.style.overflow = 'hidden';
        document.documentElement.style.scrollbarGutter = 'auto';
        viewportMeta = document.querySelector('meta[name="viewport"]');
        viewportContent = viewportMeta?.content ?? '';
        const savedVol = localStorage.getItem('player-volume');
        const savedMuted = localStorage.getItem('player-muted');
        if (savedVol) volume = parseFloat(savedVol);
        if (savedMuted) muted = savedMuted === 'true';

        initPrankKnock();
        if ('mediaSession' in navigator) {
            navigator.mediaSession.setActionHandler('play', () => {
                if (videoEl) {
                    videoEl.play();
                    playing = true;
                }
            });
            navigator.mediaSession.setActionHandler('pause', () => {
                if (videoEl) {
                    videoEl.pause();
                    playing = false;
                }
            });
            navigator.mediaSession.setActionHandler('seekbackward', () => skip(-skipDuration));
            navigator.mediaSession.setActionHandler('seekforward', () => skip(skipDuration));
            navigator.mediaSession.playbackState = 'playing';
        }
        progressInterval = setInterval(() => {
            if (videoEl && onProgress && duration > 0) {
                onProgress(videoEl.currentTime, duration);
            }
        }, 10000);
        document.addEventListener('fullscreenchange', onFullscreenChange);
        document.addEventListener('webkitfullscreenchange', onFullscreenChange);
        document.addEventListener('visibilitychange', onVisibilityChange);
        if (isIPhone && videoEl) {
            videoEl.addEventListener('webkitbeginfullscreen', () => {
                isFullscreen = true;
                const t = currentSubTrack();
                if (t) t.mode = 'showing';
            });
            videoEl.addEventListener('webkitendfullscreen', () => {
                isFullscreen = false;
                const t = currentSubTrack();
                if (t) t.mode = 'hidden';
                settleIOSViewport();
            });
        }
        resizeHandler = syncViewportSize;
        orientationHandler = () => {
            if (!isFullscreen) settleIOSViewport();
        };
        window.addEventListener('resize', resizeHandler);
        window.addEventListener('orientationchange', orientationHandler);
        if (typeof window !== 'undefined' && window.visualViewport) {
            const vv = window.visualViewport;
            const onVv = () => {
                kbOffset = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
            };
            vv.addEventListener('resize', onVv);
            vv.addEventListener('scroll', onVv);
        }
        const savedFit = localStorage.getItem('player-fit');
        if (savedFit) videoFit = savedFit as typeof videoFit;
        setOnChatMessage((msg) => {
            unreadCount++;
            if (showChat) {
                setTimeout(() => chatScrollEl?.scrollTo(0, chatScrollEl.scrollHeight), 50);
            }
            if (isMobile) return;
            if (showChat) return;
            chatToasts = [...chatToasts.slice(-2), msg];
            setTimeout(() => {
                chatToasts = chatToasts.filter((t) => t !== msg);
            }, 5000);
        });
        setRemoteCallbacks({
            onPlay(time: number) {
                if (!videoEl) return;
                if (Math.abs(videoEl.currentTime - time) > 1) videoEl.currentTime = time;
                videoEl.play();
                playing = true;
            },
            onPause(time: number) {
                if (!videoEl) return;
                videoEl.currentTime = time;
                currentTime = time;
                videoEl.pause();
                playing = false;
            },
            onSeek(time: number) {
                if (!videoEl) return;
                videoEl.currentTime = time;
                currentTime = time;
            }
        });
        loadStream();
    });
    let watchTickTimer: ReturnType<typeof setInterval> | null = null;
    const WATCH_TICK_SECONDS = 15;

    function startWatchTicks() {
        if (watchTickTimer) return;
        watchTickTimer = setInterval(() => {
            if (!videoEl || videoEl.paused || videoEl.ended) return;
            if (document.visibilityState === 'hidden') return;
            const owner = showMediaId || mediaId;
            if (!owner || !Number.isFinite(videoEl.duration) || videoEl.duration <= 0) return;
            fetch('/api/watch/tick', {
                method: 'POST',
                credentials: 'include',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    media_id: owner,
                    position: videoEl.currentTime,
                    duration: videoEl.duration
                })
            }).catch(() => {});
        }, WATCH_TICK_SECONDS * 1000);
    }

    function stopWatchTicks() {
        if (!watchTickTimer) return;
        clearInterval(watchTickTimer);
        watchTickTimer = null;
    }

    $effect(() => {
        if (playing) startWatchTicks();
        else stopWatchTicks();
    });

    onDestroy(() => {
        document.documentElement.style.overflow = prevHtmlOverflow;
        document.body.style.overflow = prevBodyOverflow;
        document.documentElement.style.scrollbarGutter = prevHtmlGutter;

        stopWatchTicks();
        clearInterval(progressInterval);
        clearInterval(nextCountdownTimer);
        clearTimeout(controlsTimer);
        clearTimeout(stillWatchingTimeout);
        prankKnockActive = false;
        if (prankKnockTimer) clearTimeout(prankKnockTimer);
        if (prankAudioCtx) {
            prankAudioCtx.close();
            prankAudioCtx = null;
        }
        document.removeEventListener('fullscreenchange', onFullscreenChange);
        document.removeEventListener('webkitfullscreenchange', onFullscreenChange);
        document.removeEventListener('visibilitychange', onVisibilityChange);
        try {
            (screen.orientation as any)?.unlock?.();
        } catch {}
        if (resizeHandler) window.removeEventListener('resize', resizeHandler);
        if (orientationHandler) window.removeEventListener('orientationchange', orientationHandler);
        for (const timer of iosViewportTimers) clearTimeout(timer);
        if (viewportMeta) viewportMeta.content = viewportContent;
        viewportMeta = null;
        if (videoEl && onProgress && duration > 0) {
            onProgress(videoEl.currentTime, duration);
        }
        leaveSession();
    });
    async function loadStream() {
        if (!videoEl) return;
        const frag = resumePosition > 0 ? `#t=${Math.floor(resumePosition)}` : '';
        selectedAudioIndex = 0;
        videoEl.src = src + frag;
        srcSwitching = false;

        const idx = await pickInitialAudioIndex();
        if (idx > 0 && videoEl) {
            selectedAudioIndex = idx;
            videoEl.src = `${src}?audio=${idx}${frag}`;
        }
    }

    async function pickInitialAudioIndex(): Promise<number> {
        if (!mediaId) return 0;
        const showKey = showMediaId || mediaId;
        const saved = localStorage.getItem(`audio-lang-${showKey}`) || localStorage.getItem('player-audio-lang');
        if (!saved) return 0;
        try {
            const tracks = await api.audioTracks(mediaId);
            if (tracks.length <= 1) return 0;
            const idx = tracks.findIndex((t) => t.language === saved);
            return idx >= 0 ? idx : 0;
        } catch {
            return 0;
        }
    }
    function onLoadedData() {
        duration = videoEl?.duration || 0;
        if (resumePosition > 0 && videoEl) {
            videoEl.currentTime = resumePosition;
        }
        if (videoEl) {
            currentTime = videoEl.currentTime;
            videoEl.volume = volume;
            videoEl.muted = muted;
            loadAudioTracks();
            if (!$watchTogether.active) {
                videoEl
                    .play()
                    .then(() => {
                        playing = true;
                        autoFullscreenMobile();
                        resetControlsTimer();
                    })
                    .catch(() => {});
            }
            applyDefaultSubSelection();
        }
    }

    let didAutoSelectSubs = $state(false);

    function applyDefaultSubSelection() {
        if (didAutoSelectSubs) return;
        if (subtitles.length === 0 && !localStorage.getItem('player-sub-label')) return;
        const savedLabel = localStorage.getItem('player-sub-label');
        if (savedLabel === '__off__') {
            selectedSubIndex = -1;
            didAutoSelectSubs = true;
            return;
        }
        if (subtitles.length === 0) return;
        if (savedLabel) {
            const idx = subtitles.findIndex((s) => s.label === savedLabel);
            if (idx >= 0) {
                selectSubtitle(idx);
                didAutoSelectSubs = true;
                return;
            }
        }
        const engIdx = subtitles.findIndex(
            (s) => s.language.toLowerCase().startsWith('en') || s.label.toLowerCase() === 'english'
        );
        selectSubtitle(engIdx >= 0 ? engIdx : 0);
        didAutoSelectSubs = true;
    }

    $effect(() => {
        if (subtitles.length > 0 && videoEl) applyDefaultSubSelection();
    });

    $effect(() => {
        src;
        didAutoSelectSubs = false;
    });

    function onTimeUpdate() {
        if (!seeking && videoEl) {
            currentTime = videoEl.currentTime;
            const lastChanceThreshold = creditsStart != null ? 10 : 15;
            if (
                autoNextEpisode &&
                onNextEpisode &&
                nextEpisode() &&
                duration > 0 &&
                duration - currentTime < lastChanceThreshold &&
                !showNextOverlay &&
                !nextEpisodeDismissed
            ) {
                triggerNextEpisode();
            }
            if (showMediaId && currentSeason > 0 && currentEpisodeNum > 0 && duration > 0 && !autoDownloadTriggered) {
                const progress = currentTime / duration;
                /*
        if (progress > 0.5) {
          autoDownloadTriggered = true;
          console.log("[binge] auto-downloading next episode");
          api
            .autoDownloadNext(showMediaId, currentSeason, currentEpisodeNum)
            .then((r) => {
              if (r.status === "downloading") {
                console.log(
                  `[binge] queued S${currentSeason}E${r.episode}: ${r.torrent}`,
                );
              }
            })
            .catch(() => {});
        }
        */
            }
        }
        updateBuffered();
    }

    function onVideoEnded() {
        playing = false;
        waiting = false;
        tapCount = 0;
        tapSide = null;
        clearTimeout(singleTapTimer);
        clearTimeout(holdSpeedTimer);
        showControlsNow();
        if (isIPhone && useNativeIOSFs && isFullscreen) exitFullscreen();
        if (onNextEpisode && nextEpisode()) {
            triggerNextEpisode();
        }
    }

    function updateBuffered() {
        if (videoEl && videoEl.buffered.length > 0) {
            buffered = videoEl.buffered.end(videoEl.buffered.length - 1);
        }
    }
    let hasAutoFsOnce = $state(false);

    function togglePlay() {
        if (!videoEl) return;
        autoPlayCount = 0;
        if (playing) {
            videoEl.pause();
            if (!isMobile) flashIcon('pause');
            sendPause(videoEl.currentTime);
        } else {
            videoEl.play();
            if (isIPhone && useNativeIOSFs && !hasAutoFsOnce && !isFullscreen) {
                hasAutoFsOnce = true;
                (videoEl as any)?.webkitEnterFullscreen?.();
                isFullscreen = true;
            }
            if (!isMobile) flashIcon('play');
            sendPlay(videoEl.currentTime);
        }
        playing = !playing;
        resetControlsTimer();
    }

    function flashIcon(type: 'play' | 'pause') {
        showPlayFlash = type;
        setTimeout(() => {
            showPlayFlash = null;
        }, 350);
    }

    function skip(seconds: number) {
        if (!videoEl) return;
        videoEl.currentTime = Math.max(0, Math.min(duration, videoEl.currentTime + seconds));
        currentTime = videoEl.currentTime;
        sendSeek(currentTime);
        resetControlsTimer();
    }

    function setVideoFit(fit: typeof videoFit) {
        videoFit = fit;
        localStorage.setItem('player-fit', fit);
        resetControlsTimer();
    }
    async function startWatchTogether() {
        try {
            const actualMediaId = showMediaId || mediaId;
            const episodeId = showMediaId ? mediaId : undefined;
            const code = await createSession(actualMediaId, episodeId);
            shareCode = code;
            showShareModal = true;
            const hostName = (localStorage.getItem('party-name') || 'Host').trim() || 'Host';
            joinSession(code, hostName, true, videoEl?.currentTime ?? 0);
        } catch (e) {
            console.error('[party] create session failed:', e);
        }
    }

    function copyShareLink() {
        const link = `${window.location.origin}/party/${shareCode}`;
        navigator.clipboard.writeText(link).catch(() => {});
    }
    function enterClipMode() {
        clipMode = true;
        clipStart = currentTime;
        clipEnd = Math.min(duration, currentTime + 15);
        clipUrl = '';
        clipShowResult = false;
        if (videoEl) videoEl.pause();
        playing = false;
    }

    function exitClipMode() {
        clipMode = false;
        clipShowResult = false;
    }

    async function createClip() {
        if (clipEnd <= clipStart) return;
        clipCreating = true;
        try {
            const result = await api.createClip({
                media_id: showMediaId || mediaId,
                episode_id: currentEpisodeId || null,
                start: clipStart,
                end: clipEnd,
                subtitle_id: null
            });
            clipUrl = `${window.location.origin}${result.url}.mp4`;
            clipShowResult = true;
        } catch (e) {
            console.error('[clip] create failed:', e);
        } finally {
            clipCreating = false;
        }
    }

    async function copyClipLink() {
        try {
            await navigator.clipboard.writeText(clipUrl);
        } catch {}
        exitClipMode();
    }

    function handleSendChat() {
        if (!chatInput.trim()) return;
        sendChat(chatInput);
        chatInput = '';
    }

    function openChat() {
        if (showChat) {
            showChat = false;
            return;
        }
        showChat = true;
        showSettings = false;
        showSubtitlePicker = false;
        showEpisodeDrawer = false;
        unreadCount = 0;
        chatToasts = [];
        resetControlsTimer();
        setTimeout(() => {
            chatScrollEl?.scrollTo(0, chatScrollEl.scrollHeight);
            chatInputEl?.focus({ preventScroll: true });
        }, 60);
    }
    function onSubFileSelect(e: Event) {
        const input = e.target as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        subUploadFile = file;
        const name = file.name.replace(/\.[^.]+$/, '');
        subUploadLabel = name;
    }

    async function doSubUpload() {
        if (!subUploadFile || !mediaId) return;
        subUploading = true;
        try {
            const newSub = await api.uploadSubtitle(mediaId, subUploadFile, subUploadLang, subUploadLabel);
            subtitles = [...subtitles, newSub];
            subCacheBust++;
            await new Promise((r) => setTimeout(r, 100));
            selectSubtitle(subtitles.length - 1);
            subUploadFile = null;
            subUploadLabel = '';
        } catch (e) {
            console.error('[subs] upload failed:', e);
        } finally {
            subUploading = false;
        }
    }

    async function deleteSubtitle(subId: string, index: number) {
        try {
            await api.deleteSubtitle(mediaId, subId);
            subtitles = subtitles.filter((s) => s.id !== subId);
            if (selectedSubIndex === index) {
                selectSubtitle(-1);
                subCacheBust++;
            } else if (selectedSubIndex > index) selectedSubIndex--;
        } catch (e) {
            console.error('[subs] delete failed:', e);
        }
    }

    async function runSync(subId: string) {
        if (syncing) return;
        syncing = subId;
        syncErr = null;
        const wasSelected = selectedSubIndex >= 0 && subtitles[selectedSubIndex]?.id === subId;
        try {
            if (syncMethod === 'alass') await api.syncSubtitleAlass(subId);
            else if (syncMethod === 'whisper') await api.syncSubtitleWhisper(subId);
            else await api.syncSubtitle(subId);
            subCacheBust++;
            if (wasSelected) {
                await new Promise((r) => setTimeout(r, 200));
                const idx = subtitles.findIndex((s) => s.id === subId);
                if (idx >= 0) selectSubtitle(idx);
            }
        } catch (e: any) {
            console.error('[subs] sync failed:', e);
            syncErr = e?.message || String(e);
        } finally {
            syncing = null;
        }
    }

    async function searchSubDL() {
        if (!mediaId) return;
        subSearching = true;
        subSearchResults = [];
        try {
            subSearchResults = await api.searchSubtitles(mediaId, subSearchLang, releaseName);
        } catch (e) {
            console.error('[subs] subdl search failed:', e);
        } finally {
            subSearching = false;
            subSearched = true;
        }
    }

    async function downloadSubResult(result: import('$lib/types').SubSearchResult) {
        if (!mediaId) return;
        subDownloading = result.url;
        subJobLabel = 'Fetching subtitle';
        const stageTimer = setTimeout(() => {
            subJobLabel = 'Syncing to audio';
        }, 2500);
        try {
            const langNames: Record<string, string> = {
                en: 'English',
                pl: 'Polish',
                ar: 'Arabic',
                es: 'Spanish',
                fr: 'French',
                de: 'German',
                ja: 'Japanese',
                ko: 'Korean',
                pt: 'Portuguese',
                ru: 'Russian',
                zh: 'Chinese',
                tr: 'Turkish',
                it: 'Italian'
            };
            const lang = result.language.toLowerCase();
            const label = `${langNames[lang] || result.language}${result.source ? ` (${result.source})` : ''}`;
            const newSub = await api.downloadSubtitle(mediaId, result.url, lang, label);
            subtitles = [...subtitles, newSub];
            subCacheBust++;
            subSearchMode = false;
            subSearchResults = [];
            await new Promise((r) => setTimeout(r, 100));
            selectSubtitle(subtitles.length - 1);
        } catch (e) {
            console.error('[subs] download failed:', e);
        } finally {
            clearTimeout(stageTimer);
            subDownloading = null;
            subJobLabel = null;
        }
    }

    function setVolume(v: number) {
        volume = Math.max(0, Math.min(1, v));
        if (videoEl) videoEl.volume = volume;
        muted = volume === 0;
        if (videoEl) videoEl.muted = muted;
        localStorage.setItem('player-volume', String(volume));
        localStorage.setItem('player-muted', String(muted));
    }

    function onVolumeClick(e: MouseEvent) {
        if (!volumeBarEl) return;
        const rect = volumeBarEl.getBoundingClientRect();
        setVolume((e.clientX - rect.left) / rect.width);
    }

    function onVolumeDragStart(e: MouseEvent) {
        volumeDragging = true;
        onVolumeClick(e);
        window.addEventListener('mousemove', onVolumeDragMove);
        window.addEventListener('mouseup', onVolumeDragEnd);
    }

    function onVolumeDragMove(e: MouseEvent) {
        if (!volumeBarEl || !volumeDragging) return;
        const rect = volumeBarEl.getBoundingClientRect();
        setVolume(Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width)));
    }

    function onVolumeDragEnd() {
        volumeDragging = false;
        window.removeEventListener('mousemove', onVolumeDragMove);
        window.removeEventListener('mouseup', onVolumeDragEnd);
    }

    function onVolumeKeydown(e: KeyboardEvent) {
        const step = e.shiftKey ? 0.1 : 0.05;
        if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') setVolume(volume - step);
        else if (e.key === 'ArrowRight' || e.key === 'ArrowUp') setVolume(volume + step);
        else if (e.key === 'Home') setVolume(0);
        else if (e.key === 'End') setVolume(1);
        else return;
        e.preventDefault();
        e.stopPropagation();
    }

    function toggleMute() {
        muted = !muted;
        if (videoEl) videoEl.muted = muted;
        localStorage.setItem('player-muted', String(muted));
    }

    function toggleFullscreen() {
        if (!containerEl) return;
        if (isFullscreen) {
            exitFullscreen();
        } else {
            enterFullscreen();
        }
    }

    async function enterFullscreen() {
        if (isIPhone && !useNativeIOSFs) {
            isFullscreen = true;
            window.scrollTo(0, 1);
            return;
        }
        if (isIPhone && useNativeIOSFs && (videoEl as any)?.webkitEnterFullscreen) {
            (videoEl as any).webkitEnterFullscreen();
            isFullscreen = true;
            return;
        }
        if (containerEl?.requestFullscreen) {
            try {
                await containerEl.requestFullscreen();
            } catch {}
        } else if ((containerEl as any)?.webkitRequestFullscreen) {
            (containerEl as any).webkitRequestFullscreen();
        } else if ((videoEl as any)?.webkitEnterFullscreen) {
            (videoEl as any).webkitEnterFullscreen();
        }
        try {
            await (screen.orientation as any)?.lock?.('landscape');
        } catch {}
    }

    function exitFullscreen() {
        if (isIPhone && !useNativeIOSFs) {
            isFullscreen = false;
            return;
        }
        if (isIPhone && useNativeIOSFs && (videoEl as any)?.webkitExitFullscreen) {
            (videoEl as any).webkitExitFullscreen();
            isFullscreen = false;
            return;
        }
        if (document.fullscreenElement) {
            document.exitFullscreen().catch(() => {});
        } else if ((document as any).webkitFullscreenElement) {
            (document as any).webkitExitFullscreen();
        } else if ((videoEl as any)?.webkitExitFullscreen) {
            (videoEl as any).webkitExitFullscreen();
        }
        try {
            (screen.orientation as any)?.unlock?.();
        } catch {}
    }

    function onFullscreenChange() {
        if (isIPhone) return;
        isFullscreen = !!(document.fullscreenElement || (document as any).webkitFullscreenElement);
    }
    function autoFullscreenMobile() {
        if (!('ontouchend' in window)) return;
        setTimeout(() => enterFullscreen(), 100);
    }
    function resetControlsTimer() {
        controlsVisible = true;
        clearTimeout(controlsTimer);
        if (playing && !showSubtitlePicker && !showEpisodeDrawer && !showVolume && !showSettings) {
            controlsTimer = setTimeout(
                () => {
                    controlsVisible = false;
                    showSubtitlePicker = false;
                    showSettings = false;
                    showEpisodeDrawer = false;
                },
                isMobile ? 3500 : 3000
            );
        }
    }

    function showControlsNow() {
        controlsVisible = true;
        controlsShownAt = Date.now();
        clearTimeout(controlsTimer);
    }

    function onContainerMouseMove() {
        if ('ontouchend' in window) return;
        resetControlsTimer();
    }

    function onContainerMouseLeave() {
        if ('ontouchend' in window) return;
        if (playing) {
            controlsTimer = setTimeout(() => {
                controlsVisible = false;
            }, 1000);
        }
    }
    function onSeekBarMouseDown(e: MouseEvent) {
        seeking = true;
        updateSeekFromMouse(e);
        window.addEventListener('mousemove', onSeekBarMouseMove);
        window.addEventListener('mouseup', onSeekBarMouseUp);
    }

    function onSeekBarMouseMove(e: MouseEvent) {
        if (seeking) updateSeekFromMouse(e);
    }

    function onSeekBarMouseUp(e: MouseEvent) {
        if (seeking && videoEl) {
            updateSeekFromMouse(e);
            videoEl.currentTime = currentTime;
            sendSeek(currentTime);
            autoPlayCount = 0;
        }
        seeking = false;
        window.removeEventListener('mousemove', onSeekBarMouseMove);
        window.removeEventListener('mouseup', onSeekBarMouseUp);
    }

    function onSeekBarHover(e: MouseEvent) {
        if (!seekTrackEl || !seekBarEl || !controlsVisible) return;
        const trackRect = seekTrackEl.getBoundingClientRect();
        const barRect = seekBarEl.getBoundingClientRect();
        const ratio = Math.max(0, Math.min(1, (e.clientX - trackRect.left) / trackRect.width));
        seekPreviewTime = ratio * duration;
        seekPreviewX = e.clientX - barRect.left;
        seekHovering = true;
    }

    function onSeekBarLeave() {
        seekHovering = false;
    }

    function updateSeekFromMouse(e: MouseEvent) {
        if (!seekTrackEl) return;
        const rect = seekTrackEl.getBoundingClientRect();
        const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        currentTime = ratio * duration;
    }
    function seekTo(time: number) {
        if (!videoEl || duration <= 0) return;
        currentTime = Math.max(0, Math.min(duration, time));
        videoEl.currentTime = currentTime;
        sendSeek(currentTime);
        autoPlayCount = 0;
    }
    function onSeekBarKeydown(e: KeyboardEvent) {
        const step = e.shiftKey ? 30 : 10;
        if (e.key === 'ArrowLeft') seekTo(currentTime - step);
        else if (e.key === 'ArrowRight') seekTo(currentTime + step);
        else if (e.key === 'Home') seekTo(0);
        else if (e.key === 'End') seekTo(duration);
        else return;
        e.preventDefault();
        e.stopPropagation();
    }
    function onSeekBarTouch(e: TouchEvent) {
        if (!seekTrackEl || !e.touches[0]) return;
        const rect = seekTrackEl.getBoundingClientRect();
        const ratio = Math.max(0, Math.min(1, (e.touches[0].clientX - rect.left) / rect.width));
        currentTime = ratio * duration;
        if (videoEl) {
            videoEl.currentTime = currentTime;
            sendSeek(currentTime);
        }
    }
    async function loadAudioTracks() {
        if (!mediaId || audioTracksLoaded) return;
        audioTracksLoaded = true;
        try {
            const tracks = await api.audioTracks(mediaId);
            if (tracks.length <= 1) {
                audioTracks = [];
                return;
            }
            audioTracks = tracks.map((t) => ({
                id: String(t.index),
                label: t.label,
                language: t.language,
                codec: t.codec
            }));
            const showKey = showMediaId || mediaId;
            const savedShowLang = localStorage.getItem(`audio-lang-${showKey}`);
            const savedGlobalLang = localStorage.getItem('player-audio-lang');
            const savedLang = savedShowLang || savedGlobalLang;
            if (savedLang) {
                const idx = audioTracks.findIndex((t) => t.language === savedLang);
                if (idx >= 0) selectedAudioIndex = idx;
            }
        } catch {
            audioTracks = [];
        }
    }

    function selectAudio(index: number) {
        if (index === selectedAudioIndex) return;
        selectedAudioIndex = index;
        const lang = audioTracks[index]?.language || '';
        localStorage.setItem('player-audio-lang', lang);
        const showKey = showMediaId || mediaId;
        if (showKey) localStorage.setItem(`audio-lang-${showKey}`, lang);
        if (videoEl) {
            const pos = videoEl.currentTime;
            const wasPlaying = playing;
            const baseUrl = src.split('?')[0];
            const newSrc = index === 0 ? baseUrl : `${baseUrl}?audio=${index}`;
            waiting = true;
            videoEl.src = newSrc;
            videoEl.addEventListener(
                'loadeddata',
                () => {
                    videoEl!.currentTime = pos;
                    if (wasPlaying) videoEl!.play().catch(() => {});
                    waiting = false;
                },
                { once: true }
            );
            videoEl.load();
        }
    }

    function langName(code: string): string {
        const map: Record<string, string> = {
            en: 'English',
            eng: 'English',
            ja: 'Japanese',
            jpn: 'Japanese',
            jap: 'Japanese',
            pl: 'Polish',
            pol: 'Polish',
            es: 'Spanish',
            spa: 'Spanish',
            fr: 'French',
            fra: 'French',
            fre: 'French',
            de: 'German',
            ger: 'German',
            deu: 'German',
            it: 'Italian',
            ita: 'Italian',
            pt: 'Portuguese',
            por: 'Portuguese',
            ru: 'Russian',
            rus: 'Russian',
            zh: 'Chinese',
            chi: 'Chinese',
            zho: 'Chinese',
            ko: 'Korean',
            kor: 'Korean',
            ar: 'Arabic',
            ara: 'Arabic'
        };
        return map[code?.toLowerCase()] || code;
    }
    let activeCueLines = $state<string[]>([]);

    function currentSubTrack(index = selectedSubIndex): TextTrack | null {
        if (!videoEl || index < 0) return null;
        return videoEl.querySelectorAll('track')[index]?.track ?? null;
    }

    function updateActiveCues() {
        if (!videoEl || selectedSubIndex < 0) {
            activeCueLines = [];
            return;
        }
        const track = currentSubTrack();
        if (!track || !track.activeCues || track.activeCues.length === 0) {
            activeCueLines = [];
            return;
        }
        const items: string[] = [];
        for (let i = 0; i < track.activeCues.length; i++) {
            const cue = track.activeCues[i] as VTTCue;
            const lines = cue.text
                .split(/\r?\n/)
                .map((l) => l.trim())
                .filter((l) => l.length > 0)
                .map((l) => sanitizeCueLine(l));
            if (lines.length === 0) continue;
            if (subBgJoin) {
                items.push(lines.join('<br>'));
            } else {
                for (const line of lines) items.push(line);
            }
        }
        activeCueLines = items;
    }

    function sanitizeCueLine(line: string): string {
        const escaped = line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
        return escaped.replace(/&lt;(\/?)(i|b|u)&gt;/gi, '<$1$2>');
    }

    let cueLoadPoller: ReturnType<typeof setInterval>;

    function selectSubtitle(index: number) {
        selectedSubIndex = index;
        subtitleOffset = 0;
        clearInterval(cueLoadPoller);
        if (videoEl) {
            for (let i = 0; i < videoEl.textTracks.length; i++) {
                const track = videoEl.textTracks[i];
                track.mode = 'disabled';
                track.oncuechange = null;
            }
            const track = currentSubTrack(index);
            if (track) {
                track.mode = 'hidden';
                track.oncuechange = updateActiveCues;
            }
            updateActiveCues();
            if (index >= 0) {
                let attempts = 0;
                cueLoadPoller = setInterval(() => {
                    attempts++;
                    const t = currentSubTrack(index);
                    if (t?.cues && t.cues.length > 0) {
                        clearInterval(cueLoadPoller);
                        updateActiveCues();
                    } else if (attempts > 50) {
                        clearInterval(cueLoadPoller);
                    }
                }, 100);
            }
        }
        if (index >= 0 && subtitles[index]) {
            localStorage.setItem('player-sub-label', subtitles[index].label);
        } else {
            localStorage.setItem('player-sub-label', '__off__');
        }
        showSubtitlePicker = false;
        resetControlsTimer();
    }
    function triggerNextEpisode() {
        if (showNextOverlay || srcSwitching) return;
        autoPlayCount++;
        if (autoPlayCount >= 5) {
            showStillWatching = true;
            clearTimeout(stillWatchingTimeout);
            stillWatchingTimeout = setTimeout(() => {
                if (showStillWatching) {
                    showStillWatching = false;
                    if (videoEl) {
                        videoEl.pause();
                        playing = false;
                    }
                    exitFullscreen();
                }
            }, 120000);
            return;
        }

        showNextOverlay = true;
        nextCountdown = 15;
        clearInterval(nextCountdownTimer);
        nextCountdownTimer = setInterval(() => {
            nextCountdown--;
            if (nextCountdown <= 0) {
                clearInterval(nextCountdownTimer);
                showNextOverlay = false;
                onNextEpisode?.();
            }
        }, 1000);
    }

    function cancelNextEpisode() {
        showNextOverlay = false;
        nextEpisodeDismissed = true;
        clearInterval(nextCountdownTimer);
    }

    function playNextNow() {
        showNextOverlay = false;
        clearInterval(nextCountdownTimer);
        onNextEpisode?.();
    }

    function stillWatchingContinue() {
        showStillWatching = false;
        clearTimeout(stillWatchingTimeout);
        autoPlayCount = 0;
        showNextOverlay = true;
        nextCountdown = 15;
        clearInterval(nextCountdownTimer);
        nextCountdownTimer = setInterval(() => {
            nextCountdown--;
            if (nextCountdown <= 0) {
                clearInterval(nextCountdownTimer);
                showNextOverlay = false;
                onNextEpisode?.();
            }
        }, 1000);
    }

    function stillWatchingStop() {
        showStillWatching = false;
        clearTimeout(stillWatchingTimeout);
        autoPlayCount = 0;
        if (videoEl) {
            videoEl.pause();
            playing = false;
        }
        exitFullscreen();
    }

    function resetSpaceHold() {
        spaceHeld = false;
        clearTimeout(holdSpeedTimer);
        if (holdSpeedActive) stopHoldSpeed();
    }

    function onVisibilityChange() {
        if (document.visibilityState === 'hidden') resetSpaceHold();
    }

    function onKeydown(e: KeyboardEvent) {
        if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

        switch (e.key) {
            case ' ':
                e.preventDefault();
                e.stopPropagation();
                if (spaceHeld) return;
                spaceHeld = true;
                clearTimeout(holdSpeedTimer);
                clearTimeout(singleClickTimer);
                lastClickTime = 0;
                holdSpeedTimer = setTimeout(() => startHoldSpeed(), 500);
                return;
            case 'k':
                e.preventDefault();
                togglePlay();
                break;
            case 'ArrowLeft':
            case 'j':
                e.preventDefault();
                skip(-skipDuration);
                break;
            case 'ArrowRight':
            case 'l':
                e.preventDefault();
                skip(skipDuration);
                break;
            case 'ArrowUp':
                e.preventDefault();
                setVolume(volume + 0.1);
                break;
            case 'ArrowDown':
                e.preventDefault();
                setVolume(volume - 0.1);
                break;
            case 'f':
                e.preventDefault();
                toggleFullscreen();
                break;
            case 'm':
                e.preventDefault();
                toggleMute();
                break;
            case ',':
                e.preventDefault();
                frameStep(false);
                break;
            case '.':
                e.preventDefault();
                frameStep(true);
                break;
            case 'z':
                e.preventDefault();
                shiftSubtitles(0.1);
                break;
            case 'x':
                e.preventDefault();
                shiftSubtitles(-0.1);
                break;
            case '[':
                e.preventDefault();
                adjustSpeed(-0.1);
                break;
            case ']':
                e.preventDefault();
                adjustSpeed(0.1);
                break;
            case 'Escape':
                if (showEpisodeDrawer) showEpisodeDrawer = false;
                else if (showSubtitlePicker) showSubtitlePicker = false;
                else if (isFullscreen) document.exitFullscreen?.();
                break;
        }
    }

    function onKeyup(e: KeyboardEvent) {
        if (e.key === ' ') {
            e.preventDefault();
            e.stopPropagation();
            if (!spaceHeld) return;
            spaceHeld = false;
            clearTimeout(holdSpeedTimer);
            if (holdSpeedActive) {
                stopHoldSpeed();
            } else {
                togglePlay();
            }
        }
    }
    let lastTapTime = 0;
    let tapCount = 0;
    let tapSide: 'left' | 'right' | null = null;
    let singleTapTimer: ReturnType<typeof setTimeout>;
    let controlsShownAt = 0;

    let skipFlash = $state<string | null>(null);
    let skipFlashSide = $state<'left' | 'right'>('right');
    let skipFlashTimer: ReturnType<typeof setTimeout>;
    let holdSpeedActive = $state(false);
    let holdSpeedTimer: ReturnType<typeof setTimeout>;
    let savedPlaybackRate = 1;
    let holdSpeedUsed = false;
    let spaceHeld = false;
    let singleClickTimer: ReturnType<typeof setTimeout>;
    let lastClickTime = 0;
    let subtitleOffset = 0;

    function frameStep(forward: boolean) {
        if (!videoEl) return;
        if (!videoEl.paused) videoEl.pause();
        const step = 1 / 30;
        const target = videoEl.currentTime + (forward ? step : -step);
        videoEl.currentTime = Math.max(0, Math.min(duration || Infinity, target));
    }

    let subShiftFlash = $state<number | null>(null);
    let subShiftFlashTimer: ReturnType<typeof setTimeout>;

    function shiftSubtitles(delta: number) {
        if (!videoEl || selectedSubIndex < 0) return;
        const track = currentSubTrack();
        if (!track || !track.cues) return;
        for (let i = 0; i < track.cues.length; i++) {
            const cue = track.cues[i] as VTTCue;
            cue.startTime = Math.max(0, cue.startTime + delta);
            cue.endTime = Math.max(0, cue.endTime + delta);
        }
        subtitleOffset += delta;
        subSync = Math.round((subSync + delta) * 10) / 10;
        try {
            localStorage.setItem('player-sub-sync', String(subSync));
        } catch {}
        subShiftFlash = subSync;
        clearTimeout(subShiftFlashTimer);
        subShiftFlashTimer = setTimeout(() => {
            subShiftFlash = null;
        }, 1200);
        updateActiveCues();
    }

    function adjustSpeed(delta: number) {
        const next = Math.round((currentSpeed + delta) * 100) / 100;
        setSpeed(Math.max(0.25, Math.min(4, next)));
    }

    function startHoldSpeed() {
        if (!videoEl || holdSpeedActive) return;
        if (videoEl.paused) videoEl.play();
        savedPlaybackRate = currentSpeed;
        videoEl.playbackRate = 2;
        holdSpeedActive = true;
        holdSpeedUsed = true;
    }

    function stopHoldSpeed() {
        if (!videoEl || !holdSpeedActive) return;
        videoEl.playbackRate = savedPlaybackRate;
        currentSpeed = savedPlaybackRate;
        holdSpeedActive = false;
        clearTimeout(holdSpeedTimer);
    }

    function onContainerTouchStart(e: TouchEvent) {
        const target = e.target as HTMLElement;
        if (target.closest('button, input, select, a')) return;
        if (showSubtitlePicker || showSettings || showEpisodeDrawer) return;
        if (showNextOverlay || videoEl?.ended) return;
        holdSpeedUsed = false;
        clearTimeout(holdSpeedTimer);
        holdSpeedTimer = setTimeout(() => startHoldSpeed(), 400);
    }

    function onVideoAreaMouseDown(e: MouseEvent) {
        if (e.button !== 0) return;
        if (spaceHeld) return;
        e.preventDefault();
        holdSpeedUsed = false;
        clearTimeout(holdSpeedTimer);
        holdSpeedTimer = setTimeout(() => startHoldSpeed(), 500);
        const onUp = () => {
            if (holdSpeedActive) {
                stopHoldSpeed();
                holdSpeedUsed = true;
            }
            clearTimeout(holdSpeedTimer);
            window.removeEventListener('mouseup', onUp);
        };
        window.addEventListener('mouseup', onUp);
    }

    function showSkipFlash(side: 'left' | 'right', total: number) {
        skipFlashSide = side;
        skipFlash = `${side === 'left' ? '-' : '+'}${total}s`;
        clearTimeout(skipFlashTimer);
        skipFlashTimer = setTimeout(() => {
            skipFlash = null;
        }, 800);
    }

    function onContainerTouchEnd(e: TouchEvent) {
        if (!('ontouchend' in window)) return;
        clearTimeout(holdSpeedTimer);
        if (holdSpeedActive) {
            stopHoldSpeed();
            return;
        }
        const target = e.target as HTMLElement;
        if (target.closest('button, input, select, a')) return;
        if (showSubtitlePicker || showSettings || showEpisodeDrawer) return;
        if (showNextOverlay || videoEl?.ended) {
            showControlsNow();
            return;
        }

        const now = Date.now();
        const touch = e.changedTouches[0];
        if (!touch) return;

        const rect = containerEl?.getBoundingClientRect();
        const mid = rect ? rect.left + rect.width / 2 : 0;
        const side = touch.clientX < mid ? 'left' : 'right';

        if (now - lastTapTime < 400 && tapSide === side) {
            tapCount++;
            clearTimeout(singleTapTimer);
            if (side === 'left') skip(-skipDuration);
            else skip(skipDuration);
            showSkipFlash(side, tapCount * 5);
            lastTapTime = now;
        } else if (now - lastTapTime < 400) {
            tapCount = 1;
            tapSide = side;
            clearTimeout(singleTapTimer);
            if (side === 'left') skip(-skipDuration);
            else skip(skipDuration);
            showSkipFlash(side, 5);
            lastTapTime = now;
        } else {
            tapCount = 0;
            tapSide = side;
            lastTapTime = now;
            singleTapTimer = setTimeout(() => {
                if (tapCount === 0) {
                    if (controlsVisible && Date.now() - controlsShownAt > 3000) {
                        controlsVisible = false;
                        clearTimeout(controlsTimer);
                    } else if (!controlsVisible) {
                        controlsVisible = true;
                        controlsShownAt = Date.now();
                        resetControlsTimer();
                    }
                }
                tapCount = 0;
            }, 250);
        }
    }

    function onVideoAreaClick(e: MouseEvent) {
        if ('ontouchend' in window) return;
        if (spaceHeld) {
            e.preventDefault();
            return;
        }
        if (holdSpeedUsed) {
            holdSpeedUsed = false;
            return;
        }

        const now = Date.now();
        if (now - lastClickTime < 300) {
            clearTimeout(singleClickTimer);
            lastClickTime = 0;
            toggleFullscreen();
            return;
        }
        lastClickTime = now;
        singleClickTimer = setTimeout(() => {
            togglePlay();
            lastClickTime = 0;
        }, 300);
    }
    function onVideoAreaKeydown(e: KeyboardEvent) {
        if (e.key !== 'Enter') return;
        e.preventDefault();
        e.stopPropagation();
        togglePlay();
    }
    function onControlAreaClick(e: MouseEvent | TouchEvent) {
        e.stopPropagation();
    }
    function fmt(s: number): string {
        if (isNaN(s)) return '0:00';
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        const sec = Math.floor(s % 60);
        if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
        return `${m}:${String(sec).padStart(2, '0')}`;
    }

    function parseReleaseTags(name: string | null | undefined): string[] {
        if (!name) return [];
        const n = name.replace(/\./g, ' ');
        const tags: string[] = [];
        const seen = new Set<string>();
        const push = (t: string) => {
            const k = t.toLowerCase();
            if (seen.has(k)) return;
            seen.add(k);
            tags.push(t);
        };

        const res = n.match(/\b(2160p|1440p|1080p|720p|480p|4k|uhd)\b/i);
        if (res) push(res[1].toLowerCase() === 'uhd' ? '2160p' : res[1].toLowerCase());

        const platform = n.match(/\b(AMZN|NF|ATVP|HMAX|DSNP|PCOK|HULU|MAX|PMTP|iTunes|iT|APTV|CRAV|STAN|CR)\b/);
        if (platform) push(platform[1]);

        const src = n.match(/\b(WEB-?DL|WEBRip|BluRay|BDRip|BRRip|HDTV|HDRip|DVDRip|REMUX)\b/i);
        if (src) push(src[1].replace('WEBDL', 'WEB-DL'));

        const codec = n.match(/\b(x265|x264|HEVC|AV1|H[\s\.\-]?264|H[\s\.\-]?265)\b/i);
        if (codec) {
            const c = codec[1].toUpperCase().replace(/[\s\.\-]/g, '');
            push(c === 'H264' ? 'H.264' : c === 'H265' ? 'H.265' : c);
        }

        if (/\b(Atmos)\b/i.test(n)) push('Atmos');
        else {
            const audio = n.match(
                /\b(DDP?\s?\d[\s\.]?\d?|DTS(?:-HD)?(?:-MA)?|TrueHD|AAC2?[\s\.]?0?|AC-?3|FLAC|OPUS)\b/i
            );
            if (audio) push(audio[1].replace(/\s+/g, '.').toUpperCase());
        }

        if (/\bDoVi|Dolby[\s\.]Vision|\bDV\b/i.test(n)) push('DV');
        if (/\bHDR10\+/i.test(n)) push('HDR10+');
        else if (/\bHDR/i.test(n)) push('HDR');

        const group = n.match(/-([A-Za-z0-9]+)(?:\s*\[[^\]]+\])?\s*$/);
        if (group && group[1].length <= 12) push(group[1]);

        return tags;
    }

    const releaseTags = $derived(parseReleaseTags(releaseName));

    let progressPct = $derived(duration > 0 ? (currentTime / duration) * 100 : 0);
    let bufferedPct = $derived(duration > 0 ? (buffered / duration) * 100 : 0);
    let lastSrc = $state('');
    let seededSrc = $state(false);
    $effect(() => {
        if (!seededSrc) {
            lastSrc = src;
            seededSrc = true;
            return;
        }
        if (src !== lastSrc && videoEl) {
            lastSrc = src;
            srcSwitching = true;
            autoDownloadTriggered = false;
            showNextOverlay = false;
            nextEpisodeDismissed = false;
            introSkipped = false;
            audioTracksLoaded = false;
            selectedSubIndex = -1;
            subtitleOffset = 0;
            activeCueLines = [];
            subCacheBust++;
            clearInterval(nextCountdownTimer);
            clearInterval(cueLoadPoller);
            for (let i = 0; i < videoEl.textTracks.length; i++) {
                videoEl.textTracks[i].mode = 'disabled';
                videoEl.textTracks[i].oncuechange = null;
            }
            loadStream();
        }
    });
    $effect(() => {
        if (showSubtitlePicker || showSettings || showEpisodeDrawer) {
            controlsVisible = true;
            clearTimeout(controlsTimer);
        }
    });
    $effect(() => {
        subBgJoin;
        updateActiveCues();
    });
</script>

<svelte:window onkeydown={onKeydown} onkeyup={onKeyup} onblur={resetSpaceHold} />
<input type="file" accept=".srt,.vtt,.ass,.ssa" class="hidden" bind:this={subFileInput} onchange={onSubFileSelect} />

<div
    bind:this={containerEl}
    role="application"
    aria-label="video player"
    class="pw-pl-root fixed inset-0 z-50 bg-black overflow-hidden select-none group/player layout-{layoutMode} {controlsVisible
        ? ''
        : 'cursor-none'}"
    style="height: 100dvh;"
    onmousemove={onContainerMouseMove}
    onmouseleave={onContainerMouseLeave}
    ontouchstart={onContainerTouchStart}
    ontouchend={onContainerTouchEnd}
>
    <video
        bind:this={videoEl}
        crossorigin="anonymous"
        playsinline
        controlslist="nodownload nofullscreen noremoteplayback"
        disableremoteplayback
        class="w-full h-full sub-{subSize} sub-color-{subColor} sub-bg-{subBg} sub-bgc-{subBgColor} sub-bgo-{subBgOpacity} {controlsVisible
            ? 'subs-up'
            : ''}"
        style="object-fit: {videoFit}; --sub-y: {subYOffset}px; pointer-events: none;"
        preload="auto"
        onloadeddata={onLoadedData}
        ontimeupdate={onTimeUpdate}
        onended={onVideoEnded}
        onplay={() => {
            playing = true;
        }}
        onpause={() => {
            playing = false;
        }}
        onwaiting={() => {
            waiting = true;
        }}
        oncanplay={() => {
            waiting = false;
        }}
    >
        {#key subCacheBust}
            {#each subtitles as sub, i (sub.media_id + ':' + sub.id)}
                <track
                    kind="subtitles"
                    src="{subUrlBuilder ? subUrlBuilder(sub) : api.subtitleUrl(sub.media_id, sub.id)}?v={subCacheBust}"
                    srclang={sub.language}
                    label={sub.label}
                />
            {/each}
        {/key}
    </video>
    {#if waiting && !controlsVisible}
        <div
            class="absolute inset-0 flex items-center justify-center pointer-events-none z-10"
            style="bottom: {isMobile ? '15%' : '0'}"
        >
            <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-white"></div>
        </div>
    {/if}
    {#if skipFlash}
        <div
            class="absolute inset-0 flex items-center pointer-events-none z-10 {skipFlashSide === 'left'
                ? 'justify-start pl-16 sm:pl-20'
                : 'justify-end pr-16 sm:pr-20'}"
            style="bottom: {isMobile ? '15%' : '0'}"
        >
            <div class="bg-black/60 backdrop-blur-sm rounded-full px-5 py-2.5">
                <span class="text-white text-lg font-bold">{skipFlash}</span>
            </div>
        </div>
    {/if}
    {#if holdSpeedActive}
        <div class="absolute left-1/2 -translate-x-1/2 pointer-events-none z-10 {isMobile ? 'top-8' : 'top-20'}">
            <div class="bg-black/40 rounded-full px-4 py-1.5 flex items-center gap-2">
                <Icon name="skip-fast" class="w-4 h-4 text-white" />
                <span class="text-white text-sm font-bold">2x</span>
            </div>
        </div>
    {/if}
    {#if showPlayFlash}
        <div
            class="absolute inset-0 flex items-center justify-center pointer-events-none z-10"
            style="bottom: {isMobile ? '15%' : '0'}"
        >
            <div class="w-20 h-20 bg-black/40 rounded-full flex items-center justify-center animate-ping-once">
                {#if showPlayFlash === 'play'}
                    <Icon name="play" class="w-10 h-10 text-white ml-1" />
                {:else}
                    <Icon name="pause" class="w-10 h-10 text-white" />
                {/if}
            </div>
        </div>
    {/if}
    {#if showSkipIntro}
        <button
            onclick={skipIntro}
            class="pw-skip-intro absolute bottom-28 sm:bottom-32 right-4 sm:right-10 z-[25]"
            in:fly={{ y: 20, duration: 220 }}
            out:fade={{ duration: 150 }}
        >
            <span class="pw-skip-intro-text">SKIP INTRO</span>
            <svg class="pw-skip-intro-icon" fill="currentColor" viewBox="0 0 24 24"
                ><path d="M5 4l10 8-10 8V4zm12 0h2v16h-2V4z" /></svg
            >
        </button>
    {/if}
    <div
        class="absolute inset-0 z-[5]"
        role="button"
        tabindex="-1"
        aria-label="toggle playback"
        onclick={onVideoAreaClick}
        onkeydown={onVideoAreaKeydown}
        onmousedown={onVideoAreaMouseDown}
    ></div>
    {#if activeCueLines.length > 0}
        <div
            class="custom-subs sub-{subSize} sub-color-{subColor} sub-bg-{subBg} sub-bgc-{subBgColor} sub-bgo-{subBgOpacity} {controlsVisible
                ? 'subs-up'
                : ''}"
        >
            {#each activeCueLines as line}
                <div class="custom-sub-line">{@html line}</div>
            {/each}
        </div>
    {/if}
    <div
        class="absolute inset-0 z-20 flex flex-col justify-between pointer-events-none transition-opacity duration-300
			{controlsVisible ? 'opacity-100' : 'opacity-0 [&>*]:pointer-events-none'}"
    >
        <div
            class="pw-pl-top bg-gradient-to-b from-black/80 via-black/40 to-transparent pointer-events-auto {compactHeader
                ? 'is-compact px-2.5 pb-6'
                : 'px-4 sm:px-10 pb-16'}"
        >
            <div class="flex items-center {compactHeader ? 'gap-2' : 'gap-3'}">
                {#if onBack}
                    <button
                        onclick={onBack}
                        class="text-white/90 hover:text-white transition-colors flex-shrink-0"
                        aria-label="back"
                    >
                        <Icon name="chevron-left" class="w-5 h-5" />
                    </button>
                {/if}
                {#if posterUrl}
                    <img
                        src={posterUrl.replace('/w500/', '/w185/')}
                        alt=""
                        class="pw-tb-poster"
                        class:is-mobile={compactHeader}
                        decoding="async"
                    />
                {/if}
                <div class="flex-1 min-w-0">
                    <p
                        class="text-white font-semibold truncate leading-tight {compactHeader
                            ? 'text-[11px]'
                            : 'text-[15px] sm:text-base'}"
                    >
                        {episodeLabel ?? title}
                    </p>
                    {#if releaseTags.length > 0}
                        <div class="mt-0.5 flex flex-wrap items-center gap-0.5">
                            {#each releaseTags as tag}
                                <span class="pw-tb-chip" class:is-mobile={compactHeader}>{tag}</span>
                            {/each}
                        </div>
                    {/if}
                </div>
                {#if $watchTogether.active}
                    <div class="flex-shrink-0">
                        <div
                            class="flex items-center gap-1.5 bg-green-500/20 border border-green-500/30 rounded-full py-1 {compactHeader
                                ? 'px-1.5'
                                : 'px-2 sm:px-2.5'}"
                        >
                            <div
                                class="bg-green-400 rounded-full animate-pulse {compactHeader
                                    ? 'w-1.5 h-1.5'
                                    : 'w-1.5 h-1.5 sm:w-2 sm:h-2'}"
                            ></div>
                            <span class="text-green-400 text-[10px] font-medium">{$watchTogether.participants}</span>
                        </div>
                    </div>
                {/if}
            </div>
        </div>
        <div class="sm:hidden mob-flex"></div>
        <div
            class="bg-gradient-to-t from-black/90 via-black/50 to-transparent pt-4 sm:pt-20 pointer-events-auto"
        >
            <div
                bind:this={seekBarEl}
                class="px-4 sm:px-10 pb-1 sm:pb-2 group/seek cursor-pointer relative"
                role="slider"
                aria-label="seek"
                aria-valuemin="0"
                aria-valuemax={Math.max(0, Math.floor(duration))}
                aria-valuenow={Math.max(0, Math.floor(currentTime))}
                aria-valuetext="{fmt(currentTime)} / {fmt(duration)}"
                tabindex="0"
                onmousedown={onSeekBarMouseDown}
                onmousemove={onSeekBarHover}
                onmouseleave={onSeekBarLeave}
                ontouchstart={onSeekBarTouch}
                ontouchmove={onSeekBarTouch}
                onkeydown={onSeekBarKeydown}
            >
                {#if seekHovering && duration > 0}
                    {@const clampedX = Math.max(30, Math.min(seekPreviewX, (seekBarEl?.clientWidth || 0) - 30))}
                    <div
                        class="absolute bottom-full mb-3 pointer-events-none"
                        style="left: {clampedX}px; transform: translateX(-50%);"
                    >
                        <span
                            class="bg-black/80 backdrop-blur-sm text-white text-xs font-mono font-medium px-2.5 py-1 rounded-lg shadow-lg ring-1 ring-white/10"
                        >
                            {fmt(seekPreviewTime)}
                        </span>
                    </div>
                {/if}

                <div
                    bind:this={seekTrackEl}
                    class="relative h-1 group-hover/seek:h-2 transition-all bg-white/20 rounded-full"
                >
                    <div class="absolute inset-y-0 left-0 bg-white/30 rounded-full" style="width: {bufferedPct}%"></div>
                    <div class="absolute inset-y-0 left-0 bg-primary-500 rounded-full" style="width: {progressPct}%">
                        <div
                            class="absolute right-0 top-1/2 -translate-y-1/2 w-4 h-4 bg-primary-400 rounded-full opacity-100 sm:opacity-0 group-hover/seek:opacity-100 transition-opacity shadow-lg"
                        ></div>
                    </div>
                    {#if seekHovering}
                        <div
                            class="absolute top-1/2 -translate-y-1/2 w-0.5 h-4 bg-white/50 pointer-events-none"
                            style="left: {(seekPreviewTime / duration) * 100}%"
                        ></div>
                    {/if}
                </div>
            </div>
            <div class="flex justify-between items-center px-5 sm:hidden mob-flex">
                <span class="text-white/50 text-[10px] font-mono">{fmt(currentTime)}</span>
                <span class="text-white/30 text-[10px] font-mono">{fmt(duration)}</span>
            </div>
            <div class="hidden sm:flex desk-flex items-center gap-6 px-10 pb-5 pt-1">
                <button onclick={togglePlay} class="text-white hover:text-gray-300 p-1.5">
                    {#if playing}
                        <Icon name="pause" class="w-8 h-8" />
                    {:else}
                        <Icon name="play" class="w-8 h-8 ml-0.5" />
                    {/if}
                </button>
                {#if onNextEpisode && nextEpisode()}
                    <button onclick={playNextNow} class="text-white hover:text-gray-300 p-1.5" title="Next Episode">
                        <Icon name="next-track" class="w-7 h-7" />
                    </button>
                {/if}
                <div class="flex items-center gap-2">
                    <div
                        class="relative flex items-center group/vol"
                        role="group"
                        aria-label="volume"
                        onmouseenter={() => {
                            showVolume = true;
                        }}
                        onmouseleave={() => {
                            if (!volumeDragging) showVolume = false;
                        }}
                    >
                        <button onclick={toggleMute} class="text-white hover:text-gray-300 p-1.5">
                            {#if muted || volume === 0}
                                <Icon name="volume-off" class="w-7 h-7" />
                            {:else if volume < 0.5}
                                <Icon name="volume-low" class="w-7 h-7" />
                            {:else}
                                <Icon name="volume-high" class="w-7 h-7" />
                            {/if}
                        </button>
                        <div
                            class="overflow-hidden transition-all duration-300 ease-out ml-2 flex items-center {showVolume
                                ? 'w-36 opacity-100'
                                : 'w-0 opacity-0'}"
                        >
                            <div
                                bind:this={volumeBarEl}
                                class="w-24 h-6 flex items-center cursor-pointer px-2"
                                role="slider"
                                aria-label="volume"
                                aria-valuemin="0"
                                aria-valuemax="100"
                                aria-valuenow={muted ? 0 : Math.round(volume * 100)}
                                tabindex="0"
                                onmousedown={onVolumeDragStart}
                                onclick={onVolumeClick}
                                onkeydown={onVolumeKeydown}
                            >
                                <div class="w-full h-1.5 bg-gray-600 rounded-full relative overflow-visible">
                                    <div
                                        class="h-full bg-primary-500 rounded-full"
                                        style="width: {muted ? 0 : volume * 100}%"
                                    ></div>
                                    <div
                                        class="absolute top-1/2 w-4 h-4 bg-primary-400 rounded-full shadow-lg pointer-events-none"
                                        style="left: {muted ? 0 : volume * 100}%; transform: translate(-50%, -50%);"
                                    ></div>
                                </div>
                            </div>
                            <span class="text-white/70 text-sm font-mono flex-shrink-0 ml-2 tabular-nums"
                                >{muted ? 0 : Math.round(volume * 100)}%</span
                            >
                        </div>
                    </div>
                    <span class="text-white text-sm font-mono tabular-nums">{fmt(currentTime)} / {fmt(duration)}</span>
                </div>
                <div class="flex-1"></div>
                {#if $watchTogether.active}
                    <div
                        class="flex items-center gap-1.5 bg-green-500/20 border border-green-500/30 rounded-full px-2.5 py-1"
                    >
                        <div class="w-2 h-2 bg-green-400 rounded-full animate-pulse"></div>
                        <span class="text-green-400 text-sm font-medium">{$watchTogether.participants}</span>
                    </div>
                {/if}
                {#if $watchTogether.active}
                    <button
                        onclick={openChat}
                        class="relative p-2.5 -m-1 rounded-lg transition-all duration-200 {showChat
                            ? 'text-primary-400 bg-primary-500/15 ring-1 ring-primary-400/30'
                            : 'text-white hover:text-gray-300 hover:bg-white/5'}"
                        aria-label="open chat"
                        title="chat"
                    >
                        <Icon name="chat-round" class="w-7 h-7" />
                        {#if chatToasts.length > 0 && !showChat}
                            <span
                                class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.7)]"
                            ></span>
                        {/if}
                    </button>
                {/if}
                {#if episodes.length > 0}
                    <button
                        onclick={() => {
                            toggleEpisodeDrawer();
                        }}
                        class="p-2.5 -m-1 rounded-lg transition-all duration-200 {showEpisodeDrawer
                            ? 'text-primary-400 bg-primary-500/15 ring-1 ring-primary-400/30'
                            : 'text-white hover:text-gray-300 hover:bg-white/5'}"
                    >
                        <Icon name="episodes" class="w-8 h-8" />
                    </button>
                {/if}
                <button
                    onclick={() => {
                        showSubtitlePicker = !showSubtitlePicker;
                        showSettings = false;
                        showEpisodeDrawer = false;
                        resetControlsTimer();
                    }}
                    class="p-2.5 -m-1 rounded-lg transition-all duration-200 {showSubtitlePicker
                        ? 'text-primary-400 bg-primary-500/15 ring-1 ring-primary-400/30'
                        : selectedSubIndex >= 0
                          ? 'text-primary-400 hover:text-gray-300 hover:bg-white/5'
                          : 'text-white hover:text-gray-300 hover:bg-white/5'}"
                >
                    <Icon name="subs" class="w-8 h-8" />
                </button>
                <button
                    onclick={() => {
                        showSettings = !showSettings;
                        showSubtitlePicker = false;
                        showEpisodeDrawer = false;
                        resetControlsTimer();
                    }}
                    class="p-2.5 -m-1 rounded-lg transition-all duration-200 {showSettings
                        ? 'settings-open text-primary-400 bg-primary-500/15 ring-1 ring-primary-400/30'
                        : 'text-white hover:text-gray-300 hover:bg-white/5'}"
                >
                    <Icon name="gear" class="w-7 h-7" />
                </button>
                <button
                    onclick={toggleFullscreen}
                    class="text-white hover:text-gray-300 p-2.5 -m-1 rounded-lg hover:bg-white/5 transition-colors"
                >
                    {#if isFullscreen}
                        <Icon name="fullscreen-enter" class="w-8 h-8" />
                    {:else}
                        <Icon name="fullscreen-exit" class="w-8 h-8" />
                    {/if}
                </button>
            </div>
            <div class="sm:hidden mob-block px-4 pb-1 pt-0">
                <div class="flex items-center justify-evenly">
                    {#if episodes.length > 0}
                        <button
                            onclick={() => {
                                toggleEpisodeDrawer();
                            }}
                            class="flex flex-col items-center gap-1 text-[11px] py-1.5 px-2 rounded-xl transition-all duration-200 {showEpisodeDrawer
                                ? 'text-primary-400 bg-primary-500/15'
                                : 'text-white/90'}"
                        >
                            <Icon name="episodes" class="w-6 h-6" />
                            <span>{$t('player.episodes')}</span>
                        </button>
                    {/if}
                    <button
                        onclick={() => {
                            showSubtitlePicker = !showSubtitlePicker;
                            showSettings = false;
                            showEpisodeDrawer = false;
                            resetControlsTimer();
                        }}
                        class="flex flex-col items-center gap-1 text-[11px] py-1.5 px-2 rounded-xl transition-all duration-200 {showSubtitlePicker
                            ? 'text-primary-400 bg-primary-500/15'
                            : selectedSubIndex >= 0
                              ? 'text-primary-400'
                              : 'text-white/90'}"
                    >
                        <Icon name="subs" class="w-6 h-6" />
                        <span>{$t('player.audio_and_subs')}</span>
                    </button>
                    <button
                        onclick={() => {
                            showSettings = !showSettings;
                            showSubtitlePicker = false;
                            showEpisodeDrawer = false;
                            resetControlsTimer();
                        }}
                        class="flex flex-col items-center gap-1 text-[11px] py-1.5 px-2 rounded-xl transition-all duration-200 {showSettings
                            ? 'settings-open text-primary-400 bg-primary-500/15'
                            : 'text-white/90'}"
                    >
                        <Icon name="gear" class="w-6 h-6" />
                        <span>{$t('player.settings')}</span>
                    </button>
                    {#if $watchTogether.active}
                        <button
                            onclick={openChat}
                            class="relative flex flex-col items-center gap-1 text-[11px] py-1.5 px-2 rounded-xl transition-all duration-200 {showChat
                                ? 'text-primary-400 bg-primary-500/15'
                                : 'text-white/90'}"
                            aria-label="open chat"
                        >
                            <Icon name="chat-round" class="w-6 h-6" />
                            <span>chat</span>
                            {#if chatToasts.length > 0 && !showChat}
                                <span class="absolute top-1 right-1 w-1.5 h-1.5 rounded-full bg-green-400"></span>
                            {/if}
                        </button>
                    {/if}
                    {#if onNextEpisode && nextEpisode()}
                        <button
                            onclick={playNextNow}
                            class="flex flex-col items-center gap-1 text-white/90 text-[11px] py-1.5 px-2"
                        >
                            <Icon name="next-track" class="w-6 h-6" />
                            <span>Next</span>
                        </button>
                    {/if}
                    <button
                        onclick={toggleFullscreen}
                        class="flex flex-col items-center gap-1 text-white/90 text-[11px] py-1.5 px-2"
                    >
                        {#if isFullscreen}
                            <Icon name="fullscreen-enter" class="w-6 h-6" />
                        {:else}
                            <Icon name="fullscreen-exit" class="w-6 h-6" />
                        {/if}
                        <span>Fullscreen</span>
                    </button>
                </div>
            </div>
        </div>
    </div>
    {#if controlsVisible}
        <div
            class="absolute inset-0 flex items-center justify-center z-[22] pointer-events-none sm:hidden mob-flex"
            style="bottom: 15%"
        >
            <button
                onclick={togglePlay}
                class="text-white active:scale-90 transition-transform w-16 h-16 flex items-center justify-center pointer-events-auto"
            >
                {#if waiting}
                    <div class="animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-white"></div>
                {:else if playing}
                    <Icon name="pause" class="w-16 h-16" />
                {:else}
                    <Icon name="play" class="w-16 h-16" />
                {/if}
            </button>
        </div>
    {/if}
    {#if showSubtitlePicker}
        <AudioSubsPanel
            {isMobile}
            {audioTracks}
            {selectedAudioIndex}
            {selectedSubIndex}
            {subtitles}
            {partyMode}
            {syncErr}
            {syncing}
            {syncMethod}
            bind:subSearchMode
            bind:subSearchLang
            bind:subSearchResults
            {subSearching}
            bind:subSearched
            {subDownloading}
            bind:subUploadFile
            bind:subUploadLabel
            bind:subUploadLang
            {subUploading}
            {aiTranslating}
            {aiTranslateErr}
            onClose={() => {
                showSubtitlePicker = false;
            }}
            onSelectAudio={selectAudio}
            onSelectSubtitle={selectSubtitle}
            onRunSync={runSync}
            onDeleteSubtitle={deleteSubtitle}
            onSearch={searchSubDL}
            onDownload={downloadSubResult}
            onAiTranslate={runAiTranslate}
            onUpload={doSubUpload}
            onOpenFile={() => subFileInput?.click()}
        />
    {/if}

    {#if showSettings}
        <PlayerSettingsPanel
            {isMobile}
            {isIPhone}
            partyActive={$watchTogether.active}
            {unreadCount}
            {currentSpeed}
            bind:skipDuration
            bind:autoNextEpisode
            bind:autoSkipIntro
            bind:useNativeIOSFs
            {layoutMode}
            {videoFit}
            bind:subSize
            bind:subColor
            bind:subBg
            bind:subBgColor
            bind:subBgOpacity
            bind:subBgJoin
            bind:subSync
            bind:subYOffset
            onClose={() => {
                showSettings = false;
            }}
            onClip={enterClipMode}
            onOpenChat={openChat}
            onStartParty={startWatchTogether}
            onLeaveParty={leaveSession}
            onSetSpeed={setSpeed}
            onSetVideoFit={setVideoFit}
            onSetLayoutMode={setLayoutMode}
        />
    {/if}
    {#if showEpisodeDrawer}
        <EpisodeDrawer
            {episodes}
            {currentEpisodeId}
            {currentSeason}
            {currentEpisodeNum}
            {tmdbEpisodes}
            {tmdbId}
            {episodeProgress}
            {isMobile}
            onClose={() => {
                showEpisodeDrawer = false;
            }}
            onSelect={(ep) => onEpisodeSelect?.(ep)}
        />
    {/if}
    {#if showStillWatching}
        <div class="absolute inset-0 z-[55] flex items-center justify-center bg-black/80 backdrop-blur-sm">
            <div class="text-center px-6">
                <p class="text-white/40 text-lg mb-2">
                    {$t('player.watching_for_a_while')}
                </p>
                <h2 class="text-white text-3xl sm:text-4xl font-bold mb-8">
                    {$t('player.still_watching')}
                </h2>
                <div class="flex items-center justify-center gap-4">
                    <button
                        onclick={stillWatchingContinue}
                        class="bg-white text-black font-semibold text-lg px-8 py-3 rounded-xl hover:bg-gray-200 transition-colors"
                    >
                        {$t('player.continue')}
                    </button>
                    <button
                        onclick={stillWatchingStop}
                        class="bg-white/10 text-white font-medium text-lg px-8 py-3 rounded-xl hover:bg-white/20 transition-colors"
                    >
                        {$t('player.stop')}
                    </button>
                </div>
                <p class="text-white/30 text-sm mt-6">{$t('player.pausing_in_2min')}</p>
            </div>
        </div>
    {:else if showNextOverlay && nextEpisode()}
        <NextEpisodeOverlay
            episode={nextEpisode()!}
            countdown={nextCountdown}
            onPlayNow={playNextNow}
            onCancel={cancelNextEpisode}
        />
    {:else if showCreditsOverlay}
        <CreditsOverlay
            hasNext={creditsHasNext}
            onSkipCredits={skipCredits}
            onNextEpisode={creditsNextEpisode}
            onDismiss={() => (creditsDismissed = true)}
        />
    {/if}
    <ClipPanel
        {clipMode}
        bind:clipStart
        bind:clipEnd
        {currentTime}
        {duration}
        {clipCreating}
        {clipShowResult}
        {fmt}
        onCopyLink={copyClipLink}
        onExit={exitClipMode}
        onCreate={createClip}
    />
    {#if subJobLabel}
        <div class="pw-job-pill" role="status" aria-live="polite">
            <span class="pw-job-spin"></span>
            <span>{subJobLabel}</span>
            <span class="pw-job-dots"><span></span><span></span><span></span></span>
        </div>
    {/if}
    {#if subShiftFlash !== null}
        <div class="pw-sub-shift" role="status" aria-live="polite">
            <Icon name="list-lines" class="w-3.5 h-3.5" strokeWidth={2} />
            <span>sub offset: <strong>{subShiftFlash > 0 ? '+' : ''}{subShiftFlash.toFixed(1)}s</strong></span>
        </div>
    {/if}
    <WatchTogetherPanel
        {isMobile}
        {controlsVisible}
        {showSettings}
        {showSubtitlePicker}
        {showEpisodeDrawer}
        {kbOffset}
        bind:showChat
        bind:chatInput
        {chatToasts}
        {shareCode}
        bind:showShareModal
        bind:chatScrollEl
        bind:chatInputEl
        onOpenChat={openChat}
        onSendChat={handleSendChat}
        onCopyShareLink={copyShareLink}
    />
</div>

<style>
    .pw-pl-top {
        padding-top: calc(1rem + env(safe-area-inset-top));
    }
    .pw-pl-top.is-compact {
        padding-top: calc(0.375rem + env(safe-area-inset-top));
    }
    @media (min-width: 640px) {
        .pw-pl-top:not(.is-compact) {
            padding-top: calc(1.25rem + env(safe-area-inset-top));
        }
    }

    .pw-skip-intro {
        display: inline-flex;
        align-items: center;
        gap: 12px;
        padding: 13px 24px 13px 19px;
        border-radius: 7px;
        background: linear-gradient(180deg, #ffffff 0%, #f4f4f6 100%);
        color: #08090b;
        border: none;
        cursor: pointer;
        transition:
            filter 0.15s ease,
            transform 0.1s ease;
        box-shadow:
            0 10px 28px -12px rgba(0, 0, 0, 0.7),
            0 2px 6px -2px rgba(0, 0, 0, 0.35),
            inset 0 1px 0 rgba(255, 255, 255, 0.8),
            inset 0 -1px 0 rgba(0, 0, 0, 0.06);
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, system-ui, sans-serif;
        justify-content: center;
    }
    .pw-skip-intro:hover {
        filter: brightness(0.97);
    }
    .pw-skip-intro:active {
        transform: translateY(1px);
    }
    .pw-skip-intro-text {
        font-size: 14px;
        font-weight: 700;
        letter-spacing: 0.09em;
        line-height: 1;
    }
    .pw-skip-intro-icon {
        width: 15px;
        height: 15px;
        flex-shrink: 0;
    }
    @media (min-width: 640px) {
        .pw-skip-intro {
            padding: 14px 28px 14px 22px;
            gap: 13px;
            border-radius: 8px;
        }
        .pw-skip-intro-text {
            font-size: 15px;
        }
        .pw-skip-intro-icon {
            width: 16px;
            height: 16px;
        }
    }

    video::-webkit-media-controls,
    video::-webkit-media-controls-enclosure,
    video::-webkit-media-controls-overlay-play-button,
    video::-webkit-media-controls-start-playback-button,
    video::-webkit-media-controls-play-button {
        display: none !important;
        -webkit-appearance: none !important;
        opacity: 0 !important;
    }

    video::cue {
        font-family: 'Segoe UI', 'Helvetica Neue', Arial, sans-serif;
        color: white;
        background: transparent;
        text-shadow:
            2px 2px 1px rgba(0, 0, 0, 1),
            -2px -2px 1px rgba(0, 0, 0, 1),
            2px -2px 1px rgba(0, 0, 0, 1),
            -2px 2px 1px rgba(0, 0, 0, 1),
            0px 0px 4px rgba(0, 0, 0, 0.8),
            0px 2px 6px rgba(0, 0, 0, 0.5);
        line-height: 1.3;
        font-weight: 700;
    }
    .custom-subs {
        position: absolute;
        left: 0;
        right: 0;
        bottom: 5%;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 3px;
        pointer-events: none;
        z-index: 10;
        padding: 0 6%;
        text-align: center;
        transition: bottom 0.3s ease;
    }
    .custom-subs.subs-up {
        bottom: calc(5% + 66px);
    }
    .custom-sub-line {
        font-family: 'Segoe UI', Tahoma, sans-serif;
        color: #ffffff;
        font-weight: 700;
        line-height: 1.1;
        padding: 0 0.15em;
        max-width: 100%;
        display: inline-block;
        text-align: center;
        font-style: normal;
        text-shadow:
            0.05em 0.05em 0 #000,
            -0.05em -0.05em 0 #000,
            0.05em -0.05em 0 #000,
            -0.05em 0.05em 0 #000,
            0 0.08em 0.12em rgba(0, 0, 0, 0.7);
    }
    .custom-subs.sub-small .custom-sub-line {
        font-size: clamp(18px, 4vh, 48px);
    }
    .custom-subs.sub-medium .custom-sub-line {
        font-size: clamp(23px, 5vh, 56px);
    }
    .custom-subs.sub-large .custom-sub-line {
        font-size: clamp(30px, 6.5vh, 72px);
    }
    .custom-subs.sub-color-white .custom-sub-line {
        color: #ffffff;
    }
    .custom-subs.sub-color-yellow .custom-sub-line {
        color: #ffd700;
    }
    .custom-subs.sub-color-cyan .custom-sub-line {
        color: #00ffff;
    }
    .custom-subs.sub-color-green .custom-sub-line {
        color: #00ff00;
    }
    .custom-subs.sub-bg-transparent .custom-sub-line {
        background: transparent;
    }
    .custom-subs.sub-bg-shadow .custom-sub-line {
        background: transparent;
        text-shadow: 0 3px 4px rgba(0, 0, 0, 0.95);
    }
    .custom-subs.sub-bg-box .custom-sub-line {
        -webkit-text-stroke: 0;
        text-shadow: none;
    }
    .custom-subs.sub-bg-box.sub-bgc-black.sub-bgo-50 .custom-sub-line {
        background: rgba(0, 0, 0, 0.5);
    }
    .custom-subs.sub-bg-box.sub-bgc-black.sub-bgo-75 .custom-sub-line {
        background: rgba(0, 0, 0, 0.75);
    }
    .custom-subs.sub-bg-box.sub-bgc-black.sub-bgo-100 .custom-sub-line {
        background: rgba(0, 0, 0, 1);
    }
    .custom-subs.sub-bg-box.sub-bgc-white.sub-bgo-50 .custom-sub-line {
        background: rgba(255, 255, 255, 0.5);
        color: #000;
    }
    .custom-subs.sub-bg-box.sub-bgc-white.sub-bgo-75 .custom-sub-line {
        background: rgba(255, 255, 255, 0.75);
        color: #000;
    }
    .custom-subs.sub-bg-box.sub-bgc-white.sub-bgo-100 .custom-sub-line {
        background: rgba(255, 255, 255, 1);
        color: #000;
    }
    .custom-subs.sub-bg-box.sub-bgc-yellow.sub-bgo-50 .custom-sub-line {
        background: rgba(180, 150, 0, 0.5);
    }
    .custom-subs.sub-bg-box.sub-bgc-yellow.sub-bgo-75 .custom-sub-line {
        background: rgba(180, 150, 0, 0.75);
    }
    .custom-subs.sub-bg-box.sub-bgc-yellow.sub-bgo-100 .custom-sub-line {
        background: rgba(180, 150, 0, 1);
    }
    .custom-subs.sub-bg-box.sub-bgc-cyan.sub-bgo-50 .custom-sub-line {
        background: rgba(0, 150, 150, 0.5);
    }
    .custom-subs.sub-bg-box.sub-bgc-cyan.sub-bgo-75 .custom-sub-line {
        background: rgba(0, 150, 150, 0.75);
    }
    .custom-subs.sub-bg-box.sub-bgc-cyan.sub-bgo-100 .custom-sub-line {
        background: rgba(0, 150, 150, 1);
    }
    .custom-subs.sub-bg-box.sub-bgc-green.sub-bgo-50 .custom-sub-line {
        background: rgba(0, 120, 0, 0.5);
    }
    .custom-subs.sub-bg-box.sub-bgc-green.sub-bgo-75 .custom-sub-line {
        background: rgba(0, 120, 0, 0.75);
    }
    .custom-subs.sub-bg-box.sub-bgc-green.sub-bgo-100 .custom-sub-line {
        background: rgba(0, 120, 0, 1);
    }
    video.sub-color-white::cue {
        color: #ffffff;
    }
    video.sub-color-yellow::cue {
        color: #ffd700;
    }
    video.sub-color-cyan::cue {
        color: #00ffff;
    }
    video.sub-color-green::cue {
        color: #00ff00;
    }
    video.sub-bg-transparent::cue {
        background: transparent;
    }
    video.sub-bg-shadow::cue {
        background: transparent;
        text-shadow:
            3px 3px 2px rgba(0, 0, 0, 1),
            -3px -3px 2px rgba(0, 0, 0, 1),
            3px -3px 2px rgba(0, 0, 0, 1),
            -3px 3px 2px rgba(0, 0, 0, 1),
            0 0 8px rgba(0, 0, 0, 0.9);
    }
    video.sub-bg-box::cue {
        text-shadow: none;
    }
    video.sub-bg-box.sub-bgc-black.sub-bgo-50::cue {
        background: rgba(0, 0, 0, 0.5);
    }
    video.sub-bg-box.sub-bgc-black.sub-bgo-75::cue {
        background: rgba(0, 0, 0, 0.75);
    }
    video.sub-bg-box.sub-bgc-black.sub-bgo-100::cue {
        background: rgba(0, 0, 0, 1);
    }
    video.sub-bg-box.sub-bgc-white.sub-bgo-50::cue {
        background: rgba(255, 255, 255, 0.5);
    }
    video.sub-bg-box.sub-bgc-white.sub-bgo-75::cue {
        background: rgba(255, 255, 255, 0.75);
    }
    video.sub-bg-box.sub-bgc-white.sub-bgo-100::cue {
        background: rgba(255, 255, 255, 1);
    }
    video.sub-bg-box.sub-bgc-yellow.sub-bgo-50::cue {
        background: rgba(180, 150, 0, 0.5);
    }
    video.sub-bg-box.sub-bgc-yellow.sub-bgo-75::cue {
        background: rgba(180, 150, 0, 0.75);
    }
    video.sub-bg-box.sub-bgc-yellow.sub-bgo-100::cue {
        background: rgba(180, 150, 0, 1);
    }
    video.sub-bg-box.sub-bgc-cyan.sub-bgo-50::cue {
        background: rgba(0, 150, 150, 0.5);
    }
    video.sub-bg-box.sub-bgc-cyan.sub-bgo-75::cue {
        background: rgba(0, 150, 150, 0.75);
    }
    video.sub-bg-box.sub-bgc-cyan.sub-bgo-100::cue {
        background: rgba(0, 150, 150, 1);
    }
    video.sub-bg-box.sub-bgc-green.sub-bgo-50::cue {
        background: rgba(0, 120, 0, 0.5);
    }
    video.sub-bg-box.sub-bgc-green.sub-bgo-75::cue {
        background: rgba(0, 120, 0, 0.75);
    }
    video.sub-bg-box.sub-bgc-green.sub-bgo-100::cue {
        background: rgba(0, 120, 0, 1);
    }
    video.sub-small::cue {
        font-size: 0.9em;
    }
    video.sub-medium::cue {
        font-size: 1.15em;
    }
    video.sub-large::cue {
        font-size: 1.5em;
    }
    :global(video::-webkit-media-text-track-container) {
        transition: transform 0.3s ease;
        transform: translateY(var(--sub-y, 0px));
    }
    :global(video.sub-small::-webkit-media-text-track-container) {
        transform: translateY(calc(-8px + var(--sub-y, 0px)));
    }
    :global(video.sub-medium::-webkit-media-text-track-container) {
        transform: translateY(calc(-3px + var(--sub-y, 0px)));
    }
    :global(video.sub-large::-webkit-media-text-track-container) {
        transform: translateY(calc(18px + var(--sub-y, 0px)));
    }
    :global(video.subs-up.sub-small::-webkit-media-text-track-container) {
        transform: translateY(calc(-74px + var(--sub-y, 0px)));
    }
    :global(video.subs-up::-webkit-media-text-track-container) {
        transform: translateY(calc(-66px + var(--sub-y, 0px)));
    }
    :global(video.subs-up.sub-medium::-webkit-media-text-track-container) {
        transform: translateY(calc(-70px + var(--sub-y, 0px)));
    }
    :global(video.subs-up.sub-large::-webkit-media-text-track-container) {
        transform: translateY(calc(-62px + var(--sub-y, 0px)));
    }

    @keyframes slide-up {
        from {
            opacity: 0;
            transform: translateY(80px) scale(0.92);
        }
        to {
            opacity: 1;
            transform: translateY(0) scale(1);
        }
    }
    @keyframes dropdown-in {
        from {
            opacity: 0;
            transform: translateY(8px) scale(0.96);
        }
        to {
            opacity: 1;
            transform: translateY(0) scale(1);
        }
    }
    .animate-dropdown-in {
        animation: slide-up 0.35s cubic-bezier(0.16, 1, 0.3, 1);
    }
    @media (min-width: 640px) {
        .animate-dropdown-in {
            animation: dropdown-in 0.2s cubic-bezier(0.16, 1, 0.3, 1);
        }
    }

    @keyframes bar1 {
        0%,
        100% {
            height: 4px;
        }
        50% {
            height: 16px;
        }
    }
    @keyframes bar2 {
        0%,
        100% {
            height: 12px;
        }
        50% {
            height: 4px;
        }
    }
    @keyframes bar3 {
        0%,
        100% {
            height: 8px;
        }
        50% {
            height: 14px;
        }
    }
    .animate-bar1 {
        animation: bar1 0.8s ease infinite;
    }
    .animate-bar2 {
        animation: bar2 0.8s ease infinite 0.2s;
    }
    .animate-bar3 {
        animation: bar3 0.8s ease infinite 0.4s;
    }

    @keyframes ping-once {
        0% {
            transform: scale(1);
            opacity: 1;
        }
        100% {
            transform: scale(1.5);
            opacity: 0;
        }
    }
    .animate-ping-once {
        animation: ping-once 0.4s ease-out forwards;
    }
    :global(button) {
        transition:
            transform 0.14s ease,
            background-color 0.14s ease,
            color 0.14s ease,
            box-shadow 0.14s ease;
    }
    :global(.pw-pl-root button:hover svg) {
        transform: scale(1.08);
    }
    :global(.pw-pl-root button:active svg) {
        transform: scale(0.92);
    }
    :global(.pw-pl-root button svg) {
        transition: transform 0.14s cubic-bezier(0.2, 0.8, 0.2, 1.2);
    }
    :global(.pw-pl-root button:hover) {
        filter: brightness(1.05);
    }
    :global(.pw-pl-root:focus),
    :global(.pw-pl-root:focus-visible) {
        outline: none;
    }
    :global(.pw-pl-root [aria-label='toggle playback']:focus),
    :global(.pw-pl-root [aria-label='toggle playback']:focus-visible) {
        outline: none;
    }
    :global(.pw-pl-root .settings-open svg) {
        transform: rotate(90deg) scale(1.05);
    }
    @keyframes pw-active-pulse {
        0%,
        100% {
            box-shadow: 0 0 0 0 rgba(99, 102, 241, 0);
        }
        50% {
            box-shadow: 0 0 0 4px rgba(99, 102, 241, 0.18);
        }
    }
    :global(.pw-pl-root .pw-pl-active) {
        animation: pw-active-pulse 2.4s ease-in-out infinite;
    }

    @keyframes fade-in {
        from {
            opacity: 0;
            transform: translateY(8px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
    .animate-fade-in {
        animation: fade-in 0.3s ease-out;
    }
    :global(.layout-mobile) .mob-flex {
        display: flex !important;
    }
    :global(.layout-mobile) .mob-block {
        display: block !important;
    }
    :global(.layout-mobile) .desk-flex {
        display: none !important;
    }
    :global(.layout-mobile) .desk-block {
        display: none !important;
    }
    :global(.layout-desktop) .desk-flex {
        display: flex !important;
    }
    :global(.layout-desktop) .desk-block {
        display: block !important;
    }
    :global(.layout-desktop) .mob-flex {
        display: none !important;
    }
    :global(.layout-desktop) .mob-block {
        display: none !important;
    }
</style>
