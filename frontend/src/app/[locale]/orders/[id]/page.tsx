'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { IntakeSlipSection } from '@/components/orders/IntakeSlipSection';
import { InspectionSection } from '@/components/inspections/InspectionSection';
import { TaskList } from '@/components/tasks/TaskList';
import { JobSheet } from '@/components/orders/JobSheet';
import { InvoicesSection } from '@/components/orders/InvoicesSection';
import { ProformasSection } from '@/components/orders/ProformasSection';
import { EmailValue } from '@/components/ui/ContactLinks';
import { StageRail, StageHistoryList, TravellerStrip } from '@/components/ui/StageRail';
import {
  OrderForm,
  orderPatchBody,
  orderSpecBody,
  type OrderFormValues,
} from '@/components/forms/OrderForm';
import type { SpecForm } from '@/components/forms/BuildSpecSection';
import { ItemsSection } from '@/components/forms/ItemsSection';
import { OrderStageDialog } from '@/components/forms/OrderStageDialog';
import { configApi, ordersApi, usersApi } from '@/lib/api/endpoints';
import { lookupLabel, useLookups } from '@/hooks/useLookups';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canAdmin, canChangeStage, canEditOrders, canSendEmail, useAuth } from '@/lib/auth/context';
import { stageTone } from '@/lib/utils/stages';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { Breadcrumbs, BackToList } from '@/components/ui/Breadcrumbs';
import { Timeline } from '@/components/timeline/Timeline';
import { CopyButton, CopyLinkButton } from '@/components/ui/CopyButton';
import { useRecentRecords } from '@/hooks/useRecent';
import type { PatchOrder } from '@/lib/api/types';
import { isBlockerOpen } from '@/lib/utils/blockers';
import { RawImportPanel } from '@/components/migration/RawImportPanel';

type Tab = 'data' | 'items' | 'invoices' | 'stages' | 'blockers' | 'audit';

/** Enum values render from the server's lookups; unknown ones read as themselves. */

