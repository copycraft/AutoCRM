'use client';

import { useMemo, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import * as Dialog from '@radix-ui/react-dialog';
import { addMonths, endOfMonth, format, getDaysInMonth, startOfMonth } from 'date-fns';
import { ChevronLeft, ChevronRight, Plus, Trash2 } from 'lucide-react';
import { hrApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { cn } from '@/lib/utils/format';
import { useRestoreFocus } from '@/hooks/useRestoreFocus';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { useToast } from '@/components/ui/Toasts';
import type { Absence, LeaveKind } from '@/lib/api/types';

const KINDS: LeaveKind[] = ['annual', 'sick', 'unpaid', 'other'];
const ISO = 'yyyy-MM-dd';

const KIND_TONE: Record<LeaveKind, StatusTone> = {
  annual: 'cold',
  sick: 'signal',
  unpaid: 'steel',
  other: 'muted',
};
const KIND_CELL: Record<LeaveKind, string> = {
  annual: 'bg-cold',
  sick: 'bg-signal',
  unpaid: 'bg-steel-500',
  other: 'bg-steel-200',
};

/** The team's leave: a month grid, the month's absences, and everyone's balance. HR-only. */
export function LeaveSection() {
  const t = useTranslations('leave');
  const tc = useTranslations('common');
  const [month, setMonth] = useState(() => startOfMonth(new Date()));
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<Absence | null>(null);
  const qc = useQueryClient();
  const toast = useToast();
  const ter = useTranslations('errors');

  const from = format(month, ISO);
  const to = format(endOfMonth(month), ISO);
  const year = month.getFullYear();

  const staff = useQuery({ queryKey: qk.employees({ active: true }), queryFn: () => hrApi.list() });
  const absences = useQuery({
    queryKey: qk.absences({ from, to }),
    queryFn: () => hrApi.absences({ from, to }),
  });
  const summary = useQuery({ queryKey: qk.leaveSummary(year), queryFn: () => hrApi.leaveSummary(year) });

  const remove = useMutation({
    mutationFn: (id: number) => hrApi.deleteAbsence(id),
    onSuccess: () => {
      toast.success(t('deleted'));
      setRemoving(null);
      void qc.invalidateQueries({ queryKey: ['absences'] });
      void qc.invalidateQueries({ queryKey: ['leave-summary'] });
    },
    onError: (e) => {
      setRemoving(null);
      toast.error(errorMessage(e, ter, ter('unknownError')));
    },
  });

  const days = getDaysInMonth(month);
  const dayList = useMemo(() => Array.from({ length: days }, (_, i) => i + 1), [days]);
  const isWeekend = (day: number) => {
    const w = new Date(month.getFullYear(), month.getMonth(), day).getDay();
    return w === 0 || w === 6;
  };
  const coverage = (employeeId: number, day: number): Absence | undefined => {
    const iso = format(new Date(month.getFullYear(), month.getMonth(), day), ISO);
    return absences.data?.items.find(
      (a) => a.employee_id === employeeId && a.start_date <= iso && iso <= a.end_date,
    );
  };

  const people = staff.data?.items ?? [];
  const list = absences.data?.items ?? [];
  const rows = summary.data?.items ?? [];

  const error = staff.error ?? absences.error ?? summary.error;
  if (error) return <ErrorState error={error} onRetry={() => { void staff.refetch(); void absences.refetch(); void summary.refetch(); }} />;
  if (staff.isLoading || absences.isLoading) return <LoadingState label={tc('loading')} />;

  return (
    <div className="mt-6 space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex items-center gap-1">
          <button className="btn-ghost btn-sm" aria-label={t('prevMonth')} onClick={() => setMonth((m) => addMonths(m, -1))}>
            <ChevronLeft className="h-4 w-4" aria-hidden />
          </button>
          <h2 className="min-w-40 text-center text-section font-semibold">{format(month, 'yyyy. MM.')}</h2>
          <button className="btn-ghost btn-sm" aria-label={t('nextMonth')} onClick={() => setMonth((m) => addMonths(m, 1))}>
            <ChevronRight className="h-4 w-4" aria-hidden />
          </button>
          <button className="btn-ghost btn-sm" onClick={() => setMonth(startOfMonth(new Date()))}>
            {t('today')}
          </button>
        </div>
        <button className="btn-primary ml-auto" onClick={() => setAdding(true)} disabled={people.length === 0}>
          <Plus className="h-4 w-4" aria-hidden />
          {t('newAbsence')}
        </button>
      </div>

      {people.length === 0 ? (
        <EmptyState title={t('noEmployees')} />
      ) : (
        <div className="card overflow-x-auto">
          <table className="w-full border-collapse text-metadata" aria-label={t('calendar')}>
            <thead>
              <tr>
                <th className="sticky left-0 min-w-40 bg-surface px-3 py-2 text-left font-medium text-steel-500">
                  {t('employee')}
                </th>
                {dayList.map((d) => (
                  <th
                    key={d}
                    className={cn('w-7 px-0 py-2 text-center font-normal', isWeekend(d) ? 'bg-panel text-steel-500' : 'text-steel-900')}
                  >
                    {d}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {people.map((p) => (
                <tr key={p.id} className="border-t border-steel-200">
                  <td className="sticky left-0 truncate bg-surface px-3 py-1.5 text-body">{p.full_name}</td>
                  {dayList.map((d) => {
                    const a = coverage(p.id, d);
                    return (
                      <td key={d} className={cn('h-8 border-l border-steel-200/60 p-0', isWeekend(d) && 'bg-panel')}>
                        {a && (
                          <div
                            className={cn('mx-px h-6 rounded-sm', KIND_CELL[a.kind])}
                            title={`${p.full_name}: ${t(`kinds.${a.kind}`)}, ${a.start_date} – ${a.end_date}`}
                          />
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="flex flex-wrap gap-3 text-metadata text-steel-500" aria-hidden>
        {KINDS.map((k) => (
          <span key={k} className="flex items-center gap-1.5">
            <span className={cn('inline-block h-3 w-3 rounded-sm', KIND_CELL[k])} />
            {t(`kinds.${k}`)}
          </span>
        ))}
      </div>

      <section className="space-y-2">
        <h2 className="text-section font-semibold">{t('title')}</h2>
        {list.length === 0 ? (
          <p className="text-body text-steel-500">{t('none')}</p>
        ) : (
          <ul className="card divide-y divide-steel-200">
            {list.map((a) => (
              <li key={a.id} className="flex flex-wrap items-center gap-3 px-4 py-2">
                <span className="min-w-40 font-medium">{a.employee_name}</span>
                <StatusBadge tone={KIND_TONE[a.kind]}>{t(`kinds.${a.kind}`)}</StatusBadge>
                <span className="font-mono text-metadata">
                  {a.start_date} – {a.end_date}
                </span>
                <span className="text-metadata text-steel-500">
                  {a.working_days} {t('days').toLowerCase()}
                </span>
                {a.note && <span className="truncate text-metadata text-steel-500">{a.note}</span>}
                <button
                  className="btn-ghost btn-sm ml-auto"
                  aria-label={`${t('delete')}: ${a.employee_name}`}
                  onClick={() => setRemoving(a)}
                >
                  <Trash2 className="h-4 w-4" aria-hidden />
                </button>
              </li>
            ))}
          </ul>
        )}
        <p className="text-metadata text-steel-500">{t('holidayHint')}</p>
      </section>

      <section className="space-y-2">
        <h2 className="text-section font-semibold">
          {t('balances')} {year}
        </h2>
        <div className="card overflow-x-auto">
          <table className="w-full text-body">
            <thead>
              <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                <th className="px-4 py-2 font-medium">{t('employee')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('allowance')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('used')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('remaining')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('sick')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('unpaid')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('other')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.employee_id} className="border-b border-steel-200 last:border-0">
                  <td className="px-4 py-2">{r.full_name}</td>
                  <td className="px-4 py-2 text-right font-mono">{r.allowance_days}</td>
                  <td className="px-4 py-2 text-right font-mono">{r.used_annual}</td>
                  <td className={cn('px-4 py-2 text-right font-mono font-semibold', r.remaining < 0 && 'text-signal')}>
                    {r.remaining}
                  </td>
                  <td className="px-4 py-2 text-right font-mono">{r.sick_days}</td>
                  <td className="px-4 py-2 text-right font-mono">{r.unpaid_days}</td>
                  <td className="px-4 py-2 text-right font-mono">{r.other_days}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {adding && <AbsenceDialog people={people} defaultDate={from} onClose={() => setAdding(false)} />}
      <ConfirmDialog
        open={removing !== null}
        title={t('deleteConfirmTitle')}
        body={t('deleteConfirmBody')}
        onConfirm={() => removing && remove.mutate(removing.id)}
        onClose={() => setRemoving(null)}
        busy={remove.isPending}
      />
    </div>
  );
}

function AbsenceDialog({
  people,
  defaultDate,
  onClose,
}: {
  people: { id: number; full_name: string }[];
  defaultDate: string;
  onClose: () => void;
}) {
  const t = useTranslations('leave');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  useRestoreFocus(true);
  const [employeeId, setEmployeeId] = useState(people[0]?.id ?? 0);
  const [kind, setKind] = useState<LeaveKind>('annual');
  const [start, setStart] = useState(defaultDate);
  const [end, setEnd] = useState(defaultDate);
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: () =>
      hrApi.createAbsence(employeeId, {
        kind,
        start_date: start,
        end_date: end,
        note: note.trim() || null,
      }),
    onSuccess: () => {
      toast.success(t('saved'));
      void qc.invalidateQueries({ queryKey: ['absences'] });
      void qc.invalidateQueries({ queryKey: ['leave-summary'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const valid = employeeId > 0 && start !== '' && end !== '' && end >= start;

  return (
    <Dialog.Root open onOpenChange={(open) => !open && !save.isPending && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[92vw] max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (valid && !save.isPending) save.mutate();
            }}
          >
            <div className="card-header">
              <Dialog.Title className="text-section font-semibold">{t('newAbsence')}</Dialog.Title>
              <Dialog.Description className="sr-only">{t('holidayHint')}</Dialog.Description>
            </div>
            <div className="card-content space-y-4">
              {error && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                  {error}
                </p>
              )}
              <div>
                <label className="label" htmlFor="ab-emp">
                  {t('employee')}
                </label>
                <select id="ab-emp" className="input" value={employeeId} onChange={(e) => setEmployeeId(Number(e.target.value))}>
                  {people.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.full_name}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label" htmlFor="ab-kind">
                  {t('kind')}
                </label>
                <select id="ab-kind" className="input" value={kind} onChange={(e) => setKind(e.target.value as LeaveKind)}>
                  {KINDS.map((k) => (
                    <option key={k} value={k}>
                      {t(`kinds.${k}`)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="label" htmlFor="ab-from">
                    {t('from')}
                  </label>
                  <input
                    id="ab-from"
                    type="date"
                    className="input"
                    value={start}
                    onChange={(e) => {
                      setStart(e.target.value);
                      if (end < e.target.value) setEnd(e.target.value);
                    }}
                  />
                </div>
                <div>
                  <label className="label" htmlFor="ab-to">
                    {t('to')}
                  </label>
                  <input id="ab-to" type="date" className="input" min={start} value={end} onChange={(e) => setEnd(e.target.value)} />
                </div>
              </div>
              <div>
                <label className="label" htmlFor="ab-note">
                  {t('note')}
                </label>
                <input id="ab-note" className="input" maxLength={500} value={note} onChange={(e) => setNote(e.target.value)} />
              </div>
            </div>
            <div className="card-footer justify-end">
              <button type="button" className="btn-ghost" onClick={onClose} disabled={save.isPending}>
                {tc('cancel')}
              </button>
              <button type="submit" className="btn-primary" disabled={!valid || save.isPending}>
                {save.isPending ? tc('saving') : tc('save')}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
