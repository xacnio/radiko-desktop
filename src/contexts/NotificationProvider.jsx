import { createContext, useContext, useState, useCallback, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { X, Music, CheckCircle2, AlertCircle, Info } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const NotificationContext = createContext(null);

export const useNotification = () => {
    const context = useContext(NotificationContext);
    if (!context) {
        throw new Error('useNotification must be used within a NotificationProvider');
    }
    return context;
};

export const NotificationProvider = ({ children, isPlayerHorizontal, linkViewOpen, sidebarWidth, playerWidth = 260 }) => {
    const [notifications, setNotifications] = useState([]);
    const [hasActiveStation, setHasActiveStation] = useState(false);

    const removeNotification = useCallback((id) => {
        setNotifications((prev) => prev.map(n => n.id === id ? { ...n, hiding: true } : n));
        setTimeout(() => {
            setNotifications((prev) => prev.filter((n) => n.id !== id));
        }, 400);
    }, []);

    const notify = useCallback((options) => {
        const id = Math.random().toString(36).substr(2, 9);
        const newNotif = {
            id,
            type: options.type || 'info', // shazam | success | error | info
            title: options.title,
            message: options.message,
            artist: options.artist, // shazam specific
            cover: options.cover,   // shazam specific
            onClick: options.onClick, // custom click handler
            duration: options.duration || 5000,
            hiding: false,
        };

        setNotifications((prev) => [...prev, newNotif]);

        return id;
    }, []);


    // When linkViewOpen is true, the player is ALWAYS at the bottom (horizontal layout forced)
    const effectiveHorizontal = isPlayerHorizontal || linkViewOpen;
    const playerVisible = hasActiveStation;

    const rightOffset = linkViewOpen ? sidebarWidth + 16 : ((effectiveHorizontal && playerVisible) ? 24 : ((!effectiveHorizontal && playerVisible) ? playerWidth + 24 : 24));
    const bottomOffset = (effectiveHorizontal && playerVisible) ? 116 : 24;

    return (
        <NotificationContext.Provider value={{ notify, removeNotification, setHasActiveStation }}>
            {children}
            {createPortal(
                <div
                    className="fixed z-[99999] pointer-events-none flex flex-col gap-3 transition-all duration-500 items-end"
                    style={{ bottom: `${bottomOffset}px`, right: `${rightOffset}px` }}
                >
                    {notifications.map((n) => (
                        <NotificationToast
                            key={n.id}
                            notif={n}
                            onClose={() => removeNotification(n.id)}
                            isPlayerHorizontal={isPlayerHorizontal}
                        />
                    ))}
                </div>,
                document.body
            )}
        </NotificationContext.Provider>
    );
};

const NotificationToast = ({ notif, onClose }) => {
    const isShazam = notif.type === 'shazam';
    const [remaining, setRemaining] = useState(notif.duration);
    const [isPaused, setIsPaused] = useState(false);
    const expiredRef = useRef(false);

    useEffect(() => {
        if (notif.duration === Infinity || notif.hiding || isPaused) return;

        const interval = setInterval(() => {
            setRemaining((prev) => {
                const next = prev - 10;
                if (next <= 0) {
                    clearInterval(interval);
                    expiredRef.current = true;
                    return 0;
                }
                return next;
            });
        }, 10);

        return () => clearInterval(interval);
    }, [isPaused, notif.hiding, notif.duration]);

    useEffect(() => {
        if (remaining <= 0 && expiredRef.current) {
            expiredRef.current = false;
            onClose();
        }
    }, [remaining, onClose]);

    const progress = (remaining / notif.duration) * 100;

    const { t } = useTranslation();
    return (
        <div
            className={`notification-toast pointer-events-auto bg-bg-secondary/95 backdrop-blur-md border shadow-[0_10px_40px_rgba(0,0,0,0.5)] rounded-2xl p-2.5 flex flex-col relative overflow-hidden transition-all duration-500 ease-in-out
            ${notif.hiding
                    ? 'opacity-0 scale-90 translate-y-4 max-h-0 py-0 my-[-6px] border-transparent'
                    : 'opacity-100 scale-100 translate-y-0 max-h-[500px] border-border/50'}
            ${isShazam && !notif.hiding ? 'border-accent/30 w-[260px]' : 'w-[280px]'}
            ${notif.onClick && !notif.hiding ? 'cursor-pointer hover:bg-bg-surface-hover hover:scale-[1.02]' : ''}
            animate-in slide-in-from-bottom-4 slide-in-from-right-4 fade-in duration-300`}
            role="alert"
            onMouseEnter={() => setIsPaused(true)}
            onMouseLeave={() => setIsPaused(false)}
            onClick={() => {
                if (notif.onClick && !notif.hiding) {
                    notif.onClick();
                    onClose();
                }
            }}
        >
            <div className={`flex items-center gap-3 w-full transition-opacity duration-300 ${notif.hiding ? 'opacity-0' : 'opacity-100'}`}>
                {/* Visual Icon / Cover */}
                <div className={`shrink-0 rounded-lg overflow-hidden border border-border/50
                    ${isShazam ? 'w-10 h-10 shadow-[0_0_15px_rgba(var(--accent),0.2)]' : 'w-9 h-9 flex items-center justify-center bg-bg-surface'}`}
                >
                    {isShazam ? (
                        notif.cover ? (
                            <img src={notif.cover} className="w-full h-full object-cover" alt="" />
                        ) : (
                            <div className="w-full h-full bg-accent/10 flex items-center justify-center text-accent">
                                <Music size={16} />
                            </div>
                        )
                    ) : (
                        <>
                            {notif.type === 'success' && <CheckCircle2 size={18} className="text-accent" />}
                            {notif.type === 'error' && <AlertCircle size={18} className="text-red-400" />}
                            {notif.type === 'info' && <Info size={18} className="text-accent" />}
                        </>
                    )}
                </div>

                {/* Content */}
                <div className="flex-1 min-w-0 flex flex-col justify-center">
                    {isShazam ? (
                        <>
                            <div className="flex items-center gap-1.5 mb-0.5">
                                <span className="w-1.5 h-1.5 rounded-full bg-accent animate-pulse shrink-0" />
                                <span className="text-[9px] font-black text-accent tracking-widest uppercase leading-none">{t('notifications.autoFound')}</span>
                            </div>
                            <div className="text-xs font-bold text-text-primary break-words leading-snug tracking-tight">{notif.title}</div>
                            <div className="text-[10px] text-text-secondary break-words leading-tight opacity-80">{notif.artist}</div>
                        </>
                    ) : (
                        <>
                            {notif.title && <div className="text-xs font-bold text-text-primary break-words leading-snug tracking-tight">{notif.title}</div>}
                            <div className="text-[10px] text-text-muted leading-relaxed mt-0.5 break-words whitespace-pre-wrap">{notif.message}</div>
                        </>
                    )}
                </div>

                {/* Close */}
                <button
                    onClick={onClose}
                    className="text-text-muted hover:text-text-primary transition-colors cursor-pointer shrink-0 ml-1 self-start mt-0.5"
                >
                    <X size={14} />
                </button>
            </div>

            {/* Progress Bar */}
            {notif.duration !== Infinity && (
                <div className="absolute bottom-0 left-0 h-[2.5px] w-full bg-accent/5 overflow-hidden">
                    <div
                        className="h-full bg-accent transition-all duration-100 ease-linear shadow-[0_0_10px_rgba(var(--accent),0.5)]"
                        style={{ width: `${progress}%` }}
                    />
                </div>
            )}
        </div>
    );
};
