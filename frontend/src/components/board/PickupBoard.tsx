'use client';

// Pickup board: the tablet/TV on the shop wall. Big plates ready for collection,
// the workshop still working underneath. Auto-refreshes every 30 seconds; no buttons,
// no nav needed — this page IS the display. Authenticated like everything else:
// customer plates on a wall screen stay inside the shop's login.

import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQueries, useQuery } from '@tanstack/react-query';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { configApi, ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { daysSince } from '@/lib/utils/format';

export function PickupBoard() {
  const t = useTranslations('board');
  const locale = useLocale();

  // The board's two halves are derived from the server's stage definitions, not
  // from a local stage list: a workshop stage the office adds appears here
  // without a web deploy. Ready = active terminal stages that are not the exit
  // (collected cars, not cancelled ones); working = every other active stage
  // past the entry one (new arrivals are not "in work" yet).
  const definitions = useQuery({
    queryKey: qk.stages('order'),
    queryFn: () => configApi.stages('order'),
    refetchInterval: 30_000,
    staleTime: 15_000,
  });
  const orderStages = (definitions.data?.items ?? []).filter((s) => s.is_active);
  const entryPosition = Math.min(
    ...orderStages.filter((s) => !s.is_terminal).map((s) => s.position),
  );
  const readyStages = orderStages.filter((s) => s.is_terminal && !s.is_exit).map((s) => s.key);
  const workStages = orderStages
    .filter((s) => !s.is_terminal && !s.is_exit && s.position > entryPosition)
    .sort((a, b) => a.position - b.position)
    .map((s) => s.key);
  const stagesReady = definitions.isSuccess;

  const readyQueries = useQueries({
    queries: (stagesReady ? readyStages : []).map((stage) => ({
      queryKey: qk.orders({ stage, board: true }),
      queryFn: () => ordersApi.list({ stage, limit: 30, sort: '-stage_entered_at' }),
      refetchInterval: 30_000,
      staleTime: 15_000,
    })),
  });
  const ready = {
    isPending: !stagesReady || readyQueries.some((q) => q.isPending),
    isError: definitions.isError || readyQueries.some((q) => q.isError),
    error: definitions.error ?? readyQueries.find((q) => q.isError)?.error ?? null,
    refetch: () => {
      void definitions.refetch();
      void Promise.all(readyQueries.map((q) => q.refetch()));
    },
  };
  // One query per workshop stage, filtered on the server. Fetching the first page of
  // *all* open orders and filtering here dropped older in-work cars as soon as newer
  // intake orders filled that page.
  const workingQueries = useQueries({
    queries: (stagesReady ? workStages : []).map((stage) => ({
      queryKey: qk.orders({ stage, board: true }),
      queryFn: () => ordersApi.list({ stage, limit: 200 }),
      refetchInterval: 30_000,
      staleTime: 15_000,
    })),
  });
  const working = {
    isPending: workingQueries.some((q) => q.isPending),
    isError: workingQueries.some((q) => q.isError),
    error: workingQueries.find((q) => q.isError)?.error ?? null,
    refetch: () => Promise.all(workingQueries.map((q) => q.refetch())),
  };

  // Progressive: each half paints when its own query lands instead of holding
  // the whole wall display behind the slower of the two.
  // Server-side sort: the 30 most recently *completed* cars per stage, not the
  // 30 most recently created completed orders (ORD-L5). The client re-sort
  // below is only a tiebreak for rows the server already ordered.
  const readyCars = readyQueries
    .flatMap((q) => q.data?.items ?? [])
    .sort((a, b) => (a.stage_entered_at > b.stage_entered_at ? -1 : 1))
    .slice(0, 30);
  const inWork = workingQueries.flatMap((q) => q.data?.items ?? []);

  // The route (app/[locale]/board/page.tsx) supplies the one AppShell.
  return (
    <>
      <section aria-label={t('readyTitle')}>
        <h2 className="text-page-title font-semibold">{t('readyTitle')}</h2>
        {ready.isPending ? (
          <DetailSkeleton />
        ) : ready.isError ? (
          <ErrorState error={ready.error} onRetry={() => void ready.refetch()} />
        ) : readyCars.length === 0 ? (
          <p className="mt-4 text-section text-steel-500">{t('readyEmpty')}</p>
        ) : (
          <ul className="mt-4 grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
            {readyCars.map((o) => (
              <li key={o.id}>
                <Link
                  href={`/${locale}/orders/${o.id}`}
                  className="card block p-6 hover:border-steel-900"
                >
                  <p className="font-mono text-record font-semibold">{o.vehicle_plate || '—'}</p>
                  <p className="mt-1 text-section font-medium">
                    #{o.number} · {o.title}
                  </p>
                  <p className="mt-1 text-metadata text-steel-500">
                    {t('readySince')}: <DateDisplay value={o.stage_entered_at} />
                  </p>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-label={t('workingTitle')} className="mt-10">
        <h2 className="text-section font-semibold">
          {t('workingTitle')} {working.isPending ? '' : `(${inWork.length})`}
        </h2>
        {working.isPending ? (
          <DetailSkeleton />
        ) : working.isError ? (
          <ErrorState error={working.error} onRetry={() => void working.refetch()} />
        ) : (
          <ul className="mt-3 space-y-2">
            {inWork.map((o) => (
              <li
                key={o.id}
                className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4"
              >
                <span className="min-w-28 font-mono text-body font-semibold">
                  {o.vehicle_plate || '—'}
                </span>
                <span className="min-w-0 flex-1 text-body">
                  #{o.number} · {o.title}
                </span>
                <span className="text-metadata text-steel-500">
                  {o.stage_label} · {daysSince(o.stage_entered_at)} nap
                </span>
              </li>
            ))}
            {inWork.length === 0 && (
              <li className="text-body text-steel-500">{t('workingEmpty')}</li>
            )}
          </ul>
        )}
      </section>
    </>
  );
}
