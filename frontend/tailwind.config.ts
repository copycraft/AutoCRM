import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      // Each colour is a role, not a shade, set per theme in globals.css (light by default,
      // dark under [data-theme]). `<alpha-value>` keeps modifiers like `bg-steel-900/80`.
      colors: {
        panel: 'rgb(var(--c-panel) / <alpha-value>)',
        surface: 'rgb(var(--c-surface) / <alpha-value>)',
        'steel-900': 'rgb(var(--c-steel-900) / <alpha-value>)',
        'steel-500': 'rgb(var(--c-steel-500) / <alpha-value>)',
        'steel-200': 'rgb(var(--c-steel-200) / <alpha-value>)',
        signal: 'rgb(var(--c-signal) / <alpha-value>)',
        cold: 'rgb(var(--c-cold) / <alpha-value>)',
        done: 'rgb(var(--c-done) / <alpha-value>)',
      },
      fontFamily: {
        sans: ['var(--font-plex-sans)', 'system-ui', 'sans-serif'],
        mono: ['var(--font-plex-mono)', 'monospace'],
      },
      fontSize: {
        'metadata': ['12.8px', { lineHeight: '1.5', letterSpacing: '0.01em' }],
        'body': ['16px', { lineHeight: '1.5', letterSpacing: '0.01em' }],
        'section': ['20px', { lineHeight: '1.4', letterSpacing: '0.01em' }],
        'record-title': ['25px', { lineHeight: '1.3', letterSpacing: '0.01em' }],
        'page-title': ['31px', { lineHeight: '1.2', letterSpacing: '0.01em' }],
      },
      spacing: {
        '18': '4.5rem',
        '88': '22rem',
      },
      boxShadow: {
        'panel': '0 1px 2px 0 rgb(0 0 0 / 0.05)',
        'card': '0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1)',
      },
      borderRadius: {
        'xl': '0.75rem',
        '2xl': '1rem',
      },
    },
  },
  // Forms plugin on class strategy: no global element restyles, so its
  // blue focus/checkbox colours never reach the built CSS. Checkboxes are
  // styled explicitly with accent-steel-900.
  plugins: [require('@tailwindcss/forms')({ strategy: 'class' })],
};

export default config;