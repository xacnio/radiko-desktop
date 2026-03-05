import { AlertCircle, AlertTriangle, X, Globe } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export default function ConfirmModal({
    isOpen,
    onClose,
    onConfirm,
    title,
    message,
    confirmText,
    cancelText,
    variant = "danger",
    showCancel = true,
    neutralText,
    onNeutral
}) {
    const { t } = useTranslation();

    if (!isOpen) return null;

    const finalTitle = title || t('common.confirm');
    const finalConfirmText = confirmText || t('common.yes');
    const finalCancelText = cancelText || t('common.cancel');

    // Simple parser for bullet points if message is a string with newlines or special markers
    const renderMessage = () => {
        if (!message) return null;
        const lines = typeof message === 'string' ? message.split('\n').filter(Boolean) : [];

        if (lines.length > 1) {
            return (
                <div className="space-y-3">
                    {lines.map((line, idx) => {
                        const isPrimary = idx === 0 && !line.startsWith('•') && !line.startsWith('-');
                        return (
                            <div key={idx} className={`flex gap-3 ${isPrimary ? 'mb-4' : ''}`}>
                                {!isPrimary && <div className={`w-1.5 h-1.5 rounded-full mt-1.5 shrink-0 ${variant === 'warning' ? 'bg-accent' : 'bg-red-500'}`} />}
                                <p className={`${isPrimary ? 'text-sm font-semibold text-text-primary' : 'text-xs text-text-muted'} leading-relaxed`}>
                                    {line.replace(/^[•-]\s*/, '')}
                                </p>
                            </div>
                        );
                    })}
                </div>
            );
        }

        return <p className="text-text-primary text-sm leading-relaxed">{message}</p>;
    };

    return (
        <div className="fixed inset-0 bg-black/80 z-[10000] flex items-center justify-center p-4 backdrop-blur-md animate-in fade-in duration-300">
            <div
                className={`ctx-menu-container bg-bg-secondary border rounded-2xl w-full max-w-md flex flex-col shadow-[0_20px_50px_rgba(0,0,0,0.5)] overflow-hidden animate-in zoom-in duration-300
                    ${variant === 'warning' ? 'border-accent/20 shadow-accent/5' : 'border-border/50'}`}
                onClick={e => e.stopPropagation()}
            >
                {/* Header Section */}
                <div className={`px-6 py-5 flex items-center gap-4 ${variant === 'warning' ? 'bg-accent/5' : 'bg-bg-surface'} border-b border-border/50`}>
                    <div className={`w-12 h-12 rounded-2xl flex items-center justify-center shrink-0
                        ${variant === 'warning' ? 'bg-accent/20 text-accent shadow-[0_0_20px_rgba(var(--accent),0.2)]' : 'bg-red-500/20 text-red-500'}`}>
                        {variant === 'warning' ? <AlertTriangle size={24} /> : <AlertCircle size={24} />}
                    </div>
                    <div className="flex-1 min-w-0">
                        <h2 className="text-base font-bold text-text-primary truncate">{finalTitle}</h2>
                    </div>
                    <button onClick={onClose} className="p-2 -mr-2 text-text-muted hover:text-text-primary hover:bg-bg-surface-hover rounded-full transition-all">
                        <X size={20} />
                    </button>
                </div>

                {/* Content Section */}
                <div className="p-6">
                    <div className={`p-4 rounded-xl mb-6 ${variant === 'warning' ? 'bg-accent/5 border border-accent/10' : 'bg-bg-surface border border-border/30'}`}>
                        {renderMessage()}

                        {neutralText && onNeutral && (
                            <button
                                onClick={onNeutral}
                                className={`mt-4 w-full py-2.5 flex items-center justify-center gap-2 rounded-lg border text-xs font-bold transition-all
                                    ${variant === 'warning'
                                        ? 'bg-accent/5 border-accent/20 text-accent hover:bg-accent/10'
                                        : 'bg-accent/5 border-accent/20 text-accent hover:bg-accent/10'}`}
                            >
                                <Globe size={14} />
                                {neutralText}
                            </button>
                        )}
                    </div>

                    <div className="flex gap-3">
                        {showCancel && (
                            <button
                                onClick={onClose}
                                className="flex-1 py-3 bg-bg-surface hover:bg-bg-surface-hover text-text-primary rounded-xl font-bold text-xs transition-all border border-border/50"
                            >
                                {finalCancelText}
                            </button>
                        )}
                        <button
                            onClick={() => {
                                onConfirm && onConfirm();
                                onClose();
                            }}
                            className="flex-[1.5] py-3 rounded-xl font-bold text-xs transition-all text-white shadow-xl bg-gradient-to-r from-accent to-accent/80 hover:from-accent/90 hover:to-accent/70 shadow-accent/20"
                            style={variant === 'warning' ? {
                                background: 'linear-gradient(to right, rgb(var(--accent)), rgb(var(--accent) / 0.8))',
                                boxShadow: '0 10px 15px -3px rgb(var(--accent) / 0.3)'
                            } : variant === 'danger' ? {
                                background: 'linear-gradient(to right, #dc2626, #b91d1d)',
                                boxShadow: '0 10px 15px -3px rgba(220, 38, 38, 0.3)'
                            } : {}}
                        >
                            {finalConfirmText}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
