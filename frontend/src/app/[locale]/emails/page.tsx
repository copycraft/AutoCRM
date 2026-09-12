'use client';

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import type { ColumnDef } from '@tanstack/react-table';
import { useMemo } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DataTable } from '@/components/tables/DataTable';
import { FilterBar, FilterField } from '@/components/tables/FilterBar';
import { Pagination } from '@/components/ui/Pagination';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useDensity, usePageSize } from '@/hooks/usePreferences';
import { emailApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import type { EmailStatus, EmailSummary } from '@/lib/api/types';

/** Delivery state, not decoration: failed and unknown outcomes read as alerts. */
function statusTone(status: EmailStatus): StatusTone {
  switch (status) {
    case 'failed':
    case 'needs_review':
      return 'signal';
    case 'sent':
      return 'done';
    case 'cancelled':
      return 'muted';
    default:
      return 'steel';
  }
}

const STATUS_KEYS: Record<EmailStatus, string> = {
  queued: 'queued',
  sending: 'sending',
  sent: 'sent',
  failed: 'failed',
  cancelled: 'cancelled',
  needs_review: 'needsReview',
};

export default function EmailsPage() {
  const t = useTranslations('emails');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const tn = useTranslations('navigation');
  const locale = useLocale();
  const [q, setQ] = useState('');
  const [status, setStatus] = useState<EmailStatus | ''>('');
  const [offset, setOffset] = useState(0);
  const debouncedQ = useDebouncedValue(q);
  const pageSize = usePageSize();
  const density = useDensity();

  // The list endpoint has no free-text search; filtering by subject/address
  // happens client-side on the loaded page.
  const query = useQuery({
    queryKey: qk.emails({ status, offset, pageSize }),
    queryFn: () =>
      emailApi.list({
        status: status || undefined,
        limit: pageSize,
        offset,
      }),
  });

  const rows = useMemo(() => {
    const needle = debouncedQ.trim().toLowerCase();
    const items = query.data?.items ?? [];
    if (!needle) return items;
    return items.filter(
      (m) =>
        m.subject.toLowerCase().includes(needle) ||
        m.to_address.toLowerCase().includes(needle),
    );
  }, [query.data, debouncedQ]);

  const columns = useMemo<ColumnDef<EmailSummary>[]>(
    () => [
      {
        header: t('subject'),
        accessorKey: 'subject',
        cell: ({ row }) => (
          <span className="flex items-center gap-2">
            <Link
              href={`/${locale}/emails/${row.original.id}`}
              className="font-medium text-steel-900 underline"
            >
              {row.original.subject}
            </Link>
            {row.original.is_automatic && (
              <StatusBadge tone="steel">{t('autoBadge')}</StatusBadge>
            )}
          </span>
        ),
      },
      {
        header: t('to'),
        accessorKey: 'to_address',
        cell: ({ getValue }) => getValue<string>(),
      },
      {
        header: t('orderLink'),
        accessorKey: 'order_id',
        cell: ({ row }) =>
          row.original.order_id ? (
            <Link
              href={`/${locale}/orders/${row.original.order_id}`}
              className="font-mono text-steel-900 underline"
            >
              #{row.original.order_id}
            </Link>
          ) : (
            '—'
          ),
      },
      {
        header: tc('status'),
        accessorKey: 'status',
        cell: ({ row }) => (
          <StatusBadge tone={statusTone(row.original.status)}>
            {t(`status.${STATUS_KEYS[row.original.status]}`)}
          </StatusBadge>
        ),
      },
      {
        header: t('queuedAt'),
        accessorKey: 'queued_at',
        cell: ({ getValue }) => <DateDisplay value={getValue<string>()} />,
      },
    ],
    [t, tc, locale],
  );

  const clear = () => {
    setQ('');
    setStatus('');
    setOffset(0);
  };

  return (
    <AppShell>
      <PageHeader title={tn('emails')} />
      <FilterBar onClear={clear}>
        <FilterField label={tc('search')}>
          <input
            className="input"
            placeholder={`${t('subject')}, ${t('to')}…`}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setOffset(0);
            }}
          />
        </FilterField>
        <FilterField label={t('statusFilter')}>
          <select
            className="input"
            value={status}
            onChange={(e) => {
              setStatus(e.target.value as EmailStatus | '');
              setOffset(0);
            }}
          >
            <option value="">{tc('all')}</option>
            {(Object.keys(STATUS_KEYS) as EmailStatus[]).map((s) => (
              <option key={s} value={s}>
                {t(`status.${STATUS_KEYS[s]}`)}
              </option>
            ))}
          </select>
        </FilterField>
      </FilterBar>

      {query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <>
          <DataTable
            columns={columns}
            data={rows}
            isLoading={query.isLoading}
            emptyTitle={te('noEmails')}
            getRowId={(r) => String(r.id)}
            density={density}
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
