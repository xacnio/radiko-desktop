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
            flexDirection: 'column',
            backgroundColor: 'rgba(21,21,21,0.95)',
            border: '1px solid rgba(255,255,255,0.08)',
            borderRadius: '12px',
            color: '#fff',
            width: '100%',
            height: '100%',
            overflow: 'hidden',
            boxSizing: 'border-box',
            padding: '12px',
            fontFamily: 'Manrope, sans-serif',
            userSelect: 'none',
            outline: 'none',
            gap: '8px',
        }}
            tabIndex={0}
            ref={focusRef}
        >
            {/* Top row: Cover + Metadata */}
            <div
                onClick={handleOpenMain}
                style={{
                    display: 'flex',
                    flexDirection: 'row',
                    alignItems: 'center',
                    gap: '12px',
                    flex: 1,
                    minHeight: 0,
                    cursor: 'pointer',
                    padding: '4px',
                    borderRadius: '8px',
                    transition: 'background-color 0.15s ease'
                }}
                onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.05)'; }}
                onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
            >
                {/* Cover Art */}
                <div style={{
                    width: '56px',
                    height: '56px',
                    borderRadius: '8px',
                    overflow: 'hidden',
                    backgroundColor: '#000',
                    flexShrink: 0,
                }}>
                    <img
                        src={displayCover}
                        alt="Cover"
                        style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                        onError={(e) => { e.currentTarget.src = '/icon.svg'; e.currentTarget.onerror = null; }}
                    />
                </div>

                {/* Metadata */}
                <div style={{
                    display: 'flex',
                    flexDirection: 'column',
                    justifyContent: 'center',
                    flex: 1,
                    minWidth: 0,
                }}>
                    <div style={{
                        fontSize: '13px',
                        fontWeight: 600,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        marginBottom: '2px',
                    }}>
                        {displayTitle}
                    </div>
                    <div style={{
                        fontSize: '11px',
                        color: 'rgba(255,255,255,0.45)',
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                    }}>
                        {displayArtist}
                    </div>
                </div>
            </div>

            {/* Bottom row: Controls + Volume */}
            <div style={{
                display: 'flex',
                alignItems: 'center',
                gap: '4px',
                justifyContent: 'space-between',
            }}>
                {/* Playback controls */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '2px' }}>
                    <button
                        onClick={handlePrev}
                        style={btnStyle}
                        title="Previous station"
                        {...btnHoverProps}
                    >
                        <SkipBack size={16} />
                    </button>
                    <button
                        onClick={handlePlayPause}
                        style={{
                            ...btnStyle,
                            color: '#fff',
                            padding: '8px',
                            backgroundColor: 'rgba(255,255,255,0.1)',
                        }}
                        onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.2)'; }}
                        onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'; }}
                    >
                        {status === 'playing' || status === 'connecting' ? <Pause size={18} fill="currentColor" /> : <Play size={18} fill="currentColor" />}
                    </button>
                    <button
                        onClick={handleNext}
                        style={btnStyle}
                        title="Next station"
                        {...btnHoverProps}
                    >
                        <SkipForward size={16} />
                    </button>
                </div>

                {/* Volume control */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                    <button
                        onClick={handleMuteToggle}
                        style={{
                            ...btnStyle,
                            color: volume === 0 ? 'rgb(16, 185, 129)' : 'rgba(255,255,255,0.5)',
                        }}
                        onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'; }}
                        onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                    >
                        {volume === 0 ? <VolumeX size={16} /> : <Volume2 size={16} />}
                    </button>
                    <input
                        type="range"
                        min="0"
                        max="1"
                        step="0.01"
                        value={volume}
                        onChange={handleVolumeChange}
                        style={{
                            width: '80px',
                            height: '4px',
                            WebkitAppearance: 'none',
                            appearance: 'none',
                            background: `linear-gradient(to right, rgba(255,255,255,0.7) ${volume * 100}%, rgba(255,255,255,0.15) ${volume * 100}%)`,
                            borderRadius: '2px',
                            outline: 'none',
                            cursor: 'pointer',
                            accentColor: '#fff',
                        }}
                    />
                </div>
            </div>
        </div>
    );
}
