'use client';

// The bar that appears above a list once rows are ticked: act on all of them at once.
// The server applies each row on its own rules and reports the ones it skipped (a stage
// a lead cannot move to, say), so one bad row never sinks the rest.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { X } from 'lucide-react';
import {
  configApi,
  incomingExtrasApi,
  leadsApi,
  leadTagsApi,
  ordersApi,
  partnerExtrasApi,
  subscribersApi,
  usersApi,
} from '@/lib/api/endpoints';
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

/** Archive, restore, classify or set the invoice language of the ticked partners (0049). */
export function PartnerBulkBar({ ids, onClear }: { ids: number[]; onClear: () => void }) {
  const t = useTranslations('bulk');
  const ts = useTranslations('statement');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [action, setAction] = useState<'archive' | 'unarchive' | 'set_role' | 'set_invoice_language'>('set_role');
  const [value, setValue] = useState('');
  const [result, setResult] = useState<{ applied: number; skipped: Skip[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const needsValue = action === 'set_role';
  const run = useMutation({
    mutationFn: () =>
      partnerExtrasApi.bulk(
        action === 'set_role'
          ? { ids, action, role: value }
          : action === 'set_invoice_language'
            ? { ids, action, language: value || null }
            : { ids, action },
      ),
    onSuccess: (r) => {
      setError(null);
      setResult(r);
      void qc.invalidateQueries({ queryKey: ['partners'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <div className="card flex flex-wrap items-center gap-2 border-steel-900 p-3" data-testid="partner-bulk-bar">
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
        <option value="set_role">{t('setRole')}</option>
        <option value="set_invoice_language">{ts('invoiceLanguage')}</option>
        <option value="archive">{t('archive')}</option>
        <option value="unarchive">{t('unarchive')}</option>
      </select>
      {action === 'set_role' && (
        <select className="input h-8 w-auto min-w-40 py-0" aria-label={t('value')} value={value} onChange={(e) => setValue(e.target.value)}>
          <option value="">—</option>
          <option value="customer">{t('roles.customer')}</option>
          <option value="supplier">{t('roles.supplier')}</option>
          <option value="both">{t('roles.both')}</option>
        </select>
      )}
      {action === 'set_invoice_language' && (
        <select className="input h-8 w-auto min-w-40 py-0" aria-label={t('value')} value={value} onChange={(e) => setValue(e.target.value)}>
          <option value="">{ts('languageDefault')}</option>
          {(['hu', 'en', 'de'] as const).map((l) => (
            <option key={l} value={l}>{ts(`languages.${l}`)}</option>
          ))}
        </select>
      )}
      <button type="button" className="btn-primary btn-sm" disabled={(needsValue && !value) || run.isPending} onClick={() => run.mutate()}>
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

/** Unsubscribe, resubscribe, set the language of, or delete the ticked readers (0049). */
export function SubscriberBulkBar({ ids, onDone }: { ids: number[]; onDone: () => void }) {
  const t = useTranslations('bulk');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [action, setAction] = useState<'unsubscribe' | 'resubscribe' | 'set_language' | 'delete'>('set_language');
  const [language, setLanguage] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const run = useMutation({
    mutationFn: () =>
      subscribersApi.bulk(
        action === 'set_language'
          ? { subscription_ids: ids, action, language: language || null }
          : { subscription_ids: ids, action },
      ),
    onSuccess: (r) => {
      setMessage(t('applied', { count: r.changed }));
      void qc.invalidateQueries({ queryKey: ['newsletter'] });
      if (action === 'delete') onDone();
    },
    onError: (e) => setMessage(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <select className="input h-8 w-auto py-0" aria-label={t('action')} value={action} onChange={(e) => setAction(e.target.value as typeof action)}>
        <option value="set_language">{t('setLanguage')}</option>
        <option value="unsubscribe">{t('unsubscribe')}</option>
        <option value="resubscribe">{t('resubscribe')}</option>
        <option value="delete">{t('delete')}</option>
      </select>
      {action === 'set_language' && (
        <select className="input h-8 w-auto py-0" aria-label={t('value')} value={language} onChange={(e) => setLanguage(e.target.value)}>
          <option value="">{t('languageUnknown')}</option>
          {NEWSLETTER_LANGUAGES.map((l) => (
            <option key={l} value={l}>{l.toUpperCase()}</option>
          ))}
        </select>
      )}
      <button
        type="button"
        className={action === 'delete' ? 'btn-danger btn-sm' : 'btn-secondary btn-sm'}
        disabled={run.isPending}
        onClick={() => {
          if (action !== 'delete' || window.confirm(t('deleteConfirm', { count: ids.length }))) run.mutate();
        }}
      >
        {t('apply')}
      </button>
      {message && <span className="text-metadata text-steel-600" role="status">{message}</span>}
    </span>
  );
}

/** Languages a newsletter variant or a reader can have. */
export const NEWSLETTER_LANGUAGES = ['hu', 'en', 'de', 'sk', 'ro', 'hr', 'sr', 'pl', 'cs'] as const;

/** Mark paid, file as books-only, or remove the ticked supplier invoices (0049). */
export function IncomingBulkBar({ ids, onClear }: { ids: number[]; onClear: () => void }) {
  const t = useTranslations('bulk');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [action, setAction] = useState<'mark_paid' | 'booking_only' | 'delete'>('mark_paid');
  const [message, setMessage] = useState<string | null>(null);
  const run = useMutation({
    mutationFn: () =>
      incomingExtrasApi.bulk(
        action === 'mark_paid'
          ? { ids, action, paid_on: null }
          : action === 'booking_only'
            ? { ids, action, on: true }
            : { ids, action },
      ),
    onSuccess: (r) => {
      setMessage(t('applied', { count: r.changed }));
      void qc.invalidateQueries({ queryKey: ['incoming'] });
      void qc.invalidateQueries({ queryKey: ['incoming-invoices'] });
      if (action === 'delete') onClear();
    },
    onError: (e) => setMessage(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <div className="card flex flex-wrap items-center gap-2 border-steel-900 p-3" data-testid="incoming-bulk-bar">
      <span className="text-body font-medium">{t('selected', { count: ids.length })}</span>
      <select className="input h-8 w-auto py-0" aria-label={t('action')} value={action} onChange={(e) => setAction(e.target.value as typeof action)}>
        <option value="mark_paid">{t('markPaid')}</option>
        <option value="booking_only">{t('bookingOnly')}</option>
        <option value="delete">{t('remove')}</option>
      </select>
      <button
        type="button"
        className="btn-primary btn-sm"
        disabled={run.isPending}
        onClick={() => {
          if (action !== 'delete' || window.confirm(t('removeConfirm', { count: ids.length }))) run.mutate();
        }}
      >
        {run.isPending ? '…' : t('apply')}
      </button>
      {message && <span className="text-metadata text-steel-600" role="status">{message}</span>}
      <button type="button" className="btn-ghost btn-sm ml-auto" onClick={onClear}>
        <X className="h-4 w-4" aria-hidden />
        {t('clear')}
      </button>
    </div>
  );
}
