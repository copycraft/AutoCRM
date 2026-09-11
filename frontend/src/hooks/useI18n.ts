'use client';

import { useRouter, usePathname, useSearchParams } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { locales, type Locale } from '@/i18n';

export function useLocaleSwitcher() {
  const locale = useLocale();
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const changeLocale = (newLocale: Locale) => {
    const qs = searchParams.toString();
    router.push(`/${newLocale}${pathname}${qs ? `?${qs}` : ''}`);
  };

  return { locale, locales, changeLocale };
}

export function useT(namespace?: string) {
  return useTranslations(namespace);
}
