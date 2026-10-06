'use client';

import { useMemo, useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import type { ColumnDef } from '@tanstack/react-table';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DataTable, columnMenuItems, nextSort, type TableSort } from '@/components/tables/DataTable';
import { ColumnMenu } from '@/components/tables/ColumnMenu';
import { useColumnVisibility } from '@/hooks/useColumnVisibility';
import { FilterBar, FilterField } from '@/components/tables/FilterBar';
import { Pagination } from '@/components/ui/Pagination';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { AssigneeField } from '@/components/forms/AssigneeField';
import { PartnerPicker, type PartnerOption } from '@/components/forms/PartnerPicker';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useDensity, usePageSize } from '@/hooks/usePreferences';
import { useUrlFlag, useUrlInt, useUrlState } from '@/hooks/useUrlState';
import { useRememberList } from '@/hooks/useListMemory';
import { SavedViewsBar } from '@/components/tables/SavedViewsBar';
import { ActiveFilterChips, type FilterChip } from '@/components/tables/ActiveFilterChips';
import { DensityToggle, useDensityWithOverride } from '@/components/tables/DensityToggle';
import { ExportCsvButton, collectAll } from '@/components/tables/ExportCsvButton';
import { OrderBulkBar, selectColumn } from '@/components/tables/BulkBar';
import { minorToMajorString } from '@/lib/utils/format';
import { configApi, ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canChangeStage, canEditOrders, useAuth } from '@/lib/auth/context';
import { stageTone } from '@/lib/utils/stages';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { OrderSummary, StageDefinition } from '@/lib/api/types';



