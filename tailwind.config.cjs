/** @type {import('tailwindcss').Config} */
module.exports = {
    content: ['./index.html', './src/**/*.{js,jsx}'],
    theme: {
        extend: {
            colors: {
                'bg-primary': 'rgb(var(--bg-primary) / <alpha-value>)',
                'bg-secondary': 'rgb(var(--bg-secondary) / <alpha-value>)',
                'bg-surface': 'rgb(var(--bg-surface) / <alpha-value>)',
                'bg-surface-hover': 'rgb(var(--bg-surface-hover) / <alpha-value>)',
                'bg-surface-active': 'rgb(var(--bg-surface-active) / <alpha-value>)',
                'accent': 'rgb(var(--accent) / <alpha-value>)',
                'accent-hover': 'rgb(var(--accent-hover) / <alpha-value>)',
                'accent-muted': 'var(--accent-muted)',
                'accent-glow': 'var(--accent-glow)',
                'text-primary': 'rgb(var(--text-primary) / <alpha-value>)',
                'text-secondary': 'rgb(var(--text-secondary) / <alpha-value>)',
                'text-muted': 'rgb(var(--text-muted) / <alpha-value>)',
                'border': 'rgb(var(--border) / <alpha-value>)',
                'border-active': 'rgb(var(--border-active) / <alpha-value>)',
                'success': 'rgb(var(--success) / <alpha-value>)',
                'warning': 'rgb(var(--warning) / <alpha-value>)',
                'danger': 'rgb(var(--danger) / <alpha-value>)',
            },
            fontFamily: {
                sans: ['Inter', 'system-ui', 'sans-serif'],
            },
        },
    },
    plugins: [
        require('@tailwindcss/typography'),
    ],
};
