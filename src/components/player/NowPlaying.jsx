import { useState, useEffect, useRef, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { toAssetUrl } from '../../utils';
import { useTranslation } from 'react-i18next';
import { Radio, Play, Pause, SkipBack, SkipForward, VolumeX, Volume1, Volume2, SlidersHorizontal, X, Globe, Users, Headphones, Loader2, Search, Music, RefreshCw, PanelRight, PanelBottom } from 'lucide-react';
import EqualizerPanel from './EqualizerPanel';

// Per-band multipliers to simulate an EQ shape from a single RMS level
const BAND_WEIGHTS = [0.8, 1.0, 0.9, 0.7, 0.6, 0.5, 0.35];
const NUM_BARS = 7;

const SmartMarquee = memo(({ text, miniMode, isAccent = false, isTitle = false, isDimmed = false }) => {
    const containerRef = useRef(null);
    const [overflows, setOverflows] = useState(false);
    const [showMarquee, setShowMarquee] = useState(false);

    const heightClass = miniMode
        ? (isTitle ? "h-[22px]" : (isDimmed ? "h-[16px]" : "h-[18px]"))
        : (isTitle ? "h-[32px]" : (isDimmed ? "h-[20px]" : "h-[24px]"));

    const fontSizeClass = miniMode
        ? (isTitle ? 'text-sm font-black' : isAccent ? 'text-[12px] font-light' : 'text-[9px] font-light')
        : (isTitle ? 'text-xl font-black' : isAccent ? 'text-base font-light' : 'text-sm font-light');

    const alignClass = miniMode ? "justify-start" : "justify-center";
    const colorClass = isDimmed ? "text-text-muted" : "text-text-primary";

    useEffect(() => {
        setOverflows(false);
        setShowMarquee(false);
        let showTimeout;
        const check = () => {
            if (containerRef.current) {
                const measurer = containerRef.current.querySelector('.marquee-measurer');
                const containerWidth = containerRef.current.offsetWidth;
                if (measurer && containerWidth > 0) {
                    const textWidth = measurer.offsetWidth;
                    const isOverflowing = textWidth > containerWidth + 8;
                    setOverflows(isOverflowing);
                    if (isOverflowing) {
                        showTimeout = setTimeout(() => setShowMarquee(true), 1500);
                    }
                }
            }
        };
        const timer = setTimeout(check, 100);
        window.addEventListener('resize', check);
        return () => {
            clearTimeout(timer);
            clearTimeout(showTimeout);
            window.removeEventListener('resize', check);
        };
    }, [text, miniMode]);

    if (!text || text.trim() === "") {
        return null;
    }

    const estimatedWidth = text.length * (isTitle ? 12 : 10) + 40;
    const repeatCount = Math.max(2, Math.ceil(1500 / estimatedWidth));
    // Calculate total duration so that the scroll speed is consistent (~45px/s)
    const scrollDuration = Math.max(12, (estimatedWidth * repeatCount) / 45);
    // Add 15% extra for the pause (it will be 15% of total)
    const animDuration = scrollDuration * 1.15;

    return (
        <div ref={containerRef} className={`${heightClass} relative overflow-hidden shrink ${miniMode ? 'flex-initial min-w-0' : 'w-full'} ${!isTitle && !isDimmed ? 'opacity-90' : ''}`}>
            {/* Hidden measurer to detect overflow */}
            <span className={`marquee-measurer absolute opacity-0 pointer-events-none whitespace-nowrap ${fontSizeClass}`} style={{ left: '-9999px' }}>
                {text}
            </span>

            {/* Content layer with smooth cross-fade */}
            <div className={`w-full h-full flex items-center ${overflows ? 'justify-start' : alignClass} relative`}>
                {/* Static Layer: Shows initially, fades out if marquee starts */}
                <div className={`transition-all duration-700 flex items-center ${overflows ? 'justify-start' : alignClass} w-full h-full ${showMarquee ? 'opacity-0 translate-y-[-2px]' : 'opacity-100 translate-y-0'}`}>
                    <span className={`whitespace-nowrap tracking-tight ${colorClass} ${fontSizeClass} transition-colors duration-300`}>
                        {text}
                    </span>
                </div>

                {/* Marquee Layer: Fades in only if it overflows */}
                {showMarquee && (
                    <div className="absolute inset-0 flex shrink-0 hover-pause-container h-full items-center animate-text-fade-in">
                        <div className="flex shrink-0 hover-pause h-full items-center" style={{ animation: `scroll-left-pause ${animDuration}s linear infinite` }}>
                            {Array(repeatCount).fill(0).map((_, i) => (
                                <span key={i} className={`whitespace-nowrap tracking-tight ${colorClass} ${fontSizeClass} mr-16 shrink-0 transition-colors duration-300`}>
                                    {text}
                                </span>
                            ))}
                        </div>
                        <div className="flex shrink-0 hover-pause h-full items-center" style={{ animation: `scroll-left-pause ${animDuration}s linear infinite` }}>
                            {Array(repeatCount).fill(0).map((_, i) => (
                                <span key={i} className={`whitespace-nowrap tracking-tight ${colorClass} ${fontSizeClass} mr-16 shrink-0 transition-colors duration-300`}>
                                    {text}
                                </span>
                            ))}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
});




export default function NowPlaying({
    station, streamMetadata, status, volume,
    onToggle, onVolumeChange,
    onPrev, onNext, setIdentifiedSongs,
    miniMode, onToggleLayout, linkViewOpen,
    linkViewWidth,

    // Shared state from App.jsx
    songCover, setSongCover, enrichedData, setEnrichedData, fetchedListeners,
    setFetchedListeners, isIdentifying, setIsIdentifying, identifyPhase,
    setIdentifyPhase, isAutoIdentify, setIsAutoIdentify, autoNotification,
    setAutoNotification, identifyResult, setIdentifyResult,
    showIdentifyModal, setShowIdentifyModal, handleIdentify, avgColor, isActuallyLight
}) {
    const { t } = useTranslation();
    const isPlaying = status === 'playing';
    const isConnecting = status === 'connecting' || status === 'reconnecting';
    const isPaused = status === 'paused';

    const [eqOpen, setEqOpen] = useState(false);
    const [prevSongCover, setPrevSongCover] = useState(null);
    const [isTransitioning, setIsTransitioning] = useState(false);
    const identifyingRef = useRef(false);
    const [preMuteVolume, setPreMuteVolume] = useState(80);
    const [volHover, setVolHover] = useState(null);
    const pressTimer = useRef(null);
    const [isIdentifyBtnHovered, setIsIdentifyBtnHovered] = useState(false);
    const [faviconError, setFaviconError] = useState(false);
    const [coverError, setCoverError] = useState(false);

    useEffect(() => { setFaviconError(false); }, [station?.favicon]);
    useEffect(() => { setCoverError(false); }, [songCover]);

    // Watch for cover changes to trigger transition animation locally
    useEffect(() => {
        if (songCover && songCover !== prevSongCover) {
            setPrevSongCover(null); // Clear previous if there was one
            // If we already have a cover, we want to crossfade
            // But since this runs AFTER songCover updated, we need to have caught the 'pre-update' state.
            // Actually, a simpler way is to use a ref to track the last cover.
        }
    }, [songCover]);

    const lastCoverRef = useRef(songCover);
    useEffect(() => {
        if (songCover !== lastCoverRef.current) {
            setPrevSongCover(lastCoverRef.current);
            setIsTransitioning(true);
            lastCoverRef.current = songCover;
            const timer = setTimeout(() => {
                setIsTransitioning(false);
                setPrevSongCover(null);
            }, 1200);
            return () => clearTimeout(timer);
        }
    }, [songCover]);

    const [isFavTransitioning, setIsFavTransitioning] = useState(false);
    const [prevMiniVisual, setPrevMiniVisual] = useState(null);
    const [prevStationName, setPrevStationName] = useState(null);
    const [prevStationFavicon, setPrevStationFavicon] = useState(null);
    const [internalStationId, setInternalStationId] = useState(station?.stationuuid || station?.url);
    const [favLoaded, setFavLoaded] = useState(true);
    const lastFavRef = useRef(station?.favicon);
    const stationRef = useRef(station);
    useEffect(() => { stationRef.current = station; }, [station]);

    // Track visual state for the mini player & dikey logo
    const currentId = station?.stationuuid || station?.url;
    if (currentId && currentId !== internalStationId) {
        // Capture what was ACTUALLY shown in mini-player (Cover OR Favicon)
        const oldVisual = lastCoverRef.current && !coverError ? lastCoverRef.current : lastFavRef.current;
        setPrevMiniVisual(oldVisual);
        setPrevStationName(stationRef.current?.name);

        // Capture only station logo for the dikey CD center
        setPrevStationFavicon(lastFavRef.current);

        setIsFavTransitioning(true);
        // Reset load state if there's an incoming image to wait for
        const hasIncomingImage = !!((songCover && !coverError) || (station?.favicon && !faviconError));
        setFavLoaded(!hasIncomingImage);
        setInternalStationId(currentId);
    }

    useEffect(() => {
        if (isFavTransitioning && favLoaded) {
            const timer = setTimeout(() => {
                setIsFavTransitioning(false);
                setPrevMiniVisual(null);
                setPrevStationName(null);
                setPrevStationFavicon(null);
                lastFavRef.current = station?.favicon;
                lastCoverRef.current = songCover;
            }, 800);
            return () => clearTimeout(timer);
        } else if (!isFavTransitioning) {
            lastFavRef.current = station?.favicon;
            lastCoverRef.current = songCover;
        }
    }, [isFavTransitioning, favLoaded, currentId, songCover]);





    const autoIdRef = useRef(isAutoIdentify);
    useEffect(() => { autoIdRef.current = isAutoIdentify; }, [isAutoIdentify]);





    const handleIdentifyDown = () => {
        if (!isPlaying && !isAutoIdentify) return;
        pressTimer.current = setTimeout(() => {
            setIsAutoIdentify(prev => !prev);
            pressTimer.current = null;
        }, 600); // 600ms hold time
    };

    const handleIdentifyUp = () => {
        if (pressTimer.current) {
            clearTimeout(pressTimer.current);
            pressTimer.current = null;
            if (isAutoIdentify) {
                // If auto was enabled, a quick click disables it
                setIsAutoIdentify(false);
                setIsIdentifying(false);
                identifyingRef.current = false;
                setIdentifyPhase('idle');
            } else {
                // Normal identify
                handleIdentify(true); // Pass true to show modal for manual identify
            }
        }
    };

    // Visualizer refs
    const barsRef = useRef([]);
    const rafRef = useRef(null);
    const currentRef = useRef(new Float32Array(NUM_BARS));
    const targetRef = useRef(new Float32Array(NUM_BARS));
    const playingRef = useRef(false);
    const pollTimerRef = useRef(null);

    useEffect(() => {
        playingRef.current = isPlaying;
        if (!isPlaying) targetRef.current.fill(0);
    }, [isPlaying]);

    useEffect(() => {
        const poll = async () => {
            if (!playingRef.current) return;
            try {
                const level = await invoke('get_audio_level');
                const isMuted = volume === 0;

                for (let i = 0; i < NUM_BARS; i++) {
                    if (!isMuted) {
                        const jitter = 0.85 + Math.random() * 0.3;
                        // Further reduce multipliers to bring back more natural peaks and valleys
                        const activeLevel = Math.max(level * 1.4, 0.02);
                        targetRef.current[i] = Math.min(0.9, activeLevel * BAND_WEIGHTS[i] * jitter);
                    } else {
                        targetRef.current[i] = 0;
                    }
                }
            } catch (e) { /* ignore */ }
        };
        pollTimerRef.current = setInterval(poll, 100);
        return () => clearInterval(pollTimerRef.current);
    }, [volume]);

    useEffect(() => {
        const animate = () => {
            const bars = barsRef.current;
            const target = targetRef.current;
            const current = currentRef.current;
            for (let i = 0; i < NUM_BARS; i++) {
                current[i] += (target[i] - current[i]) * 0.25;
                if (bars[i]) {
                    // Index 3-5 used for both mini (horizontal) and status (vertical) indicators
                    const height = 2.5 + (current[i] * 9); // Max 11.5px, safely fits in h-4 (16px)
                    bars[i].style.height = `${height}px`;
                    bars[i].style.opacity = '1';
                }
            }
            rafRef.current = requestAnimationFrame(animate);
        };
        rafRef.current = requestAnimationFrame(animate);
        return () => { if (rafRef.current) cancelAnimationFrame(rafRef.current); };
    }, []);



    const statusText = {
        playing: t('playing.playing'),
        paused: t('playing.paused'),
        stopped: t('playing.stopped'),
        connecting: t('playing.connecting'),
        reconnecting: t('playing.connecting'),
    }[status] || status;

    const statusColor = isPlaying ? 'bg-success' : isConnecting ? 'bg-warning' : isPaused ? 'bg-warning' : 'bg-text-muted';
    const rawIcy = parseInt(streamMetadata?.icy_listeners, 10);
    const liveBack = fetchedListeners !== null ? fetchedListeners : null;
    const listeners = liveBack !== null ? liveBack : (!isNaN(rawIcy) ? rawIcy : 0);
    const isRealTime = liveBack !== null || (!isNaN(rawIcy) && rawIcy > 0);
    const formatCount = (num) => num > 999 ? (num / 1000).toFixed(1) + 'K' : num;

    return (
        <aside className={`${miniMode ? 'w-full h-[100px] flex-row px-4 border-t overflow-visible' : 'w-full flex-1 flex-col border-l overflow-hidden'} relative shrink-0 bg-bg-primary border-border flex items-center transition-colors duration-300`}>

            {/* 0. Immersive Background Art (Ultra Clean & Atmospheric) */}
            {(songCover || station?.favicon) && (
                <div className="absolute inset-0 z-0 pointer-events-none overflow-hidden transition-opacity duration-1000">
                    <img
                        key={songCover || station?.favicon}
                        src={toAssetUrl(songCover || station?.favicon)}
                        alt=""
                        className={`w-full h-full object-cover blur-[24px] opacity-40 contrast-125 scale-110 transition-all duration-1000 animate-fade-in ${isActuallyLight ? 'brightness-110' : 'brightness-75'}`}
                    />
                    <div className="absolute inset-0 transition-colors duration-1000" style={{ backgroundColor: avgColor }} />
                    <div className={`absolute inset-0 bg-gradient-to-b from-transparent ${isActuallyLight ? 'via-bg-primary/2 to-bg-primary' : 'via-bg-primary/5 to-bg-primary'}`} />
                </div>
            )}

            {/* Layout Toggle (Top-Right Sleek Button) */}
            {!linkViewOpen && station && onToggleLayout && (
                <button
                    onClick={onToggleLayout}
                    className={`absolute ${miniMode ? 'top-1 right-1 h-7 px-2' : 'top-1 right-1 h-8 w-8'} z-20 text-text-secondary hover:text-accent transition-all flex items-center justify-center cursor-pointer group`}
                    title={miniMode ? t('playing.verticalMode') : t('playing.horizontalMode')}
                >
                    {miniMode ? (
                        <>
                            <PanelRight size={14} className="group-hover:scale-110 transition-transform" />
                        </>
                    ) : (
                        <PanelBottom size={16} className="group-hover:scale-110 transition-transform" />
                    )}
                </button>
            )}

            {station ? (
                miniMode ? (
                    <div className="flex items-center w-full h-full z-10 px-2">
                        {/* Left: Info */}
                        <div className="flex-1 min-w-0 flex items-center gap-4">
                            <div
                                className={`w-16 h-16 shrink-0 rounded-lg overflow-hidden shadow-lg border border-border relative bg-bg-surface flex items-center justify-center ${(enrichedData || identifyResult) ? 'cursor-pointer hover:scale-105 transition-transform' : ''}`}
                                onClick={() => {
                                    if (identifyResult) {
                                        setShowIdentifyModal(true);
                                    } else if (enrichedData) {
                                        setIdentifyResult({
                                            artist: enrichedData.artist,
                                            title: enrichedData.title,
                                            album: enrichedData.album,
                                            cover: enrichedData.cover,
                                            song_link: enrichedData.song_link
                                        });
                                        setShowIdentifyModal(true);
                                    }
                                }}
                            >
                                <div className="w-full h-full relative overflow-hidden flex items-center justify-center bg-bg-surface-active">
                                    {/* Previous visual (Ghost Layer) */}
                                    {isFavTransitioning && (
                                        <div className={`absolute inset-0 z-10 transition-opacity duration-600 ${favLoaded ? 'animate-logo-fade-out' : 'opacity-100'}`}>
                                            {prevMiniVisual ? (
                                                <img src={toAssetUrl(prevMiniVisual)} alt="" className="w-full h-full object-cover" />
                                            ) : (
                                                <div className="w-full h-full flex items-center justify-center opacity-80">
                                                    <img src="/icon.svg" className="w-[50%] h-[50%] opacity-20 grayscale" alt="Radiko" />
                                                </div>
                                            )}
                                        </div>
                                    )}

                                    {/* New visual (Entering Layer) */}
                                    {((songCover && !coverError) || (station.favicon && !faviconError)) ? (
                                        <img
                                            key={station.stationuuid || station.url}
                                            src={toAssetUrl(songCover && !coverError ? songCover : station.favicon)}
                                            className={`w-full h-full object-cover relative z-20 transition-opacity duration-600 ${favLoaded ? 'animate-logo-fade-in' : 'opacity-0'}`}
                                            alt=""
                                            onLoad={() => setFavLoaded(true)}
                                            onError={() => {
                                                if (songCover && !coverError) setCoverError(true);
                                                else setFaviconError(true);
                                                setFavLoaded(true);
                                            }}
                                        />
                                    ) : (
                                        <div
                                            key={station.stationuuid || station.url + "_initials"}
                                            className={`w-full h-full flex items-center justify-center opacity-80 relative z-20 ${favLoaded ? 'animate-logo-fade-in' : 'opacity-0'}`}
                                        >
                                            <img src="/icon.svg" className="w-[50%] h-[50%] opacity-20 grayscale" alt="Radiko" />
                                        </div>
                                    )}
                                </div>
                            </div>
                            <div className="flex-1 min-w-0 flex flex-col justify-center gap-0.5 items-start relative overflow-hidden">
                                <div
                                    key={station.stationuuid || station.url}
                                    className="flex items-center gap-2 overflow-hidden max-w-full animate-text-fade-in"
                                >
                                    <SmartMarquee text={station.name} miniMode={miniMode} isTitle={true} />
                                    <div className="flex items-end gap-[1.5px] h-4 shrink-0 px-0.5 mt-0.5 mb-0.5">
                                        {[0, 1, 2].map(i => (
                                            <div key={i} ref={el => barsRef.current[i + 3] = el}
                                                className="w-[2px] rounded-full shadow-[0_0_4px_rgba(var(--accent-rgb),0.6)]"
                                                style={{
                                                    height: '3px',
                                                    background: isPlaying ? 'rgb(var(--accent))' : 'rgb(var(--text-muted))',
                                                    willChange: 'height'
                                                }}
                                            />
                                        ))}
                                    </div>
                                </div>
                                <div className={`w-full grid transition-all duration-300 ease-in-out ${streamMetadata?.title ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'}`}>
                                    <div className="overflow-hidden">
                                        <div className="w-full flex flex-col pb-0.5">
                                            <SmartMarquee text={streamMetadata?.title} miniMode={miniMode} isAccent={true} />
                                        </div>
                                    </div>
                                </div>
                                <div className={`w-full grid transition-all duration-300 ease-in-out ${streamMetadata?.icy_name && streamMetadata.icy_name !== station.name ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'}`}>
                                    <div className="overflow-hidden">
                                        {streamMetadata?.icy_name && streamMetadata.icy_name !== station.name ? (
                                            <SmartMarquee text={streamMetadata.icy_name} miniMode={miniMode} isDimmed={true} />
                                        ) : (
                                            <div className="h-[18px] w-full shrink-0" />
                                        )}
                                    </div>
                                </div>
                            </div>
                        </div>



                        {/* Center: Playback Controls */}
                        <div className="flex items-center gap-5 shrink-0 px-8">
                            <button onClick={onPrev} className="text-text-muted hover:text-text-primary hover:scale-110 transition-all flex items-center justify-center cursor-pointer">
                                <SkipBack size={20} stroke="none" fill="currentColor" />
                            </button>
                            <button onClick={onToggle} className="w-9 h-9 rounded-full bg-text-primary hover:bg-text-secondary text-bg-primary transition-all flex items-center justify-center hover:scale-105 cursor-pointer shadow-lg">
                                {isPlaying || isConnecting ? <Pause fill="currentColor" size={16} /> : <Play fill="currentColor" size={16} className="ml-0.5" />}
                            </button>
                            <button onClick={onNext} className="text-text-muted hover:text-text-primary hover:scale-110 transition-all flex items-center justify-center cursor-pointer">
                                <SkipForward size={20} stroke="none" fill="currentColor" />
                            </button>
                        </div>

                        {/* Right: Extra Controls */}
                        <div className="flex-1 min-w-0 flex items-center justify-end gap-6 text-text-secondary">
                            {/* EQ */}
                            <button onClick={() => setEqOpen(!eqOpen)}
                                className={`transition-all hover:scale-110 cursor-pointer ${eqOpen ? 'text-accent' : 'text-text-secondary hover:text-text-primary'}`}
                                title="Equalizer"
                            >
                                <SlidersHorizontal size={18} strokeWidth={2.5} />
                            </button>

                            {/* Identify */}
                            <button
                                onMouseDown={handleIdentifyDown}
                                onMouseUp={handleIdentifyUp}
                                onMouseEnter={() => setIsIdentifyBtnHovered(true)}
                                onMouseLeave={() => { handleIdentifyUp(); setIsIdentifyBtnHovered(false); }}
                                onTouchStart={handleIdentifyDown}
                                onTouchEnd={handleIdentifyUp}
                                disabled={!isPlaying && !isAutoIdentify}
                                className={`transition-all hover:scale-110 flex items-center justify-center cursor-pointer relative focus:outline-none ${isIdentifying ? 'text-accent' : isAutoIdentify ? 'text-accent' : 'text-text-secondary hover:text-text-primary'} ${!isPlaying && !isAutoIdentify ? 'opacity-30 cursor-not-allowed' : ''}`}
                                title={t('identify.autoModeHint')}
                            >
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    className={`relative z-10
                                    ${isIdentifying ? 'animate-heartbeat text-accent' : ''}
                                    ${isAutoIdentify && !isIdentifying ? 'text-accent' : ''}
                                    ${!isIdentifying && !isAutoIdentify ? 'text-inherit opacity-80' : ''}`}
                                    viewBox="0 0 24 24" fill="currentColor" width="18" height="18"
                                >
                                    <path d="M12 0C5.373 0-.001 5.371-.001 12c0 6.625 5.374 12 12.001 12s12-5.375 12-12c0-6.629-5.373-12-12-12M9.872 16.736c-1.287 0-2.573-.426-3.561-1.281-1.214-1.049-1.934-2.479-2.029-4.024-.09-1.499.42-2.944 1.436-4.067C6.86 6.101 8.907 4.139 8.993 4.055c.555-.532 1.435-.511 1.966.045.53.557.512 1.439-.044 1.971-.021.02-2.061 1.976-3.137 3.164-.508.564-.764 1.283-.719 2.027.049.789.428 1.529 1.07 2.086.844.73 2.51.891 3.553-.043.619-.559 1.372-1.377 1.38-1.386.52-.567 1.4-.603 1.965-.081.565.52.603 1.402.083 1.969-.035.035-.852.924-1.572 1.572-1.005.902-2.336 1.357-3.666 1.357m8.41-.099c-1.143 1.262-3.189 3.225-3.276 3.309-.27.256-.615.385-.96.385-.368 0-.732-.145-1.006-.43-.531-.559-.512-1.439.044-1.971.021-.02 2.063-1.977 3.137-3.166.508-.563.764-1.283.719-2.027-.048-.789-.428-1.529-1.07-2.084-.844-.73-2.51-.893-3.552.044-.621.556-1.373 1.376-1.38 1.384-.521.566-1.399.604-1.966.084-.564-.521-.604-1.404-.082-1.971.034-.037.85-.926 1.571-1.573 1.979-1.778 5.221-1.813 7.227-.077 1.214 1.051 1.935 2.48 2.028 4.025.092 1.497-.419 2.945-1.434 4.068" />
                                </svg>

                                {/* Mini Tooltip for Identifying Phase */}
                                {isIdentifying && (isIdentifyBtnHovered || !isAutoIdentify) && (() => {
                                    const phases = {
                                        recording: { label: t('identify.listening'), progress: 25 },
                                        encoding: { label: t('identify.analyzing'), progress: 50 },
                                        sending: { label: t('identify.sending'), progress: 75 },
                                        idle: { label: t('identify.idle'), progress: 10 }
                                    };
                                    const phase = phases[identifyPhase] || phases.idle;
                                    return (
                                        <div className="absolute bottom-full mb-3 left-1/2 -translate-x-1/2 w-[120px] bg-bg-secondary border border-accent/20 rounded-lg shadow-2xl p-2.5 flex flex-col items-center gap-2 z-[60] animate-in slide-in-from-bottom-2 fade-in duration-200">
                                            {/* Pointer */}
                                            <div className="absolute -bottom-1.5 left-1/2 -translate-x-1/2 w-3 h-3 bg-bg-secondary border-b border-r border-accent/20 rotate-45" />

                                            <div className="flex items-center gap-1.5 justify-center w-full">
                                                <span className="w-1.5 h-1.5 rounded-full bg-accent animate-ping" />
                                                <span className="text-[10px] text-accent font-bold uppercase tracking-wider text-center">
                                                    {phase.label}
                                                </span>
                                            </div>

                                            {/* Progress Bar */}
                                            <div className="w-full h-1.5 bg-bg-surface-active rounded-full overflow-hidden">
                                                <div className="h-full bg-accent rounded-full transition-all duration-300" style={{ width: `${phase.progress}%` }} />
                                            </div>

                                            <span className="text-[7.5px] text-text-muted/60 font-semibold uppercase leading-none mt-0.5">{t('identify.clickToCancel')}</span>
                                        </div>
                                    );
                                })()}
                            </button>

                            {/* Volume */}
                            <div className="flex items-center gap-2 min-w-[120px] max-w-[150px] w-full ml-4">
                                <button
                                    onClick={() => {
                                        if (volume === 0) onVolumeChange(preMuteVolume > 0 ? preMuteVolume : 80);
                                        else { setPreMuteVolume(volume); onVolumeChange(0); }
                                    }}
                                    className="text-text-secondary hover:text-text-primary transition-colors flex items-center justify-center w-5 cursor-pointer shrink-0"
                                    title={volume === 0 ? t('playing.unmute') : t('playing.mute')}
                                >
                                    {volume === 0 ? <VolumeX size={24} /> : volume < 50 ? <Volume1 size={24} /> : <Volume2 size={24} />}
                                </button>
                                <div className="flex-1 w-full px-1 h-1 bg-border/50 rounded-full relative cursor-pointer group"
                                    onPointerMove={e => {
                                        const rect = e.currentTarget.getBoundingClientRect();
                                        setVolHover(Math.max(0, Math.min(100, ((e.clientX - rect.left) / rect.width) * 100)));
                                    }}
                                    onPointerLeave={() => setVolHover(null)}
                                    onPointerDown={e => {
                                        const rect = e.currentTarget.getBoundingClientRect();
                                        const perc = Math.max(0, Math.min(100, ((e.clientX - rect.left) / rect.width) * 100));
                                        onVolumeChange(perc);

                                        const handleMove = ev => {
                                            const p = Math.max(0, Math.min(100, ((ev.clientX - rect.left) / rect.width) * 100));
                                            onVolumeChange(p);
                                        };
                                        const handleUp = () => {
                                            document.removeEventListener('pointermove', handleMove);
                                            document.removeEventListener('pointerup', handleUp);
                                            document.removeEventListener('pointercancel', handleUp);
                                        };
                                        document.addEventListener('pointermove', handleMove);
                                        document.addEventListener('pointerup', handleUp);
                                        document.addEventListener('pointercancel', handleUp);
                                    }}>
                                    {/* Expanded hit area */}
                                    <div className="absolute -inset-y-4 inset-x-0 bg-transparent z-10" />
                                    {/* Hover preview track */}
                                    {volHover !== null && (
                                        <div className="absolute top-0 left-0 bottom-0 bg-text-primary/20 rounded-full pointer-events-none transition-all duration-75 z-0" style={{ width: `${volHover}%` }} />
                                    )}
                                    <div className="absolute top-0 left-0 bottom-0 bg-text-primary group-hover:bg-accent rounded-full pointer-events-none transition-all duration-75 z-10" style={{ width: `${volume}%` }} />
                                    <div className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-text-primary rounded-full shadow-md pointer-events-none transition-all duration-75 scale-0 group-hover:scale-100 z-20" style={{ left: `calc(${volume}% - 5px)` }} />
                                </div>
                                <span className="text-[10px] text-text-muted font-mono w-7 shrink-0 text-right">{Math.round(volume)}%</span>
                            </div>

                        </div>
                    </div>
                ) : (
                    <>
                        {/* Main content — fully responsive scaling based on height */}
                        <div className="flex-1 flex flex-col items-center justify-around w-full relative z-10 overflow-y-auto py-[min(40px,6vh)] scrollbar-hide transition-all duration-300">
                            {/* Modern Floating Top Badges - Fixed Position */}
                            <div className="absolute top-[min(20px,3vh)] left-0 right-0 flex items-center justify-center gap-2 px-10 z-30 w-full flex-wrap animate-text-fade-in shrink-0 pointer-events-none">
                                <div className="flex items-center justify-center gap-2 flex-wrap pointer-events-auto">
                                    {listeners > 0 && (
                                        <span className="px-2 py-0.5 rounded-md bg-bg-secondary/50 border border-border/50 text-text-primary text-[10px] font-bold tracking-wider shadow-sm flex items-center gap-1 shrink-0" title={t('playing.liveListeners')}>
                                            <Users size={10} className={"text-success opacity-90"} />
                                            {formatCount(listeners)}
                                        </span>
                                    )}
                                    {streamMetadata?.icy_genre && (
                                        <span className="px-2 py-0.5 rounded-md bg-accent/10 border border-accent/20 text-accent text-[9px] font-bold uppercase tracking-wider shadow-sm truncate max-w-[100px]">
                                            {streamMetadata.icy_genre}
                                        </span>
                                    )}
                                    {streamMetadata?.icy_br && (
                                        <span className="px-2 py-0.5 rounded-md bg-bg-secondary/50 border border-border/50 text-text-primary text-[9px] font-bold tracking-widest shadow-sm shrink-0">
                                            {streamMetadata.icy_br}K
                                        </span>
                                    )}
                                    {streamMetadata?.icy_url && (
                                        <button onClick={() => invoke('open_link_window', { url: streamMetadata.icy_url.startsWith('http') ? streamMetadata.icy_url : `http://${streamMetadata.icy_url}` })}
                                            className="shrink-0 w-5 h-5 rounded-full bg-bg-secondary/50 hover:bg-bg-secondary/80 border border-border/50 text-text-secondary hover:text-text-primary transition-all flex items-center justify-center cursor-pointer shadow-sm" title="Website">
                                            <Globe size={11} />
                                        </button>
                                    )}
                                </div>
                            </div>

                            {/* 1 & 2. Hero Section (Responsive sizing - locked to square aspect) */}
                            <div className="w-[min(85%,42vh)] aspect-square relative z-20 flex-none overflow-hidden rounded-full shadow-[0_20px_50px_rgba(0,0,0,0.5)]">

                                {/* Rotating Assembly (Isolated Layer) */}
                                <div className={`absolute inset-0 flex items-center justify-center transition-transform duration-1000 ${!isPlaying ? 'paused-anim' : ''}`} style={{ transform: 'translateZ(0)' }}>

                                    <div className={`absolute inset-0 flex items-center justify-center ${songCover ? 'animate-spin-slow' : ''}`}>

                                        <div
                                            className={`absolute aspect-square transition-all duration-1000 cubic-bezier(0.34, 1.56, 0.64, 1) flex items-center justify-center overflow-hidden z-30 rounded-full
                                        ${songCover
                                                    ? `w-[20%] border border-border shadow-lg ${isTransitioning ? 'animate-logo-boost' : ''}`
                                                    : `w-[75%] border border-border shadow-2xl ${(identifyResult || enrichedData) ? 'cursor-pointer hover:scale-[1.02]' : ''}`}`}
                                            onClick={() => {
                                                if (!songCover) {
                                                    if (identifyResult) {
                                                        setShowIdentifyModal(true);
                                                    } else if (enrichedData) {
                                                        setIdentifyResult({
                                                            artist: enrichedData.artist,
                                                            title: enrichedData.title,
                                                            album: enrichedData.album,
                                                            cover: enrichedData.cover,
                                                            song_link: enrichedData.song_link
                                                        });
                                                        setShowIdentifyModal(true);
                                                    }
                                                }
                                            }}
                                        >
                                            <div className="w-full h-full flex items-center justify-center bg-bg-secondary rounded-full relative overflow-hidden transition-colors duration-300">
                                                {prevStationFavicon && (
                                                    <img
                                                        src={toAssetUrl(prevStationFavicon)}
                                                        alt=""
                                                        className={`absolute inset-0 w-full h-full object-cover rounded-full transition-opacity duration-600 ${favLoaded ? 'animate-logo-fade-out' : 'opacity-100'}`}
                                                    />
                                                )}
                                                {station.favicon && !faviconError ? (
                                                    <img
                                                        key={station.stationuuid || station.url}
                                                        src={toAssetUrl(station.favicon)}
                                                        alt=""
                                                        className={`w-full h-full object-cover rounded-full relative z-20 transition-opacity duration-600 ${favLoaded ? 'animate-logo-fade-in' : 'opacity-0'}`}
                                                        onLoad={() => setFavLoaded(true)}
                                                        onError={() => { setFaviconError(true); setFavLoaded(true); }}
                                                    />
                                                ) : (
                                                    <div
                                                        key={station.stationuuid || station.url + "_initials_dikey"}
                                                        className={`w-full h-full bg-bg-surface-active flex items-center justify-center opacity-80 rounded-full relative z-20 ${favLoaded ? 'animate-logo-fade-in' : 'opacity-0'}`}
                                                    >
                                                        <img src="/icon.svg" className="w-[50%] h-[50%] opacity-20 grayscale" alt="Radiko" />
                                                    </div>
                                                )}
                                            </div>
                                        </div>

                                        {/* Rotating CD Disc Art (Stackable) */}
                                        {prevSongCover && isTransitioning && (
                                            <div className="w-[84%] aspect-square absolute inset-0 m-auto pointer-events-none opacity-40 scale-[0.98]">
                                                <div className="w-full h-full rounded-full overflow-hidden border border-border relative">
                                                    <img src={toAssetUrl(prevSongCover)} alt="" className="w-full h-full object-cover" />
                                                    <div className="absolute inset-0 bg-black/40" />
                                                </div>
                                            </div>
                                        )}

                                        {/* Outer CD Disc (Shows Song Cover) */}
                                        {songCover && (
                                            <div
                                                key={songCover}
                                                className={`w-[84%] aspect-square relative transition-all duration-1000 ${isTransitioning ? 'animate-album-art' : ''} ${(enrichedData || identifyResult) ? 'cursor-pointer hover:scale-[1.02]' : ''}`}
                                                onClick={() => {
                                                    if (identifyResult) {
                                                        setShowIdentifyModal(true);
                                                    } else if (enrichedData) {
                                                        setIdentifyResult({
                                                            artist: enrichedData.artist,
                                                            title: enrichedData.title,
                                                            album: enrichedData.album,
                                                            cover: enrichedData.cover,
                                                            song_link: enrichedData.song_link
                                                        });
                                                        setShowIdentifyModal(true);
                                                    }
                                                }}
                                            >
                                                <div className="w-full h-full rounded-full overflow-hidden border border-border/50 relative">
                                                    {(songCover && !coverError) ? (
                                                        <img src={toAssetUrl(songCover)} alt="" className="w-full h-full object-cover" onError={() => setCoverError(true)} />
                                                    ) : (
                                                        <div className="w-full h-full bg-bg-surface-active flex items-center justify-center opacity-80">
                                                            <img src="/icon.svg" className="w-[50%] h-[50%] opacity-20 grayscale" alt="Radiko" />
                                                        </div>
                                                    )}

                                                    {/* Center Plate (Seamless background for the logo) */}
                                                    <div className="absolute inset-0 flex items-center justify-center">
                                                        <div className="w-[24%] h-[24%] rounded-full bg-bg-primary shadow-inner transition-colors duration-300" />
                                                    </div>

                                                    {/* Texture & Reflection */}
                                                    <div className="absolute inset-0 bg-[radial-gradient(circle,transparent_35%,rgba(0,0,0,0.2)_100%)] pointer-events-none" />
                                                    <div className="absolute inset-0 bg-gradient-to-tr from-accent/5 via-transparent to-transparent pointer-events-none opacity-50" />
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </div>

                            {/* Station name and Meta Info - Fixed height to prevent pushing controls */}
                            <div className="text-center w-full px-4 flex flex-col items-center justify-center flex-none h-[92px] shrink-0 overflow-hidden">
                                <div
                                    key={station.stationuuid || station.url}
                                    className="py-0.5 animate-text-fade-in w-full text-center"
                                >
                                    <SmartMarquee text={station.name} miniMode={miniMode} isTitle={true} />
                                </div>

                                <div className={`w-full grid transition-all duration-300 ease-in-out ${streamMetadata?.title ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'}`}>
                                    <div className="overflow-hidden">
                                        <div className="py-0.5">
                                            <SmartMarquee text={streamMetadata?.title} miniMode={miniMode} isAccent={true} />
                                        </div>
                                    </div>
                                </div>

                                <div className={`w-full grid transition-all duration-300 ease-in-out ${streamMetadata?.icy_name && streamMetadata.icy_name !== station.name ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'}`}>
                                    <div className="overflow-hidden">
                                        {streamMetadata?.icy_name && streamMetadata.icy_name !== station.name && (
                                            <div className="py-0.5">
                                                <SmartMarquee text={streamMetadata.icy_name} miniMode={miniMode} isDimmed={true} />
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </div>

                            {/* Controls & Tools */}
                            <div className="flex flex-col items-center gap-[min(12px,1.5vh)] w-full shrink-0">
                                {/* Status */}
                                <div className="flex items-center gap-2">
                                    <div className="flex items-end gap-[1.5px] h-4 shrink-0 px-0.5">
                                        {[0, 1, 2].map(i => (
                                            <div key={i} ref={el => barsRef.current[i + 3] = el}
                                                className="w-[2px] rounded-full shadow-[0_0_4px_rgba(var(--accent-rgb),0.6)]"
                                                style={{
                                                    height: '3px',
                                                    background: isPlaying ? 'rgb(var(--accent))' : 'rgb(var(--text-muted))',
                                                    willChange: 'height'
                                                }}
                                            />
                                        ))}
                                    </div>
                                    <span className="text-[10px] text-text-primary uppercase font-black tracking-[0.1em]">{statusText}</span>
                                </div>

                                {/* Main Controls */}
                                <div className="flex items-center gap-6">
                                    <button onClick={onPrev} className="text-text-muted hover:text-text-primary hover:scale-110 transition-all cursor-pointer">
                                        <SkipBack size={28} stroke="none" fill="currentColor" />
                                    </button>
                                    <button onClick={onToggle} className="w-12 h-12 rounded-full bg-text-primary hover:bg-text-secondary text-bg-primary transition-all flex items-center justify-center hover:scale-105 cursor-pointer shadow-xl">
                                        {isPlaying || isConnecting ? <Pause fill="currentColor" size={20} /> : <Play fill="currentColor" size={20} className="ml-0.5" />}
                                    </button>
                                    <button onClick={onNext} className="text-text-muted hover:text-text-primary hover:scale-110 transition-all cursor-pointer">
                                        <SkipForward size={28} stroke="none" fill="currentColor" />
                                    </button>
                                </div>

                                {/* Tools */}
                                <div className="flex items-center gap-7">
                                    <button onClick={() => setEqOpen(!eqOpen)}
                                        className={`transition-all hover:scale-110 cursor-pointer ${eqOpen ? 'text-accent' : 'text-text-secondary hover:text-text-primary'}`}
                                        title="Equalizer"
                                    >
                                        <SlidersHorizontal size={18} strokeWidth={2.5} />
                                    </button>
                                    <button
                                        onMouseDown={handleIdentifyDown}
                                        onMouseUp={handleIdentifyUp}
                                        onMouseEnter={() => setIsIdentifyBtnHovered(true)}
                                        onMouseLeave={() => { handleIdentifyUp(); setIsIdentifyBtnHovered(false); }}
                                        onTouchStart={handleIdentifyDown}
                                        onTouchEnd={handleIdentifyUp}
                                        disabled={!isPlaying && !isAutoIdentify}
                                        className={`transition-all hover:scale-110 flex items-center justify-center cursor-pointer relative focus:outline-none ${isIdentifying ? 'text-accent' : isAutoIdentify ? 'text-accent' : 'text-text-secondary hover:text-text-primary'} ${!isPlaying && !isAutoIdentify ? 'opacity-30 cursor-not-allowed' : ''}`}
                                        title={t('identify.autoModeHint')}
                                    >
                                        <svg xmlns="http://www.w3.org/2000/svg" className={`relative z-10 ${isIdentifying ? 'animate-heartbeat text-accent' : ''} ${isAutoIdentify && !isIdentifying ? 'text-accent' : ''} ${!isIdentifying && !isAutoIdentify ? 'text-inherit opacity-80' : ''}`} viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
                                            <path d="M12 0C5.373 0-.001 5.371-.001 12c0 6.625 5.374 12 12.001 12s12-5.375 12-12c0-6.629-5.373-12-12-12M9.872 16.736c-1.287 0-2.573-.426-3.561-1.281-1.214-1.049-1.934-2.479-2.029-4.024-.09-1.499.42-2.944 1.436-4.067C6.86 6.101 8.907 4.139 8.993 4.055c.555-.532 1.435-.511 1.966.045.53.557.512 1.439-.044 1.971-.021.02-2.061 1.976-3.137 3.164-.508.564-.764 1.283-.719 2.027.049.789.428 1.529 1.07 2.086.844.73 2.51.891 3.553-.043.619-.559 1.372-1.377 1.38-1.386.52-.567 1.4-.603 1.965-.081.565.52.603 1.402.083 1.969-.035.035-.852.924-1.572 1.572-1.005.902-2.336 1.357-3.666 1.357m8.41-.099c-1.143 1.262-3.189 3.225-3.276 3.309-.27.256-.615.385-.96.385-.368 0-.732-.145-1.006-.43-.531-.559-.512-1.439.044-1.971.021-.02 2.063-1.977 3.137-3.166.508-.563.764-1.283.719-2.027-.048-.789-.428-1.529-1.07-2.084-.844-.73-2.51-.893-3.552.044-.621.556-1.373 1.376-1.38 1.384-.521.566-1.399.604-1.966.084-.564-.521-.604-1.404-.082-1.971.034-.037.85-.926 1.571-1.573 1.979-1.778 5.221-1.813 7.227-.077 1.214 1.051 1.935 2.48 2.028 4.025.092 1.497-.419 2.945-1.434 4.068" />
                                        </svg>
                                    </button>
                                </div>
                            </div>

                            {/* Volume */}
                            <div className="flex items-center gap-3 w-full max-w-[200px] shrink-0">
                                <button
                                    onClick={() => {
                                        if (volume === 0) {
                                            onVolumeChange(preMuteVolume > 0 ? preMuteVolume : 80);
                                        } else {
                                            setPreMuteVolume(volume);
                                            onVolumeChange(0);
                                        }
                                    }}
                                    className="text-text-secondary hover:text-text-primary transition-colors flex items-center justify-center w-5 cursor-pointer"
                                    title={volume === 0 ? t('playing.unmute') : t('playing.mute')}
                                >
                                    {volume === 0 ? <VolumeX size={24} /> : volume < 50 ? <Volume1 size={24} /> : <Volume2 size={24} />}
                                </button>
                                <div className="flex-1 w-full px-1 h-1 bg-border rounded-full relative cursor-pointer group"
                                    onPointerMove={e => {
                                        const rect = e.currentTarget.getBoundingClientRect();
                                        setVolHover(Math.max(0, Math.min(100, ((e.clientX - rect.left) / rect.width) * 100)));
                                    }}
                                    onPointerLeave={() => setVolHover(null)}
                                    onPointerDown={e => {
                                        const rect = e.currentTarget.getBoundingClientRect();
                                        const perc = Math.max(0, Math.min(100, ((e.clientX - rect.left) / rect.width) * 100));
                                        onVolumeChange(perc);

                                        const handleMove = ev => {
                                            const p = Math.max(0, Math.min(100, ((ev.clientX - rect.left) / rect.width) * 100));
                                            onVolumeChange(p);
                                        };
                                        const handleUp = () => {
                                            document.removeEventListener('pointermove', handleMove);
                                            document.removeEventListener('pointerup', handleUp);
                                            document.removeEventListener('pointercancel', handleUp);
                                        };
                                        document.addEventListener('pointermove', handleMove);
                                        document.addEventListener('pointerup', handleUp);
                                        document.addEventListener('pointercancel', handleUp);
                                    }}>
                                    {/* Expanded hit area */}
                                    <div className="absolute -inset-y-4 inset-x-0 bg-transparent z-10" />
                                    {/* Hover preview track */}
                                    {volHover !== null && (
                                        <div className="absolute top-0 left-0 bottom-0 bg-text-secondary/20 rounded-full pointer-events-none transition-all duration-75 z-0" style={{ width: `${volHover}%` }} />
                                    )}
                                    <div className="absolute top-0 left-0 bottom-0 bg-text-primary group-hover:bg-accent rounded-full pointer-events-none transition-all duration-75 z-10" style={{ width: `${volume}%` }} />
                                    <div className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-text-primary rounded-full shadow-md pointer-events-none transition-all duration-75 scale-0 group-hover:scale-100 z-20" style={{ left: `calc(${volume}% - 5px)` }} />
                                </div>
                                <span className="text-[10px] text-text-muted font-mono w-8 text-right">{Math.round(volume)}%</span>
                            </div>
                        </div>
                    </>
                )
            ) : (
                <div className="flex-1 flex flex-col items-center justify-center text-text-muted gap-3 mt-auto mb-auto">
                    <Radio size={miniMode ? 24 : 48} className="opacity-20" />
                    {!miniMode && <p className="text-sm font-semibold opacity-50">{t('playing.selectRadio')}</p>}
                </div>
            )
            }

            {
                eqOpen && (
                    <div
                        className={miniMode ? `fixed top-0 bottom-[100px] left-0 z-[10000] flex flex-col items-center justify-center bg-black/60 p-4 backdrop-blur-sm` : "absolute inset-0 z-40 flex flex-col bg-bg-secondary/95 backdrop-blur-sm"}
                        style={miniMode ? { right: linkViewOpen ? linkViewWidth : 0 } : {}}
                    >
                        <div className={miniMode ? "bg-bg-secondary rounded-2xl shadow-2xl border border-border flex flex-col w-[300px] overflow-hidden" : "flex-1 flex flex-col"}>
                            {/* Header */}
                            <div className="flex items-center justify-between px-4 py-3 border-b border-border/50 shrink-0">
                                <span className="text-xs font-bold uppercase tracking-wider text-text-primary">Equalizer</span>
                                <button onClick={() => setEqOpen(false)}
                                    className="w-7 h-7 rounded-full bg-bg-surface hover:bg-bg-surface-hover text-text-muted hover:text-text-primary transition-all flex items-center justify-center cursor-pointer">
                                    <X size={14} />
                                </button>
                            </div>
                            {/* EQ content */}
                            <div className="flex-1 flex items-center justify-center p-5">
                                <div className="w-full max-w-[260px]">
                                    <EqualizerPanel />
                                </div>
                            </div>
                        </div>
                    </div>
                )
            }

            {/* Identify Result Modal */}
            {
                showIdentifyModal && (
                    <div
                        className={miniMode ? `fixed top-0 bottom-[100px] left-0 z-[9999] flex flex-col items-center justify-center bg-black/60 p-4 backdrop-blur-sm` : "absolute inset-0 z-50 flex flex-col items-center justify-center bg-black/85 backdrop-blur-lg"}
                        style={miniMode ? { right: linkViewOpen ? linkViewWidth : 0 } : {}}
                    >
                        <div className="w-[260px] bg-bg-secondary rounded-2xl border border-border shadow-2xl overflow-hidden">
                            {/* Close button */}
                            <div className="flex justify-end p-3 pb-0">
                                <button onClick={() => setShowIdentifyModal(false)}
                                    className="w-7 h-7 rounded-full bg-bg-surface hover:bg-bg-surface-hover text-text-muted hover:text-text-primary transition-all flex items-center justify-center cursor-pointer">
                                    <X size={12} />
                                </button>
                            </div>

                            {(() => {
                                // Searching / Recording state
                                if (isIdentifying) {
                                    return (
                                        <div className="flex flex-col items-center px-5 pb-6 gap-3 pt-2">
                                            <div className="w-16 h-16 rounded-full bg-accent/10 flex items-center justify-center animate-pulse">
                                                <div className="w-8 h-8 rounded-full bg-accent/20 flex items-center justify-center">
                                                    <div className="w-4 h-4 rounded-full bg-accent" />
                                                </div>
                                            </div>
                                            <div className="text-center space-y-1">
                                                <div className="text-sm font-bold text-text-primary animate-pulse">{t('identify.recording')}</div>
                                                <div className="text-[10px] text-white/50">{t('identify.wait')}</div>
                                            </div>
                                        </div>
                                    );
                                }

                                // API error (e.g. auth failure)
                                if (identifyResult && identifyResult._error) {
                                    return (
                                        <div className="flex flex-col items-center px-5 pb-5 gap-3">
                                            <div className="w-16 h-16 rounded-full bg-red-500/10 flex items-center justify-center">
                                                <X size={28} className="text-red-400" />
                                            </div>
                                            <div className="text-center space-y-1">
                                                <div className="text-sm font-bold text-text-primary">{t('identify.apiError')}</div>
                                                <div className="text-[10px] text-text-muted leading-relaxed">{identifyResult._message}</div>
                                            </div>
                                        </div>
                                    );
                                }
                                // Song found
                                if (identifyResult && identifyResult !== 'not_found' && identifyResult !== 'error') {
                                    const artUrl = identifyResult.cover
                                        || identifyResult.spotify?.album?.images?.[0]?.url
                                        || (identifyResult.apple_music?.artwork?.url || '').replace('{w}', '300').replace('{h}', '300')
                                        || songCover;
                                    return (
                                        <div className="flex flex-col items-center px-5 pb-5 gap-3">
                                            {artUrl ? (
                                                <div className="w-28 h-28 rounded-xl overflow-hidden border border-border shadow-lg">
                                                    <img src={artUrl.startsWith('file') ? toAssetUrl(artUrl) : artUrl} alt=""
                                                        className="w-full h-full object-cover" onError={e => e.target.style.display = 'none'} />
                                                </div>
                                            ) : (
                                                <div className="w-28 h-28 rounded-xl bg-accent/10 flex items-center justify-center ring-1 ring-accent/20">
                                                    <Music size={36} className="text-accent opacity-60" />
                                                </div>
                                            )}
                                            <div className="text-center space-y-1 w-full">
                                                <div className="text-sm font-bold text-text-primary truncate">{identifyResult.title || t('identify.unknownSong')}</div>
                                                <div className="text-xs text-accent font-semibold truncate">{identifyResult.artist || t('identify.unknownArtist')}</div>
                                                {identifyResult.album && (
                                                    <div className="text-[10px] text-white/70 truncate">{identifyResult.album}</div>
                                                )}
                                                {identifyResult.release_date && (
                                                    <div className="text-[10px] text-white/50 truncate">{identifyResult.release_date}</div>
                                                )}
                                            </div>
                                            <div className="flex gap-2 mt-1">
                                                {identifyResult.song_link && (
                                                    <button onClick={() => invoke('open_link_window', { url: identifyResult.song_link })}
                                                        className="px-3 py-1.5 rounded-full bg-accent/10 border border-accent/20 text-accent text-[10px] font-bold uppercase tracking-wider hover:bg-accent/20 transition-all cursor-pointer">
                                                        {t('identify.listen')}
                                                    </button>
                                                )}
                                            </div>
                                        </div>
                                    );
                                }
                                // Not found or generic error
                                return (
                                    <div className="flex flex-col items-center px-5 pb-6 gap-3">
                                        <div className="w-16 h-16 rounded-full bg-bg-surface flex items-center justify-center">
                                            <Headphones size={28} className="text-text-muted opacity-40" />
                                        </div>
                                        <div className="text-center space-y-1">
                                            <div className="text-sm font-bold text-text-primary">
                                                {identifyResult === 'error' ? t('identify.errorOccurred') : t('identify.notFoundTitle')}
                                            </div>
                                            <div className="text-[10px] text-white/70">
                                                {identifyResult === 'error'
                                                    ? t('identify.identificationError')
                                                    : t('identify.identificationFailed')}
                                            </div>
                                        </div>
                                    </div>
                                );
                            })()}
                        </div>
                    </div>
                )
            }
        </aside >
    );
}
