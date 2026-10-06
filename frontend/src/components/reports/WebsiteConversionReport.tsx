'use client';

// Per source website: leads → quoted → won → orders → lost, and why the period's lost
// leads were lost. A website is the domain the lead was tagged from on arrival.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { reportsApi } from '@/lib/api/endpoints';
import { cn } from '@/lib/utils/format';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';
import { winRate } from './LeadSourcesReport';

const PERIODS = [30, 90, 365] as const;

const iso = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

export function WebsiteConversionReport() {
  const t = useTranslations('websiteConversion');
  const tl = useTranslations('leadSources');
  const [days, setDays] = useState<(typeof PERIODS)[number]>(90);
  const to = new Date();
  const from = new Date(to.getTime() - days * 86_400_000);
  const query = useQuery({
    queryKey: ['report-website-conversion', days],
    queryFn: () => reportsApi.websiteConversion({ from: iso(from), to: iso(to) }),
  });
  const rows = query.data?.rows ?? [];
  const reasons = query.data?.lost_reasons ?? [];
  const lostTotal = reasons.reduce((n, r) => n + r.leads, 0);

  return (
    <section className="mt-10 space-y-4" aria-labelledby="website-conversion-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="website-conversion-title" className="text-section font-semibold">{t('title')}</h2>
          <p className="text-metadata text-steel-500">{t('subtitle')}</p>
        </div>
        <div role="group" aria-label={tl('period')} className="flex gap-1">
          {PERIODS.map((p) => (
            <button
              key={p}
              aria-pressed={days === p}
              onClick={() => setDays(p)}
              className={cn('btn-sm', days === p ? 'btn-primary' : 'btn-ghost')}
            >
              {tl('lastDays', { days: p })}
            </button>
          ))}
        </div>
      </div>

      {query.isLoading ? (
        <LoadingState />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : rows.length === 0 && reasons.length === 0 ? (
        <EmptyState title={t('empty')} hint={t('emptyHint')} />
      ) : (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
          <div className="card overflow-x-auto">
            <table className="w-full text-body" aria-label={t('bySite')}>
              <thead>
                <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                  <th className="px-4 py-2 font-medium">{t('bySite')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('leads')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('quoted')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('won')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('orders')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('lost')}</th>
                  <th className="px-4 py-2 text-right font-medium">{tl('winRate')}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => (
                  <tr key={r.site} className="border-b border-steel-200 last:border-0">
                    <td className="px-4 py-2 font-mono">{r.site}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.leads}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.quoted}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.won}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.orders}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.lost}</td>
                    <td className="px-4 py-2 text-right font-mono">{winRate(r.orders, r.leads)}</td>
                  </tr>
                ))}
                {rows.length === 0 && (
                  <tr>
                    <td colSpan={7} className="px-4 py-3 text-steel-500">{t('noSites')}</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          <div className="card p-4" data-testid="lost-reasons-report">
            <h3 className="mb-2 text-body font-semibold">{t('lostReasons')}</h3>
            {reasons.length === 0 ? (
              <p className="text-metadata text-steel-500">{t('noLost')}</p>
            ) : (
              <ul className="space-y-2">
                {reasons.map((r) => (
                  <li key={r.reason}>
                    <div className="flex justify-between text-body">
                      <span>{r.reason}</span>
                      <span className="font-mono">{r.leads}</span>
                    </div>
                    <div className="mt-1 h-1.5 rounded-full bg-steel-200">
                      <div
                        className="h-1.5 rounded-full bg-steel-900"
                        style={{ width: `${Math.round((r.leads / Math.max(lostTotal, 1)) * 100)}%` }}
                      />
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
