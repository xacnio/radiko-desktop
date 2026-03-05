import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Edit, Trash2, PenLine, Globe, Settings, Search, X, Music, ExternalLink, Radio, Heart, Tags, MapPin, Languages, Image, Zap, Check } from 'lucide-react';
import TitleBar from './components/common/TitleBar';
import Sidebar from './components/layout/Sidebar';
import StationList from './components/stations/StationList';
import NowPlaying from './components/player/NowPlaying';
import EditStationModal from './components/stations/EditStationModal';
import SetupScreen from './views/SetupScreen';
import ApiSearchModal from './components/stations/ApiSearchModal';
import ConfirmModal from './components/common/ConfirmModal';
import IdentifiedSongsList from './components/stations/IdentifiedSongsList';
import SettingsView from './views/SettingsView';
import { toAssetUrl } from './utils';
import { FastAverageColor } from 'fast-average-color';
import { NotificationProvider, useNotification } from './contexts/NotificationProvider';
import { useTranslation } from 'react-i18next';
import { availableLanguages } from './i18n';
import { check } from '@tauri-apps/plugin-updater';

const win = getCurrentWindow();

export default function App() {
    // Show the main window as soon as React is mounted
    useEffect(() => {
        win.show().catch(() => { });
    }, []);

    // Layout state is here so it can be passed to NotificationProvider
    const [isPlayerHorizontal, setIsPlayerHorizontal] = useState(() => localStorage.getItem('player_horizontal') === 'true');
    const [linkViewOpen, setLinkViewOpen] = useState(false);
    const [sidebarWidth, setSidebarWidth] = useState(() => parseInt(localStorage.getItem('sidebar_width') || '400', 10));
    const [playerWidth, setPlayerWidth] = useState(() => parseInt(localStorage.getItem('player_width') || '260', 10));
    const [leftSidebarWidth, setLeftSidebarWidth] = useState(() => parseInt(localStorage.getItem('left_sidebar_width') || '220', 10));
    const [windowTooSmall, setWindowTooSmall] = useState(false);

    // Track if horizontal mode was forced by window size
    const wasForcedRef = useRef(false);

    useEffect(() => {
        localStorage.setItem('player_horizontal', isPlayerHorizontal);
    }, [isPlayerHorizontal]);

    // Auto-switch to horizontal when window is too narrow for vertical player
    const MIN_MIDDLE_WIDTH = 525;
    useEffect(() => {
        const checkSize = () => {
            const middleWidth = window.innerWidth - leftSidebarWidth - playerWidth;
            const tooSmall = middleWidth < MIN_MIDDLE_WIDTH;
            setWindowTooSmall(tooSmall);

            if (tooSmall) {
                if (!isPlayerHorizontal) {
                    setIsPlayerHorizontal(true);
                    wasForcedRef.current = true;
                }
            } else {
                if (wasForcedRef.current && isPlayerHorizontal) {
                    // Restore if it was forced and we now have plenty of space
                    if (middleWidth > MIN_MIDDLE_WIDTH + 50) {
                        setIsPlayerHorizontal(false);
                        wasForcedRef.current = false;
                    }
                }
            }
        };

        checkSize();
        window.addEventListener('resize', checkSize);
        // Also check on a timer for cases like split-view or snapping where events might lag
        const interval = setInterval(checkSize, 300);
        return () => {
            window.removeEventListener('resize', checkSize);
            clearInterval(interval);
        };
    }, [isPlayerHorizontal, leftSidebarWidth, playerWidth]);

    // Manual toggle wrapper that clears the "forced" flag
    const handleSetLayout = (val) => {
        wasForcedRef.current = false;
        setIsPlayerHorizontal(val);
    };

    return (
        <NotificationProvider isPlayerHorizontal={isPlayerHorizontal} linkViewOpen={linkViewOpen} sidebarWidth={sidebarWidth} playerWidth={playerWidth}>
            <AppInner
                isPlayerHorizontal={isPlayerHorizontal}
                setIsPlayerHorizontal={handleSetLayout}
                linkViewOpen={linkViewOpen}
                setLinkViewOpen={setLinkViewOpen}
                sidebarWidth={sidebarWidth}
                setSidebarWidth={setSidebarWidth}
                playerWidth={playerWidth}
                setPlayerWidth={setPlayerWidth}
                windowTooSmall={windowTooSmall}
                leftSidebarWidth={leftSidebarWidth}
                setLeftSidebarWidth={setLeftSidebarWidth}
            />
        </NotificationProvider>
    );
}