export default function OrdersPage() {
  const t = useTranslations('orders');
  const tb = useTranslations('bulk');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const tn = useTranslations('navigation');
  const tq = useTranslations('qol');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  useRememberList('orders');
  // List state lives in the URL (shareable, back-button safe, survives reload).
  const [q, setQ] = useUrlState('q', '');
  const [stage, setStage] = useUrlState('stage', '');
  const [partnerId, setPartnerId] = useUrlState('partner', '');
  const [partner, setPartner] = useState<PartnerOption | null>(null);
  const [projectType, setProjectType] = useUrlState('ptype', '');
  const [assigneeRaw, setAssigneeRaw] = useUrlState('assignee', '');
  const [openOnly, setOpenOnly] = useUrlFlag('open', false);
  const [page, setPage] = useUrlInt('page', 1);
  const [sortRaw, setSortRaw] = useUrlState('sort', '');
  const assignee: number | null | 'all' | 'me' =
    assigneeRaw === '' ? null : assigneeRaw === 'all' ? 'all' : assigneeRaw === 'me' ? 'me' : (Number(assigneeRaw) || null);
  const sort: TableSort | null = sortRaw === '' ? null : sortRaw.startsWith('-')
    ? { key: sortRaw.slice(1), dir: 'desc' }
    : { key: sortRaw, dir: 'asc' };
  const setAssignee = (v: number | null | 'all' | 'me') => {
    setAssigneeRaw(v === null ? '' : v === 'all' ? 'all' : v === 'me' ? 'me' : String(v));
  };
  const setSort = (s: TableSort | null) => {
    setSortRaw(!s ? '' : s.dir === 'desc' ? `-${s.key}` : s.key);
  };
  const debouncedQ = useDebouncedValue(q);
  const pageSize = usePageSize();
  const serverDensity = useDensity();
  const { density, toggle: toggleDensity, overridden: densityOverridden } =
    useDensityWithOverride(serverDensity);
  const { visibility: colVis, onChange: setColVis, reset: resetColVis } =
    useColumnVisibility('orders');
  const offset = Math.max(0, (page - 1) * pageSize);
  const resetOffset = () => setPage(1);

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
      q: debouncedQ, stage, partner: partnerId || undefined, projectType, assignedTo, openOnly, offset, pageSize, sort,
    }),
    queryFn: () =>
      ordersApi.list({
        q: debouncedQ || undefined,
        stage: stage || undefined,
        partner_id: partnerId ? Number(partnerId) : undefined,
        project_type_id: projectType ? Number(projectType) : undefined,
        assigned_to: assignedTo ?? undefined,
        open: openOnly || undefined,
        sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
        limit: pageSize,
        offset,
      }),
  });

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
    setPartnerId('');
    setProjectType('');
    setAssignee(null);
    setOpenOnly(false);
    setPage(1);
    setSort(null);
  };

  /** Current filters, first 200 rows (the API max page) — what the table shows. */
  // Ticked rows for the bulk bar; survives paging so a selection can span pages.
  const [ticked, setTicked] = useState<number[]>([]);
  const bulkable = canChangeStage(user);
  const pageRows = query.data?.items ?? [];
  const tableColumns = bulkable
    ? [selectColumn(pageRows, ticked, setTicked, tb('tick')) as ColumnDef<OrderSummary>, ...columns]
    : columns;

  const exportOrders = async () => {
    const all = await collectAll((offset, limit) =>
      ordersApi.list({
      q: debouncedQ || undefined,
      stage: stage || undefined,
      partner_id: partnerId ? Number(partnerId) : undefined,
      project_type_id: projectType ? Number(projectType) : undefined,
      assigned_to: assignedTo ?? undefined,
      open: openOnly || undefined,
      sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
        limit,
        offset,
      }),
    );
    // With rows ticked, only those are exported.
    const items = ticked.length > 0 ? all.filter((o) => ticked.includes(o.id)) : all;
    return {
      header: [t('number'), t('fieldTitle'), tc('partner'), t('stageFilter'), t('total'), t('vehiclePlate'), t('dueDate')],
      rows: items.map((o) => [
        o.number,
        o.title,
        o.partner_name,
        o.stage_label,
        o.total_minor === null || o.total_minor === undefined ? '' : `${minorToMajorString(o.total_minor)} ${o.currency}`,
        o.vehicle_plate,
        o.due_date,
      ]),
      count: items.length,
    };
  };

  const chips = useMemo<FilterChip[]>(() => {
    const list: FilterChip[] = [];
    if (q.trim() !== '') {
      list.push({ key: 'q', label: `${tc('search')}: ${q.trim()}`, onRemove: () => { setQ(''); resetOffset(); } });
    }
    if (stage !== '') {
      const label = stagesQuery.data?.items.find((d) => d.key === stage)?.label_hu ?? stage;
      list.push({ key: 'stage', label: `${t('stageFilter')}: ${label}`, onRemove: () => { setStage(''); resetOffset(); } });
    }
    if (partnerId !== '') {
      const label = partner?.name ?? `#${partnerId}`;
      list.push({ key: 'partner', label: `${t('partnerFilter')}: ${label}`, onRemove: () => { setPartner(null); setPartnerId(''); resetOffset(); } });
    }
    if (projectType !== '') {
      const label = projectTypes.data?.items.find((p) => String(p.id) === projectType)?.label_hu ?? `#${projectType}`;
      list.push({ key: 'ptype', label: `${t('projectTypeFilter')}: ${label}`, onRemove: () => { setProjectType(''); resetOffset(); } });
    }
    if (assignee !== null && assignee !== 'all') {
      const label = assignee === 'me' ? (user?.display_name ?? 'me') : `#${assignee}`;
      list.push({ key: 'assignee', label: `${t('assigneeFilter')}: ${label}`, onRemove: () => { setAssignee(null); resetOffset(); } });
    }
    if (openOnly) {
      list.push({ key: 'open', label: t('openOnly'), onRemove: () => { setOpenOnly(false); resetOffset(); } });
    }
    if (sort) {
      list.push({ key: 'sort', label: tq('sortChip', { key: sort.key, dir: sort.dir === 'desc' ? '↓' : '↑' }), onRemove: () => { setSort(null); resetOffset(); } });
    }
    return list;
  }, [q, stage, partnerId, partner, projectType, projectTypes.data, assignee, user, openOnly, sort, stagesQuery.data, tc, t, tq, setQ, setStage, setPartner, setPartnerId, setProjectType, setAssignee, setOpenOnly, setSort, resetOffset]);

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
      <SavedViewsBar listKey="orders" />
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
            <span className="text-metadata text-steel-900" role="alert">
              {errorMessage(stagesQuery.error, ter, ter('unknownError'))}
            </span>
          )}
        </FilterField>
        <div className="flex min-w-44 flex-col gap-1">
          <PartnerPicker value={partner} onChange={(p) => { setPartner(p); setPartnerId(p ? String(p.id) : ''); resetOffset(); }} label={t('partnerFilter')} />
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
            <span className="text-metadata text-steel-900" role="alert">
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
        <label className="flex items-center gap-2 pb-2 text-body">
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
      <ActiveFilterChips chips={chips} />
      {bulkable && ticked.length > 0 && <OrderBulkBar ids={ticked} onClear={() => setTicked([])} />}

      {query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <>
          <DataTable
            columns={tableColumns}
            data={query.data?.items ?? []}
            isLoading={query.isPending}
            isFetching={query.isFetching && !query.isPending}
            emptyTitle={te('noOrders')}
            emptyFilteredTitle={te('filterNoResults')}
            filtered={
              debouncedQ.trim() !== '' ||
              stage !== '' ||
              partnerId !== '' ||
              projectType !== '' ||
              openOnly ||
              (assignee !== null && assignee !== 'all')
            }
            getRowId={(r) => String(r.id)}
            density={density}
            sort={sort}
            columnVisibility={colVis}
            onColumnVisibilityChange={setColVis}
            onSort={(key) => {
              setSort(nextSort(sort, key));
              resetOffset();
            }}
          />
          <div className="flex items-center justify-between gap-3">
            <Pagination
              offset={offset}
              limit={pageSize}
              loaded={query.data?.items.length ?? 0}
              onPrev={() => setPage(Math.max(1, page - 1))}
              onNext={() => setPage(page + 1)}
              onJump={setPage}
            />
            <div className="flex flex-wrap items-center gap-2">
              <ExportCsvButton base="megrendelesek" onExport={exportOrders} />
              <DensityToggle density={density} overridden={densityOverridden} onToggle={toggleDensity} />
              <ColumnMenu
                columns={columnMenuItems(columns)}
                visibility={colVis}
                onChange={setColVis}
                onReset={resetColVis}
              />
            </div>
          </div>
        </>
      )}
    </AppShell>
  );
}
