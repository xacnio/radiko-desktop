import { useTranslation } from 'react-i18next';

export default function MiniPlayer({
    station, status, songTitle, volume,
    onToggle, onStop, onVolumeChange
}) {
    const { t } = useTranslation();
    const isPlaying = status === 'playing';
    const isConnecting = status === 'connecting' || status === 'reconnecting';

    const statusText = {
        playing: t('player.status.playing'),
        paused: t('player.status.paused'),
        stopped: t('player.status.stopped'),
        connecting: t('player.status.connecting'),
        reconnecting: t('player.status.reconnecting')
    }[status] || '';

    const dotColor = isPlaying ? 'bg-success' : isConnecting ? 'bg-warning' : status === 'paused' ? 'bg-warning' : 'bg-text-muted';

    return (
        <footer className="flex items-center gap-3 px-4 h-14 bg-bg-secondary border-t border-border shrink-0">
            {/* Play/Stop */}
            <div className="flex gap-1.5 shrink-0">
                <button
                    onClick={onToggle}
                    disabled={!station}
                    className="w-8 h-8 rounded-full bg-accent text-bg-primary flex items-center justify-center transition-all hover:bg-accent-hover disabled:opacity-25 disabled:cursor-not-allowed cursor-pointer"
                >
                    {isPlaying || isConnecting ? (
                        <svg viewBox="0 0 24 24" fill="currentColor" className="w-3.5 h-3.5">
                            <rect x="6" y="4" width="3" height="16" />
                            <rect x="14" y="4" width="3" height="16" />
                        </svg>
                    ) : (
                        <svg viewBox="0 0 24 24" fill="currentColor" className="w-3.5 h-3.5 ml-0.5">
                            <polygon points="7,4 19,12 7,20" />
                        </svg>
                    )}
                </button>
                <button
                    onClick={onStop}
                    disabled={!station}
                    className="w-8 h-8 rounded-full bg-bg-surface border border-border text-text-secondary flex items-center justify-center hover:bg-bg-surface-hover hover:text-text-primary transition-all disabled:opacity-25 disabled:cursor-not-allowed cursor-pointer"
                >
                    <svg viewBox="0 0 24 24" fill="currentColor" className="w-3 h-3">
                        <rect x="6" y="6" width="12" height="12" rx="1.5" />
                    </svg>
                </button>
            </div>

            {/* Info */}
            <div className="flex-1 min-w-0 flex items-center gap-2">
                {station && (
                    <>
                        <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${dotColor} ${isPlaying || isConnecting ? 'animate-pulse-dot' : ''}`} />
                        <div className="min-w-0">
                            <div className="text-xs font-semibold text-text-primary truncate">
                                {station.name}
                                {songTitle && <span className="text-text-secondary font-normal"> — {songTitle}</span>}
                            </div>
                            <div className="text-[10px] text-text-muted">{statusText}</div>
                        </div>
                    </>
                )}
                {!station && <span className="text-xs text-text-muted">{t('player.selectRadio', 'Select a radio')}</span>}
            </div>

            {/* Volume */}
            <div className="flex items-center gap-2 shrink-0">
                <span className="text-xs">{volume === 0 ? '🔇' : volume < 50 ? '🔉' : '🔊'}</span>
                <input
                    type="range"
                    min="0" max="100"
                    value={volume}
                    onChange={e => onVolumeChange(Number(e.target.value))}
                    className="w-24 mini-vol-slider"
                    style={{
                        background: `linear-gradient(to right, #f59e0b ${volume}%, #333333 ${volume}%)`
                    }}
                />
            </div>
        </footer>
    );
}