function AppInner({ isPlayerHorizontal, setIsPlayerHorizontal, linkViewOpen, setLinkViewOpen, sidebarWidth, setSidebarWidth, playerWidth, setPlayerWidth, windowTooSmall, leftSidebarWidth, setLeftSidebarWidth }) {
    const { notify, setHasActiveStation } = useNotification();
    const { t, i18n } = useTranslation();
    const [showLanguageSetup, setShowLanguageSetup] = useState(() => !localStorage.getItem('language_setup_done'));

    // --- Auto update logic moved below state hooks

    // --- local data ---
    const [customStations, setCustomStations] = useState([]);

    // --- dynamic lists derived from customStations ---
    const locations = useMemo(() => {
        const locMap = new Map(); // country -> { total, citiesMap: city -> count }
        customStations.forEach(c => {
            const country = c.country || 'Unknown';
            const city = c.state || '';
            if (!locMap.has(country)) {
                locMap.set(country, { total: 0, citiesMap: new Map() });
            }
            const countryData = locMap.get(country);
            countryData.total++;
            if (city) {
                countryData.citiesMap.set(city, (countryData.citiesMap.get(city) || 0) + 1);
            }
        });

        return Array.from(locMap.entries())
            .map(([country, data]) => ({
                country,
                count: data.total,
                cities: Array.from(data.citiesMap.entries())
                    .map(([name, count]) => ({ name, count }))
                    .sort((a, b) => a.name.localeCompare(b.name))
            }))
            .sort((a, b) => a.country.localeCompare(b.country));
    }, [customStations]);

    const localTags = useMemo(() => {
        const tMap = new Map(); // tag -> count
        customStations.forEach(c => {
            if (c.tags) {
                c.tags.split(',').map(tag => tag.trim().toLowerCase()).forEach(tag => {
                    if (tag) {
                        tMap.set(tag, (tMap.get(tag) || 0) + 1);
                    }
                });
            }
        });
        return Array.from(tMap.entries())
            .map(([name, count]) => ({ name, stationcount: count }))
            .sort((a, b) => a.name.localeCompare(b.name));
    }, [customStations]);

    const localLanguages = useMemo(() => {
        const lMap = new Map(); // language -> count
        customStations.forEach(c => {
            if (c.language) {
                c.language.split(',').map(l => l.trim()).filter(Boolean).forEach(lang => {
                    const key = lang.toLowerCase();
                    if (!lMap.has(key)) lMap.set(key, { display: lang, count: 0 });
                    lMap.get(key).count++;
                });
            }
        });
        return Array.from(lMap.values())
            .map(({ display, count }) => ({ name: display, stationcount: count }))
            .sort((a, b) => a.name.localeCompare(b.name));
    }, [customStations]);

    // --- UI state (initialized from localStorage to avoid race conditions) ---
    const getSavedUI = () => {
        try {
            const raw = localStorage.getItem('rx_ui_state');
            const data = raw ? JSON.parse(raw) : {};
            if (data.tab === 'settings') data.tab = 'all';
            return data;
        } catch { return {}; }
    };
    const savedUI = useMemo(() => getSavedUI(), []);

    const [tab, setTab] = useState(() => savedUI.tab || 'all');
    const [theme, setTheme] = useState(() => savedUI.theme || 'system');
    const [selectedTag, setSelectedTag] = useState(() => savedUI.selectedTag || null);
    const [selectedCountry, setSelectedCountry] = useState(() => savedUI.selectedCountry || null);
    const [selectedCity, setSelectedCity] = useState(() => savedUI.selectedCity || null);
    const [selectedLanguage, setSelectedLanguage] = useState(() => savedUI.selectedLanguage || null);
    const [accentColor, setAccentColor] = useState(() => savedUI.accentColor || 'green');
    const [mixAccent, setMixAccent] = useState(() => savedUI.mixAccent || false);
    const [searchQuery, setSearchQuery] = useState('');
    const [status, setStatus] = useState('stopped');
    const [activeUuid, setActiveUuid] = useState(() => savedUI.activeUuid || null);
    const [activeStation, setActiveStation] = useState(() => savedUI.activeStation || null);

    useEffect(() => {
        if (setHasActiveStation) {
            setHasActiveStation(!!activeStation);
        }
    }, [activeStation, setHasActiveStation]);

    // --- Auto Update Check ---
    useEffect(() => {
        const initAutoUpdate = async () => {
            if (localStorage.getItem('auto_update') !== 'false') {
                try {
                    const update = await check();
                    if (update) {
                        notify({
                            type: 'info',
                            title: t('settings.updateAvailable'),
                            message: t('settings.updateFound', { version: update.version }),
                            onClick: () => {
                                setTab('settings');
                                setTimeout(() => {
                                    window.dispatchEvent(new CustomEvent('rx_trigger_update', { detail: update }));
                                }, 300);
                            },
                            duration: 15000
                        });
                    }
                } catch (e) {
                    console.error("Auto update check failed", e);
                }
            }
        };

        const timerId = setTimeout(initAutoUpdate, 100);
        return () => clearTimeout(timerId);
    }, [notify, t, setTab]);

    const [streamMetadata, setStreamMetadata] = useState(null);
    const [volume, setVolume] = useState(100);
    const [sortBy, setSortBy] = useState(() => savedUI.sortBy || 'manual');
    const [sortOrder, setSortOrder] = useState(() => savedUI.sortOrder || 'asc');
    const [identifiedSongs, setIdentifiedSongs] = useState([]);

    // --- Theme handling ---
    const [isActuallyLight, setIsActuallyLight] = useState(() => {
        if (theme === 'light') return true;
        if (theme === 'dark') return false;
        return !window.matchMedia('(prefers-color-scheme: dark)').matches;
    });

    useEffect(() => {
        const root = document.documentElement;
        const update = () => {
            const light = theme === 'light' ? true : (theme === 'dark' ? false : !window.matchMedia('(prefers-color-scheme: dark)').matches);
            setIsActuallyLight(light);
            if (light) root.classList.add('light');
            else root.classList.remove('light');
            root.setAttribute('data-accent', accentColor);
            root.setAttribute('data-mix-accent', mixAccent);
        };
        update();
        const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
        mediaQuery.addEventListener('change', update);
        return () => mediaQuery.removeEventListener('change', update);
    }, [theme, accentColor, mixAccent]);


    // --- Context Menu & Modal ---
    const [ctxMenu, setCtxMenu] = useState(null);
    const [ctxImageSearchResults, setCtxImageSearchResults] = useState(null);
    const [isCtxSearching, setIsCtxSearching] = useState(false);
    const [editModalStation, setEditModalStation] = useState(null); // null=closed, {} = new, {station} = edit
    const [showAddChoiceModal, setShowAddChoiceModal] = useState(false);
    const [showApiSearchModal, setShowApiSearchModal] = useState(false);
    const [confirmDialog, setConfirmDialog] = useState({ isOpen: false, title: '', message: '', onConfirm: null, variant: 'danger', showCancel: true, confirmText: null });
    const [config, setConfig] = useState({});

    // --- lifted from NowPlaying for persistence ---
    const [songCover, setSongCover] = useState(null);
    const [enrichedData, setEnrichedData] = useState(null);
    const [fetchedListeners, setFetchedListeners] = useState(null);
    const [isIdentifying, setIsIdentifying] = useState(false);
    const [identifyPhase, setIdentifyPhase] = useState('idle');
    const [isAutoIdentify, setIsAutoIdentify] = useState(() => localStorage.getItem('auto_identify') === 'true');
    const [identifyResult, setIdentifyResult] = useState(null);
    const [showIdentifyModal, setShowIdentifyModal] = useState(false);
    const [avgColor, setAvgColor] = useState('rgba(0,0,0,0.6)');

    // --- Advanced Shazam settings ---
    const [autoIdentifyCooldownSuccess, setAutoIdentifyCooldownSuccess] = useState(() =>
        Math.min(120, parseInt(localStorage.getItem('auto_identify_cooldown_success') || '60', 10))
    );
    const [autoIdentifyCooldownFail, setAutoIdentifyCooldownFail] = useState(() =>
        Math.min(60, parseInt(localStorage.getItem('auto_identify_cooldown_fail') || '30', 10))
    );
    const lastAutoIdentifyTimeRef = useRef(0);
    const lastIdentifyStatusRef = useRef('success'); // 'success' | 'fail'
    const [isSettingsLoaded, setIsSettingsLoaded] = useState(false);

    // --- Panel resizing ---
    const [isResizing, setIsResizing] = useState(null); // null | 'browser' | 'left' | 'player'

    // --- Navigation History ---
    const [viewHistory, setViewHistory] = useState([]);
    const isNavigatingBackRef = useRef(false);
    const [forceScrollTop, setForceScrollTop] = useState(0);

    useEffect(() => {
        if (isNavigatingBackRef.current) {
            isNavigatingBackRef.current = false;
            return;
        }

        const currentView = { tab, selectedTag, selectedCountry, selectedCity, selectedLanguage };

        setViewHistory(prev => {
            const last = prev[prev.length - 1];
            if (last &&
                last.tab === currentView.tab &&
                last.selectedTag === currentView.selectedTag &&
                last.selectedCountry === currentView.selectedCountry &&
                last.selectedCity === currentView.selectedCity &&
                last.selectedLanguage === currentView.selectedLanguage) {
                return prev;
            }
            return [...prev, currentView].slice(-25); // keep max 25 items
        });
    }, [tab, selectedTag, selectedCountry, selectedCity, selectedLanguage]);

    const onGoBack = useCallback(() => {
        if (viewHistory.length > 1) {
            const prevView = viewHistory[viewHistory.length - 2];
            isNavigatingBackRef.current = true;
            setViewHistory(prev => prev.slice(0, -1));

            setTab(prevView.tab);
            setSelectedTag(prevView.selectedTag);
            setSelectedCountry(prevView.selectedCountry);
            setSelectedCity(prevView.selectedCity);
            setSelectedLanguage(prevView.selectedLanguage);
        } else {
            setTab('all');
            setSelectedTag(null);
            setSelectedCountry(null);
            setSelectedCity(null);
            setSelectedLanguage(null);
        }
    }, [viewHistory]);

    const canGoBack = viewHistory.length > 1 || tab !== 'all';

    useEffect(() => {
        if (!isResizing) return;

        const handleMouseMove = (e) => {
            if (isResizing === 'browser') {
                const newWidth = window.innerWidth - e.clientX;
                setSidebarWidth(Math.round(Math.max(270, Math.min(newWidth, 370))));
            } else if (isResizing === 'left') {
                const newWidth = e.clientX;
                setLeftSidebarWidth(Math.round(Math.max(180, Math.min(newWidth, 300))));
            } else if (isResizing === 'player') {
                const newWidth = window.innerWidth - e.clientX;
                setPlayerWidth(Math.round(Math.max(220, Math.min(newWidth, 340))));
            }
        };

        const handleMouseUp = () => {
            setIsResizing(null);
            document.body.style.cursor = 'default';
        };

        document.body.style.cursor = 'col-resize';
        if (isResizing === 'browser') {
            invoke('set_link_view_interaction', { enabled: false }).catch(console.error);
        }

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
            document.body.style.cursor = 'default';
            if (isResizing === 'browser') {
                invoke('set_link_view_interaction', { enabled: true }).catch(console.error);
            }
        };
    }, [isResizing]);

    useEffect(() => {
        localStorage.setItem('sidebar_width', sidebarWidth);
        if (linkViewOpen) {
            invoke('update_link_view_width', { width: sidebarWidth });
        }
    }, [sidebarWidth, linkViewOpen]);

    useEffect(() => { localStorage.setItem('left_sidebar_width', leftSidebarWidth); }, [leftSidebarWidth]);
    useEffect(() => { localStorage.setItem('player_width', playerWidth); }, [playerWidth]);

    // Dynamic background color extraction
    useEffect(() => {
        const fac = new FastAverageColor();
        const imgUrl = toAssetUrl(songCover || activeStation?.favicon);

        if (imgUrl) {
            fac.getColorAsync(imgUrl)
                .then(color => {
                    let [r, g, b] = color.value;
                    let alpha = isActuallyLight ? 0.45 : 0.8;

                    const brightness = (r * 299 + g * 587 + b * 114) / 1000;

                    if (isActuallyLight) {
                        if (brightness < 40) {
                            r = Math.round(r + (255 - r) * 0.85);
                            g = Math.round(g + (255 - g) * 0.85);
                            b = Math.round(b + (255 - b) * 0.85);
                            alpha = 0.55;
                        } else if (brightness < 120) {
                            r = Math.round(r + (255 - r) * 0.65);
                            g = Math.round(g + (255 - g) * 0.65);
                            b = Math.round(b + (255 - b) * 0.65);
                            alpha = 0.5;
                        } else {
                            r = Math.round(r + (255 - r) * 0.4);
                            g = Math.round(g + (255 - g) * 0.4);
                            b = Math.round(b + (255 - b) * 0.4);
                            alpha = 0.45;
                        }
                    } else {
                        if (brightness > 220) {
                            r = Math.round(r * 0.1);
                            g = Math.round(g * 0.1);
                            b = Math.round(b * 0.1);
                            alpha = 0.92;
                        } else if (color.isLight) {
                            r = Math.round(r * 0.22);
                            g = Math.round(g * 0.22);
                            b = Math.round(b * 0.22);
                            alpha = 0.88;
                        } else {
                            r = Math.round(r * 0.55);
                            g = Math.round(g * 0.55);
                            b = Math.round(b * 0.55);
                            alpha = 0.82;
                        }
                    }

                    setAvgColor(`rgba(${r}, ${g}, ${b}, ${alpha})`);
                })
                .catch(() => {
                    setAvgColor('rgba(13, 13, 13, 0.7)');
                });
        } else {
            setAvgColor('rgba(0,0,0,0.65)');
        }
    }, [songCover, activeStation?.favicon, isActuallyLight]);



    useEffect(() => {
        localStorage.setItem('auto_identify', isAutoIdentify);
    }, [isAutoIdentify]);

    useEffect(() => {
        localStorage.setItem('auto_identify_cooldown_success', autoIdentifyCooldownSuccess);
    }, [autoIdentifyCooldownSuccess]);

    useEffect(() => {
        localStorage.setItem('auto_identify_cooldown_fail', autoIdentifyCooldownFail);
    }, [autoIdentifyCooldownFail]);

    const identifyingRef = useRef(false);
    const lastFoundRef = useRef({ title: '', artist: '' });

    const handleIdentify = useCallback(async (isManual = false) => {
        if (status !== 'playing') return;

        if (isIdentifying) {
            if (isManual) {
                setIsIdentifying(false);
                identifyingRef.current = false;
                setIdentifyPhase('idle');
            }
            return;
        }

        // Only show "Identifying..." UI state for manual triggers or when NOT looking at a previous result
        const updateUI = isManual || !showIdentifyModal;

        if (updateUI) {
            setIsIdentifying(true);
            setIdentifyResult(null);
            setIdentifyPhase('recording');
        }

        identifyingRef.current = true;

        try {
            const found = await invoke('identify_song');
            if (!identifyingRef.current) return;

            if (found && !found._error) {
                lastIdentifyStatusRef.current = 'success';
                const isSameSong = found.title === lastFoundRef.current.title && found.artist === lastFoundRef.current.artist;

                if (!isManual && isSameSong) {
                    if (updateUI) setIdentifyResult(found);
                    return;
                }

                lastFoundRef.current = { title: found.title || '', artist: found.artist || '' };

                // Only update the "Identify Result" modal/state if it's a manual trigger 
                // OR the modal isn't already open showing something else.
                if (updateUI) {
                    setIdentifyResult(found);
                    if (isManual) setShowIdentifyModal(true);
                }

                if (!isManual) {
                    notify({
                        type: 'shazam',
                        title: found.title,
                        artist: found.artist,
                        cover: found.cover,
                        onClick: () => {
                            setIdentifyResult(found);
                            setShowIdentifyModal(true);
                        }
                    });
                }

                const artUrl = found.cover
                    || found.spotify?.album?.images?.[0]?.url
                    || (found.apple_music?.artwork?.url || '').replace('{w}', '300').replace('{h}', '300')
                    || '';
                const songRecord = {
                    title: found.title || t('app.unknown'),
                    artist: found.artist || t('app.unknown'),
                    album: found.album || '',
                    release_date: found.release_date || '',
                    cover: artUrl,
                    song_link: found.song_link || '',
                    station_name: activeStation?.name || t('app.unknownRadio'),
                    found_at: new Date().toISOString(),
                    source: 'Shazam',
                    sources: [{ name: 'Shazam', link: found.song_link || '' }]
                };

                await invoke('save_identified_song', { song: songRecord }).catch(() => { });
            } else {
                lastIdentifyStatusRef.current = 'fail';
                // If it's manual, we definitely want to show failure.
                // If it's auto-mode, we ONLY show failure if the modal wasn't already open.
                // This prevents background-fail from wiping a previous success the user is looking at.
                if (updateUI) {
                    setIdentifyResult(found || 'not_found');
                    if (isManual) setShowIdentifyModal(true);
                }
            }
        } catch (e) {
            lastIdentifyStatusRef.current = 'fail';
            if (!identifyingRef.current) return;
            console.error('Identification failed:', e);
            if (updateUI) {
                setIdentifyResult('error');
                if (isManual) setShowIdentifyModal(true);
            }
        } finally {
            if (identifyingRef.current) {
                if (!isManual) {
                    lastAutoIdentifyTimeRef.current = Date.now();
                }
                setIsIdentifying(false);
                identifyingRef.current = false;
                setIdentifyPhase('idle');
            }
        }
    }, [status, activeStation, isIdentifying, showIdentifyModal]);

    // Auto Identify Loop
    useEffect(() => {
        if (isAutoIdentify && status === 'playing') {
            const title = streamMetadata?.title || "(Unknown)";

            // Check cooldowns
            const now = Date.now();
            const cooldown = lastIdentifyStatusRef.current === 'success'
                ? autoIdentifyCooldownSuccess * 1000
                : autoIdentifyCooldownFail * 1000;

            if (now - lastAutoIdentifyTimeRef.current < cooldown) {
                return;
            }

            // Short delay to ensure buffer is full and stream is stable
            const timer = setTimeout(() => {
                handleIdentify(false);
            }, 3000);
            return () => clearTimeout(timer);
        }
    }, [activeUuid, streamMetadata?.title, isAutoIdentify, status, handleIdentify, autoIdentifyCooldownSuccess, autoIdentifyCooldownFail]);

    // Reset some states when station changes
    useEffect(() => {
        setFetchedListeners(null);
        setSongCover(null);
        setEnrichedData(null);
        setIdentifyResult(null);
        lastFoundRef.current = { title: '', artist: '' };
        lastAutoIdentifyTimeRef.current = 0; // Bypass cooldown for the first check on new station

        if (isAutoIdentify) {
            setIsIdentifying(false);
            setIdentifyPhase('idle');
        }
    }, [activeUuid]);

    // Handle auto-identify status changes separately
    useEffect(() => {
        if (!isAutoIdentify) {
            setIsIdentifying(false);
            setIdentifyPhase('idle');
            // If we were identifying, stop it via the backend too if possible
            // handleIdentify would have been aborted by state change
        }
    }, [isAutoIdentify]);

    const stopTimerRef = useRef(null);
    useEffect(() => {
        if (status === 'paused') {
            stopTimerRef.current = setTimeout(() => {
                invoke('stop').catch(() => { });
            }, 60000);
        } else {
            if (stopTimerRef.current) {
                clearTimeout(stopTimerRef.current);
                stopTimerRef.current = null;
            }
        }
        return () => {
            if (stopTimerRef.current) clearTimeout(stopTimerRef.current);
        };
    }, [status]);

    // Lifted Metadata Enrichment Listener
    useEffect(() => {
        const unlisten = listen('metadata-enriched', (event) => {
            const data = event.payload;
            if (data.original_title === streamMetadata?.title || data.title === streamMetadata?.title) {
                if (data.cover) {
                    setSongCover(data.cover);
                } else {
                    setSongCover(null);
                }
                if (!data.is_fallback) {
                    setEnrichedData(data);
                } else {
                    setEnrichedData(null);
                }
            }
        });
        return () => { unlisten.then(u => u()); };
    }, [streamMetadata?.title]);

    // Lifted Identify Phase Listener
    useEffect(() => {
        const unlisten = listen('identify_phase', (event) => {
            setIdentifyPhase(event.payload);
        });
        return () => { unlisten.then(u => u()); };
    }, []);

    // Lifted Listeners Poller
    useEffect(() => {
        let interval;
        if (status === 'playing' && activeStation?.urlResolved) {
            const doFetch = async () => {
                try {
                    const l = await invoke('fetch_live_listeners', { url: activeStation.urlResolved });
                    if (l !== null && l !== undefined) setFetchedListeners(l);
                } catch (e) { console.log('Listeners fetch err:', e); }
            };
            doFetch();
            interval = setInterval(doFetch, 15000);
        }
        return () => clearInterval(interval);
    }, [status, activeStation?.urlResolved]);

    const fetchCustomStations = useCallback(async () => {
        try {
            const custom = await invoke('get_custom_stations');
            setCustomStations(custom);
        } catch { }
    }, []);

    // --- fetch custom stations on mount ---
    useEffect(() => {
        fetchCustomStations();
        invoke('get_identified_songs').then(setIdentifiedSongs).catch(() => { });

        const unlistenHistory = listen('history-updated', () => {
            invoke('get_identified_songs').then(setIdentifiedSongs).catch(() => { });
        });

        return () => {
            unlistenHistory.then(f => f());
        };
    }, [fetchCustomStations]);

    // --- restore UI state ---
    // --- restore UI state ---
    useEffect(() => {
        const loadSettings = async () => {
            console.log("[SETTINGS] Loading started...");
            let initialSortBy = 'manual';
            let initialSortOrder = 'asc';

            try {
                const s = await invoke('get_settings');
                console.log("[SETTINGS] Rust settings received:", s);
                if (s.sortBy) initialSortBy = s.sortBy;
                if (s.sortOrder) initialSortOrder = s.sortOrder;
            } catch (e) {
                console.error("[SETTINGS] Rust load error:", e);
            }

            try {
                const raw = localStorage.getItem('rx_ui_state');
                if (raw) {
                    const s = JSON.parse(raw);
                    if (s.tab) setTab(s.tab === 'settings' ? 'all' : s.tab);
                    if (s.selectedTag) setSelectedTag(s.selectedTag);
                    if (s.selectedCountry) setSelectedCountry(s.selectedCountry);
                    if (s.selectedCity) setSelectedCity(s.selectedCity);
                    if (s.selectedLanguage) setSelectedLanguage(s.selectedLanguage);
                    if (s.activeUuid) setActiveUuid(s.activeUuid);
                    if (s.activeStation) setActiveStation(s.activeStation);
                }
            } catch (e) { }

            setSortBy(initialSortBy);
            setSortOrder(initialSortOrder);

            setIsSettingsLoaded(true);
            console.log("[SETTINGS] Loading complete.");
        };
        loadSettings();
    }, []);

    // --- close context menu on click or scroll anywhere ---
    useEffect(() => {
        const handleWinEvent = (e) => {
            // Ignore scroll events originating from inside the context menu itself (if it ever becomes scrollable)
            if (e.type === 'scroll' && e.target && e.target.closest && e.target.closest('.ctx-menu-container')) {
                return;
            }
            // Ignore scroll events on the document body itself (sometimes mobile browsers trigger these randomly)
            if (e.type === 'scroll' && e.target === document) {
                return;
            }
            setCtxMenu(null);
            setCtxImageSearchResults(null);
            setIsCtxSearching(false);
        };

        window.addEventListener('click', handleWinEvent);
        // Use capturing phase to catch scroll events on internal div containers (like react-window)
        window.addEventListener('scroll', handleWinEvent, true);

        return () => {
            window.removeEventListener('click', handleWinEvent);
            window.removeEventListener('scroll', handleWinEvent, true);
        };
    }, []);

    const handleQuickImageSearch = async (station) => {
        setIsCtxSearching(true);
        setCtxImageSearchResults(null);
        try {
            const encoded = encodeURIComponent((station.name || '') + ' logo');
            const res = await invoke('search_images_internal', { encodedQuery: encoded });
            setCtxImageSearchResults(res || []);
        } catch (e) {
            notify({ type: 'error', message: t('editStation.searchErrorAlert') + e });
        } finally {
            setIsCtxSearching(false);
        }
    };

    const handleQuickImageAuto = async (station) => {
        setCtxMenu(null);
        try {
            const encoded = encodeURIComponent((station.name || '') + ' logo');
            const res = await invoke('search_images_internal', { encodedQuery: encoded });
            if (res && res.length > 0) {
                // Apply first image directly
                await handleApplyQuickImage(station, res[0], 'auto');
            } else {
                notify({ type: 'info', message: t('ctx.quickImageNotFound') });
            }
        } catch (e) {
            notify({ type: 'error', message: t('editStation.searchErrorAlert') + e });
        }
    };

    const handleApplyQuickImage = async (station, url, index) => {
        try {
            const el = document.getElementById(`quick-img-dl-${index}`);
            if (el) el.classList.remove('hidden');

            const localPath = await invoke('download_custom_favicon', { url });
            const dataToSave = { ...station, favicon: localPath };

            await invoke('save_custom_station', { station: dataToSave });
            fetchCustomStations();

            if (activeStation && activeStation.stationuuid === station.stationuuid) {
                setActiveStation(dataToSave);
            }

            setCtxMenu(null);
            setCtxImageSearchResults(null);
        } catch (err) {
            notify({ type: 'error', message: t('editStation.downloadError') + err });
        } finally {
            const el = document.getElementById(`quick-img-dl-${index}`);
            if (el) el.classList.add('hidden');
        }
    };

    const onCtxMenu = useCallback((e, station) => {
        e.preventDefault();

        const menuWidth = 230;
        let safeX = e.clientX;
        if (safeX + menuWidth > window.innerWidth) {
            safeX = window.innerWidth - menuWidth - 10;
        }

        // Check if cursor is in the bottom half of the screen
        const isBottomHalf = e.clientY > window.innerHeight / 2;

        // Either anchor it to top (downward flow) or bottom (upward flow)
        setCtxMenu({
            x: safeX,
            y: e.clientY,
            isBottomHalf,
            windowHeight: window.innerHeight,
            station
        });

        setCtxImageSearchResults(null);
        setIsCtxSearching(false);
    }, []);

    const handleDeleteCustom = (uuid) => {
        setConfirmDialog({
            isOpen: true,
            title: t('app.deleteRadioTitle'),
            message: t('app.deleteRadioMsg'),
            onConfirm: async () => {
                try {
                    if (uuid === activeUuid) {
                        setActiveUuid(null);
                        setActiveStation(null);
                        setStatus('stopped');
                        setStreamMetadata(null);
                        invoke('stop').catch(() => { });
                    }

                    await invoke('delete_custom_station', { uuid });
                    fetchCustomStations();
                    notify({ type: 'success', title: t('app.success'), message: t('app.radioDeleted') });
                } catch (e) {
                    setConfirmDialog({
                        isOpen: true,
                        title: t('app.error'),
                        message: t('app.failedToDelete') + e,
                        showCancel: false,
                        confirmText: t('app.ok'),
                        variant: "danger"
                    });
                }
            }
        });
    };

    // Persist UI state to LocalStorage (secondary/UI only)
    const saveUIState = useCallback(() => {
        try {
            localStorage.setItem('rx_ui_state', JSON.stringify({
                tab,
                theme,
                selectedTag,
                selectedCountry,
                selectedCity,
                selectedLanguage,
                activeUuid,
                activeStation,
                sortBy,
                sortOrder,
                accentColor,
                mixAccent
            }));
        } catch (e) { }
    }, [tab, theme, selectedTag, selectedCountry, selectedCity, selectedLanguage, activeUuid, activeStation, sortBy, sortOrder, accentColor, mixAccent]);
    useEffect(() => { saveUIState(); }, [saveUIState]);

    // Persist Sorting Settings to Rust
    useEffect(() => {
        if (!isSettingsLoaded) return;
        invoke('save_sort_order', { sortBy, sortOrder })
            .catch(e => console.error("[SETTINGS] Save error:", e));
    }, [isSettingsLoaded, sortBy, sortOrder]);

    const [currentBrowserUrl, setCurrentBrowserUrl] = useState('');

    const getHostname = (url) => {
        try {
            return new URL(url).hostname.replace('www.', '');
        } catch {
            return '';
        }
    };

    // --- listen for events ---
    useEffect(() => {
        const unlisten = [];
        listen('radio-browser-detected', (event) => {
            const { url, name, favicon } = event.payload;
            console.log("Detected radio from browser:", url);
            // Open the edit modal for a new station with detected info
            setEditModalStation({
                name: name || '',
                url: url,
                favicon: favicon || '',
                isNewFromBrowser: true // Hint for the modal
            });
        }).then(u => unlisten.push(u));

        // Rust-side proxy scraping results
        listen('radio-browser-found', (event) => {
            const { streams, name, favicon, page_url } = event.payload;
            console.log("Proxy scrape found streams:", streams, "on", page_url);
            if (streams && streams.length > 0) {
                setEditModalStation({
                    name: name || '',
                    url: streams[0],
                    urlResolved: streams[0],
                    favicon: favicon || '',
                    isNewFromBrowser: true
                });
            }
        }).then(u => unlisten.push(u));

        // Link view events
        listen('link-view-show', (event) => {
            setLinkViewOpen(true);
            if (event.payload) setCurrentBrowserUrl(event.payload);
        }).then(u => unlisten.push(u));
        listen('link-view-hide', () => {
            setLinkViewOpen(false);
            setCurrentBrowserUrl('');
        }).then(u => unlisten.push(u));
        listen('link-view-navigate', (event) => {
            if (event.payload) setCurrentBrowserUrl(event.payload);
        }).then(u => unlisten.push(u));

        return () => {
            unlisten.forEach(f => f());
        };
    }, []);


    // --- init backend ---
    useEffect(() => {
        (async () => {
            try {
                const st = await invoke('get_status');
                console.log('[INIT] get_status response:', JSON.stringify(st));
                if (st.status) setStatus(st.status);
                if (st.volume !== undefined) setVolume(Math.round(st.volume * 100));
                if (st.metadata) setStreamMetadata(st.metadata);

                // If backend is playing/paused, restore active station
                if (st.url && (st.status === 'playing' || st.status === 'paused')) {
                    const stations = await invoke('get_custom_stations').catch(() => []);
                    const match = stations.find(s => s.urlResolved === st.url || s.url === st.url);
                    console.log('[INIT] station match:', match?.name, 'for URL:', st.url);
                    if (match) {
                        setActiveStation(match);
                        setActiveUuid(match.stationuuid);
                    }
                    // Re-trigger enricher to get cover art etc.
                    invoke('re_enrich').catch(() => { });
                }
            } catch (e) { console.error('[INIT] get_status error:', e); }
        })();
    }, []);

    // --- LOCAL FILTERING (no API calls!) ---
    const displayStations = useMemo(() => {
        let list = [...customStations];

        if (tab === 'favorites') {
            list = list.filter(s => s.isFavorite);
        } else if (tab === 'cities') {
            if (selectedCountry) {
                list = list.filter(s => (s.country || 'Unknown').trim().toLowerCase() === selectedCountry.trim().toLowerCase());
            }
            if (selectedCity) {
                list = list.filter(s => (s.state || '').trim().toLowerCase() === selectedCity.trim().toLowerCase());
            }
        }

        if (tab === 'tags' && selectedTag) {
            list = list.filter(s =>
                (s.tags || '').toLowerCase().split(',').map(t => t.trim()).includes(selectedTag.toLowerCase())
            );
        }

        if (tab === 'languages' && selectedLanguage) {
            list = list.filter(s =>
                (s.language || '').toLowerCase().split(',').map(l => l.trim()).includes(selectedLanguage.toLowerCase())
            );
        }

        if (searchQuery) {
            const q = searchQuery.toLowerCase();
            list = list.filter(s => (s.name || '').toLowerCase().includes(q));
        }

        // Sorting
        let effectiveSortBy = sortBy;
        if ((tab === 'tags' || tab === 'cities' || tab === 'languages') && sortBy === 'manual') {
            effectiveSortBy = 'name';
        }

        if (effectiveSortBy === 'manual') {
            list.sort((a, b) => {
                const idxA = tab === 'favorites' ? (a.favIndex || 0) : (a.allIndex || 0);
                const idxB = tab === 'favorites' ? (b.favIndex || 0) : (b.allIndex || 0);
                return idxA - idxB;
            });
        } else if (effectiveSortBy === 'name') {
            list.sort((a, b) => (a.name || '').localeCompare(b.name || ''));
        } else if (effectiveSortBy === 'country') {
            list.sort((a, b) => (a.country || '').localeCompare(b.country || ''));
        }

        if (sortOrder === 'desc' && effectiveSortBy !== 'manual') {
            list.reverse();
        }

        return list;
    }, [customStations, tab, selectedTag, selectedCountry, selectedCity, selectedLanguage, searchQuery, sortBy, sortOrder]);

    const handleReorder = useCallback((reorderedStations) => {
        if (sortBy !== 'manual') return;
        if (tab !== 'favorites' && tab !== 'all') return;

        console.log("[SETTINGS] Reorder event, updating indices...");
        const isFav = tab === 'favorites';

        // We assign new sequential indices to ONLY the stations currently visible and reordered.
        // To maintain their relative position to non-visible stations, we could find the 
        // range of indices they occupied, but sequential starting from the top is simpler 
        // and usually what users expect for manual sort.
        const updates = reorderedStations.map((s, idx) => ({
            ...s,
            [isFav ? 'favIndex' : 'allIndex']: idx + 1
        }));

        invoke('update_station_indices', { updates }).then(() => {
            setCustomStations(prev => prev.map(s => {
                const up = updates.find(u => u.stationuuid === s.stationuuid);
                return up ? { ...s, ...up } : s;
            }));
        }).catch(e => console.error("Index update failed:", e));
    }, [tab, sortBy]);

    // --- play station ---
    const playStation = useCallback(async (station) => {
        setActiveUuid(station.stationuuid);
        setActiveStation(station);
        setStreamMetadata(null);
        try {
            await invoke('play', {
                url: station.urlResolved,
                stationName: station.name || 'Unknown',
                stationImage: station.favicon || null,
            });
        } catch (e) { console.error('Play error:', e); }
    }, []);

    // --- controls ---
    const toggle = useCallback(async () => {
        if (status === 'playing') await invoke('pause').catch(() => { });
        else if (status === 'paused') await invoke('resume').catch(() => { });
        else if (status === 'connecting' || status === 'reconnecting' || status === 'buffering') await invoke('stop').catch(() => { });
        else if (activeStation) playStation(activeStation);
    }, [status, activeStation, playStation]);

    const stop = useCallback(() => invoke('stop').catch(() => { }), []);

    const changeVolume = useCallback(async (v) => {
        setVolume(v);
        await invoke('set_volume', { level: v / 100 }).catch(() => { });
    }, []);

    const playAdjacent = useCallback((dir) => {
        if (!activeUuid || displayStations.length === 0) return;
        const idx = displayStations.findIndex(s => s.stationuuid === activeUuid);
        if (idx === -1) return;
        const next = displayStations[(idx + dir + displayStations.length) % displayStations.length];
        playStation(next);
    }, [activeUuid, displayStations, playStation]);

    // --- favorites ---
    const toggleFavorite = useCallback(async (station) => {
        try {
            await invoke('toggle_favorite', { uuid: station.stationuuid });
            fetchCustomStations();
        } catch (e) {
            console.error('Toggle favorite error:', e);
        }
    }, [fetchCustomStations]);

    // --- events ---
    useEffect(() => {
        const unlistenPromises = [
            listen('playback-status', e => setStatus(e.payload)),
            listen('stream-metadata', e => {
                if (e.payload) {
                    setStreamMetadata(prev => {
                        if (prev?.title !== e.payload.title) {
                            setSongCover(null);
                            setEnrichedData(null);
                            setIdentifyResult(null);
                        }
                        return e.payload;
                    });
                }
            }),
            listen('stream-error', e => console.error('Stream error:', e.payload)),
            listen('media-key', e => {
                const key = e.payload;
                if (key === 'play') invoke('resume').catch(() => { });
                else if (key === 'pause') invoke('pause').catch(() => { });
                else if (key === 'toggle') toggle();
                else if (key === 'stop') invoke('stop').catch(() => { });
                else if (key === 'next') playAdjacent(1);
                else if (key === 'previous') playAdjacent(-1);
            })
        ];

        return () => {
            unlistenPromises.forEach(p => p.then(fn => fn()));
        };
    }, [playAdjacent, toggle]);

    // --- sidebar ---
    const onSelectTab = (t) => {
        if (tab === t && !selectedTag && !selectedCity && !selectedCountry && !selectedLanguage) {
            setForceScrollTop(Date.now());
        }
        setTab(t);
        setSelectedTag(null);
        setSelectedCountry(null);
        setSelectedCity(null);
        setSelectedLanguage(null);
        setSearchQuery('');
    };
    const onSelectTag = (tag) => { setSelectedTag(tag); setTab('tags'); };
    const onSelectCity = (city) => { setSelectedCity(city); setTab('cities'); };
    const onSelectLocation = useCallback((country, state) => {
        setSelectedCountry(country || null);
        setSelectedCity(state || null);
        setTab('cities');
    }, []);
    const onSelectLanguage = useCallback((lang) => {
        setSelectedLanguage(lang || null);
        setTab('languages');
    }, []);

    // Title & Icon
    const { title: listTitle, icon: listIcon } = useMemo(() => {
        if (tab === 'favorites') return { title: t('app.myFavorites'), icon: <Heart size={20} className="fill-current" /> };
        if (tab === 'tags' && selectedTag) return { title: selectedTag, icon: <Tags size={20} /> };
        if (tab === 'cities') {
            if (selectedCity) return { title: `${selectedCity}, ${selectedCountry}`, icon: <MapPin size={20} /> };
            if (selectedCountry) return { title: selectedCountry, icon: <MapPin size={20} /> };
            return { title: t('sidebar.cities'), icon: <MapPin size={20} /> };
        }
        if (tab === 'languages') {
            if (selectedLanguage) return { title: selectedLanguage, icon: <Languages size={20} /> };
            return { title: t('sidebar.languages'), icon: <Languages size={20} /> };
        }
        if (tab === 'identified') return { title: t('app.identifiedSongs'), icon: <Music size={20} /> };
        if (tab === 'settings') return { title: t('app.settings'), icon: <Settings size={20} /> };
        return { title: t('app.allRadios'), icon: <Radio size={20} /> };
    }, [tab, selectedTag, selectedCity, selectedCountry, selectedLanguage, t]);

    const resetAllStates = useCallback(() => {
        setCustomStations([]);
        setTab('settings');
        setSelectedTag(null);
        setSelectedCountry(null);
        setSelectedCity(null);
        setSelectedLanguage(null);
        setSearchQuery('');
        setStatus('stopped');
        setActiveUuid(null);
        setActiveStation(null);
        setStreamMetadata(null);
        setVolume(100); // Reset UI volume, actual volume reset by backend stop
        setSortBy('manual');
        setSortOrder('asc');
        setIdentifiedSongs([]);
        setCtxMenu(null);
        setEditModalStation(null);
        setShowAddChoiceModal(false);
        setShowApiSearchModal(false);
        setConfirmDialog({ isOpen: false, title: '', message: '', onConfirm: null, variant: 'danger', showCancel: true, confirmText: null });
        setConfig({}); // Config is loaded from Rust, so resetting it here might be temporary
        setSongCover(null);
        setEnrichedData(null);
        setFetchedListeners(null);
        setIsIdentifying(false);
        setIdentifyPhase('idle');
        setIsAutoIdentify(false);
        setIdentifyResult(null);
        setShowIdentifyModal(false);
        setAvgColor('rgba(0,0,0,0.6)');
        setViewHistory([]);
        isNavigatingBackRef.current = false;
        setAutoIdentifyCooldownSuccess(60);
        setAutoIdentifyCooldownFail(30);
        lastAutoIdentifyTimeRef.current = 0;
        lastIdentifyStatusRef.current = 'success';
        // isSettingsLoaded, isResizing, currentBrowserUrl are transient/internal, not reset by user action
    }, []);

    const handleResetSetup = useCallback(() => {
        setConfirmDialog({
            isOpen: true,
            title: t('app.resetSetupTitle'),
            message: t('app.resetSetupMsg'),
            onConfirm: async () => {
                try {
                    await invoke('stop').catch(() => { });
                    await invoke('reset_setup');
                    localStorage.clear();
                    notify({
                        type: 'info',
                        title: t('settings.processFinalizing'),
                        message: t('settings.resetCompleteMsg'),
                        duration: 3000
                    });
                    // Relaunch app after a brief delay so the user sees the notification
                    setTimeout(async () => {
                        try {
                            const { relaunch } = await import('@tauri-apps/plugin-process');
                            await relaunch();
                        } catch {
                            window.location.reload();
                        }
                    }, 1500);
                } catch (e) {
                    notify({
                        type: 'error',
                        title: t('app.error'),
                        message: e.toString()
                    });
                }
            }
        });
    }, [t, notify]);

    return (
        <div className="flex flex-col h-screen">
            <TitleBar onOpenSettings={() => setTab('settings')} />
            <div className="flex flex-1 overflow-hidden">
                <div className="shrink-0 relative h-full flex flex-col overflow-x-hidden" style={{ width: leftSidebarWidth }}>
                    <Sidebar
                        tab={tab}
                        onSelectTab={onSelectTab}
                        tags={localTags}
                        locations={locations}
                        languages={localLanguages}
                        selectedTag={selectedTag}
                        onSelectTag={onSelectTag}
                        selectedCountry={selectedCountry}
                        onSelectCountry={setSelectedCountry}
                        selectedCity={selectedCity}
                        onSelectCity={onSelectCity}
                        selectedLanguage={selectedLanguage}
                        onSelectLanguage={onSelectLanguage}
                        config={config}
                        onResetSetup={handleResetSetup}
                        stationCount={customStations.length}
                        identifiedCount={identifiedSongs.length}
                        onSearch={setSearchQuery}
                        mixAccent={mixAccent}
                    />
                    {/* Left sidebar resize handle */}
                    <div
                        className={`absolute right-0 top-0 bottom-0 w-1.5 cursor-col-resize z-[100] hover:bg-accent/40 transition-colors ${isResizing === 'left' ? 'bg-accent/60' : ''}`}
                        onMouseDown={(e) => { e.preventDefault(); setIsResizing('left'); }}
                    />
                </div>
                <main className="flex-1 flex flex-col overflow-x-hidden relative">
                    {tab === 'identified' ? (
                        <IdentifiedSongsList
                            songs={identifiedSongs}
                            onClear={() => setIdentifiedSongs([])}
                            onDeleteSong={(song) => {
                                setIdentifiedSongs(prev => prev.filter(s =>
                                    s.title !== song.title ||
                                    s.artist !== song.artist ||
                                    s.found_at !== song.found_at
                                ));
                            }}
                        />
                    ) : tab === 'settings' ? (
                        <SettingsView
                            onResetSetup={handleResetSetup}
                            autoIdentifyCooldownSuccess={autoIdentifyCooldownSuccess}
                            setAutoIdentifyCooldownSuccess={setAutoIdentifyCooldownSuccess}
                            autoIdentifyCooldownFail={autoIdentifyCooldownFail}
                            setAutoIdentifyCooldownFail={setAutoIdentifyCooldownFail}
                            onImportSuccess={() => {
                                fetchCustomStations();
                            }}
                            theme={theme}
                            setTheme={setTheme}
                            accentColor={accentColor}
                            setAccentColor={setAccentColor}
                            mixAccent={mixAccent}
                            setMixAccent={setMixAccent}
                        />
                    ) : (
                        <StationList
                            title={listTitle}
                            icon={listIcon}
                            stations={displayStations}
                            loading={false}
                            activeUuid={activeUuid}
                            searchQuery={searchQuery}
                            onSearch={setSearchQuery}
                            onPlay={playStation}
                            onToggleFavorite={toggleFavorite}
                            tab={tab}
                            onAddRadio={() => setShowAddChoiceModal(true)}
                            onCtxMenu={onCtxMenu}
                            onGoBack={onGoBack}
                            canGoBack={canGoBack}
                            sortBy={sortBy}
                            setSortBy={setSortBy}
                            sortOrder={sortOrder}
                            setSortOrder={setSortOrder}
                            onReorder={handleReorder}
                            onSelectTag={onSelectTag}
                            onSelectLocation={onSelectLocation}
                            onSelectLanguage={onSelectLanguage}
                            isNavigatingBack={isNavigatingBackRef.current}
                            forceScrollTop={forceScrollTop}
                        />
                    )}
                </main>

                {/* Right Sidebar Container (Browser placeholder or Normal NowPlaying) */}
                {linkViewOpen ? (
                    <div
                        className="shrink-0 flex flex-col bg-bg-surface border-l border-border relative z-10 overflow-x-hidden"
                        style={{ width: sidebarWidth }}
                    >
                        {/* Resizer Handle */}
                        <div
                            className={`absolute left-0 top-0 bottom-0 w-1.5 cursor-col-resize z-[100] hover:bg-accent/40 transition-colors ${isResizing ? 'bg-accent/60' : ''}`}
                            onMouseDown={(e) => {
                                e.preventDefault();
                                setIsResizing('browser');
                            }}
                        />

                        {/* Interaction Guard (Blocks the native webview from stealing mouse during resize) */}
                        {isResizing && (
                            <div className="fixed inset-0 z-[200] cursor-col-resize bg-transparent" />
                        )}

                        {/* Header */}
                        <div className="h-[40px] flex items-center justify-between px-3 border-b border-border bg-bg-surface shrink-0 z-20">
                            <span className="text-[10px] font-black text-accent uppercase tracking-widest truncate max-w-[180px]">
                                {getHostname(currentBrowserUrl) || t('app.browser')}
                            </span>
                            <div className="flex items-center gap-1">
                                <button
                                    onClick={async () => {
                                        await invoke('open_link_view_in_browser');
                                        invoke('close_link_view');
                                    }}
                                    className="w-7 h-7 rounded-lg hover:bg-bg-surface-hover text-text-muted hover:text-text-primary flex items-center justify-center transition-all cursor-pointer"
                                    title={t('app.openInBrowser')}
                                >
                                    <ExternalLink size={14} />
                                </button>
                                <button
                                    onClick={() => invoke('close_link_view')}
                                    className="w-7 h-7 rounded-lg hover:bg-red-500/20 text-text-muted hover:text-red-400 flex items-center justify-center transition-all cursor-pointer"
                                    title={t('app.close')}
                                >
                                    <X size={16} />
                                </button>
                            </div>
                        </div>

                        {/* The native webview covers exactly this container */}
                        <div className="flex-1 bg-transparent" />
                    </div>
                ) : !isPlayerHorizontal && activeStation && (
                    <div className="shrink-0 relative h-full flex flex-col" style={{ width: playerWidth }}>
                        {/* Player resize handle */}
                        <div
                            className={`absolute left-0 top-0 bottom-0 w-1.5 cursor-col-resize z-[100] hover:bg-accent/40 transition-colors ${isResizing === 'player' ? 'bg-accent/60' : ''}`}
                            onMouseDown={(e) => { e.preventDefault(); setIsResizing('player'); }}
                        />
                        <NowPlaying
                            station={activeStation}
                            streamMetadata={streamMetadata}
                            status={status}
                            volume={volume}
                            onToggle={toggle}
                            onVolumeChange={changeVolume}
                            onPrev={() => playAdjacent(-1)}
                            onNext={() => playAdjacent(1)}
                            setIdentifiedSongs={setIdentifiedSongs}
                            miniMode={false}
                            onToggleLayout={() => setIsPlayerHorizontal(true)}
                            linkViewWidth={sidebarWidth}

                            // Lifted props
                            songCover={songCover}
                            setSongCover={setSongCover}
                            enrichedData={enrichedData}
                            setEnrichedData={setEnrichedData}
                            fetchedListeners={fetchedListeners}
                            setFetchedListeners={setFetchedListeners}
                            isIdentifying={isIdentifying}
                            setIsIdentifying={setIsIdentifying}
                            identifyPhase={identifyPhase}
                            setIdentifyPhase={setIdentifyPhase}
                            isAutoIdentify={isAutoIdentify}
                            setIsAutoIdentify={setIsAutoIdentify}
                            handleIdentify={handleIdentify}
                            avgColor={avgColor}
                            showIdentifyModal={showIdentifyModal}
                            setShowIdentifyModal={setShowIdentifyModal}
                            identifyResult={identifyResult}
                            setIdentifyResult={setIdentifyResult}
                            isActuallyLight={isActuallyLight}
                        />
                    </div>
                )}
            </div>

            {/* Horizontal Bottom Bar (when LinkView is open OR when in isPlayerHorizontal) */}
            {(linkViewOpen || isPlayerHorizontal) && activeStation && (
                <div className="w-full shrink-0 z-20 overflow-visible">
                    <NowPlaying
                        station={activeStation}
                        streamMetadata={streamMetadata}
                        status={status}
                        volume={volume}
                        onToggle={toggle}
                        onVolumeChange={changeVolume}
                        onPrev={() => playAdjacent(-1)}
                        onNext={() => playAdjacent(1)}
                        setIdentifiedSongs={setIdentifiedSongs}
                        miniMode={true}
                        onToggleLayout={windowTooSmall ? null : () => setIsPlayerHorizontal(false)}
                        linkViewOpen={linkViewOpen}
                        linkViewWidth={sidebarWidth}

                        // Lifted props
                        songCover={songCover}
                        setSongCover={setSongCover}
                        enrichedData={enrichedData}
                        setEnrichedData={setEnrichedData}
                        fetchedListeners={fetchedListeners}
                        setFetchedListeners={setFetchedListeners}
                        isIdentifying={isIdentifying}
                        setIsIdentifying={setIsIdentifying}
                        identifyPhase={identifyPhase}
                        setIdentifyPhase={setIdentifyPhase}
                        isAutoIdentify={isAutoIdentify}
                        setIsAutoIdentify={setIsAutoIdentify}
                        handleIdentify={handleIdentify}
                        avgColor={avgColor}
                        showIdentifyModal={showIdentifyModal}
                        setShowIdentifyModal={setShowIdentifyModal}
                        identifyResult={identifyResult}
                        setIdentifyResult={setIdentifyResult}
                        isActuallyLight={isActuallyLight}
                    />
                </div>
            )}

            {/* Context Menu */}
            {ctxMenu && (
                <div
                    className="ctx-menu-container absolute bg-bg-surface border border-border rounded-lg shadow-xl py-1 z-[9000] min-w-[160px] flex flex-col pointer-events-auto transition-all"
                    style={{
                        left: ctxMenu.x,
                        ...(ctxMenu.isBottomHalf ? { bottom: ctxMenu.windowHeight - ctxMenu.y } : { top: ctxMenu.y })
                    }}
                    onClick={e => e.stopPropagation()}
                >
                    <button
                        className="w-full text-left px-4 py-2 text-sm hover:bg-accent/10 hover:text-accent transition-colors flex items-center gap-2"
                        onClick={() => { setEditModalStation(ctxMenu.station); setCtxMenu(null); }}
                    >
                        <Edit size={16} /> {t('app.edit')}
                    </button>

                    {!ctxImageSearchResults && !isCtxSearching && (
                        <>
                            <button
                                className="w-full text-left px-4 py-2 text-sm hover:bg-accent/10 hover:text-accent transition-colors flex items-center gap-2"
                                onClick={(e) => { e.stopPropagation(); handleQuickImageSearch(ctxMenu.station); }}
                            >
                                <Image size={16} /> {t('ctx.quickImage')}
                            </button>
                            <button
                                className="w-full text-left px-4 py-2 text-sm hover:bg-accent/10 hover:text-accent transition-colors flex items-center gap-2"
                                onClick={(e) => { e.stopPropagation(); handleQuickImageAuto(ctxMenu.station); }}
                            >
                                <Zap size={16} /> {t('ctx.quickImageFirst')}
                            </button>
                        </>
                    )}

                    {(isCtxSearching || ctxImageSearchResults) && (
                        <div className="px-3 py-2 border-t border-border/50 bg-bg-secondary w-[230px]">
                            {isCtxSearching && <div className="text-[10px] text-text-muted text-center py-2 animate-pulse font-medium">{t('ctx.quickImageSearching')}</div>}
                            {ctxImageSearchResults && ctxImageSearchResults.length === 0 && <div className="text-[10px] text-text-muted text-center py-2">{t('ctx.quickImageNotFound')}</div>}
                            {ctxImageSearchResults && ctxImageSearchResults.length > 0 && (
                                <div className="grid grid-cols-2 gap-2 justify-center p-1 max-h-[235px] overflow-y-auto custom-scrollbar pr-1.5 focus:outline-none" tabIndex="-1">
                                    {ctxImageSearchResults.map((url, i) => (
                                        <div key={i} className="aspect-square bg-black/20 rounded-md cursor-pointer overflow-hidden border border-transparent hover:border-accent hover:scale-[1.05] hover:z-10 hover:shadow-2xl transition-all duration-300 relative group"
                                            onClick={(e) => { e.stopPropagation(); handleApplyQuickImage(ctxMenu.station, url, i); }}
                                        >
                                            <img src={url} alt="" className="w-full h-full object-cover" />
                                            <div id={`quick-img-dl-${i}`} className="hidden absolute inset-0 bg-black/60 flex items-center justify-center text-[10px] text-white font-bold backdrop-blur-sm">{t('ctx.quickImageDownloading')}</div>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    )}

                    {customStations.some(c => c.stationuuid === ctxMenu.station.stationuuid) && (
                        <button
                            className="w-full text-left px-4 py-2 text-sm text-red-500 hover:bg-red-500/10 transition-colors flex items-center gap-2"
                            onClick={() => { handleDeleteCustom(ctxMenu.station.stationuuid); setCtxMenu(null); }}
                        >
                            <Trash2 size={16} /> {t('app.delete')}
                        </button>
                    )}
                </div>
            )}

            {/* Edit Modal */}
            {editModalStation && (
                <EditStationModal
                    station={Object.keys(editModalStation).length === 0 ? null : editModalStation}
                    onClose={() => setEditModalStation(null)}
                    onSave={(savedStation) => {
                        fetchCustomStations();
                        setEditModalStation(null);
                        if (savedStation && activeStation && savedStation.stationuuid === activeStation.stationuuid) {
                            setActiveStation(savedStation);
                            if (status === 'playing') {
                                playStation(savedStation); // Restart the stream with new metadata
                            }
                        }
                    }}
                    linkViewOpen={linkViewOpen}
                    linkViewWidth={sidebarWidth}
                />
            )}

            {/* Add Choice Modal */}
            {showAddChoiceModal && (
                <div
                    className={`fixed inset-0 bg-black/50 z-[9999] flex items-center justify-center p-4 transition-all duration-300`}
                    style={{ paddingRight: linkViewOpen ? sidebarWidth : 0 }}
                >
                    <div className="bg-bg-secondary border border-border rounded-xl p-6 w-full max-w-sm flex flex-col gap-4 shadow-2xl">
                        <div className="flex justify-between items-center bg-bg-surface px-4 py-3 -mx-6 -mt-6 border-b border-border mb-2 rounded-t-xl">
                            <h2 className="font-bold text-lg">{t('app.addRadioTitle')}</h2>
                            <button onClick={() => setShowAddChoiceModal(false)} className="text-text-muted hover:text-text-primary text-xl">
                                &times;
                            </button>
                        </div>
                        <p className="text-sm text-text-muted mb-2 text-center">{t('app.addRadioMsg')}</p>
                        <div className="flex flex-col gap-3">
                            <button className="bg-bg-surface hover:bg-accent/10 text-text-primary hover:text-accent transition-colors border border-border rounded-lg py-4 flex flex-col items-center justify-center gap-2 group" onClick={() => { setShowAddChoiceModal(false); setEditModalStation({}); }}>
                                <PenLine size={28} className="group-hover:scale-110 transition-transform" />
                                <span className="font-bold">{t('app.addManual')}</span>
                                <span className="text-xs text-text-muted group-hover:text-accent/80">{t('app.addManualDesc')}</span>
                            </button>
                            <button className="bg-bg-surface hover:bg-accent/10 text-text-primary hover:text-accent transition-colors border border-border rounded-lg py-4 flex flex-col items-center justify-center gap-2 group relative" onClick={() => {
                                setShowAddChoiceModal(false);
                                setConfirmDialog({
                                    isOpen: true,
                                    title: t('app.thirdPartyApiTitle'),
                                    message: t('app.thirdPartyApiMsg'),
                                    variant: 'warning',
                                    confirmText: t('app.thirdPartyApiConfirm'),
                                    neutralText: t('app.visitWebsite'),
                                    onNeutral: () => {
                                        invoke('open_browser_url', { url: 'https://www.radio-browser.info' });
                                    },
                                    onConfirm: () => {
                                        setShowApiSearchModal(true);
                                    }
                                });
                            }}>
                                <Globe size={28} className="group-hover:scale-110 transition-transform" />
                                <span className="font-bold">{t('app.addFromDB')}</span>
                                <span className="text-xs text-text-muted group-hover:text-accent/80">{t('app.addFromDBDesc')}</span>
                                <span className="text-[9px] font-mono text-text-muted/60 group-hover:text-accent/50 flex items-center gap-1 mt-0.5">
                                    radio-browser.info • <span className="text-accent/70 group-hover:text-accent/90">{t('app.thirdPartyApi')}</span>
                                </span>
                            </button>
                            <button className="bg-bg-surface hover:bg-accent/10 text-text-primary hover:text-accent transition-colors border border-border rounded-lg py-4 flex flex-col items-center justify-center gap-2 group" onClick={() => { setShowAddChoiceModal(false); invoke('open_radio_browser').catch(console.error); }}>
                                <Search size={28} className="group-hover:scale-110 transition-transform" />
                                <span className="font-bold">{t('app.addBrowser')}</span>
                                <span className="text-xs text-text-muted group-hover:text-accent/80">{t('app.addBrowserDesc')}</span>
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* API Search Modal */}
            {showApiSearchModal && (
                <ApiSearchModal
                    onClose={() => setShowApiSearchModal(false)}
                    onPlay={playStation}
                    onSave={(station) => {
                        setShowApiSearchModal(false);
                        fetchCustomStations();
                    }}
                    linkViewOpen={linkViewOpen}
                    linkViewWidth={sidebarWidth}
                />
            )}

            <ConfirmModal
                isOpen={confirmDialog.isOpen}
                onClose={() => setConfirmDialog(prev => ({ ...prev, isOpen: false }))}
                onConfirm={confirmDialog.onConfirm}
                title={confirmDialog.title}
                message={confirmDialog.message}
                variant={confirmDialog.variant || 'danger'}
                showCancel={confirmDialog.showCancel !== false}
                confirmText={confirmDialog.confirmText || t('common.yes')}
                neutralText={confirmDialog.neutralText}
                onNeutral={confirmDialog.onNeutral}
            />

            {showLanguageSetup && (
                <div className="fixed inset-0 top-[38px] bg-bg-primary z-[10000] flex flex-col items-center justify-center p-4" data-tauri-drag-region>
                    <div className="bg-bg-secondary w-full max-w-sm rounded-xl border border-border p-6 shadow-2xl flex flex-col gap-6 text-center pointer-events-auto">
                        <div className="mx-auto bg-accent/10 w-16 h-16 rounded-full flex items-center justify-center text-accent">
                            <Languages size={32} />
                        </div>
                        <div>
                            <h2 className="text-2xl font-bold text-text-primary mb-2">{t('setup.languageWelcomeTitle')}</h2>
                            <p className="text-sm text-text-muted">{t('setup.languageWelcomeSubtitle')}</p>
                        </div>
                        <div className="flex flex-col gap-2 max-h-[40vh] overflow-y-auto pr-1 custom-scrollbar text-left py-2 border-y border-border/50 bg-bg-surface/30">
                            {availableLanguages.map(lang => {
                                const isSelected = i18n.language.substring(0, 2).toLowerCase() === lang.code;
                                return (
                                    <button
                                        key={lang.code}
                                        onClick={() => i18n.changeLanguage(lang.code)}
                                        className={`w-full text-left px-4 py-3 rounded-lg text-sm transition-all focus:outline-none flex items-center justify-between border ${isSelected ? 'bg-accent/15 text-accent font-bold border-accent/50' : 'hover:bg-bg-surface-hover text-text-primary border-transparent'}`}
                                    >
                                        <span>{lang.name}</span>
                                        {isSelected && <Check size={16} className="text-accent" />}
                                    </button>
                                );
                            })}
                        </div>
                        <button
                            onClick={() => {
                                localStorage.setItem('language_setup_done', 'true');
                                setShowLanguageSetup(false);
                            }}
                            className="w-full bg-accent hover:bg-accent-hover text-bg-primary font-bold py-3.5 rounded-xl transition-all active:scale-[0.98] mt-2 shadow-[0_4px_14px_0_rgba(16,185,129,0.39)] hover:shadow-[0_6px_20px_rgba(16,185,129,0.23)]"
                        >
                            {t('common.save')}
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
}
