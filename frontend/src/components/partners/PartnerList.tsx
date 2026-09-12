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
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
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
  const locale = useLocale();
  const { user } = useAuth();
  const [q, setQ] = useState('');
  const [includeArchived, setIncludeArchived] = useState(false);
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<TableSort | null>(null);
  const debouncedQ = useDebouncedValue(q);
  const pageSize = usePageSize();
  const density = useDensity();

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
    setOffset(0);
    setSort(null);
  };

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
      <FilterBar onClear={clear}>
        <FilterField label={tc('search')}>
          <input
            className="input"
            placeholder={t('searchPlaceholder')}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setOffset(0);
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
              setOffset(0);
            }}
          />
          {t('includeArchived')}
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
            emptyTitle={emptyTitle}
            emptyFilteredTitle={te('filterNoResults')}
            filtered={debouncedQ.trim() !== '' || includeArchived}
            getRowId={(r) => String(r.id)}
            density={density}
            sort={sort}
            onSort={(key) => {
              setSort(nextSort(sort, key));
              setOffset(0);
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
