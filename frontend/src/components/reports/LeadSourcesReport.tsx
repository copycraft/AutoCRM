'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { reportsApi } from '@/lib/api/endpoints';
import { cn } from '@/lib/utils/format';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';

const PERIODS = [30, 90, 365] as const;

const iso = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/** Won leads as a share of leads, as a whole percent. */
export function winRate(won: number, leads: number): string {
  return leads === 0 ? '—' : `${Math.round((won / leads) * 100)}%`;
}

/** Where website leads come from, and how many of them were won. */
export function LeadSourcesReport() {
  const t = useTranslations('leadSources');
  const [days, setDays] = useState<(typeof PERIODS)[number]>(90);
  const to = new Date();
  const from = new Date(to.getTime() - days * 86_400_000);
  const query = useQuery({
    queryKey: ['report-lead-sources', days],
    queryFn: () => reportsApi.leadSources({ from: iso(from), to: iso(to) }),
  });

  return (
    <section className="mt-10 space-y-4" aria-labelledby="lead-sources-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="lead-sources-title" className="text-section font-semibold">
            {t('title')}
          </h2>
          <p className="text-metadata text-steel-500">{t('subtitle')}</p>
        </div>
        <div role="group" aria-label={t('period')} className="flex gap-1">
          {PERIODS.map((p) => (
            <button
              key={p}
              aria-pressed={days === p}
              onClick={() => setDays(p)}
              className={cn('btn-sm', days === p ? 'btn-primary' : 'btn-ghost')}
            >
              {t('lastDays', { days: p })}
            </button>
          ))}
        </div>
      </div>

      {query.isLoading ? (
        <LoadingState />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : query.data!.total === 0 ? (
        <EmptyState title={t('empty')} hint={t('emptyHint')} />
      ) : (
        <>
          <p className="text-body">
            {t('summary', { total: query.data!.total, won: query.data!.won })}
          </p>
          <div className="card overflow-x-auto">
            <table className="w-full text-body" aria-label={t('byChannel')}>
              <thead>
                <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                  <th className="px-4 py-2 font-medium">{t('byChannel')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('leads')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('won')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('winRate')}</th>
                </tr>
              </thead>
              <tbody>
                {query.data!.by_channel.map((r) => (
                  <tr key={r.channel} className="border-b border-steel-200 last:border-0">
                    <td className="px-4 py-2">{t(`channels.${r.channel}`)}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.leads}</td>
                    <td className="px-4 py-2 text-right font-mono">{r.won}</td>
                    <td className="px-4 py-2 text-right font-mono">{winRate(r.won, r.leads)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {query.data!.by_campaign.length > 0 && (
            <div className="card overflow-x-auto">
              <table className="w-full text-body" aria-label={t('byCampaign')}>
                <thead>
                  <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                    <th className="px-4 py-2 font-medium">{t('byCampaign')}</th>
                    <th className="px-4 py-2 font-medium">{t('sourceMedium')}</th>
                    <th className="px-4 py-2 text-right font-medium">{t('leads')}</th>
                    <th className="px-4 py-2 text-right font-medium">{t('won')}</th>
                    <th className="px-4 py-2 text-right font-medium">{t('winRate')}</th>
                  </tr>
                </thead>
                <tbody>
                  {query.data!.by_campaign.map((r, i) => (
                    <tr key={i} className="border-b border-steel-200 last:border-0">
                      <td className="px-4 py-2">{r.utm_campaign ?? '—'}</td>
                      <td className="px-4 py-2 text-steel-500">
                        {[r.utm_source, r.utm_medium].filter(Boolean).join(' / ') || '—'}
                      </td>
                      <td className="px-4 py-2 text-right font-mono">{r.leads}</td>
                      <td className="px-4 py-2 text-right font-mono">{r.won}</td>
                      <td className="px-4 py-2 text-right font-mono">{winRate(r.won, r.leads)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {query.data!.by_page.length > 0 && (
            <div className="card overflow-x-auto">
              <table className="w-full text-body" aria-label={t('byPage')}>
                <thead>
                  <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                    <th className="px-4 py-2 font-medium">{t('byPage')}</th>
                    <th className="px-4 py-2 text-right font-medium">{t('leads')}</th>
                    <th className="px-4 py-2 text-right font-medium">{t('won')}</th>
                  </tr>
                </thead>
                <tbody>
                  {query.data!.by_page.map((r) => (
                    <tr key={r.landing_page} className="border-b border-steel-200 last:border-0">
                      <td className="px-4 py-2 font-mono text-metadata">{r.landing_page}</td>
                      <td className="px-4 py-2 text-right font-mono">{r.leads}</td>
                      <td className="px-4 py-2 text-right font-mono">{r.won}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </section>
  );
}
