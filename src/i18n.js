import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { invoke } from '@tauri-apps/api/core';

const localeModules = import.meta.glob('./locales/*.json', { eager: true });

export const availableLanguages = [];
const resources = {};

for (const path in localeModules) {
    const langCode = path.match(/\/([^/]+)\.json$/)[1];
    const translation = localeModules[path].default || localeModules[path];
    resources[langCode] = { translation };
    availableLanguages.push({
        code: langCode,
        name: translation.common?.language_name || langCode.toUpperCase()
    });
}

i18n
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
        resources,
        fallbackLng: 'en', // Default language is English
        interpolation: {
            escapeValue: false, // React already safe from xss
        },
        detection: {
            // Attempt to detect language from localStorage first, then browser
            order: ['localStorage', 'navigator'],
            caches: ['localStorage'],
        }
    });

i18n.on('languageChanged', (lng) => {
    document.documentElement.lang = lng;
    try {
        invoke('save_language', { lang: lng }).catch(console.error);
    } catch (e) {
        console.error(e);
    }
});

// Set initial language
if (i18n.language) {
    document.documentElement.lang = i18n.language;
}

export default i18n;
