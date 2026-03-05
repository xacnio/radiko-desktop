import { toAssetUrl } from '../../utils';
import { invoke } from '@tauri-apps/api/core';
import { Heart, Plus, SortAsc, SortDesc, ArrowUpDown, ChevronDown, Check, ArrowLeft, Radio, Settings2, MapPin, Languages, Tags, AlignJustify, LayoutGrid } from 'lucide-react';
import { FixedSizeList as List, areEqual } from 'react-window';
import AutoSizer from 'react-virtualized-auto-sizer';
import { LazyLoadImage } from 'react-lazy-load-image-component';
import { memo, useMemo, useState, useEffect, useRef, forwardRef } from 'react';

const InnerList = forwardRef(({ style, ...rest }, ref) => (
    <div
        ref={ref}
        style={{
            ...style,
            height: `${parseFloat(style.height || 0) + 70}px` // +70px padding at bottom for the button
        }}
        {...rest}
    />
));
import { useTranslation } from 'react-i18next';
import {
    DndContext,
    closestCenter,
    PointerSensor,
    useSensor,
    useSensors,
    DragOverlay,
} from '@dnd-kit/core';
import {
    restrictToVerticalAxis,
    restrictToWindowEdges,
} from '@dnd-kit/modifiers';
import {
    arrayMove,
    SortableContext,
    verticalListSortingStrategy,
    rectSortingStrategy,
    useSortable,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';

const StationItem = memo(function StationItem({ station, isActive, isFav, onPlay, onToggleFav, onCtxMenu, onSelectTag, onSelectLocation, onSelectLanguage, style, dragHandleProps, isDragging, containerWidth = 800, viewSettings = {} }) {
    const [hasError, setHasError] = useState(false);
    const [showTooltip, setShowTooltip] = useState(false);

    const allItems = useMemo(() => {
        const items = [];
        if (!viewSettings.hideLocation && station.country) items.push({ type: 'location', label: `${station.country}${station.state ? `, ${station.state}` : ''}`, raw: station.country, state: station.state });
        if (!viewSettings.hideLanguage && station.language) items.push({ type: 'language', label: station.language.split(',')[0].trim() });
        if (!viewSettings.hideTags) {
            const tags = (station.tags || '').split(',').map(t => t.trim()).filter(Boolean);
            tags.forEach(t => items.push({ type: 'tag', label: t }));
        }
        return items;
    }, [station.country, station.state, station.language, station.tags, viewSettings]);

    const { visibleTags, hiddenTags } = useMemo(() => {
        const infoWidth = Math.max(150, containerWidth - 140);
        let currentWidth = 0;
        const visible = [];
        const hidden = [];
        const plusBadgeWidth = 36;
        const MIN_DISPLAY_WIDTH = 45;

        for (let i = 0; i < allItems.length; i++) {
            const item = allItems[i];
            let rawWidth = item.label.length * 7.0 + 24;

            let baseMaxWidth = 160;
            if (item.type === 'location') baseMaxWidth = 200;
            else if (item.type === 'language') baseMaxWidth = 150;

            let itemWidth = Math.min(rawWidth, baseMaxWidth);
            const isLast = i === allItems.length - 1;
            const badgeRoom = isLast ? 0 : plusBadgeWidth;

            if (currentWidth + itemWidth + badgeRoom > infoWidth) {
                const spaceLeft = Math.max(0, infoWidth - currentWidth - badgeRoom);

                // For the first item, we must squeeze it no matter what,
                // for subsequent items, we squeeze it if we have at least MIN_DISPLAY_WIDTH
                if (spaceLeft >= MIN_DISPLAY_WIDTH || i === 0) {
                    visible.push({ ...item, computedMaxWidth: Math.max(MIN_DISPLAY_WIDTH, spaceLeft) });
                    if (!isLast) hidden.push(...allItems.slice(i + 1));
                    break;
                } else {
                    hidden.push(...allItems.slice(i));
                    break;
                }
            } else {
                visible.push({ ...item, computedMaxWidth: baseMaxWidth });
                currentWidth += itemWidth;
            }
        }
        return { visibleTags: visible, hiddenTags: hidden };
    }, [allItems, containerWidth]);

    // Ignore HTTP/HTTPS links
    let faviconSrc = station.favicon;
    if (faviconSrc && (faviconSrc.startsWith('http://') || faviconSrc.startsWith('https://'))) {
        faviconSrc = '';
    } else {
        faviconSrc = faviconSrc ? toAssetUrl(faviconSrc) : '';
    }

    useEffect(() => {
        setHasError(false);
    }, [station.favicon]);

    return (
        <div
            style={style}
            className={`pr-1 pb-1 ${isDragging ? 'z-50 opacity-50 relative' : ''}`}
            {...dragHandleProps}
        >
            <div
                onClick={() => !isDragging && onPlay(station)}
                onContextMenu={e => onCtxMenu && onCtxMenu(e, station)}
                className={`group h-full flex items-center gap-3 px-3 rounded-lg cursor-pointer transition-all
                    ${isActive
                        ? 'bg-accent/10 border-l-2 border-accent'
                        : 'hover:bg-bg-surface-hover'
                    } ${isDragging ? 'scale-[1.02] shadow-xl bg-bg-surface' : ''}`}
            >
                {/* Favicon */}
                {faviconSrc && !hasError ? (
                    <LazyLoadImage
                        src={faviconSrc}
                        alt=""
                        className="w-11 h-11 rounded-lg object-cover shrink-0 bg-bg-surface-active"
                        onError={() => {
                            setHasError(true);
                            if (station.favicon && station.favicon.startsWith('file://')) {
                                invoke('clear_missing_favicon', { uuid: station.stationuuid }).catch(() => { });
                            }
                        }}
                        wrapperClassName="w-11 h-11 shrink-0"
                        threshold={200}
                        delayTime={300}
                    />
                ) : (
                    <div className="w-11 h-11 rounded-lg bg-bg-surface-active flex items-center justify-center shrink-0 border border-border/50 opacity-80">
                        <img src="/icon.svg" className="w-[50%] h-[50%] opacity-50 grayscale" alt="Radiko" />
                    </div>
                )}

                {/* Info */}
                <div className="flex-1 min-w-0">
                    <div className="text-sm font-semibold text-text-primary truncate">{station.name}</div>

                    {/* Meta & Tags Row: All details as badges */}
                    <div className="flex items-center gap-1.5 mt-1.5 overflow-visible">
                        {visibleTags.map((item, idx) => (
                            <span key={`vis-${idx}`}
                                className="tag-badge tag-badge-clickable h-[20px] leading-[16px] shrink-0 truncate max-w-full"
                                style={{ maxWidth: item.computedMaxWidth ? `${item.computedMaxWidth}px` : undefined }}
                                onClick={e => {
                                    e.stopPropagation();
                                    if (item.type === 'location' && onSelectLocation) onSelectLocation(item.raw, item.state);
                                    if (item.type === 'language' && onSelectLanguage) onSelectLanguage(item.label);
                                    if (item.type === 'tag' && onSelectTag) onSelectTag(item.label);
                                }}
                                title={item.label}
                            >{item.label}</span>
                        ))}
                        {hiddenTags.length > 0 && (
                            <div className="relative flex items-center" onMouseLeave={() => setShowTooltip(false)}>
                                <span
                                    className="tag-badge opacity-70 cursor-pointer shrink-0"
                                    onClick={e => {
                                        e.stopPropagation();
                                        setShowTooltip(!showTooltip);
                                    }}
                                >
                                    +{hiddenTags.length}
                                </span>
                                {showTooltip && (
                                    <div className="absolute bottom-full left-1/2 -translate-x-1/2 pb-2 z-[100]">
                                        <div
                                            className="bg-bg-secondary border border-border rounded-lg p-2.5 flex flex-wrap gap-1.5 w-max max-w-[220px] shadow-2xl"
                                            onClick={e => e.stopPropagation()}
                                        >
                                            <div className="w-full text-[9px] text-text-muted mb-0.5 font-bold uppercase tracking-wider">Additional Tags</div>
                                            {hiddenTags.map((item, idx) => {
                                                if (item.type === 'location') {
                                                    return <span key={`hid-${idx}`} className="tag-badge tag-badge-clickable shrink-0 max-w-[180px] truncate"
                                                        onClick={e => { e.stopPropagation(); setShowTooltip(false); onSelectLocation && onSelectLocation(item.raw, item.state); }}
                                                        title={item.label}>{item.label}</span>
                                                } else if (item.type === 'language') {
                                                    return <span key={`hid-${idx}`} className="tag-badge tag-badge-clickable shrink-0 max-w-[180px] truncate"
                                                        onClick={e => { e.stopPropagation(); setShowTooltip(false); onSelectLanguage && onSelectLanguage(item.label); }}
                                                        title={item.label}>{item.label}</span>
                                                } else {
                                                    return <span key={`hid-${idx}`} className="tag-badge tag-badge-clickable shrink-0 max-w-[180px] truncate"
                                                        onClick={e => { e.stopPropagation(); setShowTooltip(false); onSelectTag && onSelectTag(item.label); }}
                                                        title={item.label}>{item.label}</span>
                                                }
                                            })}
                                        </div>
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                </div>

                <button
                    onClick={e => { e.stopPropagation(); onToggleFav(station); }}
                    className={`shrink-0 transition-transform hover:scale-110 cursor-pointer p-1 flex items-center justify-center
                    ${isFav ? 'text-accent' : 'text-text-muted'}`}
                >
                    {isFav ? <Heart fill="currentColor" size={18} /> : <Heart size={18} />}
                </button>
            </div>
        </div>
    );
});


const StationGridItem = memo(function StationGridItem({ station, isActive, isFav, onPlay, onToggleFav, onCtxMenu, onSelectTag, onSelectLocation, onSelectLanguage, viewSettings = {} }) {
    const [hasError, setHasError] = useState(false);
    const [showTooltip, setShowTooltip] = useState(false);

    const displayTags = useMemo(() => {
        const items = [];
        if (!viewSettings.hideLocation && station.country) items.push({ type: 'location', label: `${station.country}${station.state ? `, ${station.state}` : ''}` });
        if (!viewSettings.hideLanguage && station.language) items.push({ type: 'language', label: station.language.split(',')[0].trim() });
        if (!viewSettings.hideTags) {
            const tags = (station.tags || '').split(',').map(t => t.trim()).filter(Boolean);
            tags.forEach(t => items.push({ type: 'tag', label: t }));
        }
        return items;
    }, [station.country, station.state, station.language, station.tags, viewSettings]);

    // Ignore HTTP/HTTPS links
    let assetUrl = station.favicon;
    if (assetUrl && (assetUrl.startsWith('http://') || assetUrl.startsWith('https://'))) {
        assetUrl = null;
    } else {
        assetUrl = !hasError && assetUrl ? toAssetUrl(assetUrl) : null;
    }

    return (
        <div
            onClick={() => onPlay(station)}
            onContextMenu={e => { e.preventDefault(); if (onCtxMenu) onCtxMenu(e, station); }}
            className={`w-full h-full rounded-2xl cursor-pointer transition-all duration-300 relative group hover:scale-[1.03] hover:z-50 shadow-lg hover:shadow-2xl ${showTooltip ? 'z-[100]' : ''}`}
            style={{ transform: 'translateZ(0)' }} // Create isolated stacking context without clipping!
        >
            {/* Inner clipping mask for images and gradients (to hide corners without clipping tooltips!) */}
            <div className="absolute inset-0 z-0 bg-bg-surface rounded-2xl overflow-hidden pointer-events-none">
                {/* Image Background Wrapper */}
                <div className="absolute inset-0 w-full h-full">
                    {assetUrl ? (
                        <LazyLoadImage
                            src={assetUrl}
                            alt={station.name}
                            className="w-full h-full object-cover transition-transform duration-700 group-hover:scale-110"
                            wrapperClassName="w-full h-full block"
                            onError={() => setHasError(true)}
                            effect="opacity"
                        />
                    ) : (
                        <div className="w-full h-full flex items-center justify-center bg-bg-surface">
                            <Radio size={48} className={isActive ? 'text-accent' : 'text-text-muted opacity-50'} />
                        </div>
                    )}
                </div>

                {/* Deep Black Gradient Overlay to make text pop from bottom */}
                <div className={`absolute inset-x-0 bottom-0 h-[42%] z-10 bg-gradient-to-t from-bg-surface/95 via-bg-surface/60 to-transparent transition-opacity duration-300 ${assetUrl ? 'opacity-0 group-hover:opacity-100' : 'opacity-95 group-hover:opacity-100'}`} />
            </div>

            {/* Inner Border Overlay (Covers image clipping artifacts exactly) */}
            <div className={`absolute inset-0 z-50 pointer-events-none rounded-2xl transition-all duration-300 box-border
                ${isActive ? 'border-[3px] border-accent shadow-[inset_0_0_20px_rgba(29,185,84,0.3)]' : 'border border-border group-hover:border-accent/40'}
            `} />

            {/* Content Text Layer at the bottom (Now immune to clipping and placed above border!) */}
            <div className={`absolute inset-x-0 bottom-0 z-[60] flex flex-col items-center justify-end p-4 pointer-events-none text-center transition-opacity duration-300 ${assetUrl ? 'opacity-0 group-hover:opacity-100' : 'opacity-100'}`}>
                <div className="font-extrabold text-[15px] sm:text-[17px] w-full mb-1.5 text-text-primary truncate leading-tight tracking-wide">
                    {station.name || 'Unknown'}
                </div>

                <div className="flex flex-nowrap items-center justify-center gap-1.5 w-full overflow-visible opacity-80 group-hover:opacity-100 transition-opacity min-w-0">
                    {displayTags.slice(0, 1).map((item, idx) => (
                        <span
                            key={`vis-${idx}`}
                            className="tag-badge text-[10px] sm:text-[11px] px-2 py-[2px] rounded-full pointer-events-auto bg-text-primary/10 backdrop-blur-md border border-border text-text-primary font-medium hover:bg-text-primary/20 transition-colors shrink min-w-0 truncate cursor-pointer"
                            onClick={e => {
                                e.stopPropagation();
                                if (item.type === 'location' && onSelectLocation) onSelectLocation(item.raw, item.state);
                                if (item.type === 'language' && onSelectLanguage) onSelectLanguage(item.label);
                                if (item.type === 'tag' && onSelectTag) onSelectTag(item.label);
                            }}
                            title={item.label}
                        >
                            {item.label}
                        </span>
                    ))}
                    {displayTags.length > 1 && (
                        <div className="relative flex items-center pointer-events-auto" onMouseLeave={() => setShowTooltip(false)}>
                            <span
                                className="tag-badge text-[10px] px-1.5 py-[2px] rounded-full bg-text-primary/10 backdrop-blur-md border border-border text-text-primary font-medium hover:bg-text-primary/20 transition-colors shrink-0 cursor-pointer"
                                onClick={e => { e.stopPropagation(); setShowTooltip(!showTooltip); }}
                            >
                                +{displayTags.length - 1}
                            </span>
                            {showTooltip && (
                                <div className="absolute bottom-full pb-2 left-1/2 -translate-x-1/2 z-[100] cursor-default" onClick={e => e.stopPropagation()}>
                                    <div className="bg-bg-secondary border border-border rounded-lg p-2.5 flex flex-wrap gap-1.5 w-max max-w-[220px] shadow-2xl">
                                        <div className="w-full text-[9px] text-text-muted mb-0.5 font-bold uppercase tracking-wider text-left">Additional Tags</div>
                                        {displayTags.slice(1).map((item, idx) => {
                                            if (item.type === 'location') {
                                                return <span key={`hid-${idx}`} className="tag-badge tag-badge-clickable shrink-0 max-w-[180px] truncate"
                                                    onClick={e => { e.stopPropagation(); setShowTooltip(false); onSelectLocation && onSelectLocation(item.raw, item.state); }}
                                                    title={item.label}>{item.label}</span>
                                            } else if (item.type === 'language') {
                                                return <span key={`hid-${idx}`} className="tag-badge tag-badge-clickable shrink-0 max-w-[180px] truncate"
                                                    onClick={e => { e.stopPropagation(); setShowTooltip(false); onSelectLanguage && onSelectLanguage(item.label); }}
                                                    title={item.label}>{item.label}</span>
                                            } else {
                                                return <span key={`hid-${idx}`} className="tag-badge tag-badge-clickable shrink-0 max-w-[180px] truncate"
                                                    onClick={e => { e.stopPropagation(); setShowTooltip(false); onSelectTag && onSelectTag(item.label); }}
                                                    title={item.label}>{item.label}</span>
                                            }
                                        })}
                                    </div>
                                </div>
                            )}
                        </div>
                    )}
                </div>
            </div>

            {/* Heart/Fav Action */}
            <button
                onClick={e => { e.stopPropagation(); onToggleFav(station); }}
                className={`absolute top-3 right-3 z-40 shrink-0 transition-all duration-300 hover:scale-110 cursor-pointer p-2 rounded-full flex items-center justify-center shadow-xl backdrop-blur-md 
                    ${isFav ? 'text-accent bg-bg-surface/60 border border-border' : 'text-text-primary opacity-0 group-hover:opacity-100 bg-bg-surface/30 hover:bg-bg-surface/50 border border-transparent hover:border-border'}
                `}
            >
                {isFav ? <Heart fill="currentColor" size={16} /> : <Heart size={16} />}
            </button>

            {/* Active Indicator Pulse */}
            {isActive && (
                <div className="absolute top-4 left-4 z-40 w-3 h-3 rounded-full bg-accent shadow-[0_0_12px_rgba(29,185,84,1)] animate-pulse" />
            )}
        </div>
    );
});

const SortableStationGridItem = ({ id, ...props }) => {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
        isDragging,
    } = useSortable({ id });

    const style = {
        ...props.style,
        transform: CSS.Translate.toString(transform),
        transition,
        zIndex: isDragging ? 999 : 1,
        opacity: isDragging ? 0.3 : 1,
    };

    return (
        <div ref={setNodeRef} style={style} className="w-full h-full" {...attributes} {...listeners}>
            <StationGridItem {...props} isDragging={isDragging} />
        </div>
    );
};

const SortableStationItem = ({ id, ...props }) => {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
        isDragging,
    } = useSortable({ id });

    const style = {
        ...props.style,
        transform: CSS.Translate.toString(transform),
        transition,
        zIndex: isDragging ? 999 : 1,
        opacity: isDragging ? 0.3 : 1,
    };

    return (
        <div ref={setNodeRef} style={style}>
            <StationItem
                {...props}
                style={{ height: '100%', width: '100%' }}
                dragHandleProps={listeners ? { ...attributes, ...listeners } : null}
                isDragging={isDragging}
                onSelectTag={props.onSelectTag}
                onSelectLocation={props.onSelectLocation}
                onSelectLanguage={props.onSelectLanguage}
                viewSettings={props.viewSettings}
            />
        </div>
    );
};

const Row = memo(({ index, style, data }) => {
    const { isGrid, gridCols, viewSettings, width } = data;

    if (isGrid) {
        const startIndex = index * gridCols;
        const rowStations = data.stations.slice(startIndex, startIndex + gridCols);

        const gap = 16;
        const totalGap = gap * (gridCols - 1);
        const itemWidth = Math.max(0, (width - 16 - totalGap) / gridCols);

        return (
            <div style={{ ...style, display: 'flex', gap: `${gap}px`, paddingBottom: '16px' }}>
                {rowStations.map((s) => (
                    <div key={s.stationuuid} style={{ width: itemWidth, height: '100%' }}>
                        {data.canSort ? (
                            <SortableStationGridItem
                                id={s.stationuuid}
                                station={s}
                                isActive={s.stationuuid === data.activeUuid}
                                isFav={s.isFavorite}
                                onPlay={data.onPlay}
                                onToggleFav={data.onToggleFavorite}
                                onCtxMenu={data.onCtxMenu}
                                onSelectTag={data.onSelectTag}
                                onSelectLocation={data.onSelectLocation}
                                onSelectLanguage={data.onSelectLanguage}
                                viewSettings={viewSettings}
                            />
                        ) : (
                            <StationGridItem
                                station={s}
                                isActive={s.stationuuid === data.activeUuid}
                                isFav={s.isFavorite}
                                onPlay={data.onPlay}
                                onToggleFav={data.onToggleFavorite}
                                onCtxMenu={data.onCtxMenu}
                                onSelectTag={data.onSelectTag}
                                onSelectLocation={data.onSelectLocation}
                                onSelectLanguage={data.onSelectLanguage}
                                viewSettings={viewSettings}
                            />
                        )}
                    </div>
                ))}
            </div>
        );
    }

    const s = data.stations[index];
    if (!s) return null;

    const canSort = data.canSort;
    const isDraggingThis = data.dragActiveId === s.stationuuid;

    if (canSort) {
        return (
            <SortableStationItem
                id={s.stationuuid}
                station={s}
                isActive={s.stationuuid === data.activeUuid}
                isFav={s.isFavorite}
                onPlay={data.onPlay}
                onToggleFav={data.onToggleFavorite}
                onCtxMenu={data.onCtxMenu}
                onSelectTag={data.onSelectTag}
                onSelectLocation={data.onSelectLocation}
                onSelectLanguage={data.onSelectLanguage}
                style={style}
                isDragging={isDraggingThis}
                containerWidth={data.width}
                viewSettings={data.viewSettings}
            />
        );
    }

    return (
        <StationItem
            station={s}
            isActive={s.stationuuid === data.activeUuid}
            isFav={s.isFavorite}
            onPlay={data.onPlay}
            onToggleFav={data.onToggleFavorite}
            onCtxMenu={data.onCtxMenu}
            onSelectTag={data.onSelectTag}
            onSelectLocation={data.onSelectLocation}
            onSelectLanguage={data.onSelectLanguage}
            style={style}
            containerWidth={data.width}
            viewSettings={data.viewSettings}
        />
    );
}, (prevProps, nextProps) => {
    const isGrid = prevProps.data.isGrid;

    if (isGrid) {
        // If the globally playing station changed, we must force a rerender
        // so that the previous active row can turn off, and the new active row can turn on.
        if (prevProps.data.activeUuid !== nextProps.data.activeUuid) return false;

        const startIndex = prevProps.index * prevProps.data.gridCols;
        const prevRowStations = prevProps.data.stations.slice(startIndex, startIndex + prevProps.data.gridCols);
        const nextRowStations = nextProps.data.stations.slice(startIndex, startIndex + nextProps.data.gridCols);

        // Also check if favorites changed in this row
        const prevFavs = prevRowStations.map(s => s.isFavorite).join(',');
        const nextFavs = nextRowStations.map(s => s.isFavorite).join(',');
        if (prevFavs !== nextFavs) return false;

        // Ensure row redraws if ANY data inside the stations changes (e.g. edited name, new image)
        if (JSON.stringify(prevRowStations) !== JSON.stringify(nextRowStations)) return false;

        return prevProps.style === nextProps.style &&
            prevProps.data.width === nextProps.data.width &&
            prevProps.data.canSort === nextProps.data.canSort &&
            prevProps.data.gridCols === nextProps.data.gridCols &&
            prevProps.data.viewSettings === nextProps.data.viewSettings;
    }

    const prevItem = prevProps.data.stations[prevProps.index];
    const nextItem = nextProps.data.stations[nextProps.index];

    return prevProps.index === nextProps.index &&
        prevProps.style === nextProps.style &&
        JSON.stringify(prevItem) === JSON.stringify(nextItem) &&
        prevProps.data.width === nextProps.data.width &&
        prevProps.data.canSort === nextProps.data.canSort &&
        prevProps.data.isGrid === nextProps.data.isGrid &&
        prevProps.data.viewSettings === nextProps.data.viewSettings &&
        (!prevItem ? false : (prevItem.stationuuid === prevProps.data.activeUuid)) === (!nextItem ? false : (nextItem.stationuuid === nextProps.data.activeUuid)) &&
        (!prevItem ? false : prevItem.isFavorite) === (!nextItem ? false : nextItem.isFavorite);
});

export default memo(function StationList({
    title, icon, stations, loading, activeUuid,
    searchQuery, onSearch,
    onPlay, onToggleFavorite, tab,
    onCtxMenu, onAddRadio,
    onGoBack, canGoBack,
    sortBy, setSortBy, sortOrder, setSortOrder, onReorder,
    onSelectTag, onSelectLocation, onSelectLanguage,
    isNavigatingBack, forceScrollTop
}) {
    const { t } = useTranslation();

    const sensors = useSensors(
        useSensor(PointerSensor, {
            activationConstraint: {
                distance: 8,
            },
        })
    );

    const [dragActiveId, setDragActiveId] = useState(null);
    const [internalStations, setInternalStations] = useState(stations);
    const [prevStations, setPrevStations] = useState(stations);
    const pendingSyncRef = useRef(false);

    // Synchronize internal list with prop synchronously during render to avoid 1-render lag
    // which causes react-window to apply large initialScrollOffsets to tiny mismatched lists.
    if (stations !== prevStations) {
        setPrevStations(stations);
        if (pendingSyncRef.current) {
            // Skip the immediate reset on drag end, wait for parent to push real new stations prop
            pendingSyncRef.current = false;
        } else if (!dragActiveId) {
            setInternalStations(stations);
        }
    }

    const [isSortOpen, setIsSortOpen] = useState(false);
    const [isViewSettingsOpen, setIsViewSettingsOpen] = useState(false);
    const [showJumpToActive, setShowJumpToActive] = useState(false);

    const [viewSettings, setViewSettings] = useState(() => {
        try {
            const saved = localStorage.getItem('radiko_stationList_viewSettings');
            return saved ? JSON.parse(saved) : { hideLocation: false, hideLanguage: false, hideTags: false, isGrid: false };
        } catch {
            return { hideLocation: false, hideLanguage: false, hideTags: false, isGrid: false };
        }
    });

    const canSort = (tab === 'favorites' || tab === 'all') && sortBy === 'manual';

    useEffect(() => {
        localStorage.setItem('radiko_stationList_viewSettings', JSON.stringify(viewSettings));
    }, [viewSettings]);

    const sortRef = useRef(null);
    const viewSettingsRef = useRef(null);
    const listRef = useRef(null);

    const scrollPositions = useRef({});
    const currentKey = `${tab}-${title}`;

    const handleScroll = ({ scrollOffset }) => {
        // react-window initialScrollOffset passes initial position. Just record updates here.
        if (scrollOffset > 0 || scrollPositions.current[currentKey] === undefined) {
            scrollPositions.current[currentKey] = scrollOffset;
        } else if (scrollOffset === 0 && listRef.current) {
            scrollPositions.current[currentKey] = 0;
        }
    };

    const gridColsRef = useRef(1);
    const visibleRange = useRef({ start: 0, stop: 0 });

    const handleItemsRendered = ({ visibleStartIndex, visibleStopIndex }) => {
        visibleRange.current = { start: visibleStartIndex, stop: visibleStopIndex };
        if (!activeUuid) {
            setShowJumpToActive(false);
            return;
        }
        const activeIndex = internalStations.findIndex(s => s.stationuuid === activeUuid);
        if (activeIndex === -1) {
            setShowJumpToActive(false);
        } else {
            const rowIndex = viewSettings.isGrid ? Math.floor(activeIndex / gridColsRef.current) : activeIndex;
            const isVisible = rowIndex >= visibleStartIndex && rowIndex <= visibleStopIndex;
            setShowJumpToActive(!isVisible);
        }
    };

    // Force scroll to top when user clicks an already active tab
    useEffect(() => {
        if (listRef.current && forceScrollTop > 0) {
            listRef.current.scrollTo(0);
            scrollPositions.current[currentKey] = 0; // update cache so it doesn't bounce back
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [forceScrollTop]);

    // Dynamic auto-scrolling when the active station is structurally changed (eg. "Next" button in MiniPlayer)
    // Also forces "Jump to Active" button state update if user clicks an already-visible station while scrolled
    const prevActive = useRef(activeUuid);
    useEffect(() => {
        if (activeUuid !== prevActive.current) {
            prevActive.current = activeUuid;
            if (activeUuid && listRef.current) {
                const index = internalStations.findIndex(s => s.stationuuid === activeUuid);
                if (index !== -1) {
                    const rowIndex = viewSettings.isGrid ? Math.floor(index / gridColsRef.current) : index;
                    listRef.current.scrollToItem(rowIndex, 'auto');

                    // Manually check if the new active item is already in view to hide the button instantly without waiting for a scroll event
                    const isVisible = rowIndex >= visibleRange.current.start && rowIndex <= visibleRange.current.stop;
                    setShowJumpToActive(!isVisible);
                }
            } else {
                setShowJumpToActive(false);
            }
        }
    }, [activeUuid, internalStations, viewSettings.isGrid]);

    // Recalculate button visibility when lists change rapidly (like going back from a tag list)
    useEffect(() => {
        if (!activeUuid) {
            setShowJumpToActive(false);
            return;
        }

        // Wait for react-window to restore scroll position and fire onItemsRendered before judging visibility
        const timer = setTimeout(() => {
            const activeIndex = internalStations.findIndex(s => s.stationuuid === activeUuid);
            if (activeIndex === -1) {
                setShowJumpToActive(false);
            } else {
                const rowIndex = viewSettings.isGrid ? Math.floor(activeIndex / Math.max(1, gridColsRef.current)) : activeIndex;
                const isVisible = rowIndex >= visibleRange.current.start && rowIndex <= visibleRange.current.stop;
                setShowJumpToActive(!isVisible);
            }
        }, 150);

        return () => clearTimeout(timer);
    }, [internalStations, activeUuid, viewSettings.isGrid]);

    // Close sort/settings dropdowns on click outside
    useEffect(() => {
        const handleClickOutside = (event) => {
            if (sortRef.current && !sortRef.current.contains(event.target)) setIsSortOpen(false);
            if (viewSettingsRef.current && !viewSettingsRef.current.contains(event.target)) setIsViewSettingsOpen(false);
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const itemData = useMemo(() => ({
        stations: internalStations, activeUuid, onPlay, onToggleFavorite, onCtxMenu, canSort, dragActiveId, onSelectTag, onSelectLocation, onSelectLanguage, viewSettings
    }), [internalStations, activeUuid, onPlay, onToggleFavorite, onCtxMenu, canSort, dragActiveId, onSelectTag, onSelectLocation, onSelectLanguage, viewSettings]);

    const handleDragStart = (event) => {
        const { active } = event;
        setDragActiveId(active.id);
        console.log("[STATIONLIST] Drag Start:", active.id);
    };

    const handleDragOver = (event) => {
        const { active, over } = event;
        if (!over || active.id === over.id) return;

        const oldIndex = internalStations.findIndex(s => s.stationuuid === active.id);
        const newIndex = internalStations.findIndex(s => s.stationuuid === over.id);

        if (oldIndex !== -1 && newIndex !== -1) {
            setInternalStations(prev => arrayMove(prev, oldIndex, newIndex));
        }
    };

    const handleDragEnd = (event) => {
        const { active, over } = event;
        setDragActiveId(null);
        console.log("[STATIONLIST] Drag End. Active:", active?.id, "Over:", over?.id);

        const hasChanged = JSON.stringify(internalStations.map(s => s.stationuuid)) !==
            JSON.stringify(stations.map(s => s.stationuuid));

        if (hasChanged) {
            console.log("[STATIONLIST] Reorder detected, calling onReorder");
            pendingSyncRef.current = true;
            onReorder(internalStations);
        } else {
            console.log("[STATIONLIST] No changes detected in order.");
            setInternalStations(stations); // manual reset for aborted drags
        }
    };

    const activeStationObj = useMemo(() =>
        dragActiveId ? internalStations.find(s => s.stationuuid === dragActiveId) : null
        , [dragActiveId, internalStations]);

    return (
        <div className="flex-1 flex flex-col overflow-hidden bg-bg-primary">
            {/* Header */}
            <div className="shrink-0 px-4 pt-3 pb-2 border-b border-border/30 bg-bg-secondary/50">
                <div className="flex justify-between items-start mb-3">
                    <div className="flex flex-col flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-0.5 overflow-hidden">
                            {canGoBack && (
                                <button
                                    onClick={onGoBack}
                                    className="shrink-0 w-7 h-7 flex items-center justify-center rounded-lg hover:bg-bg-surface-hover text-text-muted hover:text-text-primary transition-all cursor-pointer"
                                    title={t('common.goBack') || "Go Back"}
                                >
                                    <ArrowLeft size={18} />
                                </button>
                            )}
                            {icon && <span className="text-accent shrink-0">{icon}</span>}
                            <h2 className="text-lg font-bold text-text-primary truncate leading-tight">{title}</h2>
                        </div>
                        <div className="text-[10px] text-text-muted font-medium mb-1 ml-1 flex items-center gap-2">
                            <span>{stations.length} {stations.length === 1 ? t('common.station') : t('common.stations')}</span>
                            {loading && <span className="animate-pulse opacity-50 px-2 py-0.5 bg-accent/20 rounded text-[9px]">LOADING...</span>}
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={() => onAddRadio()}
                            className="px-3 py-1.5 bg-accent/10 hover:bg-accent text-accent hover:text-white rounded-lg transition-colors text-xs font-semibold flex items-center gap-2"
                        >
                            <Plus size={14} strokeWidth={3} /> {t('stationList.addRadio')}
                        </button>

                        <div className="relative" ref={viewSettingsRef}>
                            <button
                                onClick={() => setIsViewSettingsOpen(!isViewSettingsOpen)}
                                className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors border ${isViewSettingsOpen ? 'bg-bg-surface-hover text-text-primary border-border' : 'bg-transparent text-text-muted hover:text-text-primary border-transparent hover:border-border hover:bg-bg-surface'}`}
                                title={t('stationList.viewSettingsTitle')}
                            >
                                <Settings2 size={16} />
                            </button>
                            {isViewSettingsOpen && (
                                <div className="absolute top-full right-0 mt-2 w-48 bg-bg-secondary border border-border rounded-lg shadow-2xl z-[100] py-1 animate-in fade-in zoom-in-95 duration-100">
                                    <div className="px-3 py-1.5 text-[10px] text-text-muted font-bold uppercase tracking-wider mb-1 border-b border-border">{t('stationList.visibility')}</div>
                                    <button
                                        onClick={() => setViewSettings(p => ({ ...p, hideLocation: !p.hideLocation }))}
                                        className="w-full flex items-center justify-between px-3 py-2 text-xs hover:bg-bg-surface-hover transition-colors font-medium text-text-secondary"
                                    >
                                        <span className="flex items-center gap-2"><MapPin size={12} /> {t('stationList.hideLocation')}</span>
                                        {viewSettings.hideLocation && <Check size={12} className="text-accent" />}
                                    </button>
                                    <button
                                        onClick={() => setViewSettings(p => ({ ...p, hideLanguage: !p.hideLanguage }))}
                                        className="w-full flex items-center justify-between px-3 py-2 text-xs hover:bg-bg-surface-hover transition-colors font-medium text-text-secondary"
                                    >
                                        <span className="flex items-center gap-2"><Languages size={12} /> {t('stationList.hideLanguage')}</span>
                                        {viewSettings.hideLanguage && <Check size={12} className="text-accent" />}
                                    </button>
                                    <button
                                        onClick={() => setViewSettings(p => ({ ...p, hideTags: !p.hideTags }))}
                                        className="w-full flex items-center justify-between px-3 py-2 text-xs hover:bg-bg-surface-hover transition-colors font-medium border-b border-border text-text-secondary"
                                    >
                                        <span className="flex items-center gap-2"><Tags size={12} /> {t('stationList.hideTags')}</span>
                                        {viewSettings.hideTags && <Check size={12} className="text-accent" />}
                                    </button>
                                    <div className="px-3 py-1.5 text-[10px] text-text-muted font-bold uppercase tracking-wider mt-1 mb-1 border-b border-border">{t('stationList.layout')}</div>
                                    <button
                                        onClick={() => setViewSettings(p => ({ ...p, isGrid: false }))}
                                        className={`w-full flex items-center justify-between px-3 py-2 text-xs hover:bg-bg-surface-hover transition-colors font-medium ${!viewSettings.isGrid ? 'text-accent' : 'text-text-secondary'}`}
                                    >
                                        <span className="flex items-center gap-2"><AlignJustify size={12} /> {t('stationList.listMode')}</span>
                                        {!viewSettings.isGrid && <Check size={12} className="text-accent" />}
                                    </button>
                                    <button
                                        onClick={() => setViewSettings(p => ({ ...p, isGrid: true }))}
                                        className={`w-full flex items-center justify-between px-3 py-2 text-xs hover:bg-bg-surface-hover transition-colors font-medium ${viewSettings.isGrid ? 'text-accent' : 'text-text-secondary'}`}
                                    >
                                        <span className="flex items-center gap-2"><LayoutGrid size={12} /> {t('stationList.gridMode')}</span>
                                        {viewSettings.isGrid && <Check size={12} className="text-accent" />}
                                    </button>
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                <div className="flex items-center gap-2">
                    {/* Search */}
                    <div className="relative flex-1">
                        <svg className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-muted pointer-events-none" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
                            <circle cx="11" cy="11" r="7" /><line x1="16.5" y1="16.5" x2="21" y2="21" />
                        </svg>
                        <input
                            type="text"
                            placeholder={t('stationList.searchPlaceholder')}
                            value={searchQuery}
                            onChange={e => onSearch(e.target.value)}
                            className="w-full h-[34px] pl-9 pr-4 bg-bg-surface border border-border rounded-lg text-xs text-text-primary placeholder-text-muted outline-none focus:border-accent transition-colors"
                            spellCheck="false"
                        />
                    </div>

                    {/* Sort Controls */}
                    <div className="flex items-center gap-1 bg-bg-surface p-1 rounded-lg border border-border h-[34px]" ref={sortRef}>
                        <div className="relative h-full">
                            <button
                                onClick={() => setIsSortOpen(!isSortOpen)}
                                className="flex h-full items-center gap-2 px-3 hover:bg-bg-surface-hover rounded-md transition-colors text-[10px] font-bold text-text-secondary border-r border-border/50"
                            >
                                <span className="capitalize">
                                    {sortBy === 'manual' ? t('common.manual') :
                                        sortBy === 'country' ? t('common.country') :
                                            t('common.name')}
                                </span>
                                <ChevronDown size={12} className={`transition-transform duration-200 ${isSortOpen ? 'rotate-180' : ''}`} />
                            </button>

                            {isSortOpen && (
                                <div className="absolute top-full right-0 mt-1 w-32 bg-bg-secondary border border-border rounded-lg shadow-2xl z-[100] py-1 animate-in fade-in zoom-in-95 duration-100 overflow-hidden">
                                    <button
                                        onClick={() => { setSortBy('name'); setIsSortOpen(false); }}
                                        className={`w-full flex items-center justify-between px-3 py-2 text-[10px] font-bold hover:bg-bg-surface-hover transition-colors ${sortBy === 'name' ? 'text-accent' : 'text-text-secondary'}`}
                                    >
                                        <span>{t('common.name') || 'Name'}</span>
                                        {sortBy === 'name' && <Check size={10} />}
                                    </button>
                                    <button
                                        onClick={() => { setSortBy('country'); setIsSortOpen(false); }}
                                        className={`w-full flex items-center justify-between px-3 py-2 text-[10px] font-bold hover:bg-bg-surface-hover transition-colors ${sortBy === 'country' ? 'text-accent' : 'text-text-secondary'}`}
                                    >
                                        <span>{t('common.country') || 'Country'}</span>
                                        {sortBy === 'country' && <Check size={10} />}
                                    </button>
                                    {(tab === 'favorites' || tab === 'all') && (
                                        <button
                                            onClick={() => { setSortBy('manual'); setIsSortOpen(false); }}
                                            className={`w-full flex items-center justify-between px-3 py-2 text-[10px] font-bold hover:bg-bg-surface-hover transition-colors ${sortBy === 'manual' ? 'text-accent' : 'text-text-secondary'}`}
                                        >
                                            <span>{t('common.manual') || 'Manual'}</span>
                                            {sortBy === 'manual' && <Check size={10} />}
                                        </button>
                                    )}
                                </div>
                            )}
                        </div>

                        <button
                            disabled={sortBy === 'manual'}
                            onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
                            className={`flex h-full items-center justify-center px-2 rounded hover:bg-bg-surface-hover transition-colors
                                ${sortBy === 'manual' ? 'opacity-20 cursor-not-allowed' : 'text-accent'}
                            `}
                            title={sortOrder === 'asc' ? 'Ascending' : 'Descending'}
                        >
                            {sortOrder === 'asc' ? <SortAsc size={14} /> : <SortDesc size={14} />}
                        </button>
                    </div>
                </div>
            </div>

            {/* List */}
            <div className={`flex-1 overflow-hidden flex flex-col ${viewSettings?.isGrid ? 'px-4 pb-4 mt-4' : 'px-2 pb-3 mt-2'}`}>
                {stations.length === 0 && !loading ? (
                    <div className="flex flex-col items-center justify-center h-full text-center text-text-muted text-sm gap-2">
                        {tab === 'favorites' ? (
                            <>
                                <Heart size={32} className="opacity-20 mb-2" />
                                <span>{t('stationList.noFavorites')}</span>
                            </>
                        ) : t('stationList.notFound')}
                    </div>
                ) : (
                    <div className="h-full w-full relative">
                        {canSort ? (
                            <DndContext
                                sensors={sensors}
                                collisionDetection={closestCenter}
                                onDragStart={handleDragStart}
                                onDragOver={handleDragOver}
                                onDragEnd={handleDragEnd}
                                modifiers={viewSettings?.isGrid ? [restrictToWindowEdges] : [restrictToVerticalAxis, restrictToWindowEdges]}
                            >
                                <SortableContext
                                    items={internalStations.map(s => s.stationuuid)}
                                    strategy={viewSettings?.isGrid ? rectSortingStrategy : verticalListSortingStrategy}
                                    disabled={!canSort}
                                >
                                    <AutoSizer>
                                        {({ height, width }) => {
                                            if (height === 0 || width === 0) return null;
                                            const isGrid = viewSettings?.isGrid;
                                            const gap = 16;
                                            const MIN_CARD_WIDTH = 180;
                                            const computedCols = isGrid ? Math.max(1, Math.floor((width - 16) / MIN_CARD_WIDTH)) : 1;
                                            const itemWidth = Math.max(0, (width - 16 - (gap * (computedCols - 1))) / computedCols);
                                            gridColsRef.current = computedCols;
                                            const rowCount = isGrid ? Math.ceil(internalStations.length / computedCols) : internalStations.length;
                                            const rowHeight = isGrid ? itemWidth + gap : 68;

                                            return (
                                                <List
                                                    key={`list-${viewSettings?.isGrid ? 'grid' : 'list'}-${currentKey}`}
                                                    ref={listRef}
                                                    height={height}
                                                    itemCount={rowCount}
                                                    itemSize={rowHeight}
                                                    width={width}
                                                    initialScrollOffset={scrollPositions.current[currentKey] || 0}
                                                    className="custom-scrollbar overflow-x-hidden"
                                                    style={{ overflowX: 'hidden' }}
                                                    itemData={{ ...itemData, width, isGrid, gridCols: computedCols, canSort: true }}
                                                    onScroll={handleScroll}
                                                    onItemsRendered={handleItemsRendered}
                                                    innerElementType={InnerList}
                                                >
                                                    {Row}
                                                </List>
                                            )
                                        }}
                                    </AutoSizer>
                                </SortableContext>
                                <DragOverlay dropAnimation={null}>
                                    {activeStationObj ? (
                                        <div className="w-full pointer-events-none">
                                            {activeStationObj && !viewSettings?.isGrid ? (
                                                <div style={{ width: '100%', height: 68 }} className="opacity-90 shadow-2xl scale-[1.02] transition-transform">
                                                    <StationItem
                                                        station={activeStationObj}
                                                        isActive={activeStationObj.stationuuid === activeUuid}
                                                        isFav={activeStationObj.isFavorite}
                                                        onPlay={() => { }}
                                                        onToggleFav={() => { }}
                                                        style={{ height: '100%' }}
                                                        viewSettings={viewSettings}
                                                    />
                                                </div>
                                            ) : activeStationObj && viewSettings?.isGrid ? (
                                                <div style={{ width: 180, height: 184 }} className="opacity-90 shadow-2xl scale-105 transition-transform">
                                                    <StationGridItem
                                                        station={activeStationObj}
                                                        isActive={activeStationObj.stationuuid === activeUuid}
                                                        isFav={activeStationObj.isFavorite}
                                                        onPlay={() => { }}
                                                        onToggleFav={() => { }}
                                                        onCtxMenu={() => { }}
                                                        viewSettings={viewSettings}
                                                    />
                                                </div>
                                            ) : null}
                                        </div>
                                    ) : null}
                                </DragOverlay>
                            </DndContext>
                        ) : (
                            <AutoSizer>
                                {({ height, width }) => {
                                    if (height === 0 || width === 0) return null;
                                    const isGrid = viewSettings?.isGrid;
                                    const gap = 16;
                                    const MIN_CARD_WIDTH = 180;
                                    const computedCols = isGrid ? Math.max(1, Math.floor((width - 16) / MIN_CARD_WIDTH)) : 1;
                                    const itemWidth = Math.max(0, (width - 16 - (gap * (computedCols - 1))) / computedCols);
                                    gridColsRef.current = computedCols;
                                    const rowCount = isGrid ? Math.ceil(internalStations.length / computedCols) : internalStations.length;
                                    const rowHeight = isGrid ? itemWidth + gap : 68;

                                    return (
                                        <List
                                            key={`list-unsortable-${viewSettings?.isGrid ? 'grid' : 'list'}-${currentKey}`}
                                            ref={listRef}
                                            height={height}
                                            itemCount={rowCount}
                                            itemSize={rowHeight}
                                            width={width}
                                            initialScrollOffset={scrollPositions.current[currentKey] || 0}
                                            className="custom-scrollbar overflow-x-hidden"
                                            style={{ overflowX: 'hidden' }}
                                            itemData={{ ...itemData, width, isGrid, gridCols: computedCols, canSort: false }}
                                            onScroll={handleScroll}
                                            onItemsRendered={handleItemsRendered}
                                            innerElementType={InnerList}
                                        >
                                            {Row}
                                        </List>
                                    )
                                }}
                            </AutoSizer>
                        )}

                        {/* Jump to Active Button */}
                        <div className={`absolute bottom-6 left-1/2 -translate-x-1/2 z-[100] transition-all duration-300 ${showJumpToActive ? 'opacity-100 translate-y-0 visible' : 'opacity-0 translate-y-4 invisible'}`}>
                            <button
                                onClick={() => {
                                    const index = internalStations.findIndex(s => s.stationuuid === activeUuid);
                                    if (index !== -1 && listRef.current) {
                                        const rowIndex = viewSettings.isGrid ? Math.floor(index / gridColsRef.current) : index;
                                        listRef.current.scrollToItem(rowIndex, 'center');
                                    }
                                }}
                                className="flex items-center gap-2 px-3 sm:px-4 py-2 bg-accent/90 hover:bg-accent backdrop-blur-md text-white font-bold text-xs rounded-full shadow-[0_4px_16px_rgba(29,185,84,0.3)] hover:scale-105 transition-all text-shadow-sm border border-accent/20 cursor-pointer"
                            >
                                <Radio size={14} className="animate-pulse" />
                                <span className="hidden sm:inline">{t('stationList.jumpToActive')}</span>
                            </button>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
});
