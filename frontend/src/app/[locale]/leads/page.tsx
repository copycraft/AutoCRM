'use client';

import { useMemo, useState } from 'react';
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
import { AssigneeField } from '@/components/forms/AssigneeField';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { configApi, leadsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canEditLeads, useAuth } from '@/lib/auth/context';
import { daysSince, formatDate } from '@/lib/utils/format';
import { stageTone } from '@/lib/utils/stages';
import type { LeadSummary, StageDefinition } from '@/types/api';

const PAGE_SIZE = 50;

export default function LeadsPage() {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const tn = useTranslations('navigation');
  const locale = useLocale();
  const { user } = useAuth();
  const [q, setQ] = useState('');
  const [stage, setStage] = useState('');
  const [assignee, setAssignee] = useState<number | null | 'all' | 'me'>(null);
  const [openOnly, setOpenOnly] = useState(false);
  const [offset, setOffset] = useState(0);
  const debouncedQ = useDebouncedValue(q);

  const assignedTo =
    assignee === 'me' ? (user?.id ?? null) : assignee === 'all' ? undefined : (assignee ?? undefined);

  const stagesQuery = useQuery({
    queryKey: qk.stages('lead'),
    queryFn: () => configApi.stages('lead'),
  });
  const defs = useMemo(() => {
    const m = new Map<string, StageDefinition>();
    for (const d of stagesQuery.data?.items ?? []) m.set(d.key, d);
    return m;
  }, [stagesQuery.data]);

  const query = useQuery({
    queryKey: qk.leads({ q: debouncedQ, stage, assignedTo, openOnly, offset }),
    queryFn: () =>
      leadsApi.list({
        q: debouncedQ || undefined,
        stage: stage || undefined,
        assigned_to: assignedTo ?? undefined,
        open: openOnly || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
  });

  const columns = useMemo<ColumnDef<LeadSummary>[]>(
    () => [
      {
        header: t('title'),
        accessorKey: 'title',
        cell: ({ row }) => (
          <span className="flex items-center gap-2">
            <Link href={`./leads/${row.original.id}`} className="font-medium text-cold hover:underline">
              {row.original.title}
            </Link>
            {row.original.order_number && (
              <Link
                href={`/${locale}/orders/${row.original.order_id}`}
                className="font-mono text-metadata text-steel-500 hover:underline"
                title={t('convertedOrder')}
              >
                → {row.original.order_number}
              </Link>
            )}
          </span>
        ),
      },
      {
        header: t('partner'),
        accessorKey: 'partner_name',
        cell: ({ getValue }) => getValue<string | null>() ?? '—',
      },
      {
        header: t('stage'),
        accessorKey: 'stage_label',
        cell: ({ row }) => (
          <StatusBadge tone={stageTone(defs.get(row.original.stage_key))}>
            {row.original.stage_label}
          </StatusBadge>
        ),
      },
      {
        header: t('age'),
        accessorKey: 'created_at',
        cell: ({ getValue }) => (
          <span className="font-mono">{daysSince(getValue<string>())} nap</span>
        ),
      },
      {
        header: t('assignedTo'),
        accessorKey: 'assigned_name',
        cell: ({ getValue }) => getValue<string | null>() ?? '—',
      },
      {
        header: 'Létrehozva',
        accessorKey: 'created_at',
        cell: ({ getValue }) => <span className="font-mono">{formatDate(getValue<string>())}</span>,
      },
    ],
    [defs, locale, t],
  );

  const clear = () => {
    setQ('');
    setStage('');
    setAssignee(null);
    setOpenOnly(false);
    setOffset(0);
  };

  return (
    <AppShell>
      <PageHeader
        title={tn('leads')}
        actions={
          canEditLeads(user) && (
            <Link href={`/${locale}/leads/new`} className="btn-primary btn-sm">
              {t('newLead')}
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
        <FilterField label={t('stage')}>
          <select
            className="input"
            value={stage}
            onChange={(e) => {
              setStage(e.target.value);
              setOffset(0);
            }}
          >
            <option value="">{tc('all')}</option>
            {(stagesQuery.data?.items ?? []).map((d) => (
              <option key={d.key} value={d.key}>
                {d.label_hu}
              </option>
            ))}
          </select>
        </FilterField>
        <AssigneeField
          label={t('assigneeFilter')}
          value={assignee}
          allowAll
          onChange={(v) => {
            setAssignee(v);
            setOffset(0);
          }}
        />
        <label className="flex items-center gap-2 pb-2 text-sm">
          <input
            type="checkbox"
            className="rounded border-steel-200"
            checked={openOnly}
            onChange={(e) => {
              setOpenOnly(e.target.checked);
              setOffset(0);
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
            emptyTitle={te('noLeads')}
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
