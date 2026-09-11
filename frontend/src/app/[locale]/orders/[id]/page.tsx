'use client';

import Link from 'next/link';
import { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { StageRail, StageHistoryList } from '@/components/ui/StageRail';
import { OrderForm, orderPatchBody, type OrderFormValues } from '@/components/forms/OrderForm';
import { ItemsSection } from '@/components/forms/ItemsSection';
import { OrderStageDialog } from '@/components/forms/OrderStageDialog';
import { configApi, ordersApi, usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canAdmin, canChangeStage, canEditOrders, useAuth } from '@/lib/auth/context';
import { stageTone } from '@/lib/utils/stages';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { PatchOrder } from '@/lib/api/types';
import { isBlockerOpen } from '@/lib/utils/blockers';

type Tab = 'data' | 'items' | 'stages' | 'blockers' | 'audit';

function Info({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className={`text-sm ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  );
}

export default function OrderDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const ti = useTranslations('images');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const [tab, setTab] = useState<Tab>('data');
  const [editing, setEditing] = useState(false);
  const [stageOpen, setStageOpen] = useState(false);
  const canEdit = canEditOrders(user);
  const canStage = canChangeStage(user);

  const detail = useQuery({ queryKey: qk.order(id), queryFn: () => ordersApi.get(id) });
  const stagesQuery = useQuery({
    queryKey: qk.stages('order'),
    queryFn: () => configApi.stages('order'),
  });
  // Always loaded: the traveller derives reached state from history.
  const history = useQuery({
    queryKey: qk.orderStages(id),
    queryFn: () => ordersApi.stages(id),
  });
  const audit = useQuery({
    queryKey: qk.orderAudit(id),
    queryFn: () => ordersApi.audit(id, { limit: 100 }),
    enabled: tab === 'audit',
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });
  // GET /users is admin-only; non-admins see the raw id (backend gap).
  const usersQuery = useQuery({
    queryKey: qk.users,
    queryFn: () => usersApi.list(),
    enabled: canAdmin(user) && (detail.data?.order.assigned_to ?? null) !== null,
    retry: false,
  });

  const patch = useMutation({
    mutationFn: (body: PatchOrder) => ordersApi.patch(id, body),
    onSuccess: () => {
      setEditing(false);
      void qc.invalidateQueries({ queryKey: qk.order(id) });
      void qc.invalidateQueries({ queryKey: ['orders'] });
    },
  });

  if (detail.isLoading) {
    return (
      <AppShell>
        <DetailSkeleton />
      </AppShell>
    );
  }
  if (detail.isError || !detail.data) {
    return (
      <AppShell>
        <PageHeader title={t('orderDetails')} />
        <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
      </AppShell>
    );
  }

  const { order, partner, stage, items, value, blockers, image_counts } = detail.data;
  const currency = order.currency;
  const projectTypeName = order.project_type_id
    ? (projectTypes.data?.items.find((p) => p.id === order.project_type_id)?.label_hu ??
      `#${order.project_type_id}`)
    : '—';
  const assigneeName = order.assigned_to
    ? (usersQuery.data?.items.find((u) => u.id === order.assigned_to)?.display_name ??
      `#${order.assigned_to}`)
    : '—';
  const defs = stagesQuery.data?.items ?? [];
  const auditItems = audit.data?.items ?? [];
  const openBlockers = blockers.filter(isBlockerOpen);
  const tabs: { key: Tab; label: string }[] = [
    { key: 'data', label: t('tabsData') },
    { key: 'items', label: `${t('tabsItems')} (${items.length})` },
    { key: 'stages', label: t('tabsStages') },
    { key: 'blockers', label: `${t('tabsBlockers')} (${openBlockers.length})` },
    { key: 'audit', label: t('tabsAudit') },
  ];

  return (
    <AppShell>
      <div className="flex gap-6">
        <div className="min-w-0 flex-1 space-y-6">
          <PageHeader
            title={`#${order.number} · ${order.title}`}
            subtitle={`${partner.name} · ${stage.label_hu} · ${t('daysInStage', { days: stage.days_in_stage })}`}
            actions={
              <>
                {canEdit && !editing && tab === 'data' && (
                  <button className="btn-secondary btn-sm" onClick={() => setEditing(true)}>
                    {tc('edit')}
                  </button>
                )}
                {canStage && (
                  <button className="btn-secondary btn-sm lg:hidden" onClick={() => setStageOpen(true)}>
                    {t('changeStage')}
                  </button>
                )}
              </>
            }
          />

          <Tabs.Root
            value={tab}
            onValueChange={(v) => {
              const next = tabs.find((tb) => tb.key === v);
              if (next) setTab(next.key);
            }}
          >
            <Tabs.List className="flex gap-1 border-b border-steel-200" aria-label={t('tabsLabel')}>
              {tabs.map((tb) => (
                <Tabs.Trigger
                  key={tb.key}
                  value={tb.key}
                  className="px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors border-transparent text-steel-500 hover:text-steel-900 data-[state=active]:border-steel-900 data-[state=active]:text-steel-900"
                >
                  {tb.label}
                </Tabs.Trigger>
              ))}
            </Tabs.List>

          <Tabs.Content value="data">
            editing ? (
              <OrderForm
                initial={order}
                initialPartner={{ id: partner.id, name: partner.name }}
                currencyLocked={items.length > 0}
                submitLabel={tc('save')}
                onSubmit={(v: OrderFormValues, contactDirty: boolean) =>
                  patch
                    .mutateAsync(orderPatchBody(order, v, user?.id, contactDirty, items.length > 0))
                    .then(() => undefined)
                }
              />
            ) : (
              <div className="space-y-6">
                <section className="card">
                  <div className="card-content grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    <Info label={tc('partner')} value={`${partner.name} (#${partner.id})`} />
                    <Info label={t('projectType')} value={projectTypeName} mono />
                    <Info label={t('currencyLabel')} value={order.currency} mono />
                    <Info label={t('valuationDate')} value={<DateDisplay value={order.valuation_date} />} />
                    <Info label={t('vehicleMake')} value={order.vehicle_make ?? '—'} />
                    <Info label={t('vehicleModel')} value={order.vehicle_model ?? '—'} />
                    <Info label={t('vehiclePlate')} value={order.vehicle_plate ?? '—'} mono />
                    <Info label={t('vehicleVin')} value={order.vehicle_vin ?? '—'} mono />
                    <Info label={t('dueDate')} value={order.due_date ? <DateDisplay value={order.due_date} /> : '—'} />
                    <Info label={t('assignedTo')} value={assigneeName} mono />
                    <Info label={t('description')} value={order.description ?? '—'} />
                    <Info label={t('createdAt')} value={<DateDisplay value={order.created_at} />} />
                  </div>
                </section>

                <section className="card">
                  <div className="card-header">
                    <h2 className="text-section font-semibold">{t('valueSection')}</h2>
                  </div>
                  <div className="card-content flex flex-wrap items-baseline gap-x-8 gap-y-3">
                    <div>
                      <p className="text-metadata text-steel-500">{t('total')} ({order.currency})</p>
                      <p className="text-page-title font-mono font-semibold">
                        <Money minor={value.total_minor} currency={currency} />
                      </p>
                    </div>
                    <div>
                      <p className="text-metadata text-steel-500">HUF (MNB, <DateDisplay value={value.valuation_date} />)</p>
                      <p className="text-section font-mono font-medium">
                        {value.total_huf_minor != null ? (
                          <Money minor={value.total_huf_minor} currency="HUF" />
                        ) : (
                          <StatusBadge tone="steel">{t('missingFx')}</StatusBadge>
                        )}
                      </p>
                    </div>
                    {value.fx_rate != null && value.fx_day && (
                      <p className="text-metadata text-steel-500 font-mono">
                        {t('fxRate')}: {value.fx_rate} · {t('fxDay')}: <DateDisplay value={value.fx_day} />
                      </p>
                    )}
                  </div>
                </section>

                <section className="card">
                  <div className="card-header">
                    <h2 className="text-section font-semibold">{t('imagesSection')}</h2>
                  </div>
                  <div className="card-content flex flex-wrap gap-2">
                    {Object.keys(image_counts).length === 0 && (
                      <p className="text-sm text-steel-500">—</p>
                    )}
                    {Object.entries(image_counts).map(([cat, n]) => (
                      <StatusBadge key={cat} tone="steel">
                        {ti(cat)}: {n}
                      </StatusBadge>
                    ))}
                  </div>
                </section>
              </div>
            )
          </Tabs.Content>

          <Tabs.Content value="items">
            canEdit ? (
              <ItemsSection orderId={id} items={items} currency={currency} />
            ) : (
              <div className="table-container">
                <table className="table">
                  <thead>
                    <tr>
                      <th scope="col">{t('description')}</th>
                      <th scope="col" className="text-right">{t('quantity')}</th>
                      <th scope="col" className="text-right">{t('unitPrice')}</th>
                      <th scope="col" className="text-right">{t('lineTotal')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {items.map((it) => (
                      <tr key={it.id}>
                        <td className="font-medium">{it.description}</td>
                        <td className="text-right font-mono">{it.quantity}</td>
                        <td className="text-right"><Money minor={it.unit_price} currency={currency} /></td>
                        <td className="text-right"><Money minor={it.line_total_minor} currency={currency} /></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )
          </Tabs.Content>

          <Tabs.Content value="stages">
            <section className="card">
              <div className="card-content">
                {history.isLoading ? (
                  <p className="text-sm text-steel-500">{tc('loading')}</p>
                ) : history.isError ? (
                  <ErrorState error={history.error} onRetry={() => void history.refetch()} />
                ) : (
                  <StageHistoryList history={history.data?.items ?? []} />
                )}
              </div>
            </section>
          </Tabs.Content>

          <Tabs.Content value="blockers">
            <section className="card">
              <div className="card-header flex items-center justify-between">
                <h2 className="text-section font-semibold">{t('tabsBlockers')} ({openBlockers.length})</h2>
                <Link href={`/${locale}/blockers`} className="btn-ghost btn-sm">
                  {t('tabsBlockers')} →
                </Link>
              </div>
              <div className="card-content space-y-3">
                <p className="text-metadata text-steel-500">{t('blockersReadOnly')}</p>
                {blockers.length === 0 && <p className="text-sm text-steel-500">—</p>}
                {blockers.map((b) => {
                  const open = isBlockerOpen(b);
                  const overdue = b.is_overdue;
                  return (
                    <div key={b.id} className="rounded-lg border border-steel-200 p-4">
                      <p className="flex flex-wrap items-center gap-2 text-sm font-medium">
                        {b.what}
                        {!open && <StatusBadge tone="done">{t('blockerResolved')}</StatusBadge>}
                        {open && overdue && <StatusBadge tone="signal">{t('blockerOverdue')}</StatusBadge>}
                        {open && !overdue && <StatusBadge tone="steel">{t('blockerOpen')}</StatusBadge>}
                      </p>
                      <p className="mt-1 font-mono text-metadata text-steel-500">
                        {[b.responsible_partner_name, b.responsible_email].filter(Boolean).join(' · ') || '—'}
                        {b.due_date ? (
                          <>
                            {' '}· {t('dueOn')}: <DateDisplay value={b.due_date} />
                          </>
                        ) : (
                          ''
                        )}
                        {b.nudge_count > 0 ? ` · ${b.nudge_count} ${t('nudges')}` : ''}
                      </p>
                      {b.notes && <p className="mt-1 text-sm">{b.notes}</p>}
                      {b.resolution_note && <p className="mt-1 text-sm text-steel-500">{b.resolution_note}</p>}
                    </div>
                  );
                })}
              </div>
            </section>
          </Tabs.Content>

          <Tabs.Content value="audit">
            <section className="card">
              <div className="card-header">
                <h2 className="text-section font-semibold">{t('tabsAudit')}</h2>
              </div>
              <div className="card-content">
                {audit.isLoading ? (
                  <p className="text-sm text-steel-500">{tc('loading')}</p>
                ) : audit.isError ? (
                  <ErrorState error={audit.error} onRetry={() => void audit.refetch()} />
                ) : auditItems.length === 0 ? (
                  <p className="text-sm text-steel-500">{t('auditEmpty')}</p>
                ) : (
                  <ul className="space-y-3">
                    {auditItems.map((a) => (
                      <li key={a.id} className="text-sm">
                        <p>
                          <span className="font-medium">{a.action}</span>{' '}
                          <span className="text-steel-500">
                            · {a.user_name ?? (a.user_id ? `#${a.user_id}` : t('systemUser'))} ·{' '}
                            <DateDisplay withTime value={a.at} className="text-metadata" />
                          </span>
                        </p>
                        <pre className="mt-1 overflow-x-auto rounded-lg bg-panel p-2 font-mono text-metadata text-steel-900">
                          {JSON.stringify(a.changes, null, 1)}
                        </pre>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </section>
          </Tabs.Content>
          </Tabs.Root>
        </div>

        <aside className="hidden w-[280px] shrink-0 lg:block">
          <div className="card sticky top-6">
            <div className="card-header flex items-center justify-between">
              <h2 className="text-sm font-semibold">{t('traveller')}</h2>
              {canStage && (
                <button className="btn-secondary btn-sm" onClick={() => setStageOpen(true)}>
                  {t('changeStage')}
                </button>
              )}
            </div>
            <div className="card-content space-y-4">
              <StageRail
                stages={defs}
                currentKey={stage.key}
                daysInStage={stage.days_in_stage}
                openBlockers={openBlockers.length}
                history={history.data?.items ?? []}
              />
              <div className="border-t border-steel-200 pt-3 text-metadata text-steel-500">
                <p>
                  <StatusBadge tone={stageTone(defs.find((d) => d.key === stage.key))}>
                    {stage.label_hu}
                  </StatusBadge>
                </p>
                <p className="mt-1 font-mono">{order.number}</p>
                <p className="font-mono">
                  <Money minor={value.total_minor} currency={currency} />
                </p>
              </div>
            </div>
          </div>
        </aside>
      </div>

      {stageOpen && (
        <OrderStageDialog orderId={id} detail={detail.data} definitions={defs} onClose={() => setStageOpen(false)} />
      )}
    </AppShell>
  );
}
