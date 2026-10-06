'use client';

// The bar that appears above a list once rows are ticked: act on all of them at once.
// The server applies each row on its own rules and reports the ones it skipped (a stage
// a lead cannot move to, say), so one bad row never sinks the rest.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { X } from 'lucide-react';
import { configApi, leadsApi, leadTagsApi, ordersApi, usersApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';

/** A checkbox column for DataTable: ticks a row, or every row on the page from the header. */
export function selectColumn<T extends { id: number }>(
  rows: T[],
  ticked: number[],
  setTicked: (ids: number[]) => void,
  label: string,
) {
  const all = rows.length > 0 && rows.every((r) => ticked.includes(r.id));
  return {
    id: 'select',
    enableSorting: false,
    header: () => (
      <input
        type="checkbox"
        aria-label={label}
        className="rounded border-steel-200 accent-steel-900"
        checked={all}
        onChange={(e) =>
          setTicked(e.target.checked ? [...new Set([...ticked, ...rows.map((r) => r.id)])] : ticked.filter((id) => !rows.some((r) => r.id === id)))
        }
      />
    ),
    cell: ({ row }: { row: { original: T } }) => (
      <input
        type="checkbox"
        aria-label={label}
        className="rounded border-steel-200 accent-steel-900"
        checked={ticked.includes(row.original.id)}
        onChange={(e) =>
          setTicked(e.target.checked ? [...ticked, row.original.id] : ticked.filter((id) => id !== row.original.id))
        }
      />
    ),
  };
}

type Skip = { id: number; error: string };

function Outcome({ applied, skipped }: { applied: number; skipped: Skip[] }) {
  const t = useTranslations('bulk');
  return (
    <div className="text-metadata" role="status">
      <span className="text-done">{t('applied', { count: applied })}</span>
      {skipped.length > 0 && (
        <details className="inline">
          <summary className="ml-2 inline cursor-pointer text-signal">{t('skipped', { count: skipped.length })}</summary>
          <ul className="mt-1 space-y-0.5 text-steel-500">
            {skipped.slice(0, 20).map((s) => (
              <li key={s.id}>#{s.id}: {s.error}</li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

export function LeadBulkBar({ ids, onClear }: { ids: number[]; onClear: () => void }) {
  const t = useTranslations('bulk');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [action, setAction] = useState<'assign' | 'add_tag' | 'remove_tag' | 'stage'>('assign');
  const [value, setValue] = useState('');
  const [result, setResult] = useState<{ applied: number; skipped: Skip[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const users = useQuery({ queryKey: ['users'], queryFn: () => usersApi.list(), enabled: action === 'assign' });
  const tags = useQuery({ queryKey: ['lead-tags'], queryFn: () => leadTagsApi.list(), enabled: action.endsWith('tag') });
  const stages = useQuery({ queryKey: qk.stages('lead'), queryFn: () => configApi.stages('lead'), enabled: action === 'stage' });

  const run = useMutation({
    mutationFn: () => {
      const n = Number(value);
      const body =
        action === 'assign'
          ? { ids, action, assigned_to: n }
          : action === 'stage'
            ? { ids, action, stage: value }
            : { ids, action, tag_id: n };
      return leadsApi.bulk(body);
    },
    onSuccess: (r) => {
      setError(null);
      setResult(r);
      void qc.invalidateQueries({ queryKey: ['leads'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const options: { value: string; label: string }[] =
    action === 'assign'
      ? (users.data?.items ?? []).filter((u) => u.is_active).map((u) => ({ value: String(u.id), label: u.display_name }))
      : action === 'stage'
        ? (stages.data?.items ?? []).filter((s) => s.key !== 'won').map((s) => ({ value: s.key, label: s.label_hu }))
        : (tags.data?.items ?? []).map((x) => ({ value: String(x.id), label: `${x.market.toUpperCase()}: ${x.label}` }));

  return (
    <div className="card flex flex-wrap items-center gap-2 border-steel-900 p-3" data-testid="lead-bulk-bar">
      <span className="text-body font-medium">{t('selected', { count: ids.length })}</span>
      <select
        className="input h-8 w-auto py-0"
        aria-label={t('action')}
        value={action}
        onChange={(e) => {
          setAction(e.target.value as typeof action);
          setValue('');
          setResult(null);
        }}
      >
        <option value="assign">{t('assign')}</option>
        <option value="add_tag">{t('addTag')}</option>
        <option value="remove_tag">{t('removeTag')}</option>
        <option value="stage">{t('stage')}</option>
      </select>
      <select className="input h-8 w-auto min-w-40 py-0" aria-label={t('value')} value={value} onChange={(e) => setValue(e.target.value)}>
        <option value="">—</option>
        {options.map((o) => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>
      <button type="button" className="btn-primary btn-sm" disabled={!value || run.isPending} onClick={() => run.mutate()}>
        {run.isPending ? '…' : t('apply')}
      </button>
      {result && <Outcome applied={result.applied} skipped={result.skipped} />}
      {error && <span className="text-metadata text-signal" role="alert">{error}</span>}
      <button type="button" className="btn-ghost btn-sm ml-auto" onClick={onClear}>
        <X className="h-4 w-4" aria-hidden />
        {t('clear')}
      </button>
    </div>
  );
}

export function OrderBulkBar({ ids, onClear }: { ids: number[]; onClear: () => void }) {
  const t = useTranslations('bulk');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [stage, setStage] = useState('');
  const [note, setNote] = useState('');
  const [result, setResult] = useState<{ applied: number; skipped: Skip[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const stages = useQuery({ queryKey: qk.stages('order'), queryFn: () => configApi.stages('order') });
  const run = useMutation({
    mutationFn: () => ordersApi.bulk({ ids, stage, note: note.trim() || null }),
    onSuccess: (r) => {
      setError(null);
      setResult({ applied: r.moved, skipped: r.skipped });
      void qc.invalidateQueries({ queryKey: ['orders'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <div className="card flex flex-wrap items-center gap-2 border-steel-900 p-3" data-testid="order-bulk-bar">
      <span className="text-body font-medium">{t('selected', { count: ids.length })}</span>
      <select className="input h-8 w-auto min-w-40 py-0" aria-label={t('stage')} value={stage} onChange={(e) => setStage(e.target.value)}>
        <option value="">{t('stage')}…</option>
        {(stages.data?.items ?? []).map((s) => (
          <option key={s.key} value={s.key}>{s.label_hu}</option>
        ))}
      </select>
      <input className="input h-8 w-48 py-0" placeholder={t('note')} aria-label={t('note')} value={note} onChange={(e) => setNote(e.target.value)} />
      <button type="button" className="btn-primary btn-sm" disabled={!stage || run.isPending} onClick={() => run.mutate()}>
        {run.isPending ? '…' : t('apply')}
      </button>
      {result && <Outcome applied={result.applied} skipped={result.skipped} />}
      {error && <span className="text-metadata text-signal" role="alert">{error}</span>}
      <button type="button" className="btn-ghost btn-sm ml-auto" onClick={onClear}>
        <X className="h-4 w-4" aria-hidden />
        {t('clear')}
      </button>
    </div>
  );
}