function Info({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className={`text-body ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  );
}

export default function OrderDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const ti = useTranslations('images');
  const ts = useTranslations('spec');
  const tt = useTranslations('tasks');
  const ter = useTranslations('errors');
  const ti2 = useTranslations('invoices');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const tn = useTranslations('navigation');
  const { push: pushRecent } = useRecentRecords(6);
  const [tab, setTab] = useState<Tab>('data');
  const [editing, setEditing] = useState(false);
  const [stageOpen, setStageOpen] = useState(false);
  const canEdit = canEditOrders(user);
  const canStage = canChangeStage(user);
  const canMail = canSendEmail(user);

  const detail = useQuery({ queryKey: qk.order(id), queryFn: () => ordersApi.get(id) });
  const orderTitle = detail.data ? `#${detail.data.order.number} · ${detail.data.order.title}` : '';
  useEffect(() => {
    if (detail.data) {
      pushRecent({
        href: `/${locale}/orders/${id}`,
        title: orderTitle,
        sub: detail.data.partner.name,
      });
    }
    // Push once per loaded record, not on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail.data?.order.id]);
  const stagesQuery = useQuery({
    queryKey: qk.stages('order'),
    queryFn: () => configApi.stages('order'),
  });
  // Always loaded: the traveller derives reached state from history.
  const history = useQuery({
    queryKey: qk.orderStages(id),
    queryFn: () => ordersApi.stages(id),
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });
  const { data: lookups } = useLookups();
  // GET /users is admin-only; non-admins see the raw id (backend gap).
  const usersQuery = useQuery({
    queryKey: qk.users,
    queryFn: () => usersApi.list(),
    enabled: canAdmin(user) && (detail.data?.order.assigned_to ?? null) !== null,
    retry: false,
  });

  const patch = useMutation({
    // The order and its build spec are two requests: PATCH is a field diff, the spec is a
    // whole-row replace whose shape depends on the project type. The spec goes second, so
    // a rejected order edit never writes a spec for a project type that did not stick.
    mutationFn: async ({
      body,
      spec,
    }: {
      body: PatchOrder;
      spec: ReturnType<typeof orderSpecBody>;
    }) => {
      await ordersApi.patch(id, body);
      if (spec) await ordersApi.putSpec(id, spec);
    },
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
        <PageHeader size="record" title={t('orderDetails')} />
        <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
      </AppShell>
    );
  }

  const { order, partner, stage, items, value, blockers, image_counts, related, vehicles, spec } =
    detail.data;
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
  const openBlockers = blockers.filter(isBlockerOpen);
  const tabs: { key: Tab; label: string }[] = [
    { key: 'data', label: t('tabsData') },
    { key: 'items', label: `${t('tabsItems')} (${items.length})` },
    { key: 'invoices', label: ti2('tab') },
    { key: 'stages', label: t('tabsStages') },
    { key: 'blockers', label: `${t('tabsBlockers')} (${openBlockers.length})` },
    { key: 'audit', label: t('tabsAudit') },
  ];

  return (
    <AppShell>
      <div className="flex gap-6">
        <div className="min-w-0 flex-1 space-y-6">
          <BackToList listKey="orders" fallbackHref={`/${locale}/orders`} />
          <div className="flex items-center justify-between gap-3">
            <Breadcrumbs
              items={[
                { href: `/${locale}/orders`, label: tn('orders') },
                { label: `#${order.number}` },
              ]}
            />
            <CopyLinkButton />
          </div>
          <PageHeader size="record"
            title={
              <span className="inline-flex items-center gap-2">
                <span>{`#${order.number} · ${order.title}`}</span>
                <CopyButton value={order.number} label={t('number')} />
                {order.vehicle_plate && <CopyButton value={order.vehicle_plate} label={t('vehiclePlate')} />}
              </span>
            }
            subtitle={`${partner.name} · ${stage.label_hu} · ${t('daysInStage', { days: stage.days_in_stage })}`}
            actions={
              <>
                <button className="btn-secondary btn-sm" onClick={() => window.print()}>
                  {t('print')}
                </button>
                {canMail && (
                  <Link className="btn-secondary btn-sm" href={`/${locale}/emails/new?order_id=${order.id}`}>
                    {t('writeEmail')}
                  </Link>
                )}
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

          <div className="lg:hidden">
            <TravellerStrip
              stages={defs}
              currentKey={stage.key}
              history={history.data?.items ?? []}
              blockers={openBlockers}
            />
          </div>
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
                  className="px-4 py-2 text-body font-medium border-b-2 -mb-px transition-colors border-transparent text-steel-500 hover:text-steel-900 data-[state=active]:border-steel-900 data-[state=active]:text-steel-900"
                >
                  {tb.label}
                </Tabs.Trigger>
              ))}
            </Tabs.List>

          <Tabs.Content value="data">
            {editing ? (
              <OrderForm
                initial={order}
                initialPartner={{ id: partner.id, name: partner.name }}
                initialSpec={spec}
                currencyLocked={items.length > 0}
                submitLabel={tc('save')}
                onSubmit={(
                  v: OrderFormValues,
                  contactDirty: boolean,
                  specForm: SpecForm | null,
                ) =>
                  patch
                    .mutateAsync({
                      body: orderPatchBody(order, v, user?.id, contactDirty, items.length > 0),
                      spec: orderSpecBody(v, specForm),
                    })
                    .then(() => undefined)
                }
              />
            ) : (
              <div className="space-y-6">
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
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

                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">{t('valueSection')}</h2>
                  <div className="mt-3 flex flex-wrap items-baseline gap-x-8 gap-y-3">
                    <div>
                      <p className="text-metadata text-steel-500">{t('total')} ({order.currency})</p>
                      <p className="text-page-title font-mono font-semibold">
                        <Money minor={value.total_minor} currency={currency} />
                      </p>
                    </div>
                    <div>
                      <p className="text-metadata text-steel-500">{t('hufMnb')} <DateDisplay value={value.valuation_date} />)</p>
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

                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">{t('imagesSection')}</h2>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {Object.keys(image_counts).length === 0 && (
                      <p className="text-metadata text-steel-500">—</p>
                    )}
                    {Object.entries(image_counts).map(([cat, n]) => (
                      <StatusBadge key={cat} tone="steel">
                        {ti(cat)}: {n}
                      </StatusBadge>
                    ))}
                  </div>
                </section>

                {/* V2.1: the vans this job covers. One order can carry several. */}
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">
                    {t('vehiclesSection')} ({vehicles.length})
                  </h2>
                  <div className="mt-3 space-y-2">
                    {vehicles.length === 0 && (
                      <p className="text-metadata text-steel-500">{t('noVehicles')}</p>
                    )}
                    {vehicles.map((v) => (
                      <p key={v.id} className="text-body">
                        <span className="font-mono font-medium">{v.plate ?? v.vin ?? `#${v.id}`}</span>{' '}
                        <span className="text-steel-500">
                          · {[v.make, v.model, v.year].filter(Boolean).join(' ') || '—'}
                        </span>
                      </p>
                    ))}
                  </div>
                </section>

                {spec && (
                  <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                    <h2 className="text-section font-semibold">
                      {spec.form === 'cooling' ? ts('coolingTitle') : ts('heatingTitle')}
                    </h2>
                    <div className="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                      <Info
                        label={ts('targetTemp')}
                        value={spec.target_temp_c != null ? `${spec.target_temp_c} °C` : '—'}
                        mono
                      />
                      <Info
                        label={ts('insulation')}
                        value={spec.insulation_mm != null ? `${spec.insulation_mm} mm` : '—'}
                        mono
                      />
                      {spec.form === 'cooling' ? (
                        <>
                          <Info label={ts('coolingUnitMake')} value={spec.cooling_unit_make ?? '—'} />
                          <Info
                            label={ts('coolingUnitModel')}
                            value={spec.cooling_unit_model ?? '—'}
                          />
                          <Info label={ts('atpClass')} value={spec.atp_class ?? '—'} mono />
                          <Info
                            label={ts('compartments')}
                            value={spec.compartments != null ? String(spec.compartments) : '—'}
                            mono
                          />
                          <Info
                            label={ts('defrost')}
                            value={spec.defrost ? lookupLabel(lookups?.defrost_modes, spec.defrost) : '—'}
                          />
                          <Info
                            label={ts('electricStandby')}
                            value={spec.electric_standby ? tc('yes') : tc('no')}
                          />
                        </>
                      ) : (
                        <>
                          <Info label={ts('heaterMake')} value={spec.heater_make ?? '—'} />
                          <Info label={ts('heaterModel')} value={spec.heater_model ?? '—'} />
                          <Info
                            label={ts('heatOutput')}
                            value={spec.heat_output_kw != null ? `${spec.heat_output_kw} kW` : '—'}
                            mono
                          />
                          <Info label={ts('fuel')} value={spec.fuel ? lookupLabel(lookups?.heating_fuels, spec.fuel) : '—'} />
                          <Info
                            label={ts('thermostat')}
                            value={spec.thermostat ? tc('yes') : tc('no')}
                          />
                        </>
                      )}
                      <Info label={ts('notes')} value={spec.notes ?? '—'} />
                    </div>
                  </section>
                )}

                {/* V2.2: a warranty job with no link to the job it repairs is the kind of
                    connection nobody reconstructs later. */}
                {related && (
                  <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                    <h2 className="text-section font-semibold">{t('relatedSection')}</h2>
                    <p className="mt-3 text-body">
                      <StatusBadge tone="steel">{lookupLabel(lookups?.order_relations, related.relation)}</StatusBadge>{' '}
                      <Link
                        href={`/${locale}/orders/${related.id}`}
                        className="text-steel-900 underline"
                      >
                        <span className="font-mono">{related.number}</span> · {related.title}
                      </Link>
                    </p>
                  </section>
                )}

                {order.minicrm_id != null && <RawImportPanel entity="order" id={id} />}

                {!editing && <IntakeSlipSection order={order} editable={canEdit} />}

                {!editing && <InspectionSection orderId={id} />}

                {!editing && (
                  <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                    <h2 className="text-section font-semibold">{tt('forRecord')}</h2>
                    <div className="mt-3">
                      <TaskList entity="order" id={id} />
                    </div>
                  </section>
                )}
              </div>
            )}
          </Tabs.Content>

          <Tabs.Content value="items">
            {canEdit ? (
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
            )}
          </Tabs.Content>

          {/* Invoices report to NAV; proformas never do. Two sections, not one list. */}
          <Tabs.Content value="invoices">
            <InvoicesSection orderId={id} currency={currency} />
            <ProformasSection orderId={id} currency={currency} />
          </Tabs.Content>

          <Tabs.Content value="stages">
            <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
              <div>
                {history.isLoading ? (
                  <p className="text-metadata text-steel-500">{tc('loading')}</p>
                ) : history.isError ? (
                  <ErrorState error={history.error} onRetry={() => void history.refetch()} />
                ) : (
                  <StageHistoryList history={history.data?.items ?? []} />
                )}
              </div>
            </section>
          </Tabs.Content>

          <Tabs.Content value="blockers">
            <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
              <div className="flex items-center justify-between">
                <h2 className="text-section font-semibold">{t('tabsBlockers')} ({openBlockers.length})</h2>
                <Link href={`/${locale}/blockers`} className="btn-ghost btn-sm">
                  {t('tabsBlockers')} →
                </Link>
              </div>
              <div className="mt-3 space-y-3">
                <p className="text-metadata text-steel-500">{t('blockersReadOnly')}</p>
                {blockers.length === 0 && <p className="text-metadata text-steel-500">—</p>}
                {blockers.map((b) => {
                  const open = isBlockerOpen(b);
                  const overdue = b.is_overdue;
                  return (
                    <div key={b.id} className="rounded-lg border border-steel-200 p-4">
                      <p className="flex flex-wrap items-center gap-2 text-body font-medium">
                        {b.what}
                        {!open && <StatusBadge tone="done">{t('blockerResolved')}</StatusBadge>}
                        {open && overdue && <StatusBadge tone="signal">{t('blockerOverdue')}</StatusBadge>}
                        {open && !overdue && <StatusBadge tone="steel">{t('blockerOpen')}</StatusBadge>}
                      </p>
                      <p className="mt-1 font-mono text-metadata text-steel-500">
                        {b.responsible_partner_name}
                        {b.responsible_partner_name && b.responsible_email ? ' · ' : ''}
                        {b.responsible_email && <EmailValue value={b.responsible_email} />}
                        {!b.responsible_partner_name && !b.responsible_email && '—'}
                        {b.due_date ? (
                          <>
                            {' '}· {t('dueOn')}: <DateDisplay value={b.due_date} />
                          </>
                        ) : (
                          ''
                        )}
                        {b.nudge_count > 0 ? ` · ${b.nudge_count} ${t('nudges')}` : ''}
                      </p>
                      {b.notes && <p className="mt-1 text-body">{b.notes}</p>}
                      {b.resolution_note && <p className="mt-1 text-metadata text-steel-500">{b.resolution_note}</p>}
                    </div>
                  );
                })}
              </div>
            </section>
          </Tabs.Content>

          <Tabs.Content value="audit">
            {/* Everything that happened to the job: changes, stages, photos and files, tasks,
                emails, and MiniCRM's imported notes. */}
            <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
              <Timeline entity="order" id={id} />
            </section>
          </Tabs.Content>
          </Tabs.Root>
        </div>

        <aside className="hidden w-[280px] shrink-0 lg:block">
          <div className="card sticky top-6">
            <div className="card-header flex items-center justify-between">
              <h2 className="text-body font-semibold">{t('traveller')}</h2>
              {canStage && (
                <button className="btn-secondary btn-sm" onClick={() => setStageOpen(true)}>
                  {t('changeStage')}
                </button>
              )}
            </div>
            <div className="card-content space-y-4">
              {stagesQuery.isError || history.isError ? (
                <div role="alert">
                  <p className="text-body text-steel-900">
                    {errorMessage(
                      stagesQuery.error ?? history.error,
                      ter,
                      ter('unknownError'),
                    )}
                  </p>
                  <button
                    className="btn-ghost btn-sm mt-2"
                    onClick={() => {
                      void stagesQuery.refetch();
                      void history.refetch();
                    }}
                  >
                    {ter('retry')}
                  </button>
                </div>
              ) : (
                <StageRail
                  stages={defs}
                  currentKey={stage.key}
                  daysInStage={stage.days_in_stage}
                  openBlockers={openBlockers.length}
                  history={history.data?.items ?? []}
                />
              )}
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

      <JobSheet
        order={order}
        partner={partner}
        vehicles={vehicles}
        items={items}
        value={value}
        blockers={blockers}
        stage={stage}
        assigneeName={assigneeName}
      />
    </AppShell>
  );
}
