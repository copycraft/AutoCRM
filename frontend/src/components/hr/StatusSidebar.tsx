'use client';

// The HR status lists down the side of the staff directory, as they were in MiniCRM:
// Távollévő, Iktatásra váró, Aktív, Megszűnt. Click one to list its people;
// "Szerkesztés" turns the lists into an editor (add, rename, recolour, reorder, archive).

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, ArrowDown, ArrowUp, Pencil, RotateCcw } from 'lucide-react';
import { hrApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { cn } from '@/lib/utils/format';
import { IconButton, PALETTE, SwatchButton } from '@/components/tags/TagKit';
import type { EmployeeStatus } from '@/lib/api/types';

export const statusesKey = (archived = false) => ['hr-statuses', archived] as const;

export function useStatuses() {
  return useQuery({ queryKey: statusesKey(), queryFn: () => hrApi.statuses() });
}

/** Sections in list order, each with its statuses. */
export function statusSections(items: EmployeeStatus[]): { section: string; items: EmployeeStatus[] }[] {
  const out: { section: string; items: EmployeeStatus[] }[] = [];
  for (const s of [...items].sort((a, b) => a.position - b.position || a.id - b.id)) {
    const group = out.find((g) => g.section.toLowerCase() === s.section.toLowerCase());
    if (group) group.items.push(s);
    else out.push({ section: s.section, items: [s] });
  }
  return out;
}

export function StatusSidebar({
  value,
  onChange,
}: {
  /** The selected status id; null lists everyone current. */
  value: number | null;
  onChange: (id: number | null) => void;
}) {
  const t = useTranslations('hr.statuses');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const statuses = useStatuses();
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newSection, setNewSection] = useState('');
  const [newLabel, setNewLabel] = useState('');
  const archived = useQuery({ queryKey: statusesKey(true), queryFn: () => hrApi.statuses(true), enabled: editing });

  const refresh = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['hr-statuses'] });
    void qc.invalidateQueries({ queryKey: ['employees'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const update = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof hrApi.updateStatus>[1] }) => hrApi.updateStatus(id, body),
    onSuccess: refresh,
    onError,
  });
  const create = useMutation({
    mutationFn: (body: { section: string; label: string }) => {
      const used = new Set((statuses.data?.items ?? []).filter((x) => x.section === body.section).map((x) => x.color));
      return hrApi.createStatus({ ...body, color: PALETTE.find((c) => !used.has(c)) ?? '#dde1e6' });
    },
    onSuccess: refresh,
    onError,
  });
  const reorder = useMutation({
    mutationFn: ({ section, ids }: { section: string; ids: number[] }) => hrApi.reorderStatuses(section, ids),
    onSuccess: refresh,
    onError,
  });

  const groups = statusSections(statuses.data?.items ?? []);
  const current = (statuses.data?.items ?? []).filter((s) => !s.ends_employment).reduce((n, s) => n + s.employees, 0);

  return (
    <nav className="space-y-4 text-body" aria-label={t('title')}>
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-section font-semibold">{t('title')}</h2>
        <button type="button" className={cn('btn-sm', editing ? 'btn-primary' : 'btn-ghost')} onClick={() => setEditing((e) => !e)}>
          <Pencil className="h-3.5 w-3.5" aria-hidden />
          {editing ? t('done') : t('edit')}
        </button>
      </div>
      {error && (
        <p className="rounded-lg bg-signal/10 px-3 py-2 text-metadata text-signal" role="alert">
          {error}
        </p>
      )}
      <button
        type="button"
        className={cn('w-full rounded px-1 py-0.5 text-left hover:bg-steel-200/40', value === null && 'bg-steel-200/60 font-semibold')}
        onClick={() => onChange(null)}
      >
        {t('everyone')} <span className="font-mono text-metadata text-steel-500">({current})</span>
      </button>

      {groups.map((g) => (
        <section key={g.section} className="border-t border-steel-200 pt-3">
          <h3 className="mb-1.5 font-semibold text-steel-900">{g.section}</h3>
          <ul className="space-y-0.5">
            {g.items.map((s, i) =>
              editing ? (
                <EditRow
                  key={s.id}
                  status={s}
                  first={i === 0}
                  last={i === g.items.length - 1}
                  onUpdate={(body) => update.mutate({ id: s.id, body })}
                  onMove={(by) => {
                    const ids = g.items.map((x) => x.id);
                    const [id] = ids.splice(i, 1);
                    if (id !== undefined) ids.splice(i + by, 0, id);
                    reorder.mutate({ section: g.section, ids });
                  }}
                />
              ) : (
                <li key={s.id} className="flex items-center gap-2">
                  <span className="h-4 w-4 shrink-0 rounded ring-1 ring-inset ring-black/10" style={{ backgroundColor: s.color }} />
                  <button
                    type="button"
                    className={cn('min-w-0 flex-1 truncate rounded px-1 py-0.5 text-left hover:bg-steel-200/40', value === s.id && 'bg-steel-200/60 font-semibold')}
                    title={s.label}
                    onClick={() => onChange(value === s.id ? null : s.id)}
                  >
                    {s.label} <span className="font-mono text-metadata text-steel-500">({s.employees})</span>
                  </button>
                </li>
              ),
            )}
          </ul>
          {editing && (
            <form
              className="mt-2"
              onSubmit={(e) => {
                e.preventDefault();
                const input = e.currentTarget.elements.namedItem('label') as HTMLInputElement;
                const label = input.value.trim();
                if (label) create.mutate({ section: g.section, label }, { onSuccess: () => (input.value = '') });
              }}
            >
              <input name="label" className="input h-8" placeholder={t('newStatus')} />
            </form>
          )}
        </section>
      ))}

      {editing && (
        <form
          className="space-y-2 border-t border-steel-200 pt-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (!newSection.trim() || !newLabel.trim()) return;
            create.mutate(
              { section: newSection.trim(), label: newLabel.trim() },
              { onSuccess: () => { setNewSection(''); setNewLabel(''); } },
            );
          }}
        >
          <h3 className="font-semibold">{t('newSection')}</h3>
          <input className="input h-8" placeholder={t('newSectionName')} value={newSection} onChange={(e) => setNewSection(e.target.value)} />
          <input className="input h-8" placeholder={t('newSectionFirst')} value={newLabel} onChange={(e) => setNewLabel(e.target.value)} />
          <button className="btn-secondary btn-sm" type="submit" disabled={!newSection.trim() || !newLabel.trim()}>
            {t('add')}
          </button>
        </form>
      )}

      {editing && (archived.data?.items.length ?? 0) > 0 && (
        <details className="border-t border-steel-200 pt-3">
          <summary className="cursor-pointer font-semibold">{t('archived', { count: archived.data?.items.length ?? 0 })}</summary>
          <ul className="mt-2 space-y-1">
            {archived.data?.items.map((s) => (
              <li key={s.id} className="flex items-center gap-2">
                <span className="h-3 w-3 rounded-sm" style={{ backgroundColor: s.color }} />
                <span className="min-w-0 flex-1 truncate">{s.section} · {s.label}</span>
                <IconButton label={t('unarchive')} onClick={() => update.mutate({ id: s.id, body: { archived: false } })}>
                  <RotateCcw className="h-3.5 w-3.5" aria-hidden />
                </IconButton>
              </li>
            ))}
          </ul>
        </details>
      )}
    </nav>
  );
}

