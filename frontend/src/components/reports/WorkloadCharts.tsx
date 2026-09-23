'use client';

// Workshop load charts: orders placed per day and cars sitting in the workshop per day,
// from one `GET /reports/workload` call. Ranges are week (last 7 days), month (last 30)
// and month-to-date — the three questions the Monday meeting asks.
//
// The plots themselves live in WorkloadChartsView and arrive as a separate chunk: recharts
// is the largest thing on this route by some distance, and the totals below are worth more
// to the person opening the page than the pictures are.

import { useMemo, useState } from 'react';
import dynamic from 'next/dynamic';
import { useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { reportsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';

/** One row of `GET /reports/workload`. */
export type WorkloadDay = {
  date: string;
  placed: number;
  completed: number;
  in_workshop: number;
};

// ssr: false because recharts measures the DOM to size itself — server-rendering it
// produces markup that is thrown away on hydration, which is the cost without the benefit.
const WorkloadChartsView = dynamic(
  () => import('./WorkloadChartsView').then((m) => m.WorkloadChartsView),
  {
    ssr: false,
    loading: () => (
      <>
        <div className="card h-[21rem] animate-pulse p-4" aria-hidden />
        <div className="card h-[21rem] animate-pulse p-4" aria-hidden />
      </>
    ),
  },
);

type RangeKey = 'week' | 'month' | 'mtd';

function isoDay(d: Date): string {
  return d.toISOString().slice(0, 10);
}

function rangeFor(key: RangeKey): { from: string; to: string } {
  const to = new Date();
  const toIso = isoDay(to);
  if (key === 'week') {
    const from = new Date(to);
    from.setDate(from.getDate() - 6);
    return { from: isoDay(from), to: toIso };
  }
  if (key === 'month') {
    const from = new Date(to);
    from.setDate(from.getDate() - 29);
    return { from: isoDay(from), to: toIso };
  }
  return { from: `${toIso.slice(0, 8)}01`, to: toIso };
}

export function WorkloadCharts() {
  const t = useTranslations('reports');
  const [range, setRange] = useState<RangeKey>('week');
  const { from, to } = useMemo(() => rangeFor(range), [range]);

  const query = useQuery({
    queryKey: qk.reports('workload', { from, to }),
    queryFn: () => reportsApi.workload({ from, to }),
  });

  const ranges: { key: RangeKey; label: string }[] = [
    { key: 'week', label: t('rangeWeek') },
    { key: 'month', label: t('rangeMonth') },
    { key: 'mtd', label: t('rangeMtd') },
  ];

  if (query.isLoading) return <DetailSkeleton />;
  if (query.isError || !query.data) {
    return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  }

  const days = query.data.days;
  const totalPlaced = days.reduce((n, d) => n + d.placed, 0);
  const totalCompleted = days.reduce((n, d) => n + d.completed, 0);
  const avgInWorkshop =
    days.length > 0 ? days.reduce((n, d) => n + d.in_workshop, 0) / days.length : 0;

  return (
    <section aria-label={t('workload')} className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-section font-semibold">{t('workload')}</h2>
        <select
          value={range}
          onChange={(e) => setRange(e.target.value as RangeKey)}
          className="input w-auto"
          aria-label={t('workload')}
        >
          {ranges.map((r) => (
            <option key={r.key} value={r.key}>
              {r.label}
            </option>
          ))}
        </select>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <div className="card p-4">
          <p className="font-mono text-record font-semibold">{totalPlaced}</p>
          <p className="text-metadata text-steel-500">{t('totalPlaced')}</p>
        </div>
        <div className="card p-4">
          <p className="font-mono text-record font-semibold">{totalCompleted}</p>
          <p className="text-metadata text-steel-500">{t('totalCompleted')}</p>
        </div>
        <div className="card p-4">
          <p className="font-mono text-record font-semibold">{avgInWorkshop.toFixed(1)}</p>
          <p className="text-metadata text-steel-500">{t('avgInWorkshop')}</p>
        </div>
      </div>

      <WorkloadChartsView days={days} />
    </section>
  );
}
