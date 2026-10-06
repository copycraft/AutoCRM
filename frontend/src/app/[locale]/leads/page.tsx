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
import { AssigneeField } from '@/components/forms/AssigneeField';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useDensity, usePageSize } from '@/hooks/usePreferences';
import { useUrlFlag, useUrlInt, useUrlState } from '@/hooks/useUrlState';
import { useRememberList } from '@/hooks/useListMemory';
import { SavedViewsBar } from '@/components/tables/SavedViewsBar';
import { ActiveFilterChips, type FilterChip } from '@/components/tables/ActiveFilterChips';
import { DensityToggle, useDensityWithOverride } from '@/components/tables/DensityToggle';
import { ExportCsvButton, collectAll } from '@/components/tables/ExportCsvButton';
import { errorMessage } from '@/lib/api/errors';
import { configApi, leadsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canEditLeads, useAuth } from '@/lib/auth/context';
import { daysSince } from '@/lib/utils/format';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { stageTone } from '@/lib/utils/stages';
import { TagChip, sortMarkets, useLeadTags, useMarketName } from '@/components/leads/LeadTags';
import type { LeadRow, StageDefinition } from '@/lib/api/types';



export default function LeadsPage() {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const tn = useTranslations('navigation');
  const tq = useTranslations('qol');
  const tt = useTranslations('leadTags');
  const marketName = useMarketName();
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  useRememberList('leads');
  const [q, setQ] = useUrlState('q', '');
  const [stage, setStage] = useUrlState('stage', '');
  const [tagRaw, setTagRaw] = useUrlState('tag', '');
  const tag = Number(tagRaw) || null;
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
    useColumnVisibility('leads');
  const offset = Math.max(0, (page - 1) * pageSize);

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

  const tagsQuery = useLeadTags();
  const tagGroups = useMemo(() => {
    const items = tagsQuery.data?.items ?? [];
    return sortMarkets(items.map((x) => x.market)).map(
      (m) => [m, items.filter((x) => x.market === m)] as const,
    );
  }, [tagsQuery.data]);
  const setTag = (id: number | null) => {
    setTagRaw(id === null ? '' : String(id));
    setPage(1);
  };

  const query = useQuery({
    queryKey: qk.leads({ q: debouncedQ, stage, tag, assignedTo, openOnly, offset, pageSize, sort }),
    queryFn: () =>
      leadsApi.list({
        q: debouncedQ || undefined,
        stage: stage || undefined,
        tag: tag ?? undefined,
        assigned_to: assignedTo ?? undefined,
        open: openOnly || undefined,
        sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
        limit: pageSize,
        offset,
      }),
  });

  const columns = useMemo<ColumnDef<LeadRow>[]>(
    () => [
      {
        header: t('title'),
        accessorKey: 'title',
        meta: { sortKey: 'title' },
        cell: ({ row }) => (
          <span className="flex items-center gap-2">
            <Link href={`./leads/${row.original.id}`} className="font-medium text-steel-900 underline">
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
        header: tt('tags'),
        id: 'tags',
        accessorFn: (r) => r.tags.map((x) => x.label).join(', '),
        cell: ({ row }) =>
          row.original.tags.length === 0 ? (
            '—'
          ) : (
            <span className="flex max-w-xs flex-wrap gap-1">
              {row.original.tags.map((x) => (
                <TagChip key={x.id} tag={x} onClick={() => setTag(x.id)} />
              ))}
            </span>
          ),
      },
      {
        header: t('age'),
        id: 'age',
        accessorKey: 'created_at',
        meta: { align: 'right' },
        cell: ({ getValue }) => (
          <span className="font-mono">{tc('ageDays', { days: daysSince(getValue<string>()) })}</span>
        ),
      },
      {
        header: t('assignedTo'),
        accessorKey: 'assigned_name',
        cell: ({ getValue }) => getValue<string | null>() ?? '—',
      },
      {
        header: t('createdAt'),
        accessorKey: 'created_at',
        meta: { sortKey: 'created_at', align: 'right' },
        cell: ({ getValue }) =>           <DateDisplay value={getValue<string>()} />,
      },
    ],
    // setTag only writes the URL.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [defs, locale, t, tc, tt],
  );

  const clear = () => {
    setQ('');
    setStage('');
    setTagRaw('');
    setAssignee(null);
    setOpenOnly(false);
    setPage(1);
    setSort(null);
  };

  const setPage1 = () => setPage(1);

  /** Current filters, first 200 rows (the API max page). */
  const exportLeads = async () => {
    const all = await collectAll((offset, limit) =>
      leadsApi.list({
      q: debouncedQ || undefined,
      stage: stage || undefined,
      tag: tag ?? undefined,
      assigned_to: assignedTo ?? undefined,
      open: openOnly || undefined,
      sort: sort ? (sort.dir === 'desc' ? `-${sort.key}` : sort.key) : undefined,
        limit,
        offset,
      }),
    );
    const items = all;
    return {
      header: [
        t('title'), t('partner'), t('contactName'), t('contactEmail'), t('source'), t('stage'), tt('tags'),
        t('quotedValue'), tc('currency'), t('assignedTo'), t('createdAt'),
      ],
      rows: items.map((l) => [
        l.title,
        l.partner_name,
        l.contact_name,
        l.contact_email,
        l.source,
        l.stage_label,
        l.tags.map((x) => `${x.market.toUpperCase()}: ${x.label}`).join(', '),
        l.quoted_value_minor != null ? l.quoted_value_minor / 100 : null,
        l.currency,
        l.assigned_name,
        l.created_at,
      ]),
      count: items.length,
    };
  };

  const chips = useMemo<FilterChip[]>(() => {
    const list: FilterChip[] = [];
    if (q.trim() !== '') {
      list.push({ key: 'q', label: `${tc('search')}: ${q.trim()}`, onRemove: () => { setQ(''); setPage1(); } });
    }
    if (stage !== '') {
      const label = stagesQuery.data?.items.find((d) => d.key === stage)?.label_hu ?? stage;
      list.push({ key: 'stage', label: `${t('stage')}: ${label}`, onRemove: () => { setStage(''); setPage1(); } });
    }
    if (tag !== null) {
      const found = tagsQuery.data?.items.find((x) => x.id === tag);
      const label = found ? `${marketName(found.market)} · ${found.label}` : `#${tag}`;
      list.push({ key: 'tag', label: `${tt('filter')}: ${label}`, onRemove: () => setTag(null) });
    }
    if (assignee !== null && assignee !== 'all') {
      const label = assignee === 'me' ? (user?.display_name ?? 'me') : `#${assignee}`;
      list.push({ key: 'assignee', label: `${t('assigneeFilter')}: ${label}`, onRemove: () => { setAssignee(null); setPage1(); } });
    }
    if (openOnly) {
      list.push({ key: 'open', label: t('openOnly'), onRemove: () => { setOpenOnly(false); setPage1(); } });
    }
    if (sort) {
      list.push({ key: 'sort', label: tq('sortChip', { key: sort.key, dir: sort.dir === 'desc' ? '↓' : '↑' }), onRemove: () => { setSort(null); setPage1(); } });
    }
    return list;
    // setPage1 is a stable wrapper around setPage.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q, stage, tag, assignee, user, openOnly, sort, stagesQuery.data, tagsQuery.data, tc, t, tq, tt]);

  return (
    <AppShell>
      <PageHeader
        title={tn('leads')}
        actions={
          canEditLeads(user) && (
            <>
              <Link href={`/${locale}/leads/tags`} className="btn-secondary btn-sm">
                {tt('manage')}
              </Link>
              <Link href={`/${locale}/leads/new`} className="btn-primary btn-sm">
                {t('newLead')}
              </Link>
            </>
          )
        }
      />
      <SavedViewsBar listKey="leads" />
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
        <FilterField label={t('stage')}>
          <select
            className="input"
            value={stage}
            disabled={stagesQuery.isLoading}
            onChange={(e) => {
              setStage(e.target.value);
              setPage(1);
            }}
          >
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
        <FilterField label={tt('filter')}>
          <select
            className="input"
            value={tag ?? ''}
            disabled={tagsQuery.isLoading}
            onChange={(e) => setTag(e.target.value === '' ? null : Number(e.target.value))}
          >
            <option value="">{tc('all')}</option>
            {tagGroups.map(([market, list]) => (
              <optgroup key={market} label={marketName(market)}>
                {list.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.label} ({x.open_leads}/{x.total_leads})
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </FilterField>
        <AssigneeField
          label={t('assigneeFilter')}
          value={assignee}
          allowAll
          allowEmpty={false}
          onChange={(v) => {
            setAssignee(v);
            setPage(1);
          }}
        />
        <label className="flex items-center gap-2 pb-2 text-body">
          <input
            type="checkbox"
            className="rounded border-steel-200 accent-steel-900"
            checked={openOnly}
            onChange={(e) => {
              setOpenOnly(e.target.checked);
              setPage(1);
            }}
          />
          {t('openOnly')}
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
            emptyTitle={te('noLeads')}
            emptyFilteredTitle={te('filterNoResults')}
            filtered={
              debouncedQ.trim() !== '' ||
              stage !== '' ||
              tag !== null ||
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
              <ExportCsvButton base="leadek" onExport={exportLeads} />
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
