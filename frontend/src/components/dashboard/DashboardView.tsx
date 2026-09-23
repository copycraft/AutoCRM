'use client';

// The home screen: cars in work, things needing attention, the quote pipeline.
// Four cheap queries, no new endpoint — every number here is a link into the list
// that owns it. Sorted for the yard: due first, longest-waiting next.

import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQueries } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { RecentRecords, ResumeBanner } from '@/components/layout/RecentRecords';
import { blockersApi, leadsApi, ordersApi, reportsApi, tasksApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { daysSince } from '@/lib/utils/format';
import { budapestIsoPlus } from '@/components/forms/DateQuickPicks';
import type { Currency } from '@/lib/api/types';

/** Link to the record a task is pinned to. */
function taskHref(locale: string, entity: string, id: number): string {
  if (entity === 'order') return `/${locale}/orders/${id}`;
  if (entity === 'lead') return `/${locale}/leads/${id}`;
  return `/${locale}/partners/${id}`;
}

function SectionSkeleton({ rows = 3 }: { rows?: number }) {
  return (
    <div className="space-y-2" aria-label="loading">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="card h-16 animate-pulse p-4" aria-hidden />
      ))}
    </div>
  );
}

function SectionError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  return <ErrorState error={error} onRetry={onRetry} />;
}

