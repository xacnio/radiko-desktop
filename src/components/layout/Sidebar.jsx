import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Radio, Heart, Tags, MapPin, Music, Settings, History, Languages } from 'lucide-react';

export default function Sidebar({
    tab, onSelectTab, tags, locations, languages,
    selectedTag, selectedCountry, selectedCity, selectedLanguage,
    onSelectTag, onSelectCountry, onSelectCity, onSelectLanguage,
    config, onResetSetup, stationCount, identifiedCount,
    onSearch, mixAccent
}) {
    const { t } = useTranslation();
    const [subOpen, setSubOpen] = useState(null); // 'tags' | 'cities' | 'languages'
    const [tagSearch, setTagSearch] = useState('');
    const [locationSearch, setLocationSearch] = useState('');
    const [languageSearch, setLanguageSearch] = useState('');

    const navItems = [
        { id: 'all', icon: <Radio size={18} />, label: t('sidebar.allRadios') },
        { id: 'favorites', icon: <Heart size={18} />, label: t('sidebar.favorites') },
        { id: 'tags', icon: <Tags size={18} />, label: t('sidebar.categories'), expandable: true },
    ];

    if (locations && locations.length > 0) {
        navItems.push({ id: 'cities', icon: <MapPin size={18} />, label: t('sidebar.cities'), expandable: true });
    }

    if (languages && languages.length > 0) {
        navItems.push({ id: 'languages', icon: <Languages size={18} />, label: t('sidebar.languages'), expandable: true });
    }

    const handleNav = (id) => {
        const item = navItems.find(n => n.id === id);
        if (item && item.expandable) {
            // If opening a new expandable tab, or closing the current one
            if (subOpen !== id) {
                setSubOpen(id);
                // Clear filters when switching top-level expandable tabs
                if (id === 'tags') onSelectTag(null);
                if (id === 'cities') { onSelectCountry(null); onSelectCity(null); }
                if (id === 'languages') onSelectLanguage(null);
            } else {
                setSubOpen(null);
            }
            onSelectTab(id);
        } else {
            setSubOpen(null);
            onSelectTab(id);
        }
    };

    const filteredTags = tagSearch
        ? tags.filter(t => t.name.toLowerCase().includes(tagSearch.toLowerCase()))
        : tags;

    const filteredLocations = locationSearch
        ? (selectedCountry
            ? locations.find(l => (l.country || '').trim().toLowerCase() === (selectedCountry || '').trim().toLowerCase())?.cities?.filter(c => c.name.toLowerCase().includes(locationSearch.toLowerCase()))
            : locations?.filter(l => (l.country || '').toLowerCase().includes(locationSearch.toLowerCase()))
        )
        : (selectedCountry
            ? locations.find(l => (l.country || '').trim().toLowerCase() === (selectedCountry || '').trim().toLowerCase())?.cities
            : locations
        );

    const filteredLanguages = languageSearch
        ? (languages || []).filter(l => l.name.toLowerCase().includes(languageSearch.toLowerCase()))
        : (languages || []);

    return (
        <aside className="w-full flex-1 shrink-0 bg-bg-secondary border-r border-border flex flex-col overflow-hidden">
            <div className="px-3 pt-3 pb-2">
                <div className={`flex items-center gap-2 px-3 py-2 bg-bg-surface rounded-lg transition-all duration-300
                    ${mixAccent ? 'border border-accent/20 shadow-[0_4px_12px_rgba(var(--accent),0.05)]' : 'border border-border/50'}`}>
                    <div className="w-8 h-8 rounded-full bg-accent/10 flex items-center justify-center shrink-0 border border-accent/20 relative overflow-hidden">
                        <div
                            className="w-[50%] h-[50%] bg-accent transition-colors duration-300"
                            style={{
                                maskImage: 'url(/icon.svg)',
                                maskSize: 'contain',
                                maskRepeat: 'no-repeat',
                                maskPosition: 'center',
                                WebkitMaskImage: 'url(/icon.svg)',
                                WebkitMaskSize: 'contain',
                                WebkitMaskRepeat: 'no-repeat',
                                WebkitMaskPosition: 'center'
                            }}
                        />
                        <div className="absolute inset-0 bg-accent/10 animate-pulse"></div>
                    </div>
                    <div className="flex-1 min-w-0">
                        <div className="text-xs font-black text-accent truncate uppercase tracking-tight">{t('sidebar.myLibrary')}</div>
                        <div className="text-[10px] text-text-muted font-medium opacity-70 italic">{stationCount} {t('common.stations')}</div>
                    </div>
                </div>
            </div>

            {/* Nav */}
            <nav className="flex-1 flex flex-col gap-0.5 px-3 pb-1 overflow-y-auto custom-scrollbar">
                {navItems.map(item => (
                    <div key={item.id} className="flex flex-col gap-0.5">
                        <button
                            onClick={() => handleNav(item.id)}
                            className={`flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all cursor-pointer w-full
                                ${(tab === item.id && !item.expandable) || (item.expandable && subOpen === item.id)
                                    ? 'bg-accent-muted text-accent'
                                    : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'
                                }`}
                        >
                            <span className="flex items-center justify-center w-5">{item.icon}</span>
                            <span>{item.label}</span>
                            {item.badge > 0 && (
                                <span className="ml-auto px-1.5 py-0.5 bg-accent/20 text-accent text-[10px] font-bold rounded">
                                    {item.badge}
                                </span>
                            )}
                            {item.expandable && (
                                <svg
                                    className={`ml-auto w-4 h-4 transition-transform ${subOpen === item.id ? 'rotate-180' : ''}`}
                                    fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24"
                                >
                                    <polyline points="6 9 12 15 18 9" />
                                </svg>
                            )}
                        </button>

                        {/* In-place Accordion Content */}
                        {item.id === subOpen && (
                            <div className="flex flex-col overflow-hidden bg-bg-surface/20 rounded-lg mb-1 border border-border/30">
                                {item.id === 'tags' ? (
                                    <>
                                        <div className="p-2">
                                            <input
                                                type="text"
                                                placeholder={t('sidebar.searchCategory')}
                                                value={tagSearch}
                                                onChange={e => setTagSearch(e.target.value)}
                                                className="w-full px-3 py-1.5 bg-bg-surface border border-border rounded text-xs text-text-primary placeholder-text-muted outline-none focus:border-accent"
                                            />
                                        </div>
                                        <div className="max-h-[300px] overflow-y-auto px-1 pb-2 scrollbar-thin">
                                            {filteredTags.map(tagItem => (
                                                <button
                                                    key={tagItem.name}
                                                    onClick={() => onSelectTag?.(tagItem.name)}
                                                    className={`w-full text-left px-3 py-2 rounded-md text-xs transition-colors cursor-pointer mb-0.5
                                                        ${selectedTag === tagItem.name ? 'bg-accent-muted text-accent' : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'}`}
                                                >
                                                    <div className="font-semibold truncate">{tagItem.name}</div>
                                                    <div className="text-text-muted text-[10px]">{tagItem.stationcount} {t('common.stations')}</div>
                                                </button>
                                            ))}
                                        </div>
                                    </>
                                ) : item.id === 'cities' ? (
                                    <>
                                        <div className="p-2 border-b border-border/10 bg-bg-surface/10">
                                            {/* Location Breadcrumb / Reset */}
                                            <div className="flex items-center gap-1.5 mb-2 overflow-hidden px-1">
                                                <button
                                                    onClick={() => { onSelectCountry(null); onSelectCity(null); setLocationSearch(''); onSearch(''); }}
                                                    className={`text-[10px] font-bold tracking-wider shrink-0 px-1.5 py-0.5 rounded transition-colors ${!selectedCountry ? 'text-accent bg-accent/10' : 'text-text-muted hover:text-text-primary'}`}
                                                >
                                                    {t('sidebar.allCountries') || 'World'}
                                                </button>
                                                {selectedCountry && (
                                                    <>
                                                        <div className="text-text-muted text-[10px]">/</div>
                                                        <button
                                                            onClick={() => { onSelectCity(null); setLocationSearch(''); onSearch(''); }}
                                                            className={`text-[10px] font-bold tracking-wider truncate px-1.5 py-0.5 rounded transition-colors ${!selectedCity ? 'text-accent bg-accent/10' : 'text-text-muted hover:text-text-primary'}`}
                                                        >
                                                            {selectedCountry}
                                                        </button>
                                                    </>
                                                )}
                                            </div>

                                            <input
                                                type="text"
                                                placeholder={selectedCountry ? t('sidebar.searchCity') : (t('sidebar.searchCountry') || 'Search country...')}
                                                value={locationSearch}
                                                onChange={e => setLocationSearch(e.target.value)}
                                                className="w-full px-3 py-1.5 bg-bg-surface border border-border rounded text-xs text-text-primary placeholder-text-muted outline-none focus:border-accent"
                                            />
                                        </div>
                                        <div className="max-h-[300px] overflow-y-auto px-1 pb-2 scrollbar-thin">
                                            {selectedCountry ? (
                                                (filteredLocations || []).map(city => (
                                                    <button
                                                        key={city.name}
                                                        onClick={() => { onSelectCity(city.name); onSearch(''); }}
                                                        className={`w-full text-left px-3 py-2 rounded-md text-xs transition-colors cursor-pointer mb-0.5
                                                            ${selectedCity === city.name ? 'bg-accent-muted text-accent' : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'}`}
                                                    >
                                                        <div className="font-semibold truncate">{city.name || 'Unknown City'}</div>
                                                        <div className="text-text-muted text-[10px]">{city.count} {city.count === 1 ? t('common.station') : t('common.stations')}</div>
                                                    </button>
                                                ))
                                            ) : (
                                                (filteredLocations || []).map(loc => (
                                                    <button
                                                        key={loc.country}
                                                        onClick={() => {
                                                            onSelectCountry(loc.country);
                                                            onSelectCity(null);
                                                            onSelectTab('cities');
                                                            setLocationSearch('');
                                                            onSearch('');
                                                        }}
                                                        className={`w-full text-left px-3 py-2 rounded-md text-xs transition-colors cursor-pointer mb-0.5
                                                            ${selectedCountry === loc.country ? 'bg-accent-muted text-accent' : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'}`}
                                                    >
                                                        <div className="font-semibold truncate">{loc.country}</div>
                                                        <div className="text-text-muted text-[10px]">{loc.count} {loc.count === 1 ? t('common.station') : t('common.stations')}</div>
                                                    </button>
                                                ))
                                            )}
                                        </div>
                                    </>
                                ) : item.id === 'languages' ? (
                                    <>
                                        <div className="p-2">
                                            <input
                                                type="text"
                                                placeholder={t('sidebar.searchLanguage')}
                                                value={languageSearch}
                                                onChange={e => setLanguageSearch(e.target.value)}
                                                className="w-full px-3 py-1.5 bg-bg-surface border border-border rounded text-xs text-text-primary placeholder-text-muted outline-none focus:border-accent"
                                            />
                                        </div>
                                        <div className="max-h-[300px] overflow-y-auto px-1 pb-2 scrollbar-thin">
                                            {filteredLanguages.map(langItem => (
                                                <button
                                                    key={langItem.name}
                                                    onClick={() => onSelectLanguage?.(langItem.name)}
                                                    className={`w-full text-left px-3 py-2 rounded-md text-xs transition-colors cursor-pointer mb-0.5
                                                        ${selectedLanguage?.toLowerCase() === langItem.name.toLowerCase() ? 'bg-accent-muted text-accent' : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'}`}
                                                >
                                                    <div className="font-semibold truncate">{langItem.name}</div>
                                                    <div className="text-text-muted text-[10px]">{langItem.stationcount} {t('common.stations')}</div>
                                                </button>
                                            ))}
                                        </div>
                                    </>
                                ) : null}
                            </div>
                        )}
                    </div>
                ))}
            </nav>

            {/* Bottom section */}
            <div className={`mt-auto px-3 py-3 border-t border-border flex flex-col gap-1 transition-colors duration-300
                ${mixAccent ? 'bg-bg-secondary/40' : 'bg-bg-secondary/80'} backdrop-blur-sm`}>
                <button
                    onClick={() => { setSubOpen(null); onSelectTab('identified'); }}
                    className={`flex items-center gap-3 px-3 py-2.5 w-full rounded-lg text-sm font-medium transition-all cursor-pointer group
                        ${tab === 'identified'
                            ? 'bg-accent-muted text-accent'
                            : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'
                        }`}
                >
                    <History className={`shrink-0 transition-colors ${tab === 'identified' ? 'text-accent' : 'text-text-muted group-hover:text-accent'}`} size={18} />
                    <span className="truncate text-left leading-tight font-bold tracking-tight">{t('sidebar.identified')}</span>
                </button>
                <button
                    onClick={() => { setSubOpen(null); onSelectTab('settings'); }}
                    className={`flex items-center gap-3 px-3 py-2.5 w-full rounded-lg text-sm font-medium transition-all cursor-pointer group
                        ${tab === 'settings'
                            ? 'bg-accent-muted text-accent'
                            : 'text-text-secondary hover:bg-bg-surface-hover hover:text-text-primary'
                        }`}
                >
                    <Settings className={`shrink-0 transition-colors ${tab === 'settings' ? 'text-accent' : 'text-text-muted group-hover:text-accent'}`} size={18} />
                    <span className="truncate text-left leading-tight font-bold tracking-tight">{t('sidebar.settings')}</span>
                </button>
            </div>
        </aside>
    );
}
