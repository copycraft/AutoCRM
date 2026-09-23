'use client';

import { useMemo } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import type { ColumnDef } from '@tanstack/react-table';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DataTable, columnMenuItems } from '@/components/tables/DataTable';
import { ColumnMenu } from '@/components/tables/ColumnMenu';
import { useColumnVisibility } from '@/hooks/useColumnVisibility';
import { FilterBar, FilterField } from '@/components/tables/FilterBar';
import { Pagination } from '@/components/ui/Pagination';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useDensity, usePageSize } from '@/hooks/usePreferences';
import { useUrlInt, useUrlState } from '@/hooks/useUrlState';
import { useRememberList } from '@/hooks/useListMemory';
import { SavedViewsBar } from '@/components/tables/SavedViewsBar';
import { ActiveFilterChips, type FilterChip } from '@/components/tables/ActiveFilterChips';
import { DensityToggle, useDensityWithOverride } from '@/components/tables/DensityToggle';
import { ExportCsvButton } from '@/components/tables/ExportCsvButton';
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
  useRememberList('emails');
  const [q, setQ] = useUrlState('q', '');
  const [statusRaw, setStatusRaw] = useUrlState('status', '');
  const status = (statusRaw || '') as EmailStatus | '';
  const setStatus = (s: EmailStatus | '') => setStatusRaw(s);
  const [page, setPage] = useUrlInt('page', 1);
  const debouncedQ = useDebouncedValue(q);
  const pageSize = usePageSize();
  const serverDensity = useDensity();
  const { density, toggle: toggleDensity, overridden: densityOverridden } =
    useDensityWithOverride(serverDensity);
  const { visibility: colVis, onChange: setColVis, reset: resetColVis } =
    useColumnVisibility('emails');
  const offset = Math.max(0, (page - 1) * pageSize);

  // Free-text search runs server-side over the whole log; the pager counts
  // server-filtered rows, so searching beyond the loaded page just works.
  const query = useQuery({
    queryKey: qk.emails({ status, q: debouncedQ.trim(), offset, pageSize }),
    queryFn: () =>
      emailApi.list({
        status: status || undefined,
        q: debouncedQ.trim() || undefined,
        limit: pageSize,
        offset,
      }),
  });

  const rows = useMemo(() => query.data?.items ?? [], [query.data]);

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
        meta: { align: 'right' },
        cell: ({ getValue }) => <DateDisplay value={getValue<string>()} />,
      },
    ],
    [t, tc, locale],
  );

  const clear = () => {
    setQ('');
    setStatus('');
    setPage(1);
  };

  /** Current status filter, first 200 rows (the API max page); text search stays client-side. */
  const exportEmails = async () => {
    const data = await emailApi.list({
      status: status || undefined,
      limit: 200,
      offset: 0,
    });
    const needle = debouncedQ.trim().toLowerCase();
    const items = (data.items ?? []).filter(
      (m) =>
        !needle ||
        m.subject.toLowerCase().includes(needle) ||
        m.to_address.toLowerCase().includes(needle),
    );
    return {
      header: [t('subject'), t('to'), t('orderLink'), tc('status'), t('queuedAt')],
      rows: items.map((m) => [m.subject, m.to_address, m.order_id, t(`status.${STATUS_KEYS[m.status]}`), m.queued_at]),
      count: items.length,
    };
  };

  const chips = useMemo<FilterChip[]>(() => {
    const list: FilterChip[] = [];
    if (q.trim() !== '') {
      list.push({ key: 'q', label: `${tc('search')}: ${q.trim()}`, onRemove: () => { setQ(''); setPage(1); } });
    }
    if (status !== '') {
      list.push({ key: 'status', label: `${t('statusFilter')}: ${t(`status.${STATUS_KEYS[status]}`)}`, onRemove: () => { setStatus(''); setPage(1); } });
    }
    return list;
  }, [q, status, tc, t, setQ, setStatus]);

  return (
    <AppShell>
      <PageHeader
        title={tn('emails')}
        actions={
          <Link className="btn-primary btn-sm" href={`/${locale}/emails/new`}>
            {t('newEmail')}
          </Link>
        }
      />
      <SavedViewsBar listKey="emails" />
      <FilterBar onClear={clear}>
        <FilterField label={tc('search')}>
          <input
            className="input"
            placeholder={`${t('subject')}, ${t('to')}…`}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setPage(1);
            }}
          />
        </FilterField>
        <FilterField label={t('statusFilter')}>
          <select
            className="input"
            value={status}
            onChange={(e) => {
              setStatus(e.target.value as EmailStatus | '');
              setPage(1);
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
      <ActiveFilterChips chips={chips} />

      {query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <>
          <DataTable
            columns={columns}
            data={rows}
            isLoading={query.isPending}
            isFetching={query.isFetching && !query.isPending}
            emptyTitle={te('noEmails')}
            emptyFilteredTitle={te('filterNoResults')}
            filtered={debouncedQ.trim() !== '' || status !== ''}
            getRowId={(r) => String(r.id)}
            density={density}
            columnVisibility={colVis}
            onColumnVisibilityChange={setColVis}
          />
          <div className="flex items-center justify-between gap-3">
            <Pagination
              offset={offset}
              limit={pageSize}
              loaded={rows.length}
              onPrev={() => setPage(Math.max(1, page - 1))}
              onNext={() => setPage(page + 1)}
              onJump={setPage}
            />
            <div className="flex flex-wrap items-center gap-2">
              <ExportCsvButton base="emailek" onExport={exportEmails} />
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
