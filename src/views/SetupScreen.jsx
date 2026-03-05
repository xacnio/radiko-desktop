import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';

export default function SetupScreen({ onComplete }) {
    const { t } = useTranslation();
    const [countries, setCountries] = useState([]);
    const [languages, setLanguages] = useState([]);
    const [selectedCountry, setSelectedCountry] = useState(null);
    const [selectedLang, setSelectedLang] = useState('');
    const [countrySearch, setCountrySearch] = useState('');
    const [langSearch, setLangSearch] = useState('');
    const [step, setStep] = useState(1); // 1=country, 2=language, 3=downloading
    const [progress, setProgress] = useState('');
    const [error, setError] = useState('');

    useEffect(() => {
        (async () => {
            try {
                const c = await invoke('get_countries');
                c.sort((a, b) => b.stationcount - a.stationcount);
                setCountries(c);
            } catch (e) { setError(t('setup.errorLoadingCountries') + e); }
            try {
                const l = await invoke('get_languages');
                l.sort((a, b) => b.stationcount - a.stationcount);
                setLanguages(l);
            } catch { }
        })();
    }, []);

    const filteredCountries = countrySearch
        ? countries.filter(c => c.name.toLowerCase().includes(countrySearch.toLowerCase()))
        : countries;

    const filteredLangs = langSearch
        ? languages.filter(l => l.name.toLowerCase().includes(langSearch.toLowerCase()))
        : languages;

    const handleFinish = async () => {
        if (!selectedCountry) return;
        setStep(3);
        setProgress(t('setup.downloadingRadios'));
        try {
            const stations = await invoke('get_all_country_stations', { country: selectedCountry.name });
            setProgress(t('setup.radiosDownloaded', { count: stations.length }));

            // Get states/cities
            let states = [];
            try {
                states = await invoke('get_states', { country: selectedCountry.name });
            } catch { }

            // Download favicons with progress
            const faviconEntries = stations
                .filter(s => s.favicon && s.favicon.length > 0)
                .map(s => ({ uuid: s.stationuuid, url: s.favicon }));

            if (faviconEntries.length > 0) {
                setProgress(t('setup.downloadingImages_start', { total: faviconEntries.length }));
                // Listen for progress events
                const { listen } = await import('@tauri-apps/api/event');
                const unlisten = await listen('favicon-progress', (e) => {
                    const { done, total } = e.payload;
                    setProgress(t('setup.downloadingImages_progress', { done, total }));
                });
                try {
                    const faviconMap = await invoke('batch_cache_favicons', { entries: faviconEntries });
                    unlisten();
                    const cachedCount = Object.keys(faviconMap).length;
                    stations.forEach(s => {
                        if (faviconMap[s.stationuuid]) {
                            s.favicon = faviconMap[s.stationuuid];
                        }
                    });
                    setProgress(t('setup.imagesDownloaded', { count: cachedCount, total: faviconEntries.length }));
                } catch (e) {
                    unlisten();
                    console.error('Favicon cache error:', e);
                    setProgress(t('setup.downloadingImages_failed'));
                }
            }

            // Extract unique tags from stations
            const tagMap = {};
            stations.forEach(s => {
                (s.tags || '').split(',').map(t => t.trim().toLowerCase()).filter(Boolean).forEach(t => {
                    tagMap[t] = (tagMap[t] || 0) + 1;
                });
            });
            const localTags = Object.entries(tagMap)
                .map(([name, count]) => ({ name, stationcount: count }))
                .filter(t => t.stationcount >= 2)
                .sort((a, b) => b.stationcount - a.stationcount);

            // Save to localStorage
            const config = {
                country: selectedCountry,
                language: selectedLang,
                setupDone: true,
                lastSync: Date.now(),
            };
            localStorage.setItem('rx_config', JSON.stringify(config));
            localStorage.setItem('rx_stations', JSON.stringify(stations));
            localStorage.setItem('rx_states', JSON.stringify(states));
            localStorage.setItem('rx_local_tags', JSON.stringify(localTags));

            setProgress(t('setup.completed'));
            setTimeout(() => onComplete(config, stations, states, localTags), 500);
        } catch (e) {
            setError(t('setup.errorMsg') + e);
            setStep(2);
        }
    };

    return (
        <div className="flex items-center justify-center h-screen bg-bg-primary">
            <div className="w-[500px] max-h-[600px] bg-bg-secondary rounded-2xl border border-border overflow-hidden flex flex-col">
                {/* Header */}
                <div className="text-center py-6 px-6 border-b border-border">
                    <img src="/icon.svg" alt="Radiko Logo" className="w-16 h-16 mx-auto mb-3 drop-shadow-md" />
                    <h1 className="text-xl font-bold text-text-primary">{t('setup.setupTitle')}</h1>
                    <p className="text-xs text-text-muted mt-1">
                        {step === 1 && t('setup.selectCountry')}
                        {step === 2 && t('setup.selectLanguage')}
                        {step === 3 && t('setup.downloadingRadios')}
                    </p>
                </div>

                {error && (
                    <div className="mx-4 mt-3 p-3 bg-danger/10 border border-danger/30 rounded-lg text-xs text-danger">{error}</div>
                )}

                {/* Step 1: Country */}
                {step === 1 && (
                    <div className="flex-1 flex flex-col overflow-hidden p-4">
                        <input
                            type="text"
                            placeholder={t('setup.searchCountry')}
                            value={countrySearch}
                            onChange={e => setCountrySearch(e.target.value)}
                            className="w-full px-4 py-2 bg-bg-surface border border-border rounded-lg text-sm text-text-primary placeholder-text-muted outline-none focus:border-accent mb-3"
                            spellCheck="false"
                        />
                        <div className="flex-1 overflow-y-auto space-y-1">
                            {filteredCountries.map(c => (
                                <button
                                    key={c.name}
                                    onClick={() => { setSelectedCountry(c); setStep(2); }}
                                    className={`w-full text-left flex items-center gap-3 px-4 py-3 rounded-lg transition-all cursor-pointer
                    ${selectedCountry?.name === c.name ? 'bg-accent-muted border border-accent/30' : 'hover:bg-bg-surface-hover border border-transparent'}`}
                                >
                                    {c.iso_3166_1 && (
                                        <img
                                            src={`https://flagcdn.com/w40/${c.iso_3166_1.toLowerCase()}.png`}
                                            alt=""
                                            className="w-8 h-5 rounded object-cover"
                                            onError={e => { e.target.style.display = 'none'; }}
                                        />
                                    )}
                                    <div className="flex-1">
                                        <div className="text-sm font-semibold text-text-primary">{c.name}</div>
                                        <div className="text-[10px] text-text-muted">{c.stationcount} {t('common.stations')}</div>
                                    </div>
                                </button>
                            ))}
                        </div>
                    </div>
                )}

                {/* Step 2: Language */}
                {step === 2 && (
                    <div className="flex-1 flex flex-col overflow-hidden p-4">
                        <div className="mb-3 p-3 bg-bg-surface rounded-lg flex items-center gap-3">
                            {selectedCountry?.iso_3166_1 && (
                                <img
                                    src={`https://flagcdn.com/w40/${selectedCountry.iso_3166_1.toLowerCase()}.png`}
                                    alt=""
                                    className="w-8 h-5 rounded object-cover"
                                />
                            )}
                            <div>
                                <div className="text-sm font-semibold text-accent">{selectedCountry?.name}</div>
                                <div className="text-[10px] text-text-muted">{selectedCountry?.stationcount} {t('common.stations')}</div>
                            </div>
                            <button onClick={() => setStep(1)} className="ml-auto text-xs text-text-muted hover:text-text-primary cursor-pointer">{t('setup.change')}</button>
                        </div>

                        <input
                            type="text"
                            placeholder={t('setup.searchLanguage')}
                            value={langSearch}
                            onChange={e => setLangSearch(e.target.value)}
                            className="w-full px-4 py-2 bg-bg-surface border border-border rounded-lg text-sm text-text-primary placeholder-text-muted outline-none focus:border-accent mb-3"
                            spellCheck="false"
                        />
                        <div className="flex-1 overflow-y-auto space-y-1">
                            {filteredLangs.slice(0, 50).map(l => (
                                <button
                                    key={l.name}
                                    onClick={() => setSelectedLang(l.name)}
                                    className={`w-full text-left flex items-center gap-3 px-4 py-2 rounded-lg transition-all cursor-pointer text-sm
                    ${selectedLang === l.name ? 'bg-accent-muted border border-accent/30 text-accent' : 'hover:bg-bg-surface-hover border border-transparent text-text-secondary'}`}
                                >
                                    <span className="font-medium">{l.name}</span>
                                    <span className="text-[10px] text-text-muted ml-auto">{l.stationcount} {t('common.stations')}</span>
                                </button>
                            ))}
                        </div>

                        <button
                            onClick={handleFinish}
                            className="mt-4 w-full py-3 bg-accent hover:bg-accent-hover text-bg-primary font-bold rounded-lg transition-colors cursor-pointer text-sm"
                        >
                            {selectedLang ? t('setup.startWithCountryAndLang', { country: selectedCountry?.name, lang: selectedLang }) : t('setup.startWithCountry', { country: selectedCountry?.name })}
                        </button>
                    </div>
                )}

                {/* Step 3: Downloading */}
                {step === 3 && (() => {
                    const match = progress.match(/(\d+)\/(\d+)/);
                    const pctVal = match ? Math.round((parseInt(match[1]) / parseInt(match[2])) * 100) : 0;
                    return (
                        <div className="flex-1 flex flex-col items-center justify-center p-8 gap-4">
                            <div className="w-10 h-10 border-2 border-accent border-t-transparent rounded-full animate-spin" />
                            <p className="text-sm text-text-secondary">{progress}</p>
                            {match && (
                                <div className="w-64">
                                    <div className="h-2 bg-bg-surface rounded-full overflow-hidden">
                                        <div
                                            className="h-full bg-accent rounded-full transition-all duration-200"
                                            style={{ width: `${pctVal}%` }}
                                        />
                                    </div>
                                    <p className="text-[10px] text-text-muted text-center mt-1">%{pctVal}</p>
                                </div>
                            )}
                        </div>
                    );
                })()}
            </div>
        </div>
    );
}
