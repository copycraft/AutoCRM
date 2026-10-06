'use client';

// The papers HR keeps per employee. Personal, financial and family data: while something
// is missing the employee sits on the matching "…adatokra vár" status, and moves on by
// itself once it is filled in. Documents with an expiry (medical, contract, licence...):
// HR is notified a month before and on the day they run out.

import { useEffect, useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, ExternalLink, Paperclip, Plus, Trash2 } from 'lucide-react';
import { hrApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { budapestIsoPlus } from '@/components/forms/DateQuickPicks';
import type { components } from '@/lib/api/schema.gen';

type Details = components['schemas']['EmployeeDetails'];
type TextKey = Exclude<keyof Details, 'employee_id' | 'birth_date' | 'children_count' | 'updated_at'>;

const GROUPS: { key: 'personal' | 'financial' | 'family'; fields: (keyof Details)[] }[] = [
  { key: 'personal', fields: ['birth_name', 'birth_date', 'birth_place', 'mother_name', 'nationality', 'address', 'id_card_number'] },
  { key: 'financial', fields: ['tax_id', 'taj_number', 'bank_account'] },
  { key: 'family', fields: ['marital_status', 'children_count', 'emergency_contact_name', 'emergency_contact_phone'] },
];

/** Which group still waits for data — the same rule the server applies. */
export function missingGroups(d: Partial<Details>): ('personal' | 'financial' | 'family')[] {
  const blank = (v: unknown) => v === null || v === undefined || (typeof v === 'string' && v.trim() === '');
  const out: ('personal' | 'financial' | 'family')[] = [];
  if (['birth_name', 'birth_date', 'birth_place', 'mother_name', 'address'].some((k) => blank(d[k as keyof Details]))) out.push('personal');
  if (['tax_id', 'taj_number', 'bank_account'].some((k) => blank(d[k as keyof Details]))) out.push('financial');
  if (['marital_status', 'children_count', 'emergency_contact_name'].some((k) => blank(d[k as keyof Details]))) out.push('family');
  return out;
}

export function EmployeeDetailsForm({ employeeId }: { employeeId: number }) {
  const t = useTranslations('hrPapers');
  const ter = useTranslations('errors');
  const tc = useTranslations('common');
  const qc = useQueryClient();
  const query = useQuery({ queryKey: ['employee-details', employeeId], queryFn: () => hrApi.details(employeeId) });
  const [draft, setDraft] = useState<Details | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    if (query.data) setDraft(query.data);
  }, [query.data]);

  const save = useMutation({
    mutationFn: (d: Details) => hrApi.saveDetails(employeeId, d),
    onSuccess: (d) => {
      setDraft(d);
      setError(null);
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
      qc.setQueryData(['employee-details', employeeId], d);
      // The status may have moved on by itself.
      void qc.invalidateQueries({ queryKey: ['employees'] });
      void qc.invalidateQueries({ queryKey: ['hr-statuses'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  if (!draft) return null;
  const set = <K extends keyof Details>(k: K, v: Details[K]) => setDraft({ ...draft, [k]: v });
  const missing = missingGroups(draft);

  return (
    <section className="card-content space-y-4 border-t border-steel-200" data-testid="employee-details">
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-section font-semibold">{t('detailsTitle')}</h3>
        {missing.length === 0 ? (
          <StatusBadge tone="done">{t('complete')}</StatusBadge>
        ) : (
          missing.map((g) => (
            <StatusBadge key={g} tone="signal">{t(`missing.${g}`)}</StatusBadge>
          ))
        )}
      </div>
      {GROUPS.map((g) => (
        <fieldset key={g.key} className="space-y-2">
          <legend className="text-metadata font-semibold uppercase tracking-wide text-steel-500">{t(`groups.${g.key}`)}</legend>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {g.fields.map((k) => {
              const id = `ed-${employeeId}-${k}`;
              return (
                <div key={k}>
                  <label className="label" htmlFor={id}>{t(`fields.${k}`)}</label>
                  {k === 'birth_date' ? (
                    <input id={id} type="date" className="input" value={draft.birth_date ?? ''} onChange={(e) => set('birth_date', e.target.value || null)} />
                  ) : k === 'children_count' ? (
                    <input
                      id={id}
                      type="number"
                      min={0}
                      max={30}
                      className="input"
                      value={draft.children_count ?? ''}
                      onChange={(e) => set('children_count', e.target.value === '' ? null : Number(e.target.value))}
                    />
                  ) : (
                    <input
                      id={id}
                      className="input"
                      value={(draft[k as TextKey] as string | null | undefined) ?? ''}
                      onChange={(e) => set(k as TextKey, e.target.value || null)}
                    />
                  )}
                </div>
              );
            })}
          </div>
        </fieldset>
      ))}
      {error && <p className="text-body text-signal" role="alert">{error}</p>}
      <div className="flex items-center justify-end gap-2">
        {saved && <span className="text-metadata text-done">{t('saved')}</span>}
        <button type="button" className="btn-secondary" disabled={save.isPending} onClick={() => save.mutate(draft)}>
          {save.isPending ? tc('saving') : t('saveDetails')}
        </button>
      </div>
    </section>
  );
}

const KINDS = ['medical', 'contract', 'licence', 'training', 'other'] as const;

export function EmployeeDocuments({ employeeId }: { employeeId: number }) {
  const t = useTranslations('hrPapers');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const key = ['employee-documents', employeeId];
  const docs = useQuery({ queryKey: key, queryFn: () => hrApi.documents(employeeId) });
  const [kind, setKind] = useState<(typeof KINDS)[number]>('medical');
  const [title, setTitle] = useState('');
  const [validUntil, setValidUntil] = useState('');
  const [error, setError] = useState<string | null>(null);
  const file = useRef<HTMLInputElement>(null);
  const today = budapestIsoPlus(0);
  const soon = budapestIsoPlus(30);

  const refresh = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: key });
    void qc.invalidateQueries({ queryKey: ['hr-expiring-documents'] });
    void qc.invalidateQueries({ queryKey: ['timeline'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const create = useMutation({
    mutationFn: async () => {
      const doc = await hrApi.createDocument(employeeId, {
        kind,
        title: title.trim(),
        valid_until: validUntil || null,
      });
      const picked = file.current?.files?.[0];
      if (picked) await hrApi.uploadDocumentFile(employeeId, doc.id, picked);
    },
    onSuccess: () => {
      setTitle('');
      setValidUntil('');
      if (file.current) file.current.value = '';
      refresh();
    },
    onError,
  });
  const remove = useMutation({
    mutationFn: (id: number) => hrApi.deleteDocument(employeeId, id),
    onSuccess: refresh,
    onError,
  });
  const open = async (id: number) => {
    try {
      const { url } = await hrApi.documentFileUrl(employeeId, id);
      window.open(url, '_blank', 'noopener');
    } catch (e) {
      onError(e);
    }
  };

  return (
    <section className="card-content space-y-3 border-t border-steel-200" data-testid="employee-documents">
      <h3 className="text-section font-semibold">{t('documentsTitle')}</h3>
      {error && <p className="text-body text-signal" role="alert">{error}</p>}
      <ul className="divide-y divide-steel-200">
        {(docs.data?.items ?? []).map((d) => {
          const expired = !!d.valid_until && d.valid_until < today;
          const expiring = !expired && !!d.valid_until && d.valid_until <= soon;
          return (
            <li key={d.id} className="flex flex-wrap items-center gap-2 py-2">
              <StatusBadge tone="steel">{t(`kinds.${d.kind}`)}</StatusBadge>
              <span className="min-w-0 flex-1 text-body">{d.title}</span>
              {d.valid_until && (
                <span className={`text-metadata ${expired || expiring ? 'text-signal' : 'text-steel-500'}`}>
                  {(expired || expiring) && <AlertTriangle className="mr-1 inline h-3.5 w-3.5" aria-hidden />}
                  {expired ? t('expired') : t('validUntil')} <DateDisplay value={d.valid_until} />
                </span>
              )}
              {d.file_name && (
                <button type="button" className="btn-ghost btn-sm" onClick={() => void open(d.id)} title={d.file_name}>
                  <ExternalLink className="h-4 w-4" aria-hidden />
                </button>
              )}
              <button type="button" className="btn-ghost btn-sm" aria-label={t('remove')} onClick={() => remove.mutate(d.id)}>
                <Trash2 className="h-4 w-4" aria-hidden />
              </button>
            </li>
          );
        })}
        {docs.data && docs.data.items.length === 0 && (
          <li className="py-2 text-metadata text-steel-500">{t('noDocuments')}</li>
        )}
      </ul>
      <form
        className="grid grid-cols-1 gap-2 rounded-lg bg-steel-200/40 p-3 sm:grid-cols-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (title.trim()) create.mutate();
        }}
      >
        <select className="input" aria-label={t('kind')} value={kind} onChange={(e) => setKind(e.target.value as (typeof KINDS)[number])}>
          {KINDS.map((k) => (
            <option key={k} value={k}>{t(`kinds.${k}`)}</option>
          ))}
        </select>
        <input className="input" placeholder={t('docTitle')} aria-label={t('docTitle')} value={title} onChange={(e) => setTitle(e.target.value)} />
        <label className="flex items-center gap-2 text-metadata text-steel-500">
          {t('validUntil')}
          <input type="date" className="input h-8 py-0" value={validUntil} onChange={(e) => setValidUntil(e.target.value)} />
        </label>
        <label className="flex items-center gap-2 text-metadata text-steel-500">
          <Paperclip className="h-4 w-4" aria-hidden />
          <input ref={file} type="file" accept="application/pdf,image/jpeg,image/png,image/webp" className="min-w-0 text-metadata" />
        </label>
        <div className="sm:col-span-2 flex justify-end">
          <button type="submit" className="btn-secondary btn-sm" disabled={!title.trim() || create.isPending}>
            <Plus className="h-4 w-4" aria-hidden />
            {t('addDocument')}
          </button>
        </div>
      </form>
    </section>
  );
}

/** Documents that ran out or run out within a month, across all staff. */
export function ExpiringDocumentsBanner({ onOpen }: { onOpen: (employeeId: number) => void }) {
  const t = useTranslations('hrPapers');
  const docs = useQuery({ queryKey: ['hr-expiring-documents'], queryFn: () => hrApi.expiringDocuments() });
  const rows = docs.data?.items ?? [];
  if (rows.length === 0) return null;
  const today = budapestIsoPlus(0);
  return (
    <div className="card border-signal/40 p-3" data-testid="expiring-documents">
      <p className="mb-2 flex items-center gap-2 text-body font-semibold">
        <AlertTriangle className="h-4 w-4 text-signal" aria-hidden />
        {t('expiringTitle', { count: rows.length })}
      </p>
      <ul className="space-y-1">
        {rows.slice(0, 8).map((d) => (
          <li key={d.id} className="flex flex-wrap items-center gap-2 text-body">
            <button type="button" className="font-medium underline" onClick={() => onOpen(d.employee_id)}>
              {d.employee_name}
            </button>
            <span className="text-steel-500">{t(`kinds.${d.kind}`)} · {d.title}</span>
            <span className={`ml-auto text-metadata ${d.valid_until < today ? 'text-signal' : 'text-steel-500'}`}>
              <DateDisplay value={d.valid_until} />
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
