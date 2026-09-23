'use client';

import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useEffect, useState } from 'react';
import { ArrowRight, X } from 'lucide-react';
import { useRecentRecords } from '@/hooks/useRecent';

export function RecentRecords() {  const t = useTranslations('qol');
  const locale = useLocale();
  const { records, clear } = useRecentRecords(6);
  void locale;
  if (records.length === 0) return null;
  return (
    <section aria-label={t('recentTitle')}>
      <div className="flex items-center justify-between">
        <h2 className="text-section font-semibold">{t('recentTitle')}</h2>
        <button
          type="button"
          className="btn-ghost btn-sm"
          onClick={clear}
          aria-label={t('recentClear')}
        >
          {t('recentClear')}
        </button>
      </div>
      <ul className="mt-3 space-y-2">
        {records.map((r) => (
          <li key={r.href}>
            <Link
              href={r.href}
              className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4 hover:border-steel-900"
            >
              <span className="min-w-0 flex-1 text-body font-medium">{r.title}</span>
              {r.sub && (
                <span className="truncate text-metadata text-steel-500">{r.sub}</span>
              )}
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** One-line "pick up where you left off" banner for the dashboard top. */
export function ResumeBanner() {
  const t = useTranslations('qol');
  const { records } = useRecentRecords(6);
  const [dismissed, setDismissed] = useState<string | null>(null);

  useEffect(() => {
    try {
      setDismissed(window.localStorage.getItem('autocrm:resume-dismissed'));
    } catch {
      /* best-effort */
    }
  }, []);

  const top = records[0];
  if (!top || dismissed === top.href) return null;

  const dismiss = () => {
    setDismissed(top.href);
    try {
      window.localStorage.setItem('autocrm:resume-dismissed', top.href);
    } catch {
      /* best-effort */
    }
  };

  return (
    <div className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4">
      <div className="min-w-0 flex-1">
        <p className="text-metadata text-steel-500">
          {t('resumeTitle')} · {t('resumeBody')}
        </p>
        <Link href={top.href} className="text-body font-medium underline">
          {top.title}
        </Link>
        {top.sub && <span className="text-metadata text-steel-500"> · {top.sub}</span>}
      </div>
      <Link href={top.href} className="btn-secondary btn-sm" aria-label={t('resumeTitle')}>
        <ArrowRight className="h-4 w-4" aria-hidden />
      </Link>
      <button
        type="button"
        className="btn-ghost btn-sm"
        onClick={dismiss}
        aria-label={t('resumeDismiss')}
      >
        <X className="h-4 w-4" aria-hidden />
      </button>
    </div>
  );
}
