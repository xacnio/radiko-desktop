import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * Convert a favicon URL for display in the webview.
 * file:/// URLs are converted to Tauri asset protocol URLs.
 * Remote URLs are passed through unchanged.
 */
export function toAssetUrl(url) {
    if (!url) return '';
    if (url.startsWith('file:///')) {
        const path = url.slice(8); // strip "file:///"
        return convertFileSrc(path);
    }
    return url;
}