export function DashboardView() {
  const t = useTranslations('dashboard');
  const tn = useTranslations('navigation');
  const locale = useLocale();

  // Each query resolves independently — the page paints progressively instead
  // of blocking everything on the slowest of 6 requests. Previously a single
  // `loading = a.isLoading || b.isLoading || ...` gate held the whole
  // dashboard (stats + all sections) behind one DetailSkeleton.
  const [openOrders, openBlockers, openLeads, stalled, myTasks, readyCars] = useQueries({
    queries: [
      {
        queryKey: qk.orders({ open: true, dashboard: true }),
        queryFn: () => ordersApi.list({ open: true, limit: 100 }),
        staleTime: 60_000,
      },
      {
        queryKey: qk.blockers({ open: true, dashboard: true }),
        queryFn: () => blockersApi.listOpen({ limit: 100 }),
        staleTime: 60_000,
      },
      {
        queryKey: qk.leads({ open: true, dashboard: true }),
        queryFn: () => leadsApi.list({ open: true, limit: 100 }),
        staleTime: 60_000,
      },
      {
        queryKey: qk.reports('stalled', {}),
        queryFn: () => reportsApi.stalled(),
        staleTime: 60_000,
      },
      {
        queryKey: qk.tasksMine,
        queryFn: () => tasksApi.mine(),
        staleTime: 30_000,
      },
      {
        queryKey: qk.orders({ stage: 'completed', dashboard: true }),
        queryFn: () => ordersApi.list({ stage: 'completed', limit: 30 }),
        staleTime: 60_000,
      },
    ],
  });

  const cars = [...(openOrders.data?.items ?? [])].sort((a, b) => {
    if (a.due_date && !b.due_date) return -1;
    if (!a.due_date && b.due_date) return 1;
    if (a.due_date && b.due_date && a.due_date !== b.due_date) {
      return a.due_date < b.due_date ? -1 : 1;
    }
    return daysSince(b.stage_entered_at) - daysSince(a.stage_entered_at);
  });
  const blockers = [...(openBlockers.data?.items ?? [])].sort((a, b) => {
    if (a.is_overdue !== b.is_overdue) return a.is_overdue ? -1 : 1;
    return (a.due_date ?? '9') < (b.due_date ?? '9') ? -1 : 1;
  });
  const stalledRows = stalled.data?.items ?? [];
  const expiring = (openLeads.data?.items ?? []).filter(
    (l) => l.quote_valid_until && l.quote_valid_until <= budapestIsoPlus(7),
  );
  const pipeline = [...(openLeads.data?.items ?? [])]
    .sort((a, b) => (a.quote_valid_until ?? '9') < (b.quote_valid_until ?? '9') ? -1 : 1)
    .slice(0, 10);
  const today = budapestIsoPlus(0);
  const tasks = [...(myTasks.data?.items ?? [])].sort((a, b) => {
    const aOver = !!a.due_date && a.due_date < today;
    const bOver = !!b.due_date && b.due_date < today;
    if (aOver !== bOver) return aOver ? -1 : 1;
    return (a.due_date ?? '9') < (b.due_date ?? '9') ? -1 : 1;
  });
  const ready = [...(readyCars.data?.items ?? [])].sort((a, b) =>
    a.stage_entered_at > b.stage_entered_at ? -1 : 1,
  );

  // Stats paint as soon as their own query lands ("—" until then) instead of
  // waiting for all six.
  const stats = [
    {
      label: t('statCars'),
      value: openOrders.isPending ? null : cars.length,
      href: `/${locale}/orders`,
    },
    {
      label: t('statBlockers'),
      value: openBlockers.isPending ? null : blockers.length,
      href: `/${locale}/orders`,
    },
    {
      label: t('statStalled'),
      value: stalled.isPending ? null : stalledRows.length,
      href: `/${locale}/reports`,
    },
    {
      label: t('statQuotes'),
      value: openLeads.isPending ? null : expiring.length,
      href: `/${locale}/leads`,
    },
  ];

  const attentionPending =
    openBlockers.isPending || stalled.isPending || openLeads.isPending;
  const attentionError = openBlockers.error ?? stalled.error ?? openLeads.error;

  return (
    <AppShell>
      <PageHeader title={tn('dashboard')} />

      <ResumeBanner />

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {stats.map((s) => (
          <Link key={s.label} href={s.href} className="card p-4 hover:border-steel-900">
            <p className="text-page-title font-mono font-semibold">
              {s.value === null ? <span className="animate-pulse text-steel-200">—</span> : s.value}
            </p>
            <p className="text-metadata text-steel-500">{s.label}</p>
          </Link>
        ))}
      </div>

      <RecentRecords />

      <CollapsibleSection
        storageKey="dash-inwork"
        title={t('inWork')}
        count={openOrders.isPending ? undefined : cars.length}
      >
        {openOrders.isPending ? (
          <SectionSkeleton />
        ) : openOrders.error ? (
          <SectionError error={openOrders.error} onRetry={() => void openOrders.refetch()} />
        ) : cars.length === 0 ? (
          <p className="text-body text-steel-500">{t('inWorkEmpty')}</p>
        ) : (
          <ul className="space-y-2">
            {cars.map((o) => (
              <li key={o.id}>
                <Link
                  href={`/${locale}/orders/${o.id}`}
                  className="card flex flex-wrap items-center gap-x-6 gap-y-1 p-4 hover:border-steel-900"
                >
                  <span className="min-w-32 font-mono text-record font-semibold">
                    {o.vehicle_plate || '—'}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-body font-medium">
                      #{o.number} · {o.title}
                    </span>
                    <span className="block text-metadata text-steel-500">
                      {o.stage_label} · {t('daysInStage', { days: daysSince(o.stage_entered_at) })}
                      {' · '}
                      {o.due_date ? (
                        <>
                          {t('dueDate')}: <DateDisplay value={o.due_date} />
                        </>
                      ) : (
                        t('noDue')
                      )}
                    </span>
                  </span>
                  {o.open_blockers > 0 && (
                    <StatusBadge tone="signal">{t('statBlockers')}</StatusBadge>
                  )}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>

      <CollapsibleSection
        storageKey="dash-attention"
        title={t('attention')}
        count={attentionPending ? undefined : blockers.length + stalledRows.length + expiring.length}
      >
        {attentionPending ? (
          <SectionSkeleton />
        ) : attentionError ? (
          <SectionError
            error={attentionError}
            onRetry={() => {
              void openBlockers.refetch();
              void stalled.refetch();
              void openLeads.refetch();
            }}
          />
        ) : blockers.length === 0 && stalledRows.length === 0 && expiring.length === 0 ? (
          <p className="text-body text-steel-500">{t('attentionEmpty')}</p>
        ) : (
          <ul className="space-y-2">
            {blockers.map((b) => (
              <li key={`b-${b.id}`} className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4">
                {b.is_overdue && <StatusBadge tone="signal">{t('quoteExpired')}</StatusBadge>}
                <span className="min-w-0 flex-1 text-body">
                  <span className="font-medium">{b.what}</span>{' '}
                  <Link
                    href={`/${locale}/orders/${b.order_id}`}
                    className="text-steel-500 underline"
                  >
                    {t('blockedOrder', { number: b.order_number })}
                  </Link>
                </span>
                {b.due_date && (
                  <span className="text-metadata text-steel-500">
                    <DateDisplay value={b.due_date} />
                  </span>
                )}
              </li>
            ))}
            {stalledRows.map((s) => (
              <li key={`s-${s.order_id}`} className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4">
                <span className="min-w-0 flex-1 text-body">
                  <Link href={`/${locale}/orders/${s.order_id}`} className="font-medium underline">
                    #{s.number} · {s.title}
                  </Link>{' '}
                  <span className="text-steel-500">
                    {s.stage_label} · {t('stalledDays', { days: s.days_in_stage })}
                  </span>
                </span>
              </li>
            ))}
            {expiring.map((l) => (
              <li key={`q-${l.id}`} className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4">
                <span className="min-w-0 flex-1 text-body">
                  <Link href={`/${locale}/leads/${l.id}`} className="font-medium underline">
                    {l.title}
                  </Link>{' '}
                  {l.quote_valid_until && (
                    <span className="text-steel-500">
                      {t('quoteExpires')}: <DateDisplay value={l.quote_valid_until} />
                    </span>
                  )}
                </span>
                {l.quoted_value_minor != null && l.currency && (
                  <Money minor={l.quoted_value_minor} currency={l.currency as Currency} />
                )}
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>

      <CollapsibleSection
        storageKey="dash-pipeline"
        title={t('pipeline')}
        count={openLeads.isPending ? undefined : pipeline.length}
      >
        {openLeads.isPending ? (
          <SectionSkeleton />
        ) : openLeads.error ? (
          <SectionError error={openLeads.error} onRetry={() => void openLeads.refetch()} />
        ) : pipeline.length === 0 ? (
          <p className="text-body text-steel-500">{t('pipelineEmpty')}</p>
        ) : (
          <ul className="space-y-2">
            {pipeline.map((l) => (
              <li key={l.id}>
                <Link
                  href={`/${locale}/leads/${l.id}`}
                  className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4 hover:border-steel-900"
                >
                  <span className="min-w-0 flex-1 text-body font-medium">{l.title}</span>
                  {l.quoted_value_minor != null && l.currency && (
                    <Money minor={l.quoted_value_minor} currency={l.currency as Currency} />
                  )}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>

      <CollapsibleSection
        storageKey="dash-tasks"
        title={t('tasksTitle')}
        count={myTasks.isPending ? undefined : tasks.length}
      >
        {myTasks.isPending ? (
          <SectionSkeleton />
        ) : myTasks.error ? (
          <SectionError error={myTasks.error} onRetry={() => void myTasks.refetch()} />
        ) : tasks.length === 0 ? (
          <p className="text-body text-steel-500">{t('mineEmpty')}</p>
        ) : (
          <ul className="space-y-2">
            {tasks.map((x) => {
              const overdue = !!x.due_date && x.due_date < today;
              return (
                <li
                  key={x.id}
                  className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4"
                >
                  {overdue && <StatusBadge tone="signal">{t('overdue')}</StatusBadge>}
                  <Link
                    href={taskHref(locale, x.entity_type, x.entity_id)}
                    className="min-w-0 flex-1 text-body font-medium underline"
                  >
                    {x.title}
                  </Link>
                  {x.due_date && (
                    <span className="text-metadata text-steel-500">
                      <DateDisplay value={x.due_date} />
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </CollapsibleSection>

      <CollapsibleSection
        storageKey="dash-ready"
        title={t('readyTitle')}
        count={readyCars.isPending ? undefined : ready.length}
      >
        {readyCars.isPending ? (
          <SectionSkeleton />
        ) : readyCars.error ? (
          <SectionError error={readyCars.error} onRetry={() => void readyCars.refetch()} />
        ) : ready.length === 0 ? (
          <p className="text-body text-steel-500">{t('readyEmpty')}</p>
        ) : (
          <ul className="space-y-2">
            {ready.map((o) => (
              <li key={o.id}>
                <Link
                  href={`/${locale}/orders/${o.id}`}
                  className="card flex flex-wrap items-center gap-x-6 gap-y-1 p-4 hover:border-steel-900"
                >
                  <span className="min-w-32 font-mono text-record font-semibold">
                    {o.vehicle_plate || '—'}
                  </span>
                  <span className="min-w-0 flex-1 text-body font-medium">
                    #{o.number} · {o.title}
                  </span>
                  <span className="text-metadata text-steel-500">
                    {t('readySince')}: <DateDisplay value={o.stage_entered_at} />
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>
    </AppShell>
  );
}
