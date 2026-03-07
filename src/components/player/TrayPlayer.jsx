import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, emitTo } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Play, Pause, Volume2, VolumeX, SkipBack, SkipForward } from 'lucide-react';
import { toAssetUrl } from '../../utils';

export default function TrayPlayer() {
    const [status, setStatus] = useState('stopped');
    const [stationName, setStationName] = useState('Radiko Desktop');
    const [stationImage, setStationImage] = useState(null);
    const [title, setTitle] = useState('');
    const [cover, setCover] = useState(null);

    const win = getCurrentWindow();
    const stationNameRef = useRef('Radiko Desktop');
    const focusRef = useRef(null);

    const [volume, setVolume] = useState(1);
    const preMuteVolumeRef = useRef(1);
    const [showVolume, setShowVolume] = useState(false);

    const syncStatus = async () => {
        try {
            const res = await invoke('get_status');
            if (res) {
                setStatus(res.status);

                const newStation = res.station_name || 'Radiko Desktop';
                setStationName(newStation);
                setStationImage(res.station_image || null);
                setTitle(res.metadata?.title || '');
                setVolume(res.volume);

                if (stationNameRef.current !== newStation) {
                    setCover(res.station_image || null);
                    stationNameRef.current = newStation;
                } else {
                    setCover(c => c || res.station_image || null);
                }
            }
        } catch (e) {
            console.error(e);
        }
    };

    useEffect(() => {
        // Essential for rounded corners to not show the main window's background color
        document.body.style.background = 'transparent';
        document.documentElement.style.background = 'transparent';
        const root = document.getElementById('root');
        if (root) root.style.background = 'transparent';

        syncStatus();

        const unlistenStatus = listen('playback-status', (event) => {
            setStatus(event.payload);
            syncStatus();
        });

        const unlistenMetadata = listen('stream-metadata', (event) => {
            setTitle(event.payload?.title || '');
        });

        const unlistenEnriched = listen('metadata-enriched', (event) => {
            if (!event.payload.is_fallback) {
                if (event.payload.cover) setCover(event.payload.cover);
            } else {
                setCover(null);
            }
        });

        const unlistenVolume = listen('volume-changed', (event) => {
            setVolume(event.payload);
        });

        const unlistenOpened = listen('tray-opened', async () => {
            syncStatus();
            setTimeout(() => {
                window.focus();
                if (focusRef.current) {
                    focusRef.current.focus();
                }
            }, 50);
        });

        const unlistenFocus = win.onFocusChanged(({ payload: focused }) => {
            if (focused) {
                syncStatus();
            } else {
                win.hide();
            }
        });

        const unlistenHideTray = listen('hide-tray', () => {
            win.hide();
        });

        const handleBlur = () => {
            win.hide();
        };

        window.addEventListener('blur', handleBlur);

        return () => {
            // Restore (though not strictly necessary since tray window lives forever)
            document.body.style.background = '';
            document.documentElement.style.background = '';
            if (root) root.style.background = '';

            unlistenStatus.then(u => u());
            unlistenMetadata.then(u => u());
            unlistenEnriched.then(u => u());
            unlistenVolume.then(u => u());
            unlistenOpened.then(u => u());
            unlistenFocus.then(u => u());
            unlistenHideTray.then(u => u());
            window.removeEventListener('blur', handleBlur);
        };
    }, []);

    const handlePlayPause = async () => {
        if (status === 'playing') {
            await invoke('pause');
        } else if (status === 'paused' || status === 'stopped') {
            if (status === 'paused') {
                await invoke('resume');
            } else {
                try {
                    await emitTo('main', 'media-key', 'toggle');
                } catch (e) {
                    console.error("Play from stopped failed:", e);
                }
            }
        }
    };

    const handleOpenMain = async () => {
        try {
            win.hide();
            const { Window } = await import('@tauri-apps/api/window');
            const mainWin = new Window('main');
            if (mainWin) {
                try { await mainWin.unminimize(); } catch (err) { }
                await mainWin.show();
                await mainWin.setFocus();
            }
        } catch (e) {
            console.error("Failed to open main win:", e);
        }
    };

    const handleMuteToggle = async () => {
        try {
            if (volume > 0) {
                preMuteVolumeRef.current = volume;
                await invoke('set_volume', { level: 0 });
                setVolume(0);
            } else {
                const restoredVolume = preMuteVolumeRef.current > 0 ? preMuteVolumeRef.current : 0.8;
                await invoke('set_volume', { level: restoredVolume });
                setVolume(restoredVolume);
            }
        } catch (e) {
            console.error("Mute toggle failed:", e);
        }
    };

    const handleVolumeChange = async (e) => {
        const v = parseFloat(e.target.value);
        setVolume(v);
        try {
            await invoke('set_volume', { level: v });
        } catch (err) {
            console.error("Volume change failed:", err);
        }
    };

    const handlePrev = async () => {
        try {
            await emitTo('main', 'media-key', 'previous');
        } catch (e) {
            console.error("Prev failed:", e);
        }
    };

    const handleNext = async () => {
        try {
            await emitTo('main', 'media-key', 'next');
        } catch (e) {
            console.error("Next failed:", e);
        }
    };

    const displayCover = toAssetUrl(cover) || toAssetUrl(stationImage) || '/icon.svg';
    const displayTitle = title || stationName || 'No station playing';
    const displayArtist = title ? stationName : 'Radiko Desktop';

    const btnStyle = {
        background: 'transparent',
        border: 'none',
        color: 'rgba(255,255,255,0.6)',
        cursor: 'pointer',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '6px',
        borderRadius: '50%',
        transition: 'all 0.15s ease',
    };

    const btnHoverProps = {
        onMouseEnter: (e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'; e.currentTarget.style.color = '#fff'; },
        onMouseLeave: (e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'rgba(255,255,255,0.6)'; },
    };

    return (
        <div style={{
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            backgroundColor: 'rgba(21, 21, 25, 0.95)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4)',
            borderRadius: '12px',
            color: '#fff',
            width: '100%',
            height: '100%',
            overflow: 'hidden',
            boxSizing: 'border-box',
            padding: '10px',
            fontFamily: 'Manrope, sans-serif',
            userSelect: 'none',
            outline: 'none',
            gap: '12px',
        }}
            tabIndex={0}
            ref={focusRef}
        >
            {/* Left Cover Art */}
            <div
                onClick={handleOpenMain}
                style={{
                    width: '60px',
                    height: '60px',
                    borderRadius: '8px',
                    overflow: 'hidden',
                    backgroundColor: 'rgba(255,255,255,0.05)',
                    flexShrink: 0,
                    cursor: 'pointer',
                    boxShadow: '0 4px 10px rgba(0,0,0,0.3)',
                    transition: 'transform 0.15s ease',
                }}
                onMouseEnter={(e) => e.currentTarget.style.transform = 'scale(1.03)'}
                onMouseLeave={(e) => e.currentTarget.style.transform = 'scale(1)'}
            >
                <img
                    src={displayCover}
                    alt="Cover"
                    style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                    onError={(e) => { e.currentTarget.src = '/icon.svg'; e.currentTarget.onerror = null; }}
                />
            </div>

            {/* Right Column: Metadata + Controls */}
            <div style={{
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'center',
                flex: 1,
                minWidth: 0,
                height: '100%',
            }}>
                {/* Metadata */}
                <div
                    onClick={handleOpenMain}
                    style={{
                        cursor: 'pointer',
                        display: 'flex',
                        flexDirection: 'column',
                        // Optional padding to push text up slightly
                        paddingTop: '2px',
                    }}
                >
                    <div style={{
                        fontSize: '13px',
                        fontWeight: 600,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        lineHeight: '1.2',
                    }}>
                        {displayTitle}
                    </div>
                    <div style={{
                        fontSize: '11px',
                        color: 'rgba(255,255,255,0.5)',
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        lineHeight: '1.2',
                        marginTop: '3px',
                    }}>
                        {displayArtist}
                    </div>
                </div>

                {/* Controls - Playback & Volume inline */}
                <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    marginTop: 'auto',
                }}>
                    {/* Playback controls */}
                    <div style={{ display: 'flex', alignItems: 'center', gap: '2px', marginLeft: '-6px' }}>
                        <button
                            onClick={handlePrev}
                            style={{ ...btnStyle, padding: '4px' }}
                            title="Previous station"
                            {...btnHoverProps}
                        >
                            <SkipBack size={15} />
                        </button>
                        <button
                            onClick={handlePlayPause}
                            style={{
                                ...btnStyle,
                                color: '#fff',
                                padding: '4px 6px',
                            }}
                            {...btnHoverProps}
                        >
                            {status === 'playing' || status === 'connecting' ? <Pause size={17} fill="currentColor" /> : <Play size={17} fill="currentColor" />}
                        </button>
                        <button
                            onClick={handleNext}
                            style={{ ...btnStyle, padding: '4px' }}
                            title="Next station"
                            {...btnHoverProps}
                        >
                            <SkipForward size={15} />
                        </button>
                    </div>

                    {/* Volume */}
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginRight: '4px' }}>
                        <button
                            onClick={handleMuteToggle}
                            style={{
                                ...btnStyle,
                                padding: '4px',
                                color: volume === 0 ? 'rgb(16, 185, 129)' : 'rgba(255,255,255,0.6)',
                            }}
                            {...btnHoverProps}
                        >
                            {volume === 0 ? <VolumeX size={14} /> : <Volume2 size={14} />}
                        </button>
                        <input
                            type="range"
                            min="0"
                            max="1"
                            step="0.01"
                            value={volume}
                            onChange={handleVolumeChange}
                            style={{
                                width: '60px',
                                height: '4px',
                                WebkitAppearance: 'none',
                                appearance: 'none',
                                background: `linear-gradient(to right, ${volume === 0 ? 'rgba(255,255,255,0.2)' : 'rgb(16, 185, 129)'} ${volume * 100}%, rgba(255,255,255,0.15) ${volume * 100}%)`,
                                borderRadius: '2px',
                                outline: 'none',
                                cursor: 'pointer',
                            }}
                        />
                    </div>
                </div>
            </div>
        </div>
    );
}
