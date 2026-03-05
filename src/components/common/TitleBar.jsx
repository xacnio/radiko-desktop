import { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { Settings, Minus, X, Maximize2, Square } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const win = getCurrentWindow();

export default function TitleBar({ onOpenSettings }) {
    const { t } = useTranslation();
    const [os, setOs] = useState('macos'); // Default to macos for safety during transition

    useEffect(() => {
        invoke('get_os').then(setOs).catch(() => setOs('macos'));
    }, []);

    const isMac = os === 'macos' || os === 'darwin';

    return (
        <header
            className="flex items-center justify-between h-[38px] pl-3 pr-0 bg-bg-secondary border-b border-border shrink-0 select-none"
            data-tauri-drag-region
        >
            {isMac ? (
                <>
                    {/* macOS Layout: Traffic Lights Left */}
                    <div className="flex items-center gap-2 w-20 group/controls" data-tauri-drag-region>
                        <button
                            onClick={() => win.close()}
                            className="w-3.5 h-3.5 rounded-full bg-[#ff5f57] border border-[#e0443e] flex items-center justify-center transition-all active:brightness-75"
                        >
                            <X size={8} className="text-[#4c0000] opacity-0 group-hover/controls:opacity-100 transition-opacity" />
                        </button>
                        <button
                            onClick={() => win.minimize()}
                            className="w-3.5 h-3.5 rounded-full bg-[#febc2e] border border-[#d8a124] flex items-center justify-center transition-all active:brightness-75"
                        >
                            <Minus size={8} className="text-[#985712] opacity-0 group-hover/controls:opacity-100 transition-opacity" />
                        </button>
                        <button
                            onClick={() => win.toggleMaximize()}
                            className="w-3.5 h-3.5 rounded-full bg-[#28c840] border border-[#1aab29] flex items-center justify-center transition-all active:brightness-75"
                        >
                            <Maximize2 size={7} className="text-[#06540d] opacity-0 group-hover/controls:opacity-100 transition-opacity font-bold" />
                        </button>
                    </div>

                    <div className="flex-1 flex justify-center items-center gap-2 pointer-events-none" data-tauri-drag-region>
                        <img src="/icon.svg" alt="Radiko" className="w-4 h-4 pointer-events-none" data-tauri-drag-region />
                        <span className="text-[11px] font-bold text-text-muted uppercase tracking-widest opacity-80" data-tauri-drag-region>Radiko Desktop</span>
                    </div>

                    <div className="flex items-center justify-end w-20">
                        <button
                            onClick={onOpenSettings}
                            className="w-8 h-8 rounded-lg flex items-center justify-center text-text-muted hover:bg-bg-surface-hover hover:text-white transition-all cursor-pointer"
                            title={t('sidebar.settings')}
                        >
                            <Settings size={16} />
                        </button>
                    </div>
                </>
            ) : (
                <>
                    {/* Windows/Linux Layout: Controls Right */}
                    <div className="flex items-center gap-2" data-tauri-drag-region>
                        <img src="/icon.svg" alt="Radiko" className="w-4 h-4 ml-2 pointer-events-none" data-tauri-drag-region />
                        <span className="text-[11px] font-bold text-text-muted uppercase tracking-widest opacity-80" data-tauri-drag-region>Radiko Desktop</span>
                    </div>

                    <div className="flex items-center justify-end">
                        <button
                            onClick={() => win.minimize()}
                            className="w-10 h-[38px] flex items-center justify-center text-text-muted hover:bg-bg-surface-hover hover:text-white transition-colors"
                        >
                            <Minus size={16} />
                        </button>
                        <button
                            onClick={() => win.toggleMaximize()}
                            className="w-10 h-[38px] flex items-center justify-center text-text-muted hover:bg-bg-surface-hover hover:text-white transition-colors"
                        >
                            <Square size={14} />
                        </button>
                        <button
                            onClick={() => win.close()}
                            className="w-10 h-[38px] flex items-center justify-center text-text-muted hover:bg-red-500 hover:text-white transition-colors"
                        >
                            <X size={18} />
                        </button>
                    </div>
                </>
            )}
        </header>
    );
}
