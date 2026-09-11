import { getRequestConfig } from 'next-intl/server';
import { routing, type Locale } from './routing';

// Re-exported so existing imports from '@/i18n' keep working.
export { routing };
export const locales = routing.locales;
export const defaultLocale = routing.defaultLocale;
export type { Locale };

export default getRequestConfig(async ({ requestLocale }) => {
  const requested = await requestLocale;
  const locale: Locale =
    requested && (routing.locales as readonly string[]).includes(requested)
      ? (requested as Locale)
      : routing.defaultLocale;
  return {
    locale,
    messages: (await import(`./messages/${locale}.json`)).default,
  };
});
