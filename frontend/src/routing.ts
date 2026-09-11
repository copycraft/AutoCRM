// Shared routing config — NO server imports (middleware runs on Edge).
// Hungarian is the only locale (R4.3): a half-translated second locale is
// worse than one complete one.
export const routing = {
  locales: ['hu'],
  defaultLocale: 'hu',
  localePrefix: 'always',
} as const;

export type Locale = (typeof routing.locales)[number];
