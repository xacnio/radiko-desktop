import { Settings, RefreshCw, Info, Shield, Github, Globe, Key, Zap, Type, ChevronRight, Heart, ExternalLink, Code, Download, Upload, Check, FileText, X, Plus, Save, Monitor, Moon, Sun } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useNotification } from '../contexts/NotificationProvider';
import { availableLanguages } from '../i18n';
import ReactMarkdown from 'react-markdown';

// Clean markdown payload from CI artifacts table
const cleanMarkdown = (text) => {
    if (!text) return '';

    // Attempt to extract only the "What's New" section
    const whatsNewMatch = text.match(/### 📝 What's New\s+([\s\S]*?)(?=\n---|###|$)/);
    if (whatsNewMatch && whatsNewMatch[1]) {
        return whatsNewMatch[1].trim();
    }

    // Fallback: strip "Download Links" and CI footer
    let result = text.replace(/### 📦 Download Links[\s\S]*?(?=\n### |$)/, '');
    result = result.replace(/\n---[\s\S]*$/, '');
    return result.trim() || text;
};

// Fallback for version if plugin fails
const getAppVersion = async () => {
    try {
        const { getVersion } = await import('@tauri-apps/api/app');
        return await getVersion();
    } catch (e) {
        return '0.1.0';
    }
};

export default function SettingsView({
    onResetSetup,
    initialTab = 'general',
    autoIdentifyCooldownSuccess,
    setAutoIdentifyCooldownSuccess,
    autoIdentifyCooldownFail,
    setAutoIdentifyCooldownFail,
    onImportSuccess,
    theme,
    setTheme,
    accentColor,
    setAccentColor,
    mixAccent,
    setMixAccent,
    minimizeToTray,
    setMinimizeToTray,
    closeToTray,
    setCloseToTray,
    skipAds,
    setSkipAds
}) {
    const { t, i18n } = useTranslation();
    const { notify } = useNotification();
    const [activeTab, setActiveTab] = useState(initialTab);
    const [windowWidth, setWindowWidth] = useState(window.innerWidth);

    useEffect(() => {
        const handleResize = () => setWindowWidth(window.innerWidth);
        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, []);

    const isSmall = windowWidth < 800;

    const [isProcessing, setIsProcessing] = useState(false);
    const [importProgress, setImportProgress] = useState(0);
    const [exportProgress, setExportProgress] = useState(0);
    const [showExportOptions, setShowExportOptions] = useState(false);

    // Export options
    const [exportRadios, setExportRadios] = useState(true);
    const [exportSongs, setExportSongs] = useState(true);
    const [exportImages, setExportImages] = useState(true);

    // Import options
    const [importMetadata, setImportMetadata] = useState(null); // { radio_count, song_count, has_images, path }
    const [importRadios, setImportRadios] = useState(true);
    const [importSongs, setImportSongs] = useState(true);
    const [importImages, setImportImages] = useState(true);

    // Update state
    const [appVersion, setAppVersion] = useState('0.1.0');
    const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
    const [updateStatus, setUpdateStatus] = useState('idle'); // idle, loading, found, error, downloading, ready
    const [updateResult, setUpdateResult] = useState(null);
    const [releaseNotes, setReleaseNotes] = useState(null);
    const [downloadProgress, setDownloadProgress] = useState(0);
    const [autoUpdate, setAutoUpdate] = useState(() => localStorage.getItem('auto_update') !== 'false');
    const [isHistoryOpen, setIsHistoryOpen] = useState(false);
    const [releaseHistory, setReleaseHistory] = useState(null);
    const [isLoadingHistory, setIsLoadingHistory] = useState(false);

    // Audio devices
    const [audioDevices, setAudioDevices] = useState([]);
    const [selectedDevice, setSelectedDevice] = useState('');

    useEffect(() => {
        if (activeTab === 'general') {
            invoke('get_audio_devices').then(setAudioDevices).catch(console.error);
            invoke('get_settings').then(s => {
                setSelectedDevice(s.output_device || '');
            }).catch(console.error);
        }
    }, [activeTab]);

    useEffect(() => {
        getAppVersion().then(setAppVersion).catch(console.error);
    }, []);

    useEffect(() => {
        localStorage.setItem('auto_update', autoUpdate);
    }, [autoUpdate]);

    useEffect(() => {
        if (!updateResult?.version) {
            setReleaseNotes(null);
            return;
        }

        // Show whatever we have from updater first
        if (updateResult.body) setReleaseNotes(updateResult.body);

        // Fetch fresh metadata from GitHub in the background
        fetch(`https://api.github.com/repos/xacnio/radiko-desktop/releases/tags/v${updateResult.version}`)
            .then(res => res.json())
            .then(data => {
                if (data && data.body) {
                    setReleaseNotes(data.body);
                }
            })
            .catch(e => console.error('Failed to fetch rich release notes:', e));
    }, [updateResult]);

    useEffect(() => {
        if (isHistoryOpen && !releaseHistory && !isLoadingHistory) {
            setIsLoadingHistory(true);
            fetch('https://api.github.com/repos/xacnio/radiko-desktop/releases')
                .then(res => res.json())
                .then(data => {
                    if (Array.isArray(data)) {
                        setReleaseHistory(data);
                    } else {
                        setReleaseHistory([]);
                    }
                })
                .catch(e => {
                    console.error('Failed to fetch release history', e);
                    setReleaseHistory([]);
                })
                .finally(() => {
                    setIsLoadingHistory(false);
                });
        }
    }, [isHistoryOpen, releaseHistory, isLoadingHistory]);

    useEffect(() => {
        const handleTrigger = (e) => {
            const update = e.detail;
            setActiveTab('about');
            setUpdateResult(update);
            setUpdateStatus('found');

            setTimeout(() => {
                const aboutEl = document.getElementById('about-section');
                if (aboutEl) aboutEl.scrollIntoView({ behavior: 'smooth' });
            }, 100);
        };

        window.addEventListener('rx_trigger_update', handleTrigger);
        return () => window.removeEventListener('rx_trigger_update', handleTrigger);
    }, [notify]);


    // Dependency logic: If radios are off, images MUST be off
    useEffect(() => {
        if (!exportRadios) setExportImages(false);
    }, [exportRadios]);

    useEffect(() => {
        if (!importRadios) setImportImages(false);
    }, [importRadios]);

    const menuItems = [
        { id: 'general', label: t('settings.tabGeneral'), icon: Type, color: 'text-accent' },
        { id: 'reset', label: t('settings.tabReset'), icon: RefreshCw, color: 'text-red-400' },
        { id: 'advanced', label: t('settings.tabAdvanced'), icon: Zap, color: 'text-accent' },
        { id: 'about', label: t('settings.tabAbout'), icon: Info, color: 'text-accent' },
    ];

    const handleExport = async () => {
        if (!exportRadios && !exportSongs && !exportImages) {
            notify({
                type: 'error',
                title: t('app.error'),
                message: t('settings.selectOneItem')
            });
            return;
        }

        setIsProcessing(true);
        setExportProgress(0);
        try {
            await invoke('export_backup', {
                options: {
                    include_radios: exportRadios,
                    include_songs: exportSongs,
                    include_images: exportImages
                }
            });
            notify({
                type: 'success',
                title: t('settings.exportSuccessTitle'),
                message: t('settings.exportSuccessMsg')
            });
        } catch (e) {
            console.error('Export failed:', e);
            notify({
                type: 'error',
                title: t('settings.exportErrorTitle'),
                message: e.toString()
            });
        } finally {
            setIsProcessing(false);
        }
    };

    const handleAnalyzeImport = async () => {
        setIsProcessing(true);
        try {
            const res = await invoke('analyze_backup');
            if (res) {
                setImportMetadata(res);
                setImportRadios(res.radio_count > 0);
                setImportSongs(res.song_count > 0);
                setImportImages(res.has_images);
            }
        } catch (e) {
            console.error('Analysis failed:', e);
            notify({
                type: 'error',
                title: t('settings.analysisErrorTitle'),
                message: t('settings.analysisErrorMsg') + e
            });
        } finally {
            setIsProcessing(false);
        }
    };

    useEffect(() => {
        const unimport = listen('import-progress', (event) => {
            setImportProgress(event.payload);
        });
        const unexport = listen('export-progress', (event) => {
            setExportProgress(event.payload);
        });
        return () => {
            unimport.then(f => f());
            unexport.then(f => f());
        };
    }, []);

    const handleImport = async () => {
        if (!importMetadata) return;
        if (!importRadios && !importSongs && !importImages) {
            notify({
                type: 'error',
                title: t('app.error'),
                message: t('settings.selectOneItem')
            });
            return;
        }

        setIsProcessing(true);
        setImportProgress(0);
        try {
            await invoke('import_backup', {
                path: importMetadata.path,
                options: {
                    include_radios: importRadios,
                    include_songs: importSongs,
                    include_images: importImages
                }
            });
            notify({
                type: 'success',
                title: t('settings.importSuccessTitle'),
                message: t('settings.importSuccessMsg')
            });
            onImportSuccess(); // Refresh and navigate back
            setImportMetadata(null);
        } catch (e) {
            console.error('Import failed:', e);
            notify({
                type: 'error',
                title: t('settings.importErrorTitle'),
                message: e.toString()
            });
        } finally {
            setIsProcessing(false);
        }
    };

    const Checkbox = ({ label, checked, onChange, disabled = false }) => (
        <label className={`flex items-center gap-3 py-1.5 transition-opacity ${disabled ? 'opacity-30 cursor-not-allowed' : 'cursor-pointer group'}`}>
            <div className="relative flex items-center justify-center">
                <input
                    type="checkbox"
                    checked={checked}
                    onChange={(e) => !disabled && onChange(e.target.checked)}
                    disabled={disabled}
                    className="sr-only"
                />
                <div className={`w-5 h-5 border-2 rounded-md transition-all duration-200 flex items-center justify-center
                    ${checked
                        ? `bg-accent border-accent`
                        : 'border-border/50 bg-bg-surface/30 hover:border-border'}`}
                >
                    {checked && <Check size={14} className="text-bg-primary stroke-[3px]" />}
                </div>
            </div>
            <span className={`text-[11px] font-bold transition-colors ${checked ? 'text-text-primary' : 'text-text-muted group-hover:text-text-primary'}`}>
                {label}
            </span>
        </label>
    );

    const renderContent = () => {
        switch (activeTab) {
            case 'general':
                return (
                    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                        <div className="flex flex-col gap-1">
                            <h3 className="text-lg font-bold text-text-primary tracking-tight">{t('settings.app')}</h3>
                            <p className="text-xs text-text-muted">{t('settings.adjustAppExperience')}</p>
                        </div>

                        <div className="bg-bg-secondary/50 border border-border/50 rounded-2xl p-6 space-y-6">
                            <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
                                <div className="space-y-1">
                                    <h4 className="text-sm font-bold text-text-primary">{t('settings.language')}</h4>
                                    <p className="text-xs text-text-muted">{t('settings.languageDesc')}</p>
                                </div>
                                <div className="relative group w-full md:w-auto">
                                    <select
                                        className="appearance-none bg-bg-surface border border-border text-text-primary text-sm rounded-xl focus:ring-2 focus:ring-accent focus:border-accent block pl-4 pr-10 py-2.5 cursor-pointer hover:bg-bg-surface-hover transition-all w-full md:min-w-[140px]"
                                        value={i18n.language.substring(0, 2).toLowerCase()}
                                        onChange={(e) => i18n.changeLanguage(e.target.value)}
                                    >
                                        {availableLanguages.map((lang) => (
                                            <option key={lang.code} value={lang.code}>{lang.name}</option>
                                        ))}
                                    </select>
                                    <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-text-muted group-hover:text-accent transition-colors">
                                        <ChevronRight size={14} className="rotate-90" />
                                    </div>
                                </div>
                            </div>

                            <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pt-4 border-t border-border/30">
                                <div className="space-y-1">
                                    <h4 className="text-sm font-bold text-text-primary">{t('settings.outputDevice') || 'Audio Output Device'}</h4>
                                    <p className="text-xs text-text-muted">{t('settings.outputDeviceDesc') || 'Select the device to play sound from'}</p>
                                </div>
                                <div className="relative group w-full md:w-auto">
                                    <select
                                        className="appearance-none bg-bg-surface border border-border text-text-primary text-sm rounded-xl focus:ring-2 focus:ring-accent focus:border-accent block pl-4 pr-10 py-2.5 cursor-pointer hover:bg-bg-surface-hover transition-all w-full md:min-w-[140px] max-w-[250px] truncate"
                                        value={selectedDevice}
                                        onChange={(e) => {
                                            const dev = e.target.value;
                                            setSelectedDevice(dev);
                                            invoke('set_audio_device', { device: dev }).catch(console.error);
                                        }}
                                    >
                                        <option value="">{t('settings.defaultDevice') || 'Default Device'}</option>
                                        {audioDevices.map((dev, idx) => (
                                            <option key={idx} value={dev}>{dev}</option>
                                        ))}
                                    </select>
                                    <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-text-muted group-hover:text-accent transition-colors">
                                        <ChevronRight size={14} className="rotate-90" />
                                    </div>
                                </div>
                            </div>

                            <div className="flex flex-col gap-4 pt-4 border-t border-border/30">
                                <div className="space-y-1">
                                    <h4 className="text-sm font-bold text-text-primary">{t('settings.theme')}</h4>
                                    <p className="text-xs text-text-muted">{t('settings.themeDesc')}</p>
                                </div>
                                <div className="grid grid-cols-3 gap-2">
                                    {[
                                        { id: 'system', icon: Monitor, label: t('settings.themeSystemShort') || t('settings.themeSystem') },
                                        { id: 'dark', icon: Moon, label: t('settings.themeDark') },
                                        { id: 'light', icon: Sun, label: t('settings.themeLight') }
                                    ].map(opt => {
                                        const Icon = opt.icon;
                                        const isActive = theme === opt.id;
                                        return (
                                            <button
                                                key={opt.id}
                                                onClick={() => setTheme(opt.id)}
                                                className={`flex flex-col items-center justify-center gap-2 p-3 rounded-xl border-2 transition-all duration-200 cursor-pointer
                                                    ${isActive
                                                        ? 'bg-accent/10 border-accent text-accent'
                                                        : 'bg-bg-surface border-border/50 text-text-muted hover:border-border hover:text-text-primary'}`}
                                            >
                                                <Icon size={18} strokeWidth={isActive ? 2.5 : 2} />
                                                <span className="text-[9px] font-black uppercase tracking-wider text-center line-clamp-1">{opt.label}</span>
                                            </button>
                                        );
                                    })}
                                </div>
                            </div>

                            <div className="flex flex-col gap-4 pt-4 border-t border-border/30">
                                <div className="space-y-1">
                                    <h4 className="text-sm font-bold text-text-primary">{t('settings.accentColor') || 'Accent Color'}</h4>
                                    <p className="text-xs text-text-muted">{t('settings.accentColorDesc') || 'Change the highlight color of the app'}</p>
                                </div>
                                <div className="flex flex-wrap items-center gap-3 py-2">
                                    {[
                                        { id: 'green', color: '#1db954', label: 'Green' },
                                        { id: 'blue', color: '#3b82f6', label: 'Blue' },
                                        { id: 'purple', color: '#a855f7', label: 'Purple' },
                                        { id: 'yellow', color: '#eab308', label: 'Yellow' },
                                        { id: 'orange', color: '#f97316', label: 'Orange' },
                                        { id: 'red', color: '#ef4444', label: 'Red' },
                                        { id: 'pink', color: '#ec4899', label: 'Pink' },
                                        { id: 'sky', color: '#0ea5e9', label: 'Sky' },
                                        { id: 'indigo', color: '#6366f1', label: 'Indigo' },
                                        { id: 'teal', color: '#14b8a6', label: 'Teal' },
                                        { id: 'slate', color: '#64748b', label: 'Slate' }
                                    ].map(opt => (
                                        <button
                                            key={opt.id}
                                            onClick={() => setAccentColor(opt.id)}
                                            className={`w-8 h-8 rounded-full border-2 transition-all relative flex items-center justify-center p-0.5 cursor-pointer
                                                ${accentColor === opt.id ? 'border-text-primary scale-110 shadow-lg' : 'border-transparent hover:scale-110 opacity-70 hover:opacity-100'}`}
                                            title={opt.label}
                                        >
                                            <div className="w-full h-full rounded-full shadow-inner" style={{ backgroundColor: opt.color }} />
                                            {accentColor === opt.id && (
                                                <div className="absolute inset-0 flex items-center justify-center">
                                                    <Check size={14} className="text-white stroke-[4px] drop-shadow-md" />
                                                </div>
                                            )}
                                        </button>
                                    ))}
                                </div>
                            </div>

                            <div className="flex flex-col gap-4 pt-4 border-t border-border/30">
                                <div className="flex items-center justify-between gap-4 py-1">
                                    <div className="space-y-1">
                                        <h4 className="text-sm font-bold text-text-primary">{t('settings.mixAccent')}</h4>
                                        <p className="text-xs text-text-muted">{t('settings.mixAccentDesc')}</p>
                                    </div>
                                    <button
                                        onClick={() => setMixAccent(!mixAccent)}
                                        className={`w-10 h-5 rounded-full transition-all relative p-1 cursor-pointer outline-none shrink-0
                                            ${mixAccent ? 'bg-accent shadow-lg shadow-accent/20' : 'bg-bg-surface-active border border-border/50'}`}
                                    >
                                        <div className={`w-3 h-3 bg-white rounded-full transition-all duration-300 shadow-sm ${mixAccent ? 'translate-x-5' : 'translate-x-0'}`} />
                                    </button>
                                </div>

                                <div className="flex items-center justify-between gap-4 py-1 border-t border-border/30 pt-4">
                                    <div className="space-y-1">
                                        <h4 className="text-sm font-bold text-text-primary">{t('settings.minimizeToTray')}</h4>
                                        <p className="text-xs text-text-muted">{t('settings.minimizeToTrayDesc')}</p>
                                    </div>
                                    <button
                                        onClick={() => setMinimizeToTray(!minimizeToTray)}
                                        className={`w-10 h-5 rounded-full transition-all relative p-1 cursor-pointer outline-none shrink-0
                                            ${minimizeToTray ? 'bg-accent shadow-lg shadow-accent/20' : 'bg-bg-surface-active border border-border/50'}`}
                                    >
                                        <div className={`w-3 h-3 bg-white rounded-full transition-all duration-300 shadow-sm ${minimizeToTray ? 'translate-x-5' : 'translate-x-0'}`} />
                                    </button>
                                </div>

                                <div className="flex items-center justify-between gap-4 py-1 border-t border-border/30 pt-4">
                                    <div className="space-y-1">
                                        <h4 className="text-sm font-bold text-text-primary">{t('settings.closeToTray')}</h4>
                                        <p className="text-xs text-text-muted">{t('settings.closeToTrayDesc')}</p>
                                    </div>
                                    <button
                                        onClick={() => setCloseToTray(!closeToTray)}
                                        className={`w-10 h-5 rounded-full transition-all relative p-1 cursor-pointer outline-none shrink-0
                                            ${closeToTray ? 'bg-accent shadow-lg shadow-accent/20' : 'bg-bg-surface-active border border-border/50'}`}
                                    >
                                        <div className={`w-3 h-3 bg-white rounded-full transition-all duration-300 shadow-sm ${closeToTray ? 'translate-x-5' : 'translate-x-0'}`} />
                                    </button>
                                </div>

                                <div className="flex items-center justify-between gap-4 py-1 border-t border-border/30 pt-4">
                                    <div className="space-y-1">
                                        <h4 className="text-sm font-bold text-text-primary">{t('settings.skipAds')}</h4>
                                        <p className="text-xs text-text-muted">{t('settings.skipAdsDesc')}</p>
                                        <p className="text-[10px] text-accent/80 font-medium italic">{t('settings.skipAdsWarning')}</p>
                                    </div>
                                    <button
                                        onClick={() => {
                                            const newVal = !skipAds;
                                            setSkipAds(newVal);
                                            invoke('save_skip_ads', { skipAds: newVal }).catch(console.error);
                                        }}
                                        className={`w-10 h-5 rounded-full transition-all relative p-1 cursor-pointer outline-none shrink-0
                                            ${skipAds ? 'bg-accent shadow-lg shadow-accent/20' : 'bg-bg-surface-active border border-border/50'}`}
                                    >
                                        <div className={`w-3 h-3 bg-white rounded-full transition-all duration-300 shadow-sm ${skipAds ? 'translate-x-5' : 'translate-x-0'}`} />
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                );
            case 'advanced':
                return (
                    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                        <div className="flex flex-col gap-1">
                            <h3 className="text-lg font-bold text-text-primary tracking-tight">{t('settings.autoIdentifyAdv')}</h3>
                            <p className="text-xs text-text-muted">{t('settings.fineTuneShazam')}</p>
                        </div>

                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                            {/* Success Cooldown */}
                            <div className="bg-bg-secondary/40 border border-border/40 rounded-xl p-4 space-y-3">
                                <div className="flex justify-between items-center gap-2">
                                    <h4 className="text-[10px] font-bold text-text-muted uppercase tracking-tight truncate flex-1" title={t('settings.successCooldown')}>
                                        {t('settings.successCooldown')}
                                    </h4>
                                    <div className="text-accent font-mono text-[10px] font-black bg-accent/10 px-2 py-0.5 rounded-md border border-accent/10 shrink-0">
                                        {autoIdentifyCooldownSuccess}s
                                    </div>
                                </div>
                                <div className="space-y-1.5">
                                    <input
                                        type="range"
                                        min="10"
                                        max="120"
                                        step="5"
                                        value={autoIdentifyCooldownSuccess}
                                        onChange={(e) => setAutoIdentifyCooldownSuccess(parseInt(e.target.value))}
                                        className="w-full h-1 bg-bg-surface-active rounded-full appearance-none cursor-pointer accent-accent"
                                    />
                                    <div className="flex justify-between text-[7px] text-text-muted/50 font-bold uppercase px-0.5">
                                        <span>10s</span>
                                        <span>{t('settings.balanced')}</span>
                                        <span>120s</span>
                                    </div>
                                </div>
                            </div>

                            {/* Fail Cooldown */}
                            <div className="bg-bg-secondary/40 border border-border/40 rounded-xl p-4 space-y-3">
                                <div className="flex justify-between items-center gap-2">
                                    <h4 className="text-[10px] font-bold text-text-muted uppercase tracking-tight truncate flex-1" title={t('settings.failCooldown')}>
                                        {t('settings.failCooldown')}
                                    </h4>
                                    <div className="text-warning font-mono text-[10px] font-black bg-warning/10 px-2 py-0.5 rounded-md border border-warning/10 shrink-0">
                                        {autoIdentifyCooldownFail}s
                                    </div>
                                </div>
                                <div className="space-y-1.5">
                                    <input
                                        type="range"
                                        min="5"
                                        max="60"
                                        step="5"
                                        value={autoIdentifyCooldownFail}
                                        onChange={(e) => setAutoIdentifyCooldownFail(parseInt(e.target.value))}
                                        className="w-full h-1 bg-bg-surface-active rounded-full appearance-none cursor-pointer accent-warning"
                                    />
                                    <div className="flex justify-between text-[7px] text-text-muted/50 font-bold uppercase px-0.5">
                                        <span>5s</span>
                                        <span>{t('settings.rare')}</span>
                                        <span>60s</span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                );
            case 'reset':
                return (
                    <div className="space-y-6">
                        <div className="flex flex-col gap-1">
                            <h3 className="text-lg font-bold text-text-primary tracking-tight">{t('settings.tabReset')}</h3>
                            <p className="text-xs text-text-muted">{t('settings.manageData')}</p>
                        </div>

                        {/* Backup & Restore Section */}
                        <div className="space-y-4">
                            <div className="flex items-center gap-2 px-2 text-text-muted/40">
                                <div className="h-px flex-1 bg-border/30" />
                                <span className="text-[10px] font-black uppercase tracking-widest">{t('settings.backupRestore')}</span>
                                <div className="h-px flex-1 bg-border/30" />
                            </div>

                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                {/* Export Box */}
                                <div className="bg-bg-secondary rounded-2xl p-6 space-y-5 flex flex-col min-h-[280px]">
                                    {!showExportOptions ? (
                                        <div className="flex flex-col h-full space-y-5">
                                            <div className="space-y-1 flex-1">
                                                <h4 className="text-sm font-bold text-text-primary flex items-center gap-2">
                                                    <Download size={14} className="text-accent" /> {t('settings.exportBackup')}
                                                </h4>
                                                <p className="text-[11px] text-text-muted leading-relaxed">
                                                    {t('settings.exportDesc')}
                                                </p>
                                            </div>
                                            <div className="h-[90px] bg-bg-surface/30 border border-border/30 rounded-xl flex items-center justify-center text-accent/20">
                                                <Download size={28} />
                                            </div>
                                            <button
                                                onClick={() => setShowExportOptions(true)}
                                                className="w-full py-2.5 bg-accent/10 hover:bg-accent text-accent hover:text-bg-primary rounded-xl font-bold text-xs transition-all flex items-center justify-center gap-2 active:scale-95"
                                            >
                                                <Download size={14} />
                                                {t('settings.prepareExport')}
                                            </button>
                                        </div>
                                    ) : (
                                        <div className="flex flex-col h-full space-y-5">
                                            <div className="flex items-center justify-between">
                                                <h4 className="text-[10px] font-black uppercase tracking-widest text-accent">{t('settings.exportBackup')}</h4>
                                                <button
                                                    onClick={() => setShowExportOptions(false)}
                                                    className="p-1.5 hover:bg-bg-surface-hover rounded-lg text-text-muted transition-colors"
                                                >
                                                    <X size={14} />
                                                </button>
                                            </div>

                                            <div className="bg-bg-surface/50 border border-border/20 rounded-xl p-3 px-4 space-y-1">
                                                <Checkbox
                                                    label={t('settings.includeRadios')}
                                                    checked={exportRadios}
                                                    onChange={setExportRadios}
                                                />
                                                <Checkbox
                                                    label={t('settings.includeImages')}
                                                    checked={exportImages}
                                                    onChange={setExportImages}
                                                    disabled={!exportRadios}
                                                />
                                                <Checkbox
                                                    label={t('settings.includeSongs')}
                                                    checked={exportSongs}
                                                    onChange={setExportSongs}
                                                />
                                            </div>

                                            <button
                                                onClick={handleExport}
                                                disabled={isProcessing}
                                                className="w-full py-2.5 bg-accent hover:bg-accent-hover text-bg-primary rounded-xl font-bold text-xs transition-all flex items-center justify-center gap-2 shadow-lg shadow-accent/20 active:scale-95 disabled:opacity-50"
                                            >
                                                {isProcessing ? <RefreshCw size={14} className="animate-spin" /> : <Save size={14} />}
                                                {isProcessing ? `${exportProgress}%` : t('settings.saveBackup')}
                                            </button>

                                            {isProcessing && (
                                                <div className="w-full h-1 bg-bg-surface-active/50 rounded-full overflow-hidden">
                                                    <div
                                                        className="h-full bg-accent transition-all duration-300 ease-out"
                                                        style={{ width: `${exportProgress}%` }}
                                                    />
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>

                                {/* Import Box */}
                                <div className="bg-bg-secondary rounded-2xl p-6 space-y-5 flex flex-col relative overflow-hidden">
                                    {!importMetadata ? (
                                        <div className="flex flex-col h-full space-y-5">
                                            <div className="space-y-1 flex-1">
                                                <h4 className="text-sm font-bold text-text-primary flex items-center gap-2">
                                                    <Upload size={14} className="text-blue-400" /> {t('settings.importBackup')}
                                                </h4>
                                                <p className="text-[11px] text-text-muted leading-relaxed">
                                                    {t('settings.importDesc')}
                                                </p>
                                            </div>
                                            <div className="h-[90px] bg-bg-surface/30 border border-border/30 rounded-xl flex items-center justify-center text-blue-400/20">
                                                <Upload size={28} />
                                            </div>
                                            <button
                                                onClick={handleAnalyzeImport}
                                                disabled={isProcessing}
                                                className="w-full py-2.5 bg-blue-400/10 hover:bg-blue-400 text-blue-400 hover:text-bg-primary rounded-xl font-bold text-xs transition-all flex items-center justify-center gap-2 hover:shadow-lg hover:shadow-blue-400/20 active:scale-95 disabled:opacity-50"
                                            >
                                                {isProcessing ? <RefreshCw size={14} className="animate-spin" /> : <Plus size={14} />}
                                                {t('settings.selectFile')}
                                            </button>
                                        </div>
                                    ) : (
                                        <div className="flex flex-col h-full space-y-4">
                                            <div className="flex items-center justify-between">
                                                <h4 className="text-[10px] font-black uppercase tracking-widest text-blue-400">{t('settings.backupAnalysis')}</h4>
                                                <button
                                                    onClick={() => setImportMetadata(null)}
                                                    className="p-1.5 hover:bg-bg-surface-hover rounded-lg text-text-muted transition-colors"
                                                >
                                                    <X size={14} />
                                                </button>
                                            </div>

                                            <div className="bg-bg-surface/50 border border-border/20 rounded-xl p-3 px-4 space-y-1">
                                                <Checkbox
                                                    label={`${t('settings.includeRadios')} (${importMetadata.radio_count})`}
                                                    checked={importRadios}
                                                    onChange={setImportRadios}
                                                    disabled={importMetadata.radio_count === 0}
                                                />
                                                <Checkbox
                                                    label={t('settings.includeImages')}
                                                    checked={importImages}
                                                    onChange={setImportImages}
                                                    disabled={!importRadios || !importMetadata.has_images}
                                                />
                                                <Checkbox
                                                    label={`${t('settings.includeSongs')} (${importMetadata.song_count})`}
                                                    checked={importSongs}
                                                    onChange={setImportSongs}
                                                    disabled={importMetadata.song_count === 0}
                                                />
                                            </div>

                                            <button
                                                onClick={handleImport}
                                                disabled={isProcessing}
                                                className="w-full py-2.5 bg-blue-400 hover:bg-blue-500 text-bg-primary rounded-xl font-bold text-xs transition-all flex items-center justify-center gap-2 shadow-lg shadow-blue-400/20 active:scale-95 disabled:opacity-50"
                                            >
                                                {isProcessing ? <RefreshCw size={14} className="animate-spin" /> : <Check size={14} strokeWidth={3} />}
                                                {isProcessing ? `${importProgress}%` : t('settings.startRestore')}
                                            </button>

                                            {isProcessing && (
                                                <div className="w-full h-1 bg-bg-surface-active/50 rounded-full overflow-hidden">
                                                    <div
                                                        className="h-full bg-blue-400 transition-all duration-300 ease-out"
                                                        style={{ width: `${importProgress}%` }}
                                                    />
                                                </div>
                                            )}

                                            <p className="text-[10px] text-blue-400/80 text-center truncate italic font-medium">
                                                {importMetadata.path.split('\\').pop().split('/').pop()}
                                            </p>
                                        </div>
                                    )}
                                </div>
                            </div>
                        </div>

                        {/* Dangerous Zone */}
                        <div className="space-y-4 pt-4">
                            <div className="flex items-center gap-2 px-2 text-red-500/20">
                                <div className="h-px flex-1 bg-red-500/10" />
                                <span className="text-[10px] font-black uppercase tracking-widest text-red-500/40">{t('settings.dangerZone')}</span>
                                <div className="h-px flex-1 bg-red-500/10" />
                            </div>

                            <div className="bg-red-500/5 border border-red-500/20 rounded-2xl p-6 space-y-6">
                                <div className="flex flex-col md:flex-row md:items-start justify-between gap-6">
                                    <div className="flex-1 space-y-2">
                                        <h4 className="text-sm font-bold text-red-400 flex items-center gap-2">
                                            <RefreshCw size={14} /> {t('settings.resetSetup')}
                                        </h4>
                                        <p className="text-xs text-text-muted leading-relaxed opacity-80">
                                            {t('settings.resetSetupWarning2')}
                                        </p>
                                    </div>
                                    <button
                                        onClick={onResetSetup}
                                        className="px-5 py-2.5 bg-red-500 hover:bg-red-600 text-white rounded-xl font-bold text-xs transition-all shadow-lg shadow-red-500/20 shrink-0 flex items-center gap-2 active:scale-95 w-full md:w-auto justify-center"
                                    >
                                        {t('settings.factoryReset')}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                );
            case 'about':
                return (
                    <div id="about-section" className="space-y-10 animate-in fade-in slide-in-from-bottom-2 duration-300">
                        <div className="bg-bg-secondary/30 border border-border/50 rounded-3xl p-8 text-center relative overflow-hidden group">
                            <div className="absolute top-0 left-0 w-full h-[3px] bg-gradient-to-r from-transparent via-accent to-transparent opacity-50" />

                            <div className="w-24 h-24 bg-accent rounded-[2.2rem] mx-auto flex items-center justify-center text-bg-primary shadow-2xl shadow-accent/40 mb-6 transform rotate-6 group-hover:rotate-0 transition-all duration-500 overflow-hidden border-4 border-accent">
                                <img src="/icon.svg" alt="Radiko Logo" className="w-full h-full object-cover p-3" />
                            </div>

                            <div className="space-y-1">
                                <h4 className="text-2xl font-black text-text-primary tracking-tighter">RADIKO Desktop</h4>
                                <div className="flex items-center justify-center gap-2 text-[10px] text-accent font-bold uppercase tracking-[0.2em] opacity-80">
                                    <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
                                    Stable v{appVersion}
                                </div>
                            </div>

                            <div className="flex justify-center gap-3 mt-8">
                                <button
                                    onClick={() => invoke('open_browser_url', { url: 'https://github.com/xacnio/radiko-desktop' })}
                                    className="px-5 py-2.5 rounded-xl bg-bg-surface/50 hover:bg-bg-surface-hover transition-all text-text-primary border border-border/30 flex items-center gap-2 font-bold text-xs group"
                                >
                                    <Github size={16} /> GitHub <ExternalLink size={12} className="opacity-40 group-hover:opacity-100 transition-opacity" />
                                </button>
                                <button
                                    onClick={async () => {
                                        if (isCheckingUpdate) return;
                                        setIsCheckingUpdate(true);
                                        setUpdateStatus('loading');
                                        try {
                                            const update = await check();
                                            if (update) {
                                                setUpdateResult(update);
                                                setUpdateStatus('found');
                                                notify({
                                                    type: 'info',
                                                    title: t('settings.updateAvailable'),
                                                    message: t('settings.updateFound', { version: update.version })
                                                });
                                            } else {
                                                setUpdateStatus('latest');
                                            }
                                        } catch (e) {
                                            const errStr = e.toString();
                                            if (errStr.includes('404') || errStr.includes('Not Found') || errStr.includes('403')) {
                                                setUpdateStatus('latest');
                                            } else {
                                                setUpdateStatus('error');
                                                notify({ type: 'error', title: t('settings.updateError'), message: errStr });
                                            }
                                        } finally {
                                            setIsCheckingUpdate(false);
                                        }
                                    }}
                                    disabled={isCheckingUpdate || updateStatus === 'downloading' || updateStatus === 'ready'}
                                    className={`px-5 py-2.5 rounded-xl transition-all border flex items-center gap-2 font-bold text-xs group active:scale-95
                                        ${updateStatus === 'found' ? 'bg-accent text-bg-primary border-accent shadow-lg shadow-accent/20' :
                                            updateStatus === 'loading' ? 'bg-bg-surface-active text-text-muted border-border/30' :
                                                'bg-bg-surface/50 hover:bg-bg-surface-hover text-text-primary border-border/30'}`}
                                >
                                    {updateStatus === 'loading' ? <RefreshCw size={14} className="animate-spin" /> : <RefreshCw size={14} />}
                                    {updateStatus === 'found' ? t('settings.updateAvailable') :
                                        updateStatus === 'loading' ? t('settings.checkingUpdates') :
                                            updateStatus === 'latest' ? t('settings.updateLatest') :
                                                t('settings.checkForUpdates')}
                                </button>
                            </div>

                            {/* Update Found / Downloading Panel */}
                            {(updateStatus === 'found' || updateStatus === 'downloading' || updateStatus === 'ready') && (
                                <div className="mt-8 overflow-hidden bg-bg-surface/80 border border-border/40 rounded-2xl animate-in zoom-in-95 duration-300 shadow-xl">
                                    <div className="flex items-start justify-between gap-4 p-6 border-b border-border/20 bg-bg-surface-hover/30">
                                        <div className="text-left flex-1 min-w-0 pr-4">
                                            <div className="text-[10px] font-black uppercase tracking-widest text-[#22c55e] mb-1">
                                                {updateStatus === 'ready' ? 'READY TO INSTALL' : 'NEW UPDATE AVAILABLE!'}
                                            </div>
                                            <div className="flex items-center gap-2">
                                                <div className="text-xl font-bold text-text-primary">v{updateResult?.version}</div>
                                                <div className="px-2 py-0.5 bg-[#22c55e]/10 text-[#22c55e] rounded border border-[#22c55e]/20 text-[9px] font-black uppercase transition-colors">Latest</div>
                                            </div>
                                        </div>
                                        <div className="shrink-0 pt-1">
                                            {updateStatus === 'ready' ? (
                                                <button
                                                    onClick={async () => {
                                                        await relaunch();
                                                    }}
                                                    className="px-5 py-2.5 bg-[#22c55e] hover:bg-[#16a34a] text-black rounded-lg font-bold text-[13px] flex items-center gap-2 shadow-lg shadow-[#22c55e]/20 active:scale-95 transition-all outline-none"
                                                >
                                                    <RefreshCw size={16} /> {t('settings.installUpdate')}
                                                </button>
                                            ) : (
                                                <button
                                                    onClick={async () => {
                                                        if (updateStatus === 'downloading') return;
                                                        setUpdateStatus('downloading');
                                                        try {
                                                            let downloaded = 0;
                                                            let contentLength = 0;
                                                            await updateResult.downloadAndInstall((event) => {
                                                                switch (event.event) {
                                                                    case 'Started':
                                                                        contentLength = event.data.contentLength || 0;
                                                                        break;
                                                                    case 'Progress':
                                                                        downloaded += event.data.chunkLength;
                                                                        if (contentLength > 0) {
                                                                            setDownloadProgress(Math.round((downloaded / contentLength) * 100));
                                                                        }
                                                                        break;
                                                                    case 'Finished':
                                                                        setUpdateStatus('ready');
                                                                        break;
                                                                }
                                                            });
                                                        } catch (e) {
                                                            setUpdateStatus('error');
                                                            notify({ type: 'error', message: e.toString() });
                                                        }
                                                    }}
                                                    disabled={updateStatus === 'downloading'}
                                                    className={`px-5 py-2.5 ${updateStatus === 'downloading' ? 'bg-bg-surface-active text-text-muted border border-border/40' : 'bg-[#22c55e] hover:bg-[#16a34a] text-black shadow-lg shadow-[#22c55e]/20'} rounded-lg font-bold text-[13px] flex items-center gap-2 active:scale-95 transition-all outline-none`}
                                                >
                                                    {updateStatus === 'downloading' ? <RefreshCw className="animate-spin" size={16} /> : <Download size={16} />}
                                                    {updateStatus === 'downloading' ? t('settings.downloadingUpdate', { percent: downloadProgress }) : t('settings.downloadUpdate')}
                                                </button>
                                            )}
                                        </div>
                                    </div>

                                    {releaseNotes && cleanMarkdown(releaseNotes) && (
                                        <div className="p-6 bg-bg-primary/40 max-h-[300px] overflow-y-auto scrollbar-thin scrollbar-thumb-border/50 text-left">
                                            <div className="prose prose-sm prose-invert max-w-none text-text-muted prose-headings:text-text-primary prose-a:text-accent prose-strong:text-text-primary prose-p:text-text-muted prose-li:text-text-muted break-words leading-relaxed text-[13px]">
                                                <ReactMarkdown components={{ a: ({ href, children }) => (<a href="#" onClick={(e) => { e.preventDefault(); if (href) invoke('open_browser_url', { url: href }); }} className="text-accent hover:underline cursor-pointer">{children}</a>) }}>{cleanMarkdown(releaseNotes)}</ReactMarkdown>
                                            </div>
                                        </div>
                                    )}

                                    {(updateStatus === 'downloading') && (
                                        <div className="p-4 bg-bg-surface-active/30 border-t border-border/20">
                                            <div className="w-full h-1.5 bg-bg-primary/50 rounded-full overflow-hidden">
                                                <div
                                                    className="h-full bg-[#22c55e] transition-all duration-300 ease-out relative overflow-hidden"
                                                    style={{ width: `${downloadProgress}%` }}
                                                >
                                                    <div className="absolute inset-0 bg-white/20 animate-pulse" />
                                                </div>
                                            </div>
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>

                        {/* Release History Accordion */}
                        <div className="bg-bg-secondary/30 border border-border/50 rounded-3xl overflow-hidden transition-all duration-300">
                            <button
                                onClick={() => setIsHistoryOpen(!isHistoryOpen)}
                                className="w-full flex items-center justify-between p-6 hover:bg-bg-surface/50 transition-colors text-left"
                            >
                                <div className="flex items-center gap-4">
                                    <div className="w-10 h-10 rounded-2xl bg-accent/10 flex items-center justify-center text-accent">
                                        <FileText size={18} />
                                    </div>
                                    <div>
                                        <div className="text-sm font-bold text-text-primary">{t('settings.updateHistory')}</div>
                                        <div className="text-[10px] text-text-muted uppercase tracking-widest font-black mt-0.5">{t('settings.fetchHistory')}</div>
                                    </div>
                                </div>
                                <ChevronRight size={20} className={`text-text-muted transition-transform duration-300 ${isHistoryOpen ? 'rotate-90' : ''}`} />
                            </button>

                            <div className={`transition-all duration-300 ease-in-out overflow-hidden ${isHistoryOpen ? 'max-h-[600px] opacity-100' : 'max-h-0 opacity-0'}`}>
                                <div className="p-6 bg-bg-primary/30 max-h-[500px] overflow-y-auto scrollbar-thin scrollbar-thumb-border/50 border-t border-border/40">
                                    {isLoadingHistory ? (
                                        <div className="text-center py-6 text-text-muted text-xs font-bold flex items-center justify-center gap-2">
                                            <RefreshCw size={14} className="animate-spin" /> {t('settings.loadingHistory')}
                                        </div>
                                    ) : releaseHistory && releaseHistory.length > 0 ? (
                                        <div className="space-y-8">
                                            {releaseHistory.map(release => (
                                                <div key={release.id} className="space-y-3">
                                                    <div className="flex items-center gap-3">
                                                        <h5 className="font-bold text-sm text-text-primary">{release.name || release.tag_name}</h5>
                                                        <span className="text-[10px] uppercase font-black tracking-wider text-accent bg-accent/10 px-2 py-0.5 rounded-lg shrink-0">
                                                            {new Date(release.published_at).toLocaleDateString()}
                                                        </span>
                                                    </div>
                                                    <div className="text-[13px] text-text-muted prose prose-sm prose-invert max-w-none prose-headings:text-text-primary prose-a:text-accent prose-strong:text-text-primary prose-p:text-text-muted prose-li:text-text-muted break-words leading-relaxed pl-3 border-l-2 border-border/30">
                                                        <ReactMarkdown components={{ a: ({ href, children }) => (<a href="#" onClick={(e) => { e.preventDefault(); if (href) invoke('open_browser_url', { url: href }); }} className="text-accent hover:underline cursor-pointer">{children}</a>) }}>{cleanMarkdown(release.body) || 'No release notes.'}</ReactMarkdown>
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    ) : (
                                        <div className="text-center py-6 text-text-muted text-xs font-bold">
                                            {t('settings.errorHistory')}
                                        </div>
                                    )}
                                </div>
                            </div>
                        </div>

                        <div className="space-y-6">
                            <div className="flex items-center gap-2 text-text-muted px-2">
                                <Settings size={14} className="text-accent" />
                                <h3 className="text-xs font-black uppercase tracking-widest opacity-60">{t('settings.title')}</h3>
                            </div>

                            <div className="bg-bg-secondary/40 border border-border/40 rounded-2xl p-6 space-y-6">
                                <div className="flex items-center justify-between gap-4">
                                    <div className="space-y-1">
                                        <h4 className="text-sm font-bold text-text-primary">{t('settings.autoUpdate')}</h4>
                                        <p className="text-xs text-text-muted">{t('settings.autoUpdateDesc')}</p>
                                    </div>
                                    <button
                                        onClick={() => setAutoUpdate(!autoUpdate)}
                                        className={`w-10 h-5 rounded-full transition-all relative p-1 cursor-pointer outline-none shrink-0
                                            ${autoUpdate ? 'bg-accent shadow-lg shadow-accent/20' : 'bg-bg-surface-active border border-border/50'}`}
                                    >
                                        <div className={`w-3 h-3 bg-white rounded-full transition-all duration-300 shadow-sm ${autoUpdate ? 'translate-x-5' : 'translate-x-0'}`} />
                                    </button>
                                </div>
                            </div>
                        </div>

                        <div className="space-y-4">
                            <div className="flex items-center gap-2 text-text-muted px-2">
                                <Code size={14} className="text-accent" />
                                <h3 className="text-xs font-black uppercase tracking-widest opacity-60">{t('settings.credits')}</h3>
                            </div>

                            <div className="bg-bg-secondary/40 border border-border/40 rounded-2xl overflow-hidden divide-y divide-border/20">
                                {[
                                    { name: 'SongRec', desc: 'Open-source Shazam client and library', license: 'GPL-3.0', url: 'https://github.com/marin-m/SongRec' },
                                    { name: 'radio-browser.info', desc: 'Community-driven radio database', license: 'GPL-3.0', url: 'https://www.radio-browser.info' },
                                    { name: 'Tauri', desc: 'Lightweight desktop app framework', license: 'MIT', url: 'https://tauri.app' },
                                    { name: 'Symphonia', desc: 'Symphonia decoding library', license: 'MPL-2.0', url: 'https://github.com/pdeljanov/Symphonia' },
                                ].map(item => (
                                    <button
                                        key={item.name}
                                        onClick={() => invoke('open_browser_url', { url: item.url })}
                                        className="w-full text-left px-5 py-4 hover:bg-bg-surface-hover transition-all cursor-pointer group flex items-center gap-4"
                                    >
                                        <div className="flex-1 min-w-0">
                                            <div className="flex items-center gap-2">
                                                <span className="text-sm font-bold text-text-primary group-hover:text-accent transition-colors">{item.name}</span>
                                                <span className="text-[9px] font-mono text-accent/70 bg-accent/10 px-1.5 py-0.5 rounded-md">{item.license}</span>
                                            </div>
                                            <p className="text-[11px] text-text-muted mt-0.5 truncate opacity-70 group-hover:opacity-100">{item.desc}</p>
                                        </div>
                                        <ChevronRight size={14} className="text-text-muted/20 group-hover:text-accent group-hover:translate-x-1 transition-all" />
                                    </button>
                                ))}
                            </div>
                        </div>

                        <div className="text-center pb-2 flex flex-col items-center gap-3">
                            <div className="flex justify-center items-center gap-1.5 text-text-muted/30">
                                <Heart size={10} fill="currentColor" />
                                <span className="text-[9px] font-bold uppercase tracking-[0.3em]">Built with Passion</span>
                            </div>
                        </div>
                    </div>
                );
            default:
                return null;
        }
    };

    return (
        <div className="flex-1 flex flex-col overflow-hidden bg-bg-primary animate-in fade-in duration-500">
            {/* Horizontal Header */}
            <div className="px-8 h-[64px] flex items-center justify-between border-b border-border/40 shrink-0">
                <h2 className="text-sm font-black text-text-primary uppercase tracking-[0.2em] flex items-center gap-3">
                    <div className="p-1.5 bg-accent/10 rounded-lg">
                        <Settings size={16} className="text-accent" />
                    </div>
                    {t('settings.title')}
                </h2>
            </div>

            {/* Top Navigation Menu */}
            <div className="px-8 border-b border-border/20 py-2 shrink-0 bg-bg-primary/50 backdrop-blur-md sticky top-0 z-20">
                {isSmall ? (
                    <div className="relative group">
                        <select
                            value={activeTab}
                            onChange={(e) => setActiveTab(e.target.value)}
                            className="w-full appearance-none bg-bg-surface border border-border text-text-primary text-xs font-bold rounded-xl px-4 py-3 cursor-pointer focus:ring-2 focus:ring-accent"
                        >
                            {menuItems.map(item => (
                                <option key={item.id} value={item.id}>{item.label}</option>
                            ))}
                        </select>
                        <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-accent">
                            <ChevronRight size={14} className="rotate-90" />
                        </div>
                    </div>
                ) : (
                    <div className="flex gap-2 overflow-x-auto scrollbar-hide pt-1.5">
                        {menuItems.map((item) => {
                            const Icon = item.icon;
                            const isActive = activeTab === item.id;
                            return (
                                <button
                                    key={item.id}
                                    onClick={() => setActiveTab(item.id)}
                                    className={`flex items-center gap-2 px-4 py-3 text-xs font-bold transition-all duration-300 relative group whitespace-nowrap
                                        ${isActive
                                            ? 'text-accent'
                                            : 'text-text-muted hover:text-text-primary'
                                        }`}
                                >
                                    <Icon size={14} className={`transition-transform duration-300 ${isActive ? 'scale-110' : 'group-hover:scale-110'}`} />
                                    {item.label}
                                    {isActive && (
                                        <div className="absolute bottom-0 left-0 right-0 h-[3px] bg-accent rounded-t-full shadow-[0_-2px_8px_rgba(29,185,84,0.4)] animate-in fade-in zoom-in duration-300" />
                                    ) || (
                                            <div className="absolute bottom-0 left-0 right-0 h-[3px] bg-transparent group-hover:bg-text-primary/10 rounded-t-full transition-all duration-300" />
                                        )}
                                </button>
                            );
                        })}
                    </div>
                )}
            </div>

            {/* Content Area */}
            <main className="flex-1 overflow-y-auto custom-scrollbar">
                <div className="max-w-[700px] mx-auto p-10 py-8">
                    {renderContent()}
                </div>
            </main>
        </div>
    );
}
