'use client';

import { useMemo } from 'react';
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
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useUrlFlag, useUrlInt, useUrlState } from '@/hooks/useUrlState';
import { useRememberList } from '@/hooks/useListMemory';
import { SavedViewsBar } from '@/components/tables/SavedViewsBar';
import { ActiveFilterChips, type FilterChip } from '@/components/tables/ActiveFilterChips';
import { DensityToggle, useDensityWithOverride } from '@/components/tables/DensityToggle';
import { ExportCsvButton } from '@/components/tables/ExportCsvButton';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canEditPartners, useAuth } from '@/lib/auth/context';
import { useDensity, usePageSize } from '@/hooks/usePreferences';
import type { Partner, PartnerKind } from '@/lib/api/types';

export function PartnerList({
  kind,
  title,
  newLabel,
  emptyTitle,
}: {
  /** Fixed by the menu entry — the menu itself is the consumer/business switch. */
  kind: PartnerKind;
  title: string;
  newLabel: string;
  emptyTitle: string;
}) {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const tq = useTranslations('qol');
  const locale = useLocale();
  const { user } = useAuth();
  useRememberList('partners');
  const [q, setQ] = useUrlState('q', '');
  const [includeArchived, setIncludeArchived] = useUrlFlag('arch', false);
  const [page, setPage] = useUrlInt('page', 1);
  const [sortRaw, setSortRaw] = useUrlState('sort', '');
  const sort: TableSort | null = sortRaw === '' ? null : sortRaw.startsWith('-')
    ? { key: sortRaw.slice(1), dir: 'desc' }
    : { key: sortRaw, dir: 'asc' };
  const setSort = (s: TableSort | null) => {
    setSortRaw(!s ? '' : s.dir === 'desc' ? `-${s.key}` : s.key);
  };
  const debouncedQ = useDebouncedValue(q);
  const pageSize = usePageSize();
  const serverDensity = useDensity();
  const { density, toggle: toggleDensity, overridden: densityOverridden } =
    useDensityWithOverride(serverDensity);
  const { visibility: colVis, onChange: setColVis, reset: resetColVis } =
    useColumnVisibility('partners');
  const offset = Math.max(0, (page - 1) * pageSize);

  const query = useQuery({
    queryKey: qk.partners({ q: debouncedQ, kind, includeArchived, offset, pageSize, sort }),
    queryFn: () =>
      partnersApi.list({
        q: debouncedQ || undefined,
        kind,
        include_archived: includeArchived || undefined,
        sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
        limit: pageSize,
        offset,
      }),
  });

  const columns = useMemo<ColumnDef<Partner>[]>(
    () => [
      {
        header: tc('name'),
        accessorKey: 'name',
        meta: { sortKey: 'name' },
        cell: ({ row }) => (
          <span className="flex items-center gap-2">
            <Link
              href={`/${locale}/partners/${row.original.id}`}
              className="font-medium text-steel-900 underline"
            >
              {row.original.name}
            </Link>
            {row.original.archived_at && <StatusBadge tone="steel">{t('archived')}</StatusBadge>}
          </span>
        ),
      },
      { header: t('taxNumber'), accessorKey: 'tax_number', cell: ({ getValue }) => getValue<string>() ?? '—' },
      { header: tc('email'), accessorKey: 'email', cell: ({ getValue }) => getValue<string>() ?? '—' },
      { header: tc('phone'), accessorKey: 'phone', cell: ({ getValue }) => getValue<string>() ?? '—' },
      { header: t('city'), accessorKey: 'city', cell: ({ getValue }) => getValue<string>() ?? '—' },
    ],
    [t, tc, locale],
  );

  const clear = () => {
    setQ('');
    setIncludeArchived(false);
    setPage(1);
    setSort(null);
  };

  /** Current filters, first 200 rows (the API max page). */
  const exportPartners = async () => {
    const data = await partnersApi.list({
      q: debouncedQ || undefined,
      kind,
      include_archived: includeArchived || undefined,
      sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
      limit: 200,
      offset: 0,
    });
    const items = data.items ?? [];
    return {
      header: [tc('name'), t('taxNumber'), tc('email'), tc('phone'), t('city')],
      rows: items.map((p) => [p.name, p.tax_number, p.email, p.phone, p.city]),
      count: items.length,
    };
  };

  const chips = useMemo<FilterChip[]>(() => {
    const list: FilterChip[] = [];
    if (q.trim() !== '') {
      list.push({ key: 'q', label: `${tc('search')}: ${q.trim()}`, onRemove: () => { setQ(''); setPage(1); } });
    }
    if (includeArchived) {
      list.push({ key: 'arch', label: t('includeArchived'), onRemove: () => { setIncludeArchived(false); setPage(1); } });
    }
    if (sort) {
      list.push({ key: 'sort', label: tq('sortChip', { key: sort.key, dir: sort.dir === 'desc' ? '↓' : '↑' }), onRemove: () => { setSort(null); setPage(1); } });
    }
    return list;
  }, [q, includeArchived, sort, tc, t, tq, setQ, setIncludeArchived, setSort]);

  return (
    <AppShell>
      <PageHeader
        title={title}
        actions={
          canEditPartners(user) && (
            <Link href={`/${locale}/partners/new?kind=${kind}`} className="btn-primary btn-sm">
              {newLabel}
            </Link>
          )
        }
      />
      <SavedViewsBar listKey="partners" />
      <FilterBar onClear={clear}>
        <FilterField label={tc('search')}>
          <input
            className="input"
            placeholder={t('searchPlaceholder')}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setPage(1);
            }}
          />
        </FilterField>
        <label className="flex items-center gap-2 pb-2 text-body">
          <input
            type="checkbox"
            className="rounded border-steel-200 accent-steel-900"
            checked={includeArchived}
            onChange={(e) => {
              setIncludeArchived(e.target.checked);
              setPage(1);
            }}
          />
          {t('includeArchived')}
        </label>
      </FilterBar>
      <ActiveFilterChips chips={chips} />

      {query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <>
          <DataTable
            columns={columns}
            data={query.data?.items ?? []}
            isLoading={query.isPending}
            isFetching={query.isFetching && !query.isPending}
            emptyTitle={emptyTitle}
            emptyFilteredTitle={te('filterNoResults')}
            filtered={debouncedQ.trim() !== '' || includeArchived}
            getRowId={(r) => String(r.id)}
            density={density}
            sort={sort}
            columnVisibility={colVis}
            onColumnVisibilityChange={setColVis}
            onSort={(key) => {
              setSort(nextSort(sort, key));
              setPage(1);
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
              <ExportCsvButton base="partnerek" onExport={exportPartners} />
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
