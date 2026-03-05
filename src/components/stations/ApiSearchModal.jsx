import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { FixedSizeList as List } from 'react-window';
import AutoSizer from 'react-virtualized-auto-sizer';
import { LazyLoadImage } from 'react-lazy-load-image-component';
import { useTranslation } from 'react-i18next';
import { useNotification } from '../../contexts/NotificationProvider';

// Simple icon that shows the API favicon directly
const StationIcon = React.memo(({ favicon, name }) => {
    const [hasError, setHasError] = useState(false);

    useEffect(() => {
        setHasError(false);
    }, [favicon]);

    const fallback = (
        <div className="w-full h-full bg-bg-surface-active flex items-center justify-center text-accent/50 text-xl font-bold tracking-tighter">
            {(name || '?')[0].toUpperCase()}
        </div>
    );
    const hasFavicon = favicon && (favicon.startsWith('http') || favicon.startsWith('file:///')) && !hasError;
    return (
        <div className="w-12 h-12 shrink-0 flex items-center justify-center bg-bg-surface-active rounded-lg overflow-hidden shadow-inner border border-border/50">
            {hasFavicon ? (
                <LazyLoadImage
                    src={favicon}
                    className="w-full h-full object-cover"
                    wrapperClassName="w-12 h-12 shrink-0"
                    onError={() => setHasError(true)}
                    threshold={200}
                    delayTime={300}
                />
            ) : fallback}
        </div>
    );
});

const StationRow = React.memo(({ index, style, data }) => {
    const { results, checkedUuids, playingUuid, handlePreviewPlay, setCheckedUuids, isAddingBulk, t } = data;
    const station = results[index];
    const isChecked = checkedUuids.has(station.stationuuid);
    const isPlaying = playingUuid === station.stationuuid;

    return (
        <div style={style} className="pr-2 pb-1">
            <label className={`flex items-center gap-3 p-3 h-full rounded-lg cursor-pointer transition-all border ${isChecked ? 'bg-accent/10 border-accent/30 shadow-inner' : 'bg-transparent border-transparent hover:bg-bg-surface-hover hover:border-border/30'}`}>
                <input type="checkbox" checked={isChecked} disabled={isAddingBulk}
                    onChange={(e) => {
                        const isCheckedNow = e.target.checked;
                        setCheckedUuids(prevSet => {
                            const newSet = new Set(prevSet);
                            if (isCheckedNow) newSet.add(station.stationuuid);
                            else newSet.delete(station.stationuuid);
                            return newSet;
                        });
                    }}
                    className="accent-accent w-4 h-4 cursor-pointer shrink-0" />
                <StationIcon favicon={station.favicon} name={station.name} />
                <div className="flex-1 min-w-0 px-2 flex flex-col justify-center">
                    <div className="font-bold text-sm truncate flex items-center gap-2">
                        <span className="truncate">{station.name}</span>
                        {station.lastcheckok === 1 && (
                            <span className="shrink-0 w-4 h-4 rounded-full bg-accent text-white flex items-center justify-center shadow-lg shadow-accent/20" title={t('apiSearch.verified')}>
                                <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12" /></svg>
                            </span>
                        )}
                    </div>
                    <div className="text-[11px] text-text-muted truncate mt-0.5">
                        {station.country} {station.state && `• ${station.state}`} {station.tags && `• ${station.tags}`}
                    </div>
                    <div className="text-[10px] text-text-muted/40 truncate flex items-center gap-2 mt-1">
                        {station.codec && <span className="px-1.5 py-0.5 rounded uppercase bg-bg-surface-hover/50 text-text-muted">{station.codec}</span>}
                        {station.bitrate > 0 && <span>• {station.bitrate} kbps</span>}
                        {station.clickcount > 0 && <span className="flex items-center gap-1 border-l border-border/30 pl-2">
                            <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>
                            {station.clickcount.toLocaleString()}
                        </span>}
                    </div>
                </div>
                <button type="button"
                    onClick={(e) => { e.preventDefault(); e.stopPropagation(); handlePreviewPlay(station); }}
                    className={`w-8 h-8 shrink-0 rounded-full flex items-center justify-center transition-all ${isPlaying ? 'bg-accent text-white scale-110 shadow-lg shadow-accent/30' : 'bg-bg-surface hover:bg-accent/20 text-text-muted hover:text-accent'}`}
                    title={isPlaying ? t('apiSearch.stop') : t('apiSearch.listen')}>
                    {isPlaying ? (
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1" /><rect x="14" y="4" width="4" height="16" rx="1" /></svg>
                    ) : (
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3" /></svg>
                    )}
                </button>
            </label>
        </div>
    );
}, (prev, next) => {
    const prevStation = prev.data.results[prev.index];
    const nextStation = next.data.results[next.index];

    // Only re-render if its own station state changed
    const prevChecked = prev.data.checkedUuids.has(prevStation.stationuuid);
    const nextChecked = next.data.checkedUuids.has(nextStation.stationuuid);
    const prevPlaying = prev.data.playingUuid === prevStation.stationuuid;
    const nextPlaying = next.data.playingUuid === nextStation.stationuuid;

    return prev.index === next.index &&
        prev.style === next.style &&
        prevChecked === nextChecked &&
        prevPlaying === nextPlaying &&
        prev.data.isAddingBulk === next.data.isAddingBulk &&
        prev.data.results === next.data.results;
});