function EditRow({
  status,
  first,
  last,
  onUpdate,
  onMove,
}: {
  status: EmployeeStatus;
  first: boolean;
  last: boolean;
  onUpdate: (body: Parameters<typeof hrApi.updateStatus>[1]) => void;
  onMove: (by: -1 | 1) => void;
}) {
  const t = useTranslations('hr.statuses');
  const [label, setLabel] = useState<string | null>(null);
  const save = () => {
    const next = label?.trim();
    setLabel(null);
    if (next && next !== status.label) onUpdate({ label: next });
  };
  // The start and end statuses cannot be archived (the server refuses too).
  const fixed = status.is_default || status.ends_employment;
  return (
    <li className="flex items-center gap-1.5">
      <SwatchButton color={status.color} editable label={t('color')} onPick={(c) => onUpdate({ color: c })} />
      {label !== null ? (
        <input
          autoFocus
          className="input h-7 min-w-0 flex-1 py-0"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          onBlur={save}
          onKeyDown={(e) => {
            if (e.key === 'Enter') save();
            if (e.key === 'Escape') setLabel(null);
          }}
        />
      ) : (
        <button type="button" className="min-w-0 flex-1 truncate text-left hover:underline" title={t('rename')} onClick={() => setLabel(status.label)}>
          {status.label}
        </button>
      )}
      <IconButton label={t('moveUp')} disabled={first} onClick={() => onMove(-1)}>
        <ArrowUp className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
      <IconButton label={t('moveDown')} disabled={last} onClick={() => onMove(1)}>
        <ArrowDown className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
      <IconButton label={fixed ? t('fixed') : t('archive')} disabled={fixed} onClick={() => onUpdate({ archived: true })}>
        <Archive className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
    </li>
  );
}

/** A select for moving one employee to another status, grouped by section. */
export function StatusSelect({
  employeeId,
  statusId,
}: {
  employeeId: number;
  statusId: number | null | undefined;
}) {
  const t = useTranslations('hr.statuses');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const statuses = useStatuses();
  const set = useMutation({
    mutationFn: (id: number) => hrApi.setStatus(employeeId, id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['employees'] });
      void qc.invalidateQueries({ queryKey: ['hr-statuses'] });
    },
  });
  const items = statuses.data?.items ?? [];
  const current = items.find((s) => s.id === statusId);
  return (
    <div className="flex items-center gap-2">
      <span className="h-3 w-3 shrink-0 rounded-sm ring-1 ring-inset ring-black/10" style={{ backgroundColor: current?.color ?? 'transparent' }} />
      <select
        className="input h-8 min-w-0 flex-1 py-0 text-metadata"
        aria-label={t('title')}
        value={statusId ?? ''}
        disabled={set.isPending || items.length === 0}
        onChange={(e) => set.mutate(Number(e.target.value))}
      >
        {statusId == null && <option value="">—</option>}
        {statusSections(items).map((g) => (
          <optgroup key={g.section} label={g.section}>
            {g.items.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </optgroup>
        ))}
      </select>
      {set.isError && (
        <span className="text-metadata text-signal" role="alert">
          {errorMessage(set.error, ter, ter('unknownError'))}
        </span>
      )}
    </div>
  );
}
