import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toAssetUrl } from '../../utils';
import { useTranslation } from 'react-i18next';
import TagInput from '../common/TagInput';
import { useNotification } from '../../contexts/NotificationProvider';
import i18n from '../../i18n';
import { Camera, Search, Upload, Trash2, X, Image as ImageIcon } from 'lucide-react';

export default function EditStationModal({ station, onClose, onSave, linkViewOpen, linkViewWidth }) {
    const { t } = useTranslation();
    const { notify } = useNotification();
    const [formData, setFormData] = useState({
        stationuuid: '',
        name: '',
        urlResolved: '',
        favicon: '',
        country: '',
        state: '',
        language: '',
        tags: '',
        codec: '',
        bitrate: 0,
        isFavorite: false
    });

    // ... image search state
    const [imageSearchResults, setImageSearchResults] = useState(null);
    const [isSearching, setIsSearching] = useState(false);
    const [imageSearchQuery, setImageSearchQuery] = useState('');
    const [searchTouched, setSearchTouched] = useState(false);

    // UI states for new image picker
    const [isImageDropdownOpen, setIsImageDropdownOpen] = useState(false);
    const [isSearchModalOpen, setIsSearchModalOpen] = useState(false);
    const imageDropdownRef = useRef(null);

    useEffect(() => {
        const handleClickOutside = (e) => {
            if (imageDropdownRef.current && !imageDropdownRef.current.contains(e.target)) {
                setIsImageDropdownOpen(false);
            }
        };
        if (isImageDropdownOpen) document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isImageDropdownOpen]);



    useEffect(() => {
        if (station) {
            setFormData({
                stationuuid: station.stationuuid || crypto.randomUUID(),
                name: station.name || '',
                urlResolved: station.urlResolved || station.url_resolved || station.url || '',
                favicon: station.favicon || '',
                country: station.country || '',
                state: station.state || '',
                language: station.language || '',
                tags: station.tags || '',
                codec: station.codec || '',
                bitrate: station.bitrate || 0,
                isFavorite: station.isFavorite || station.is_favorite || false
            });
            setImageSearchQuery((station.name || '') + ' logo');
            setSearchTouched(false);
        } else {
            setFormData({
                stationuuid: crypto.randomUUID(),
                name: '',
                urlResolved: '',
                favicon: '',
                country: '',
                state: '',
                language: '',
                tags: '',
                codec: '',
                bitrate: 0,
                isFavorite: false
            });
        }
    }, [station]);

    const handleSubmit = async (e) => {
        e.preventDefault();



        try {
            const dataToSave = { ...formData, codec: formData.codec || '', bitrate: Number(formData.bitrate) || 0, isFavorite: !!formData.isFavorite };
            await invoke('save_custom_station', { station: dataToSave });
            onSave(dataToSave);
        } catch (e) {
            notify({ type: 'error', message: t('editStation.errorAlert') + e });
        }
    };

    const handleSearchClick = async () => {
        const q = imageSearchQuery.trim() || (formData.name + ' logo');
        if (!q) {
            notify({ type: 'error', message: t('editStation.searchEmptyAlert') });
            return;
        }
        setIsSearching(true);
        try {
            const encoded = encodeURIComponent(q);
            const res = await invoke('search_images_internal', { encodedQuery: encoded });
            setImageSearchResults(res);
        } catch (e) {
            notify({ type: 'error', message: t('editStation.searchErrorAlert') + e });
        } finally {
            setIsSearching(false);
        }
    };

    return (
        <div
            className={`fixed inset-0 bg-black/50 z-[9999] flex items-center justify-center p-4 transition-all duration-300`}
            style={{ paddingRight: linkViewOpen ? linkViewWidth : 0 }}
        >
            <div className="bg-bg-secondary rounded-xl w-full max-w-xl flex flex-col shadow-2xl overflow-hidden border border-border max-h-[90vh]">
                <div className="px-5 py-4 border-b border-border flex justify-between items-center bg-bg-surface shrink-0">
                    <h2 className="text-lg font-bold">{station ? t('editStation.editTitle') : t('editStation.newTitle')}</h2>
                    <button onClick={onClose} className="px-5 py-2 text-sm font-semibold hover:text-text-primary transition-colors">{t('apiSearch.cancel')}</button>
                </div>

                <form onSubmit={handleSubmit} className="p-5 flex flex-col gap-4 overflow-y-auto">
                    <div className="flex gap-5 items-start">
                        {/* Image/Avatar - Left */}
                        <div className="relative group shrink-0" ref={imageDropdownRef}>
                            <div
                                onClick={() => setIsImageDropdownOpen(!isImageDropdownOpen)}
                                className="w-24 h-24 rounded-full bg-bg-surface border-2 border-border flex items-center justify-center cursor-pointer overflow-hidden group-hover:border-accent transition-colors relative shadow-lg"
                            >
                                {formData.favicon ? (
                                    <img src={toAssetUrl(formData.favicon)} alt="Station" className="w-full h-full object-cover" onError={(e) => { e.target.style.display = 'none'; e.target.nextSibling && (e.target.nextSibling.style.display = 'flex'); }} />
                                ) : null}
                                <div className={`w-full h-full bg-gradient-to-br from-accent/30 to-accent/10 items-center justify-center text-accent text-3xl font-bold ${formData.favicon ? 'hidden' : 'flex'}`}>
                                    {(formData.name || '?')[0].toLocaleUpperCase(i18n.language)}
                                </div>

                                <div className="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity text-white text-[10px] font-bold z-10">
                                    <Camera className="w-8 h-8" />
                                </div>
                            </div>

                            {isImageDropdownOpen && (
                                <div className="absolute top-full mt-2 left-0 bg-bg-surface border border-border rounded-lg shadow-xl py-1 z-50 w-48 animate-in fade-in zoom-in-95">
                                    <button type="button" onClick={() => { setIsImageDropdownOpen(false); setIsSearchModalOpen(true); handleSearchClick(); }} className="w-full text-left px-4 py-2 text-sm hover:bg-accent/10 hover:text-accent transition-colors flex items-center gap-2">
                                        <Search size={14} className="shrink-0" /> {t('editStation.searchInternet')}
                                    </button>
                                    <label className="w-full text-left px-4 py-2 text-sm hover:bg-accent/10 hover:text-accent transition-colors flex items-center gap-2 cursor-pointer whitespace-nowrap">
                                        <Upload size={14} className="shrink-0" /> {t('editStation.uploadNewImage')}
                                        <input type="file" accept="image/*" className="hidden" onChange={async (e) => {
                                            setIsImageDropdownOpen(false);
                                            const file = e.target.files[0];
                                            if (!file) return;
                                            try {
                                                const arrayBuffer = await file.arrayBuffer();
                                                const bytes = Array.from(new Uint8Array(arrayBuffer));
                                                const ext = file.name.split('.').pop() || 'png';
                                                const path = await invoke('upload_custom_favicon', { bytes, ext });
                                                setFormData({ ...formData, favicon: path });
                                            } catch (err) {
                                                notify({ type: 'error', message: t('editStation.imageUploadError') + err });
                                            }
                                        }} />
                                    </label>
                                    {formData.favicon && (
                                        <button type="button" onClick={() => { setIsImageDropdownOpen(false); setFormData({ ...formData, favicon: '' }); }} className="w-full text-left px-4 py-2 text-sm text-red-500 hover:bg-red-500/10 transition-colors flex items-center gap-2">
                                            <Trash2 size={14} className="shrink-0" /> {t('editStation.removeImage')}
                                        </button>
                                    )}
                                </div>
                            )}
                        </div>

                        {/* Details - Right */}
                        <div className="flex-1 flex flex-col gap-3">
                            <div>
                                <label className="block text-sm text-text-muted mb-1">{t('editStation.nameLabel')}</label>
                                <input required value={formData.name} onChange={e => {
                                    const newName = e.target.value;
                                    setFormData({ ...formData, name: newName });
                                    if (!searchTouched) setImageSearchQuery(newName ? newName + ' logo' : '');
                                }} className="w-full bg-bg-surface border border-border rounded-lg px-3 py-2 text-sm outline-none focus:border-accent shadow-inner" />
                            </div>

                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <label className="block text-sm text-text-muted mb-1">{t('editStation.countryLabel')}</label>
                                    <input value={formData.country} onChange={e => setFormData({ ...formData, country: e.target.value })} className="w-full bg-bg-surface border border-border rounded-lg px-3 py-2 text-sm outline-none focus:border-accent shadow-inner" />
                                </div>
                                <div>
                                    <label className="block text-sm text-text-muted mb-1">{t('editStation.stateLabel')}</label>
                                    <input value={formData.state} onChange={e => setFormData({ ...formData, state: e.target.value })} className="w-full bg-bg-surface border border-border rounded-lg px-3 py-2 text-sm outline-none focus:border-accent shadow-inner" />
                                </div>
                            </div>
                        </div>
                    </div>
                    <div>
                        <label className="block text-sm text-text-muted mb-1">{t('editStation.urlLabel')}</label>
                        <input required value={formData.urlResolved} onChange={e => {
                            setFormData({ ...formData, urlResolved: e.target.value });
                        }} className="w-full bg-bg-surface border border-border rounded-lg px-3 py-2 text-sm outline-none focus:border-accent shadow-inner" />
                    </div>


                    <div className="grid grid-cols-2 gap-4">
                        <div>
                            <label className="block text-sm text-text-muted mb-1">{t('editStation.languageLabel')}</label>
                            <TagInput value={formData.language} onChange={val => setFormData({ ...formData, language: val })} placeholder={t('editStation.languagePlaceholder')} className="w-full bg-bg-surface border border-border rounded-lg focus-within:border-accent" />
                        </div>
                        <div>
                            <label className="block text-sm text-text-muted mb-1">{t('editStation.tagsLabel')}</label>
                            <TagInput value={formData.tags} onChange={val => setFormData({ ...formData, tags: val })} placeholder={t('editStation.tagsPlaceholder')} className="w-full bg-bg-surface border border-border rounded-lg focus-within:border-accent" />
                        </div>
                        <div>
                            <label className="flex items-center gap-2 cursor-pointer select-none">
                                <input type="checkbox" checked={formData.isFavorite} onChange={e => setFormData({ ...formData, isFavorite: e.target.checked })} className="w-4 h-4 accent-accent cursor-pointer" />
                                <span className="text-sm font-semibold text-text-primary">{t('editStation.addToFavorites')}</span>
                            </label>
                        </div>
                    </div>

                    <button type="submit" className="w-full py-2.5 mt-2 bg-accent hover:bg-accent-hover text-white rounded-lg font-bold text-sm transition-colors flex items-center justify-center gap-2">
                        {t('editStation.save')}
                    </button>
                </form>
            </div>

            {/* Search Modal Nested / Overlay */}
            {isSearchModalOpen && (
                <div className="fixed inset-0 bg-black/80 z-[10000] flex items-center justify-center p-4 animate-in fade-in duration-200">
                    <div className="bg-bg-secondary w-full max-w-lg rounded-xl flex flex-col shadow-2xl overflow-hidden border border-border max-h-[80vh]">
                        <div className="px-5 py-4 border-b border-border flex justify-between items-center bg-bg-surface shrink-0">
                            <h3 className="font-bold flex items-center gap-2"><ImageIcon size={18} /> {t('editStation.imageSearchModalTitle')}</h3>
                            <button type="button" onClick={() => setIsSearchModalOpen(false)} className="text-text-muted hover:text-text-primary"><X size={20} /></button>
                        </div>
                        <div className="p-5 flex flex-col gap-4 overflow-hidden">
                            <div className="flex gap-2">
                                <input
                                    value={imageSearchQuery}
                                    onChange={e => { setSearchTouched(true); setImageSearchQuery(e.target.value); }}
                                    placeholder={t('editStation.searchImagePlaceholder')}
                                    className="flex-1 bg-bg-surface border border-border rounded-lg px-4 py-2.5 text-sm outline-none focus:border-accent shadow-inner"
                                    onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); handleSearchClick(); } }}
                                    autoFocus
                                />
                                <button type="button" onClick={handleSearchClick} disabled={isSearching} className="bg-accent hover:bg-accent-hover disabled:opacity-50 text-white px-5 py-2.5 rounded-lg text-sm font-bold transition-colors shrink-0 shadow-lg">
                                    {isSearching ? t('apiSearch.searching') : t('editStation.searchButton')}
                                </button>
                            </div>

                            <div className="flex-1 bg-bg-surface border border-border rounded-lg p-3 min-h-[250px] overflow-y-auto custom-scrollbar">
                                {imageSearchResults === null ? (
                                    <div className="text-center text-sm text-text-muted py-16 opacity-50 flex flex-col items-center gap-3">
                                        <Search size={32} />
                                        <span>{t('apiSearch.startSearching')}</span>
                                    </div>
                                ) : imageSearchResults.length === 0 ? (
                                    <div className="text-center text-sm text-text-muted py-16">{t('editStation.imageNotFound')}</div>
                                ) : (
                                    <div className="grid grid-cols-4 gap-3">
                                        {imageSearchResults.map((url, i) => (
                                            <div key={i} className="aspect-square bg-black/20 rounded-lg shadow cursor-pointer overflow-hidden border-2 border-transparent hover:border-accent transition-all hover:scale-105 hover:z-10 hover:shadow-xl relative group" onClick={async () => {
                                                try {
                                                    const e = document.getElementById(`modal-img-dl-${i}`);
                                                    if (e) e.classList.remove('hidden');
                                                    const localPath = await invoke('download_custom_favicon', { url });
                                                    setFormData({ ...formData, favicon: localPath });
                                                    setImageSearchResults(null);
                                                    setIsSearchModalOpen(false);
                                                } catch (err) {
                                                    notify({ type: 'error', message: t('editStation.downloadError') + err });
                                                } finally {
                                                    const e = document.getElementById(`modal-img-dl-${i}`);
                                                    if (e) e.classList.add('hidden');
                                                }
                                            }}>
                                                <img src={url} alt="" className="w-full h-full object-cover bg-black/10" />
                                                <div id={`modal-img-dl-${i}`} className="hidden absolute inset-0 bg-black/60 flex items-center justify-center text-[10px] text-white font-bold backdrop-blur-sm">{t('editStation.downloading')}</div>
                                            </div>
                                        ))}
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div >
    );
}
