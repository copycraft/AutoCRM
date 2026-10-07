'use client';

// Marketing holds two sections: the newsletter (/marketing) and the leads (/leads). They keep
// their own URLs — lead links in mail, notifications and the phone point at /leads — and
// share this tab bar. The sidebar's Marketing entry opens whichever was used last.

import { useEffect } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { cn } from '@/lib/utils/format';

const LAST_KEY = 'autocrm.marketing.lastTab';

export type MarketingTab = 'newsletter' | 'leads' | 'emails' | 'templates';

const TABS: { key: MarketingTab; href: string; label: 'tabNewsletter' | 'tabLeads' | 'tabEmails' | 'tabTemplates' }[] = [
  { key: 'newsletter', href: '/marketing', label: 'tabNewsletter' },
  { key: 'leads', href: '/leads', label: 'tabLeads' },
  { key: 'emails', href: '/emails', label: 'tabEmails' },
  { key: 'templates', href: '/templates', label: 'tabTemplates' },
];

/** Where the sidebar's Marketing entry goes: the tab used last, the newsletter at first. */
export function marketingHref(locale: string): string {
  let last: string | null = null;
  try {
    last = window.localStorage.getItem(LAST_KEY);
  } catch {
    /* storage blocked: the default is fine */
  }
  const tab = TABS.find((x) => x.key === last) ?? TABS[0]!;
  return `/${locale}${tab.href}`;
}

/** True for every page that belongs to Marketing, leads' own pages included. */
export function isMarketingPath(pathname: string, locale: string): boolean {
  return TABS.some((x) => {
    const base = `/${locale}${x.href}`;
    return pathname === base || pathname.startsWith(`${base}/`);
  });
}

export function MarketingTabs() {
  const t = useTranslations('navigation');
  const locale = useLocale();
  const pathname = usePathname();
  const active = TABS.find((x) => {
    const base = `/${locale}${x.href}`;
    return pathname === base || pathname.startsWith(`${base}/`);
  })?.key;

  useEffect(() => {
    if (!active) return;
    try {
      window.localStorage.setItem(LAST_KEY, active);
    } catch {
      /* storage blocked: nothing to remember */
    }
  }, [active]);

  return (
    <nav className="-mt-2 flex gap-1 border-b border-steel-200" aria-label={t('marketing')}>
      {TABS.map((x) => (
        <Link
          key={x.key}
          href={`/${locale}${x.href}`}
          aria-current={active === x.key ? 'page' : undefined}
          className={cn(
            '-mb-px border-b-2 px-4 py-2 text-body font-medium transition-colors',
            active === x.key
              ? 'border-steel-900 text-steel-900'
              : 'border-transparent text-steel-500 hover:text-steel-900',
          )}
        >
          {t(x.label)}
        </Link>
      ))}
    </nav>
  );
}
