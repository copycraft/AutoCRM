'use client';

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import type { ColumnDef } from '@tanstack/react-table';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DataTable } from '@/components/tables/DataTable';
import { FilterBar, FilterField } from '@/components/tables/FilterBar';
import { Pagination } from '@/components/ui/Pagination';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canEditPartners, useAuth } from '@/lib/auth/context';
import type { Partner, PartnerKind } from '@/lib/api/types';

function toKind(value: string): PartnerKind | '' {
  return value === 'business' || value === 'person' ? value : '';
}

const PAGE_SIZE = 50;

const columns: ColumnDef<Partner>[] = [
  {
    header: 'Név',
    accessorKey: 'name',
    cell: ({ row }) => (
      <span className="flex items-center gap-2">
        <Link
          href={`./partners/${row.original.id}`}
          className="font-medium text-cold hover:underline"
        >
          {row.original.name}
        </Link>
        {row.original.archived_at && <StatusBadge tone="steel">Archivált</StatusBadge>}
      </span>
    ),
  },
  {
    header: 'Típus',
    accessorKey: 'kind',
    cell: ({ getValue }) => (getValue<string>() === 'business' ? 'Vállalkozás' : 'Személy'),
  },
  { header: 'Adószám', accessorKey: 'tax_number', cell: ({ getValue }) => getValue<string>() ?? '—' },
  { header: 'E-mail', accessorKey: 'email', cell: ({ getValue }) => getValue<string>() ?? '—' },
  { header: 'Telefon', accessorKey: 'phone', cell: ({ getValue }) => getValue<string>() ?? '—' },
  { header: 'Város', accessorKey: 'city', cell: ({ getValue }) => getValue<string>() ?? '—' },
];

export default function PartnersPage() {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const locale = useLocale();
  const { user } = useAuth();
  const [q, setQ] = useState('');
  const [kind, setKind] = useState<PartnerKind | ''>('');
  const [includeArchived, setIncludeArchived] = useState(false);
  const [offset, setOffset] = useState(0);
  const debouncedQ = useDebouncedValue(q);

  const query = useQuery({
    queryKey: qk.partners({ q: debouncedQ, kind, includeArchived, offset }),
    queryFn: () =>
      partnersApi.list({
        q: debouncedQ || undefined,
        kind: kind || undefined,
        include_archived: includeArchived || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
  });

  const clear = () => {
    setQ('');
    setKind('');
    setIncludeArchived(false);
    setOffset(0);
  };

  return (
    <AppShell>
      <PageHeader
        title={t('title')}
        actions={
          canEditPartners(user) && (
            <Link href={`/${locale}/partners/new`} className="btn-primary btn-sm">
              {t('newPartner')}
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
        <FilterField label={t('kindLabel')}>
          <select
            className="input"
            value={kind}
            onChange={(e) => {
              setKind(toKind(e.target.value));
              setOffset(0);
            }}
          >
            <option value="">{t('allKinds')}</option>
            <option value="business">{t('business')}</option>
            <option value="person">{t('person')}</option>
          </select>
        </FilterField>
        <label className="flex items-center gap-2 pb-2 text-sm">
          <input
            type="checkbox"
            className="rounded border-steel-200"
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
            emptyTitle={te('noPartners')}
            getRowId={(r) => String(r.id)}
          />
          <Pagination
            offset={offset}
            limit={PAGE_SIZE}
            loaded={query.data?.items.length ?? 0}
            onPrev={() => setOffset((o) => Math.max(0, o - PAGE_SIZE))}
            onNext={() => setOffset((o) => o + PAGE_SIZE)}
          />
        </>
      )}
    </AppShell>
  );
}