export default function ApiSearchModal({ onClose, onSave, onPlay, linkViewOpen, linkViewWidth }) {
    const { t } = useTranslation();
    const { notify } = useNotification();
    const [query, setQuery] = useState('');
    const [selectedCountry, setSelectedCountry] = useState('');
    const [selectedState, setSelectedState] = useState('');
    const [selectedLanguage, setSelectedLanguage] = useState('');
    const [selectedTag, setSelectedTag] = useState('');
    const [countries, setCountries] = useState([]);
    const [states, setStates] = useState([]);
    const [languages, setLanguages] = useState([]);
    const [tags, setTags] = useState([]);
    const [results, setResults] = useState([]);
    const [isSearching, setIsSearching] = useState(false);
    const [hideBroken, setHideBroken] = useState(true);
    const [onlyVerified, setOnlyVerified] = useState(false);

    const [checkedUuids, setCheckedUuids] = useState(new Set());
    const [isAddingBulk, setIsAddingBulk] = useState(false);
    const [bulkProgress, setBulkProgress] = useState(0);
    const [playingUuid, setPlayingUuid] = useState(null);
    const [currentPage, setCurrentPage] = useState(0);
    const [hasNextPage, setHasNextPage] = useState(false);
    const isSearchingRef = useRef(false);
    const hasNextPageRef = useRef(false);
    const resultsRef = useRef([]); // To keep track of length without dependency
    const pagesCacheRef = useRef({}); // { 0: [stations], 1: [...] }
    const isInitialOpen = useRef(true);
    const listRef = useRef(null);
    const isAbortedRef = useRef(false);
    const [scrollOffset, setScrollOffset] = useState(0);
    const [containerHeight, setContainerHeight] = useState(0);
    const pageLimit = 100;
    const handlePreviewPlay = useCallback(async (station) => {
        if (playingUuid === station.stationuuid) {
            try { await invoke('preview_stop'); } catch (e) { }
            setPlayingUuid(null);
            return;
        }

        const streamUrl = station.urlResolved || station.url_resolved || station.url;
        if (!streamUrl) {
            notify({ type: 'error', message: "Preview failed: No stream URL found for this station." });
            return;
        }

        try {
            setPlayingUuid(station.stationuuid);
            await invoke('preview_play', { url: String(streamUrl) });
        } catch (err) {
            console.error("Preview failed:", err);
            const errorMsg = typeof err === 'string' ? err : (err.message || err.name || "Unknown error");
            notify({ type: 'error', message: "Preview failed: " + errorMsg });
            setPlayingUuid(null);
        }
    }, [playingUuid, notify]);

    // Cleanup audio and abort on unmount
    useEffect(() => {
        return () => {
            isAbortedRef.current = true;
            invoke('preview_stop').catch(() => { });
        };
    }, []);

    // Also stop preview if user closes modal
    useEffect(() => {
        return () => {
            invoke('preview_stop').catch(() => { });
        };
    }, [onClose]);

    // Load filter options
    useEffect(() => {
        invoke('get_countries').then(res => {
            res.sort((a, b) => a.name.localeCompare(b.name));
            setCountries(res);
        }).catch(console.error);
        invoke('get_languages').then(res => {
            res.sort((a, b) => (b.stationcount || 0) - (a.stationcount || 0));
            setLanguages(res.filter(l => l.name && (l.stationcount || 0) > 5));
        }).catch(console.error);
        invoke('get_tags', { limit: 200 }).then(res => {
            res.sort((a, b) => (b.stationcount || 0) - (a.stationcount || 0));
            setTags(res.filter(t => t.name && (t.stationcount || 0) > 10));
        }).catch(console.error);
    }, []);

    // Load states when country changes
    useEffect(() => {
        setSelectedState('');
        setStates([]);
        if (selectedCountry) {
            invoke('get_states', { country: selectedCountry }).then(res => {
                res.sort((a, b) => a.name.localeCompare(b.name));
                setStates(res);
            }).catch(console.error);
        }
    }, [selectedCountry]);

    // Helper to update filter dropdowns from a chunk of stations
    const updateFiltersFromStations = useCallback((stations) => {
        const langSet = new Map();
        const tagSet = new Map();
        const stateSet = new Map();

        stations.forEach(s => {
            if (!selectedLanguage && s.language) {
                s.language.split(',').forEach(l => {
                    const v = l.trim().toLowerCase();
                    if (v) langSet.set(v, (langSet.get(v) || 0) + 1);
                });
            }
            if (!selectedTag && s.tags) {
                s.tags.split(',').forEach(t => {
                    const v = t.trim().toLowerCase();
                    if (v) tagSet.set(v, (tagSet.get(v) || 0) + 1);
                });
            }
            if (!selectedState && s.state) {
                const v = s.state.trim();
                if (v) stateSet.set(v, (stateSet.get(v) || 0) + 1);
            }
        });

        const toList = (map) => Array.from(map.entries())
            .map(([name, count]) => ({ name, stationcount: count }))
            .sort((a, b) => b.stationcount - a.stationcount);

        if (!selectedLanguage && langSet.size > 0) setLanguages(toList(langSet));
        if (!selectedTag && tagSet.size > 0) setTags(toList(tagSet));
        if (!selectedState && stateSet.size > 0) setStates(toList(stateSet));
    }, [selectedLanguage, selectedTag, selectedState]);

    // Search function - now handles INFINITE SCROLL
    const handleSearch = useCallback(async (pageIndex = 0, isAuto = false, isLoadAll = false) => {
        // Prevent double loading same page or loading next when there aren't any
        if (isSearchingRef.current || (pageIndex > 0 && !hasNextPageRef.current && !isLoadAll)) return;

        isSearchingRef.current = true;
        isAbortedRef.current = false;
        setIsSearching(true);

        try {
            if (isLoadAll) {
                let offset = resultsRef.current.length;
                while (true) {
                    if (isAbortedRef.current) break;
                    const res = await invoke('search_stations', {
                        name: query.trim() || null,
                        country: selectedCountry || null,
                        state: selectedState || null,
                        language: selectedLanguage || null,
                        tag: selectedTag || null,
                        limit: pageLimit,
                        offset: offset,
                        hide_broken: hideBroken,
                        only_verified: onlyVerified
                    });

                    const chunk = res || [];
                    if (chunk.length === 0) break;

                    // Incrementally update list
                    setResults(prev => {
                        const combined = [...prev, ...chunk];
                        resultsRef.current = combined;
                        return combined;
                    });

                    // Update dropdowns incrementally too
                    updateFiltersFromStations(chunk);

                    offset += pageLimit;
                    if (chunk.length < pageLimit) {
                        hasNextPageRef.current = false;
                        setHasNextPage(false);
                        break;
                    }
                }
            } else {
                const res = await invoke('search_stations', {
                    name: query.trim() || null,
                    country: selectedCountry || null,
                    state: selectedState || null,
                    language: selectedLanguage || null,
                    tag: selectedTag || null,
                    limit: pageLimit,
                    offset: pageIndex * pageLimit,
                    hide_broken: hideBroken,
                    only_verified: onlyVerified
                });

                const fetched = res || [];
                if (pageIndex === 0) {
                    setResults(fetched);
                    resultsRef.current = fetched;
                } else {
                    setResults(prev => {
                        const combined = [...prev, ...fetched];
                        resultsRef.current = combined;
                        return combined;
                    });
                }
                const hasNext = fetched.length === pageLimit;
                hasNextPageRef.current = hasNext;
                setHasNextPage(hasNext);
                setCurrentPage(pageIndex);

                if (fetched.length > 0) {
                    updateFiltersFromStations(fetched);
                }
            }
        } catch (err) {
            console.error("Search failed:", err);
            if (!isAuto) notify({ type: 'error', message: t('apiSearch.searchError') + err });
        } finally {
            isSearchingRef.current = false;
            setIsSearching(false);
        }
    }, [query, selectedCountry, selectedState, selectedLanguage, selectedTag, hideBroken, onlyVerified, notify, t, updateFiltersFromStations]);

    // Clear results when filters change (Reset to page 0)
    useEffect(() => {
        setResults([]);
        resultsRef.current = [];
        setCurrentPage(0);
        setHasNextPage(false);
        hasNextPageRef.current = false;
    }, [query, selectedCountry, selectedState, selectedLanguage, selectedTag, hideBroken, onlyVerified]);

    const searchTimer = useRef(null);
    useEffect(() => {
        if (isInitialOpen.current) {
            isInitialOpen.current = false;
            return;
        }
        clearTimeout(searchTimer.current);
        searchTimer.current = setTimeout(() => {
            handleSearch(0, true);
        }, 500); // 500ms debounce for auto-search
        return () => clearTimeout(searchTimer.current);
    }, [query, selectedCountry, selectedState, selectedLanguage, selectedTag, hideBroken, onlyVerified, handleSearch]);

    // Initial load for global categories
    useEffect(() => {
        invoke('get_countries').then(res => {
            res.sort((a, b) => a.name.localeCompare(b.name));
            setCountries(res);
        }).catch(console.error);
    }, []);

    const handleBulkAdd = async () => {
        const toAdd = results.filter(r => checkedUuids.has(r.stationuuid));
        if (toAdd.length === 0) return;

        setIsAddingBulk(true);
        isAbortedRef.current = false;
        setBulkProgress(0);

        let unlisten = null;
        try {
            // 1. Listen for individual progress updates from Rust
            unlisten = await listen('favicon-progress', (event) => {
                const { done } = event.payload;
                setBulkProgress(done);
            });

            // 2. Prepare entries for batch favicon download
            const faviconEntries = toAdd.map(s => ({
                uuid: s.stationuuid,
                url: s.favicon || ''
            }));

            // 3. Batch download in Rust (Parallel, high performance)
            const faviconMap = await invoke('batch_cache_favicons', { entries: faviconEntries });

            if (isAbortedRef.current) return;

            // 4. Map results and prepare for final save
            const stationsToSave = toAdd.map(s => {
                const cleanStr = (val) => val === null || val === undefined ? '' : String(val);
                return {
                    stationuuid: crypto.randomUUID(),
                    name: cleanStr(s.name),
                    urlResolved: cleanStr(s.urlResolved || s.url_resolved || s.url),
                    favicon: cleanStr(faviconMap[s.stationuuid] || s.favicon),
                    country: cleanStr(s.country),
                    state: cleanStr(s.state),
                    language: cleanStr(s.language),
                    tags: cleanStr(s.tags),
                    codec: cleanStr(s.codec),
                    bitrate: Number(s.bitrate) || 0,
                    isFavorite: false,
                    favIndex: 0,
                    allIndex: 0,
                    lastcheckok: Number(s.lastcheckok) || 0,
                    clickcount: Number(s.clickcount) || 0
                };
            });

            // 5. Final atomic save
            const count = await invoke('save_custom_stations_batch', { stations: stationsToSave });

            if (count > 0) {
                notify({ type: 'success', message: t('apiSearch.addedSuccess', { count }) });
                onSave();
            }
        } catch (err) {
            console.error("Bulk add failed:", err);
            notify({ type: 'error', message: t('apiSearch.addedFail') + " " + (err.message || err.toString() || err) });
        } finally {
            if (unlisten) unlisten();
            setIsAddingBulk(false);
            setBulkProgress(0);
        }
    };

    const itemData = React.useMemo(() => ({
        results,
        checkedUuids,
        playingUuid,
        handlePreviewPlay,
        setCheckedUuids,
        isAddingBulk,
        t
    }), [results, checkedUuids, playingUuid, handlePreviewPlay, setCheckedUuids, isAddingBulk, t]);

    return (
        <div
            className={`fixed inset-0 bg-black/50 z-[9999] flex items-center justify-center p-4 transition-all duration-300`}
            style={{ paddingRight: linkViewOpen ? linkViewWidth : 0 }}
            onKeyDown={(e) => {
                if (e.key === 'End') {
                    e.preventDefault();
                    listRef.current?.scrollToItem(results.length - 1, "end");
                } else if (e.key === 'Home') {
                    e.preventDefault();
                    listRef.current?.scrollToItem(0, "start");
                } else if (e.key === 'PageDown') {
                    e.preventDefault();
                    listRef.current?.scrollTo(scrollOffset + containerHeight);
                } else if (e.key === 'PageUp') {
                    e.preventDefault();
                    listRef.current?.scrollTo(scrollOffset - containerHeight);
                }
            }}
        >
            <div className="ctx-menu-container bg-bg-secondary rounded-xl w-full max-w-3xl flex flex-col shadow-2xl overflow-hidden border border-border h-[90vh] animate-in fade-in zoom-in duration-200">
                <div className="px-5 py-4 border-b border-border flex justify-between items-center bg-bg-surface shrink-0">
                    <h2 className="text-lg font-bold">{t('apiSearch.title')}</h2>
                    <button onClick={onClose} className="text-text-muted hover:text-text-primary text-xl transition-colors">&times;</button>
                </div>

                <div className="p-5 flex flex-col overflow-hidden flex-1">
                    <form onSubmit={e => { e.preventDefault(); handleSearch(); }} className="flex flex-col gap-2 mb-4 shrink-0">
                        <div className="flex gap-2">
                            <input type="text" value={query} onChange={e => setQuery(e.target.value)}
                                placeholder={t('apiSearch.searchPlaceholder')}
                                className="flex-1 bg-bg-surface border border-border rounded-lg px-3 py-2 text-sm outline-none focus:border-accent" />
                            <button type="button" onClick={() => handleSearch()} disabled={isSearching}
                                className="bg-accent hover:bg-accent-hover disabled:opacity-50 text-white px-5 py-2 rounded-lg font-bold text-sm transition-colors cursor-pointer shrink-0">
                                {isSearching ? t('apiSearch.searching') : t('apiSearch.searchButton')}
                            </button>
                        </div>
                        <div className="grid grid-cols-4 gap-1.5">
                            <select value={selectedCountry} onChange={e => setSelectedCountry(e.target.value)}
                                className="bg-bg-surface border border-border rounded px-1.5 py-1.5 text-[11px] outline-none focus:border-accent truncate">
                                <option value="">{t('apiSearch.country')}</option>
                                {countries.map(c => <option key={c.name} value={c.name}>{c.name}</option>)}
                            </select>
                            <select value={selectedState} onChange={e => setSelectedState(e.target.value)}
                                disabled={!selectedCountry || states.length === 0}
                                className="bg-bg-surface border border-border rounded px-1.5 py-1.5 text-[11px] outline-none focus:border-accent truncate disabled:opacity-40">
                                <option value="">{t('apiSearch.state')}</option>
                                {states.map(s => <option key={s.name} value={s.name}>{s.name}</option>)}
                            </select>
                            <select value={selectedLanguage} onChange={e => setSelectedLanguage(e.target.value)}
                                className="bg-bg-surface border border-border rounded px-1.5 py-1.5 text-[11px] outline-none focus:border-accent truncate">
                                <option value="">{t('apiSearch.language')}</option>
                                {languages.map(l => <option key={l.name} value={l.name}>{l.name}</option>)}
                            </select>
                            <select value={selectedTag} onChange={e => setSelectedTag(e.target.value)}
                                className="bg-bg-surface border border-border rounded px-1.5 py-1.5 text-[11px] outline-none focus:border-accent truncate">
                                <option value="">{t('apiSearch.tag')}</option>
                                {tags.map(t => <option key={t.name} value={t.name}>{t.name}</option>)}
                            </select>
                        </div>
                        <div className="flex gap-4 mt-1 px-1">
                            <label className="flex items-center gap-2 cursor-pointer group">
                                <input type="checkbox" checked={onlyVerified} onChange={e => setOnlyVerified(e.target.checked)}
                                    className="accent-accent w-3.5 h-3.5 cursor-pointer" />
                                <span className="text-[11px] text-text-muted group-hover:text-text-primary transition-colors">{t('apiSearch.onlyVerified')}</span>
                            </label>
                            <label className="flex items-center gap-2 cursor-pointer group">
                                <input type="checkbox" checked={hideBroken} onChange={e => setHideBroken(e.target.checked)}
                                    className="accent-accent w-3.5 h-3.5 cursor-pointer" />
                                <span className="text-[11px] text-text-muted group-hover:text-text-primary transition-colors">{t('apiSearch.hideBroken')}</span>
                            </label>
                        </div>
                    </form>

                    {results.length > 0 && (
                        <div className="flex justify-between items-center mb-3 px-1 shrink-0">
                            <label className="flex items-center gap-2 text-sm font-semibold cursor-pointer text-text-muted hover:text-white transition-colors">
                                <input type="checkbox" checked={checkedUuids.size === results.length} disabled={isAddingBulk}
                                    onChange={(e) => {
                                        if (e.target.checked) setCheckedUuids(new Set(results.map(r => r.stationuuid)));
                                        else setCheckedUuids(new Set());
                                    }}
                                    className="accent-accent w-4 h-4 cursor-pointer" />
                                <span>{t('apiSearch.selectAll')}</span>
                            </label>
                            <div className="text-xs font-bold text-accent">{t('apiSearch.selected', { count: checkedUuids.size })}</div>
                        </div>
                    )}

                    <div className="flex-1 rounded-lg bg-bg-surface overflow-hidden relative p-1">
                        {isSearching && results.length === 0 && (
                            <div className="flex flex-col items-center justify-center h-full gap-3 transition-all">
                                <span className="w-8 h-8 rounded-full border-3 border-accent/20 border-t-accent animate-spin inline-block" style={{ borderWidth: '3px' }} />
                                <div className="text-sm text-text-muted font-medium animate-pulse">{t('apiSearch.searching')}</div>
                                <button
                                    onClick={(e) => { e.stopPropagation(); isAbortedRef.current = true; }}
                                    className="px-4 py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-500 text-xs font-bold rounded-full transition-colors mt-2"
                                >
                                    {t('common.stop')}
                                </button>
                            </div>
                        )}
                        {results.length === 0 && !isSearching && (
                            <div className="flex items-center justify-center h-full text-text-muted text-sm">{t('apiSearch.startSearching')}</div>
                        )}
                        {results.length > 0 && (
                            <AutoSizer>
                                {({ height, width }) => {
                                    if (containerHeight !== height) {
                                        setTimeout(() => setContainerHeight(height), 0);
                                    }
                                    return (
                                        <List
                                            ref={listRef}
                                            height={height}
                                            itemCount={results.length}
                                            itemSize={72}
                                            width={width}
                                            itemData={itemData}
                                            className="custom-scrollbar"
                                            onScroll={({ scrollOffset: offset }) => setScrollOffset(offset)}
                                            onItemsRendered={({ visibleStopIndex }) => {
                                                if (!isSearchingRef.current && hasNextPageRef.current && visibleStopIndex >= resultsRef.current.length - 20) {
                                                    handleSearch(currentPage + 1);
                                                }
                                            }}
                                        >
                                            {StationRow}
                                        </List>
                                    );
                                }}
                            </AutoSizer>
                        )}
                        <div className="absolute bottom-8 right-16 flex flex-col gap-3 z-30 pointer-events-none">
                            <button
                                onClick={() => listRef.current?.scrollToItem(0, "start")}
                                className={`w-10 h-10 rounded-full bg-accent text-white flex items-center justify-center shadow-2xl backdrop-blur-md hover:bg-accent-hover hover:scale-110 active:scale-95 transition-all duration-300 pointer-events-auto ${scrollOffset > 300 ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-10 focus-within:hidden'}`}
                                title={t('apiSearch.scrollToTop')}
                            >
                                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="18 15 12 9 6 15" /></svg>
                            </button>
                            <button
                                onClick={() => listRef.current?.scrollToItem(results.length - 1, "end")}
                                className={`w-10 h-10 rounded-full bg-accent text-white flex items-center justify-center shadow-2xl backdrop-blur-md hover:bg-accent-hover hover:scale-110 active:scale-95 transition-all duration-300 pointer-events-auto ${(hasNextPage || (scrollOffset + containerHeight < (results.length * 72) - 100)) && results.length > 50 ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-10 focus-within:hidden'}`}
                                title={t('apiSearch.scrollToBottom')}
                            >
                                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
                            </button>
                        </div>

                        {isSearching && results.length > 0 && (
                            <div className="absolute bottom-4 left-1/2 -translate-x-1/2 z-20 px-4 py-2 bg-bg-surface/80 border border-border/50 backdrop-blur-xl rounded-full shadow-2xl flex items-center gap-3 animate-in fade-in slide-in-from-bottom-2 duration-300">
                                <span className="w-3.5 h-3.5 rounded-full border-2 border-accent/20 border-t-accent animate-spin inline-block" style={{ borderWidth: '2px' }} />
                                <span className="text-[11px] font-bold text-text-primary tracking-tight">{t('apiSearch.loadingMore')}</span>
                                <button
                                    onClick={(e) => { e.stopPropagation(); isAbortedRef.current = true; }}
                                    className="ml-2 w-5 h-5 flex items-center justify-center rounded-full bg-red-500/20 hover:bg-red-500 text-red-500 hover:text-white transition-all transition-colors group"
                                    title={t('common.stop')}
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2" /></svg>
                                </button>
                            </div>
                        )}
                    </div>

                    <div className="mt-4 pt-4 border-t border-border shrink-0">
                        {isAddingBulk && (
                            <div className="mb-4">
                                <div className="flex justify-between text-xs mb-1.5 font-bold tracking-tight">
                                    <span className="text-text-muted">{t('apiSearch.addingRadios')}</span>
                                    <div className="flex items-center gap-2">
                                        <span className="text-accent">{bulkProgress} / {checkedUuids.size}</span>
                                        <button
                                            onClick={() => isAbortedRef.current = true}
                                            className="ml-1 text-[10px] text-red-500 hover:text-red-400 font-bold uppercase transition-colors"
                                        >
                                            {t('common.stop')}
                                        </button>
                                    </div>
                                </div>
                                <div className="w-full h-1.5 bg-bg-surface rounded-full overflow-hidden">
                                    <div className="h-full bg-accent transition-all duration-300 ease-out" style={{ width: `${(bulkProgress / checkedUuids.size) * 100}%` }}></div>
                                </div>
                            </div>
                        )}
                        <div className="flex justify-between items-center gap-4">
                            <div className="flex flex-col gap-0.5">
                                <div className="text-[11px] font-bold text-accent uppercase tracking-wider">
                                    {results.length > 0 ? t('apiSearch.totalFound', { count: results.length }) : ""}
                                </div>
                                <div className="text-[10px] text-text-muted italic flex items-center gap-2">
                                    {hasNextPage && (
                                        <>
                                            <span>{t('apiSearch.moreAvailable')}</span>
                                            <button
                                                onClick={() => handleSearch(0, false, true)}
                                                className="text-accent hover:underline font-bold uppercase"
                                            >
                                                [{t('apiSearch.loadAll')}]
                                            </button>
                                        </>
                                    )}
                                </div>
                            </div>
                            <div className="flex gap-3">
                                <button onClick={onClose} className="px-5 py-2 text-sm font-semibold hover:text-white transition-colors">{t('apiSearch.cancel')}</button>
                                <button onClick={handleBulkAdd} disabled={checkedUuids.size === 0 || isAddingBulk}
                                    className="bg-accent hover:bg-accent-hover disabled:bg-bg-surface disabled:text-text-muted disabled:opacity-50 text-white px-8 py-2 rounded-lg font-bold text-sm transition-all shadow-lg active:scale-95">
                                    {isAddingBulk ? t('apiSearch.adding', { done: bulkProgress, total: checkedUuids.size }) : t('apiSearch.addToLibrary')}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
