'use client';

import { useMemo, useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import type { ColumnDef } from '@tanstack/react-table';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DataTable, nextSort, type TableSort } from '@/components/tables/DataTable';
import { FilterBar, FilterField } from '@/components/tables/FilterBar';
import { Pagination } from '@/components/ui/Pagination';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { AssigneeField } from '@/components/forms/AssigneeField';
import { PartnerPicker, type PartnerOption } from '@/components/forms/PartnerPicker';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useDensity, usePageSize } from '@/hooks/usePreferences';
import { configApi, ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canEditOrders, useAuth } from '@/lib/auth/context';
import { stageTone } from '@/lib/utils/stages';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { OrderSummary, StageDefinition } from '@/lib/api/types';



export default function OrdersPage() {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const tn = useTranslations('navigation');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  const [q, setQ] = useState('');
  const [stage, setStage] = useState('');
  const [partner, setPartner] = useState<PartnerOption | null>(null);
  const [projectType, setProjectType] = useState('');
  const [assignee, setAssignee] = useState<number | null | 'all' | 'me'>(null);
  const [openOnly, setOpenOnly] = useState(false);
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<TableSort | null>(null);
  const debouncedQ = useDebouncedValue(q);
  const pageSize = usePageSize();
  const density = useDensity();

  const assignedTo =
    assignee === 'me' ? (user?.id ?? null) : assignee === 'all' ? undefined : (assignee ?? undefined);

  const stagesQuery = useQuery({
    queryKey: qk.stages('order'),
    queryFn: () => configApi.stages('order'),
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });
  const defs = useMemo(() => {
    const m = new Map<string, StageDefinition>();
    for (const d of stagesQuery.data?.items ?? []) m.set(d.key, d);
    return m;
  }, [stagesQuery.data]);

  const query = useQuery({
    queryKey: qk.orders({
      q: debouncedQ, stage, partner: partner?.id, projectType, assignedTo, openOnly, offset, pageSize, sort,
    }),
    queryFn: () =>
      ordersApi.list({
        q: debouncedQ || undefined,
        stage: stage || undefined,
        partner_id: partner?.id,
        project_type_id: projectType ? Number(projectType) : undefined,
        assigned_to: assignedTo ?? undefined,
        open: openOnly || undefined,
        sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
        limit: pageSize,
        offset,
      }),
  });

  const resetOffset = () => setOffset(0);

  const columns = useMemo<ColumnDef<OrderSummary>[]>(
    () => [
      {
        header: t('number'),
        accessorKey: 'number',
        meta: { sortKey: 'number' },
        cell: ({ row }) => (
          <Link href={`./orders/${row.original.id}`} className="font-mono font-medium text-steel-900 underline">
            {row.original.number}
          </Link>
        ),
      },
      {
        header: t('fieldTitle'),
        accessorKey: 'title',
        cell: ({ row }) => (
          <span className="flex items-center gap-2">
            <span className="font-medium">{row.original.title}</span>
            {/* Signal is reserved for blocked/overdue: an order with open blockers is blocked. */}
            {row.original.open_blockers > 0 && (
              <StatusBadge tone="signal">
                {row.original.open_blockers} {t('openBlockers')}
              </StatusBadge>
            )}
          </span>
        ),
      },
      {
        header: tc('partner'),
        accessorKey: 'partner_name',
        cell: ({ getValue }) => getValue<string>(),
      },
      {
        header: t('stageFilter'),
        accessorKey: 'stage_label',
        cell: ({ row }) => (
          <StatusBadge tone={stageTone(defs.get(row.original.stage_key))}>
            {row.original.stage_label}
          </StatusBadge>
        ),
      },
      {
        header: t('total'),
        accessorKey: 'total_minor',
        meta: { sortKey: 'total', align: 'right' },
        cell: ({ row }) => (
          <Money minor={row.original.total_minor} currency={row.original.currency} />
        ),
      },
      {
        header: t('vehiclePlate'),
        accessorKey: 'vehicle_plate',
        cell: ({ getValue }) => <span className="font-mono">{getValue<string | null>() ?? '—'}</span>,
      },
      {
        header: t('dueDate'),
        accessorKey: 'due_date',
        meta: { sortKey: 'due_date', align: 'right' },
        cell: ({ getValue }) =>
          getValue<string | null>() ? <DateDisplay value={getValue<string>()} /> : <span>—</span>,
      },
    ],
    [defs, t, tc],
  );

  const clear = () => {
    setQ('');
    setStage('');
    setPartner(null);
    setProjectType('');
    setAssignee(null);
    setOpenOnly(false);
    setOffset(0);
    setSort(null);
  };

  return (
    <AppShell>
      <PageHeader
        title={tn('orders')}
        actions={
          canEditOrders(user) && (
            <Link href={`/${locale}/orders/new`} className="btn-primary btn-sm">
              {t('newOrder')}
            </Link>
          )
        }
      />
      <FilterBar onClear={clear}>
        <FilterField label={tc('search')}>
          <input
            className="input"
            placeholder={t('searchPlaceholder')}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              resetOffset();
            }}
          />
        </FilterField>
        <FilterField label={t('stageFilter')}>
          <select className="input" value={stage} disabled={stagesQuery.isLoading} onChange={(e) => { setStage(e.target.value); resetOffset(); }}>
            <option value="">{tc('all')}</option>
            {(stagesQuery.data?.items ?? []).map((d) => (
              <option key={d.key} value={d.key}>
                {d.label_hu}
              </option>
            ))}
          </select>
          {stagesQuery.isError && (
            <span className="text-xs text-steel-900" role="alert">
              {errorMessage(stagesQuery.error, ter, ter('unknownError'))}
            </span>
          )}
        </FilterField>
        <div className="flex min-w-44 flex-col gap-1">
          <PartnerPicker value={partner} onChange={(p) => { setPartner(p); resetOffset(); }} label={t('partnerFilter')} />
        </div>
        <FilterField label={t('projectTypeFilter')}>
          <select className="input" value={projectType} disabled={projectTypes.isLoading} onChange={(e) => { setProjectType(e.target.value); resetOffset(); }}>
            <option value="">{tc('all')}</option>
            {(projectTypes.data?.items ?? [])
              .filter((p) => p.is_active)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label_hu}
                </option>
              ))}
          </select>
          {projectTypes.isError && (
            <span className="text-xs text-steel-900" role="alert">
              {errorMessage(projectTypes.error, ter, ter('unknownError'))}
            </span>
          )}
        </FilterField>
        <AssigneeField
          label={t('assigneeFilter')}
          value={assignee}
          allowAll
          allowEmpty={false}
          onChange={(v) => {
            setAssignee(v);
            resetOffset();
          }}
        />
        <label className="flex items-center gap-2 pb-2 text-sm">
          <input
            type="checkbox"
            className="rounded border-steel-200 accent-steel-900"
            checked={openOnly}
            onChange={(e) => {
              setOpenOnly(e.target.checked);
              resetOffset();
            }}
          />
          {t('openOnly')}
        </label>
      </FilterBar>

      {query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <>
          <DataTable
            columns={columns}
            data={query.data?.items ?? []}
            isLoading={query.isLoading}
            emptyTitle={te('noOrders')}
            emptyFilteredTitle={te('filterNoResults')}
            filtered={
              debouncedQ.trim() !== '' ||
              stage !== '' ||
              partner !== null ||
              projectType !== '' ||
              openOnly ||
              (assignee !== null && assignee !== 'all')
            }
            getRowId={(r) => String(r.id)}
            density={density}
            sort={sort}
            onSort={(key) => {
              setSort(nextSort(sort, key));
              resetOffset();
            }}
          />
          <Pagination
            offset={offset}
            limit={pageSize}
            loaded={query.data?.items.length ?? 0}
            onPrev={() => setOffset((o) => Math.max(0, o - pageSize))}
            onNext={() => setOffset((o) => o + pageSize)}
          />
        </>
      )}
    </AppShell>
  );
}
