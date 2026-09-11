import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        panel: '#F7F8F7',
        surface: '#FFFFFF',
        'steel-900': '#1B2327',
        'steel-500': '#6B767C',
        'steel-200': '#D5DBDC',
        signal: '#E8590C',
        cold: '#0F5C7A',
        done: '#2F7A3E',
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
  plugins: [
    require('@tailwindcss/forms')({ strategy: 'class' }),
    require('@tailwindcss/typography'),
  ],
};

export default config;