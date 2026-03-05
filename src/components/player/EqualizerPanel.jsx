import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';

const BAND_LABELS = ['60', '170', '310', '600', '1K', '3K', '6K'];
const NUM_BANDS = 7;

const PRESETS = {
    'flat': [0, 0, 0, 0, 0, 0, 0],
    'bass': [6, 4, 1, 0, -1, -1, -2],
    'treble': [-2, -1, 0, 1, 3, 5, 6],
    'rock': [4, 2, -1, -2, 1, 3, 4],
    'pop': [-1, 1, 3, 3, 1, -1, -2],
    'live': [3, 0, -2, -2, 0, 2, 4],
    'classic': [3, 2, 0, -1, 0, 2, 3],
    'vocal': [-2, 0, 2, 4, 3, 1, -1],
};

export default function EqualizerPanel() {
    const { t } = useTranslation();
    const [gains, setGains] = useState(new Array(NUM_BANDS).fill(0));
    const [enabled, setEnabled] = useState(true);
    const [activePreset, setActivePreset] = useState('flat');

    // Load initial state
    useEffect(() => {
        invoke('get_eq_gains').then(g => {
            if (Array.isArray(g) && g.length === NUM_BANDS) setGains(g);
        }).catch(() => { });
        invoke('get_eq_enabled').then(e => setEnabled(e)).catch(() => { });
    }, []);

    const handleSlider = useCallback((index, value) => {
        const newGains = [...gains];
        newGains[index] = value;
        setGains(newGains);
        setActivePreset('');
        invoke('set_eq_gains', { gains: newGains }).catch(() => { });
    }, [gains]);

    const applyPreset = useCallback((name) => {
        const preset = PRESETS[name];
        if (preset) {
            setGains([...preset]);
            setActivePreset(name);
            invoke('set_eq_gains', { gains: preset }).catch(() => { });
        }
    }, []);

    const toggleEnabled = useCallback(() => {
        const next = !enabled;
        setEnabled(next);
        invoke('set_eq_enabled', { enabled: next }).catch(() => { });
    }, [enabled]);

    const resetAll = useCallback(() => {
        const flat = new Array(NUM_BANDS).fill(0);
        setGains(flat);
        setActivePreset('flat');
        invoke('set_eq_gains', { gains: flat }).catch(() => { });
    }, []);

    return (
        <div className="w-full">
            {/* Header */}
            <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                    <span className="text-xs font-bold uppercase tracking-wider text-text-secondary">{t('equalizer.title')}</span>
                    <button
                        onClick={toggleEnabled}
                        className={`w-8 h-[18px] rounded-full relative transition-colors cursor-pointer ${enabled ? 'bg-accent' : 'bg-bg-surface-active'}`}
                    >
                        <span className={`absolute top-[2px] w-[14px] h-[14px] rounded-full bg-white shadow transition-all ${enabled ? 'left-[16px]' : 'left-[2px]'}`} />
                    </button>
                </div>
                <button
                    onClick={resetAll}
                    className="text-[10px] text-text-muted hover:text-accent transition-colors cursor-pointer"
                >
                    {t('equalizer.reset')}
                </button>
            </div>

            {/* Presets */}
            <div className="flex flex-wrap gap-1 mb-3">
                {Object.keys(PRESETS).map(name => (
                    <button
                        key={name}
                        onClick={() => applyPreset(name)}
                        className={`px-2 py-0.5 rounded text-[10px] font-semibold transition-all cursor-pointer border ${activePreset === name
                            ? 'bg-accent/20 text-accent border-accent/40'
                            : 'bg-bg-surface text-text-muted border-border/30 hover:border-accent/30 hover:text-text-primary'
                            }`}
                    >
                        {t(`equalizer.preset_${name}`)}
                    </button>
                ))}
            </div>

            {/* Sliders */}
            <div className={`flex gap-1 items-stretch transition-opacity ${enabled ? 'opacity-100' : 'opacity-30 pointer-events-none'}`}>
                {gains.map((gain, i) => (
                    <div key={i} className="flex-1 flex flex-col items-center gap-1">
                        {/* dB value */}
                        <span className="text-[9px] font-mono text-text-muted w-full text-center">
                            {gain > 0 ? '+' : ''}{gain.toFixed(0)}
                        </span>

                        {/* Vertical slider container */}
                        <div className="relative w-full flex justify-center" style={{ height: '100px' }}>
                            <input
                                type="range"
                                min={-12}
                                max={12}
                                step={0.5}
                                value={gain}
                                onChange={e => handleSlider(i, parseFloat(e.target.value))}
                                className="eq-slider"
                                style={{
                                    '--eq-pct': `${((gain + 12) / 24) * 100}%`,
                                }}
                            />
                        </div>

                        {/* Frequency label */}
                        <span className="text-[9px] font-semibold text-text-muted">
                            {BAND_LABELS[i]}
                        </span>
                    </div>
                ))}
            </div>
        </div>
    );
}
