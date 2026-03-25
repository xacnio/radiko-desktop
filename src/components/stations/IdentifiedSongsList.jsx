import { Music, Clock, Radio, Search, Trash2, Calendar, Filter, ArrowDownAz, ExternalLink } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useState, useEffect, useMemo } from 'react';
import ConfirmModal from '../common/ConfirmModal';
import { useTranslation } from 'react-i18next';
import { useHttpLink } from '../../hooks/useHttpLink';


function timeAgo(dateStr, t) {
    const now = new Date();
    const date = new Date(dateStr);
    const diff = Math.floor((now - date) / 1000);
    if (diff < 60) return t('identified_songs.justNow');
    if (diff < 3600) return t('identified_songs.minutesAgo', { count: Math.floor(diff / 60) });
    if (diff < 86400) return t('identified_songs.hoursAgo', { count: Math.floor(diff / 3600) });
    if (diff < 2592000) return t('identified_songs.daysAgo', { count: Math.floor(diff / 86400) });
    if (diff < 31536000) return t('identified_songs.monthsAgo', { count: Math.floor(diff / 2592000) });
    return t('identified_songs.yearsAgo', { count: Math.floor(diff / 31536000) });
}

export default function IdentifiedSongsList({ songs, onClear, onDeleteSong }) {
    const { t } = useTranslation();
    const { openLink, modal: httpModal } = useHttpLink(t);
    const [ctxMenu, setCtxMenu] = useState(null);
    const [isClearModalOpen, setIsClearModalOpen] = useState(false);

    // Filters and Sort State
    const [searchQuery, setSearchQuery] = useState('');
    const [filterRadio, setFilterRadio] = useState('all');
    const [filterDate, setFilterDate] = useState('all');
    const [filterSource, setFilterSource] = useState('all');
    const [sortBy, setSortBy] = useState('date_desc');

    const uniqueRadios = useMemo(() => {
        const set = new Set(songs.map(s => s.station_name).filter(Boolean));
        return Array.from(set).sort();
    }, [songs]);

    const filteredAndSortedSongs = useMemo(() => {
        let result = [...songs];

        if (searchQuery.trim() !== '') {
            const q = searchQuery.toLowerCase();
            result = result.filter(s =>
                (s.title || '').toLowerCase().includes(q) ||
                (s.artist || '').toLowerCase().includes(q)
            );
        }

        if (filterRadio !== 'all') {
            result = result.filter(s => s.station_name === filterRadio);
        }

        if (filterDate !== 'all') {
            const now = new Date();
            const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
            const thisWeek = today - (7 * 24 * 60 * 60 * 1000);
            const thisMonth = new Date(now.getFullYear(), now.getMonth() - 1, now.getDate()).getTime();

            result = result.filter(s => {
                const songTime = new Date(s.found_at).getTime();
                if (filterDate === 'today') return songTime >= today;
                if (filterDate === 'this_week') return songTime >= thisWeek;
                if (filterDate === 'this_month') return songTime >= thisMonth;
                return true;
            });
        }

        if (filterSource !== 'all') {
            result = result.filter(s => {
                const src = s.source || 'iTunes';
                return src === filterSource;
            });
        }

        result.sort((a, b) => {
            if (sortBy === 'date_desc') return new Date(b.found_at).getTime() - new Date(a.found_at).getTime();
            if (sortBy === 'date_asc') return new Date(a.found_at).getTime() - new Date(b.found_at).getTime();
            if (sortBy === 'title_asc') return (a.title || '').localeCompare(b.title || '');
            if (sortBy === 'artist_asc') return (a.artist || '').localeCompare(b.artist || '');
            return 0;
        });

        return result;
    }, [songs, searchQuery, filterRadio, filterDate, filterSource, sortBy]);

    // Close menu on click outside
    useEffect(() => {
        const hide = () => setCtxMenu(null);
        window.addEventListener('click', hide);
        return () => window.removeEventListener('click', hide);
    }, []);

    if (songs.length === 0) {
        return (
            <div className="flex-1 flex flex-col items-center justify-center text-text-muted gap-4 animate-in fade-in duration-500">
                <div className="w-20 h-20 rounded-full bg-bg-surface flex items-center justify-center border border-border">
                    <Search size={40} className="opacity-10" />
                </div>
                <div className="text-center">
                    <p className="text-sm font-bold text-text-primary">{t('identified_songs.noSongsFound')}</p>
                    <p className="text-xs opacity-50 mt-1">{t('identified_songs.noSongsFoundDesc')}</p>
                </div>
            </div>
        );
    }

    const handleGlobalClear = () => {
        setIsClearModalOpen(true);
    };

    const confirmClear = async () => {
        await invoke('clear_identified_songs');
        onClear();
    };

    const handleDeleteSong = async (song) => {
        await invoke('delete_identified_song', { songToDelete: song });
        onDeleteSong(song);
        setCtxMenu(null);
    };

    const handleCtx = (e, song) => {
        e.preventDefault();
        setCtxMenu({ x: e.clientX, y: e.clientY, song });
    };

    return (
        <div className="flex-1 flex flex-col overflow-hidden bg-bg-primary animate-in fade-in duration-300">
            {/* Header */}
            <div className="px-6 py-4 border-b border-border flex flex-col gap-4 shrink-0">
                <div className="flex justify-between items-center">
                    <h2 className="text-sm font-bold text-text-primary uppercase tracking-widest">{t('identified_songs.title')}</h2>
                    <button
                        onClick={handleGlobalClear}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-surface hover:bg-red-500/10 text-text-muted hover:text-red-400 text-xs font-bold transition-all border border-border"
                    >
                        <Trash2 size={13} /> {t('identified_songs.clearHistory')}
                    </button>
                </div>

                {/* Filters & Controls */}
                <div className="flex flex-wrap items-center gap-2">
                    {/* Search */}
                    <div className="relative flex-1 min-w-[200px]">
                        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted opacity-50" />
                        <input
                            type="text"
                            placeholder={t('identified_songs.searchPlaceholder')}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                            className="w-full bg-bg-surface border border-border rounded-lg pl-9 pr-4 py-2 text-xs text-text-primary focus:outline-none focus:border-accent transition-colors placeholder:text-text-muted"
                        />
                    </div>
                    {/* Date Filter */}
                    <div className="relative hidden md:block">
                        <select
                            value={filterDate}
                            onChange={(e) => setFilterDate(e.target.value)}
                            className="appearance-none bg-bg-surface border border-border rounded-lg pl-8 pr-8 py-2 text-xs text-text-primary focus:outline-none focus:border-accent cursor-pointer"
                        >
                            <option value="all">{t('identified_songs.allTime')}</option>
                            <option value="today">{t('identified_songs.today')}</option>
                            <option value="this_week">{t('identified_songs.thisWeek')}</option>
                            <option value="this_month">{t('identified_songs.thisMonth')}</option>
                        </select>
                        <Calendar size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none" />
                    </div>
                    {/* Radio Filter */}
                    <div className="relative hidden lg:block">
                        <select
                            value={filterRadio}
                            onChange={(e) => setFilterRadio(e.target.value)}
                            className="appearance-none bg-bg-surface border border-border rounded-lg pl-8 pr-8 py-2 text-xs text-text-primary focus:outline-none focus:border-accent cursor-pointer w-36 truncate"
                        >
                            <option value="all">{t('identified_songs.allRadios')}</option>
                            {uniqueRadios.map(r => (
                                <option key={r} value={r}>{r}</option>
                            ))}
                        </select>
                        <Filter size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none" />
                    </div>
                    {/* Source Filter */}
                    <div className="relative hidden lg:block">
                        <select
                            value={filterSource}
                            onChange={(e) => setFilterSource(e.target.value)}
                            className="appearance-none bg-bg-surface border border-border rounded-lg pl-8 pr-8 py-2 text-xs text-text-primary focus:outline-none focus:border-accent cursor-pointer"
                        >
                            <option value="all">{t('identified_songs.allSources')}</option>
                            <option value="Shazam">Shazam</option>
                            <option value="iTunes">{t('identified_songs.autoITunes')}</option>
                        </select>
                        <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none" />
                    </div>
                    {/* Sort */}
                    <div className="relative ml-auto">
                        <select
                            value={sortBy}
                            onChange={(e) => setSortBy(e.target.value)}
                            className="appearance-none bg-bg-surface border border-border rounded-lg pl-8 pr-8 py-2 text-xs text-text-primary focus:outline-none focus:border-accent cursor-pointer"
                        >
                            <option value="date_desc">{t('identified_songs.newest')}</option>
                            <option value="date_asc">{t('identified_songs.oldest')}</option>
                            <option value="title_asc">{t('identified_songs.titleAZ')}</option>
                            <option value="artist_asc">{t('identified_songs.artistAZ')}</option>
                        </select>
                        <ArrowDownAz size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted pointer-events-none" />
                    </div>
                </div>
            </div>

            {/* List */}
            <div className="flex-1 overflow-y-auto custom-scrollbar p-2">
                {filteredAndSortedSongs.length === 0 ? (
                    <div className="flex flex-col items-center justify-center p-12 text-center">
                        <Search size={32} className="text-text-muted opacity-20 mb-3" />
                        <p className="text-text-primary text-sm font-semibold">{t('identified_songs.noResults')}</p>
                        <p className="text-text-muted text-xs mt-1">{t('identified_songs.noResultsDesc')}</p>
                    </div>
                ) : (
                    <div className="flex flex-col gap-1">
                        {filteredAndSortedSongs.map((song, i) => {
                            const relativeTime = timeAgo(song.found_at, t);

                            return (
                                <div
                                    key={i}
                                    onClick={() => {
                                        const primaryLink = song.sources?.[0]?.link || song.song_link;
                                        if (primaryLink) openLink(primaryLink);
                                    }}
                                    onContextMenu={(e) => handleCtx(e, song)}
                                    className="group flex items-center gap-4 px-3 py-2.5 rounded-lg cursor-pointer transition-all hover:bg-bg-surface-hover active:scale-[0.98]"
                                >
                                    {/* Art */}
                                    <div className="w-12 h-12 rounded-lg overflow-hidden bg-bg-surface shrink-0 border border-border relative shadow-sm">
                                        {song.cover ? (
                                            <img src={song.cover} alt="" className="w-full h-full object-cover group-hover:scale-110 transition-transform duration-500" />
                                        ) : (
                                            <div className="w-full h-full flex items-center justify-center">
                                                <Music size={20} className="text-text-muted opacity-20" />
                                            </div>
                                        )}
                                    </div>

                                    {/* Song Info - fills remaining space */}
                                    <div className="flex-1 min-w-0">
                                        <div className="text-sm font-bold text-text-primary truncate">{song.title}</div>
                                        <div className="text-[11px] text-text-muted font-medium truncate leading-tight mt-0.5">{song.artist}</div>
                                    </div>

                                    {/* Right Block - fixed width, always aligned */}
                                    <div className="shrink-0 flex items-center gap-3">
                                        {/* Links */}
                                        <div className="flex items-center gap-1.5">
                                            {(song.sources || [{ name: song.source || 'iTunes', link: song.song_link }]).map(src => (
                                                src.link && (
                                                    <button
                                                        key={src.name}
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            openLink(src.link);
                                                        }}
                                                        className={`flex items-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] font-bold transition-all border ${src.name === 'Shazam'
                                                            ? 'bg-success/5 hover:bg-success/20 text-success border-success/10 hover:border-success/30'
                                                            : 'bg-accent/5 hover:bg-accent/20 text-accent border-accent/10 hover:border-accent/30'
                                                            }`}
                                                        title={t('identified_songs.listenOn', { source: src.name })}
                                                    >
                                                        <ExternalLink size={10} />
                                                        <span>{src.name}</span>
                                                    </button>
                                                )
                                            ))}
                                        </div>

                                        {/* Station & Time - fixed width */}
                                        <div className="flex flex-col gap-0.5 w-[110px]">
                                            <div className="flex items-center gap-1.5 text-[10px] text-text-muted font-semibold uppercase tracking-tight">
                                                <Radio size={11} className="shrink-0 opacity-60" /> <span className="truncate">{song.station_name}</span>
                                            </div>
                                            <div className="flex items-center gap-1.5 text-[10px] text-text-muted opacity-70">
                                                <Clock size={11} className="shrink-0" /> <span className="truncate">{relativeTime}</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>

            {/* Context Menu */}
            {ctxMenu && (
                <div
                    className="fixed z-[60] bg-bg-secondary border border-border shadow-2xl rounded-xl py-1.5 min-w-[140px] animate-in zoom-in-95 duration-100"
                    style={{ left: ctxMenu.x, top: ctxMenu.y }}
                    onClick={e => e.stopPropagation()}
                >
                    <button
                        onClick={() => handleDeleteSong(ctxMenu.song)}
                        className="w-full flex items-center gap-3 px-3 py-2 text-xs font-bold text-red-400 hover:bg-red-500/10 transition-colors"
                    >
                        <Trash2 size={14} /> {t('identified_songs.removeFromList')}
                    </button>
                </div>
            )}

            <ConfirmModal
                isOpen={isClearModalOpen}
                onClose={() => setIsClearModalOpen(false)}
                onConfirm={confirmClear}
                title={t('identified_songs.clearHistoryConfirmTitle')}
                message={t('identified_songs.clearHistoryConfirmDesc')}
                confirmText={t('identified_songs.yesClear')}
                cancelText={t('identified_songs.cancel')}
                variant="danger"
            />
            {httpModal}
        </div>
    );
}

