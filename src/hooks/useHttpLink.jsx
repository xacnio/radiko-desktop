import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import ConfirmModal from '../components/common/ConfirmModal';

export function useHttpLink(t) {
    const [pending, setPending] = useState(null);

    const openLink = (url) => {
        if (!url) return;
        if (!url.startsWith('http://') && !url.startsWith('https://')) url = 'https://' + url;
        if (url.startsWith('http://')) {
            setPending({ url });
        } else {
            invoke('open_link_window', { url }).catch(() => {});
        }
    };

    const modal = pending ? (
        <ConfirmModal
            isOpen={true}
            title={t?.('app.httpWarningTitle') || 'Insecure Connection'}
            message={t?.('app.httpWarningMsg', { url: pending.url }) || `This site does not use an encrypted connection (HTTPS).\n${pending.url}`}
            confirmText={t?.('app.httpWarningConfirm') || 'Open Anyway'}
            cancelText={t?.('common.cancel') || 'Cancel'}
            variant="warning"
            showCancel={true}
            onConfirm={() => {
                invoke('open_link_window', { url: pending.url }).catch(() => {});
            }}
            onClose={() => setPending(null)}
        />
    ) : null;

    return { openLink, modal };
}
