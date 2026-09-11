// Shared routing config — NO server imports (middleware runs on Edge).
export const routing = {
  locales: ['hu', 'en'],
  defaultLocale: 'hu',
  localePrefix: 'always',
} as const;

export type Locale = (typeof routing.locales)[number];
