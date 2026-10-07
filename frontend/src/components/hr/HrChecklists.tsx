'use client';

// Joining and leaving (0049): the paper behind an absence (a sick note), the steps HR goes
// through for every starter and leaver, starting them for one person (each step becomes a
// task on the employee), and the CRM account a person signs in with.

import { useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { FileUp, Paperclip, Plus, Trash2 } from 'lucide-react';
import { hrExtrasApi, tasksApi, usersApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { canAdmin, useAuth } from '@/lib/auth/context';
import { useToast } from '@/components/ui/Toasts';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { Absence, Employee } from '@/lib/api/types';

/** Open or attach the sick note of one absence. */
export function AbsencePaper({ absence }: { absence: Absence }) {
  const t = useTranslations('hrChecklists');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const input = useRef<HTMLInputElement>(null);
  const upload = useMutation({
    mutationFn: (file: File) => hrExtrasApi.uploadAbsenceFile(absence.id, file),
    onSuccess: () => {
      toast.success(t('paperSaved'));
      void qc.invalidateQueries({ queryKey: ['hr'] });
      void qc.invalidateQueries({ queryKey: ['absences'] });
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });
  const open = useMutation({
    mutationFn: () => hrExtrasApi.absenceFileUrl(absence.id),
    onSuccess: (r) => window.open(r.url, '_blank', 'noopener'),
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <span className="inline-flex items-center gap-1">
      {absence.file_name && (
        <button type="button" className="btn-ghost btn-sm" title={absence.file_name} onClick={() => open.mutate()}>
          <Paperclip className="h-4 w-4" aria-hidden />
          <span className="max-w-[10rem] truncate">{absence.file_name}</span>
        </button>
      )}
      <button
        type="button"
        className="btn-ghost btn-sm"
        disabled={upload.isPending}
        title={absence.kind === 'sick' ? t('attachSickNote') : t('attachPaper')}
        aria-label={absence.kind === 'sick' ? t('attachSickNote') : t('attachPaper')}
        onClick={() => input.current?.click()}
      >
        <FileUp className="h-4 w-4" aria-hidden />
        {!absence.file_name && absence.kind === 'sick' && <span>{t('sickNote')}</span>}
      </button>
      <input
        ref={input}
        type="file"
        accept="application/pdf,image/jpeg,image/png,image/webp"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) upload.mutate(f);
          e.target.value = '';
        }}
      />
    </span>
  );
}

/** The two step lists, editable. */
export function ChecklistSettings() {
  const t = useTranslations('hrChecklists');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const items = useQuery({ queryKey: ['hr', 'checklist-items'], queryFn: () => hrExtrasApi.checklistItems() });
  const refresh = () => void qc.invalidateQueries({ queryKey: ['hr', 'checklist-items'] });
  const onError = (e: unknown) => toast.error(errorMessage(e, ter, ter('unknownError')));
  const [drafts, setDrafts] = useState<Record<'onboarding' | 'offboarding', { title: string; due: string }>>({
    onboarding: { title: '', due: '0' },
    offboarding: { title: '', due: '0' },
  });
  const create = useMutation({
    mutationFn: (kind: 'onboarding' | 'offboarding') =>
      hrExtrasApi.createChecklistItem({ kind, title: drafts[kind].title, due_days: Number(drafts[kind].due) || 0 }),
    onSuccess: (_, kind) => {
      setDrafts((d) => ({ ...d, [kind]: { title: '', due: '0' } }));
      refresh();
    },
    onError,
  });
  const update = useMutation({
    mutationFn: ({ id, title, due }: { id: number; title: string; due: number }) =>
      hrExtrasApi.updateChecklistItem(id, { title, due_days: due }),
    onSuccess: refresh,
    onError,
  });
  const archive = useMutation({ mutationFn: (id: number) => hrExtrasApi.archiveChecklistItem(id), onSuccess: refresh, onError });

  if (items.isLoading) return <LoadingState />;
  if (items.isError) return <ErrorState error={items.error} onRetry={() => void items.refetch()} />;
  return (
    <div className="mt-6 grid gap-6 lg:grid-cols-2">
      {(['onboarding', 'offboarding'] as const).map((kind) => (
        <section key={kind} className="card">
          <div className="card-header">
            <h2 className="text-section font-semibold">{t(`kinds.${kind}`)}</h2>
            <p className="text-metadata text-steel-500">{t(`intro.${kind}`)}</p>
          </div>
          <ul className="divide-y divide-steel-200">
            {(items.data?.items ?? [])
              .filter((i) => i.kind === kind)
              .map((i) => (
                <li key={i.id} className="flex items-center gap-2 px-4 py-2">
                  <input
                    className="input h-8 min-w-0 flex-1 py-0"
                    defaultValue={i.title}
                    aria-label={t('stepTitle')}
                    onBlur={(e) => {
                      const title = e.target.value.trim();
                      if (title && title !== i.title) update.mutate({ id: i.id, title, due: i.due_days });
                    }}
                  />
                  <input
                    className="input h-8 w-20 py-0 text-right"
                    type="number"
                    min={0}
                    max={365}
                    defaultValue={i.due_days}
                    aria-label={t('dueDays')}
                    title={t('dueDays')}
                    onBlur={(e) => {
                      const due = Number(e.target.value);
                      if (Number.isFinite(due) && due !== i.due_days) update.mutate({ id: i.id, title: i.title, due });
                    }}
                  />
                  <span className="text-metadata text-steel-500">{t('days')}</span>
                  <button className="btn-ghost btn-sm" aria-label={t('removeStep')} onClick={() => archive.mutate(i.id)}>
                    <Trash2 className="h-4 w-4" aria-hidden />
                  </button>
                </li>
              ))}
          </ul>
          <div className="card-footer flex gap-2">
            <input
              className="input h-8 min-w-0 flex-1 py-0"
              placeholder={t('newStep')}
              value={drafts[kind].title}
              onChange={(e) => setDrafts((d) => ({ ...d, [kind]: { ...d[kind], title: e.target.value } }))}
            />
            <input
              className="input h-8 w-20 py-0 text-right"
              type="number"
              min={0}
              max={365}
              aria-label={t('dueDays')}
              value={drafts[kind].due}
              onChange={(e) => setDrafts((d) => ({ ...d, [kind]: { ...d[kind], due: e.target.value } }))}
            />
            <button
              className="btn-secondary btn-sm"
              disabled={!drafts[kind].title.trim() || create.isPending}
              onClick={() => create.mutate(kind)}
            >
              <Plus className="h-4 w-4" aria-hidden />
              {t('add')}
            </button>
          </div>
        </section>
      ))}
    </div>
  );
}

/** On an employee: the CRM account, starting a checklist, and the open steps. */
export function EmployeeOnboarding({ employee }: { employee: Employee }) {
  const t = useTranslations('hrChecklists');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const { user } = useAuth();
  const admin = canAdmin(user);
  const users = useQuery({ queryKey: ['users'], queryFn: () => usersApi.list(), enabled: admin });
  const tasks = useQuery({
    queryKey: ['tasks', 'employee', employee.id],
    queryFn: () => tasksApi.forEntity('employee', employee.id),
  });
  const [kind, setKind] = useState<'onboarding' | 'offboarding'>(employee.archived_at ? 'offboarding' : 'onboarding');
  const [start, setStart] = useState('');
  const [deactivate, setDeactivate] = useState(true);
  const onError = (e: unknown) => toast.error(errorMessage(e, ter, ter('unknownError')));
  const link = useMutation({
    mutationFn: (userId: number | null) => hrExtrasApi.setUser(employee.id, userId),
    onSuccess: () => {
      toast.success(t('linked'));
      void qc.invalidateQueries({ queryKey: ['hr'] });
    },
    onError,
  });
  const begin = useMutation({
    mutationFn: () =>
      hrExtrasApi.startChecklist(employee.id, {
        kind,
        start_date: start || null,
        deactivate_user: kind === 'offboarding' && deactivate && admin && employee.user_id != null,
      }),
    onSuccess: (r) => {
      toast.success(
        r.user_deactivated ? t('startedDeactivated', { count: r.tasks_created }) : t('started', { count: r.tasks_created }),
      );
      void qc.invalidateQueries({ queryKey: ['tasks'] });
      void qc.invalidateQueries({ queryKey: ['users'] });
    },
    onError,
  });
  const done = useMutation({
    mutationFn: ({ id, value }: { id: number; value: boolean }) => tasksApi.setDone(id, value),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['tasks', 'employee', employee.id] }),
    onError,
  });
  const linkedUser = (users.data?.items ?? []).find((u) => u.id === employee.user_id);

  return (
    <section className="space-y-3 border-t border-steel-200 pt-4">
      <h3 className="text-section font-semibold">{t('title')}</h3>
      {admin && (
        <div className="flex flex-wrap items-center gap-2">
          <label className="text-metadata text-steel-500" htmlFor={`emp-user-${employee.id}`}>{t('account')}</label>
          <select
            id={`emp-user-${employee.id}`}
            className="input h-8 w-auto py-0"
            value={employee.user_id ?? ''}
            disabled={link.isPending}
            onChange={(e) => link.mutate(e.target.value ? Number(e.target.value) : null)}
          >
            <option value="">{t('noAccount')}</option>
            {(users.data?.items ?? []).map((u) => (
              <option key={u.id} value={u.id}>
                {u.display_name} ({u.email}){u.is_active ? '' : ` – ${t('inactive')}`}
              </option>
            ))}
          </select>
          {linkedUser && !linkedUser.is_active && <span className="badge-muted">{t('inactive')}</span>}
        </div>
      )}
      <div className="flex flex-wrap items-end gap-2">
        <div>
          <label className="label" htmlFor={`cl-kind-${employee.id}`}>{t('checklist')}</label>
          <select
            id={`cl-kind-${employee.id}`}
            className="input h-8 w-auto py-0"
            value={kind}
            onChange={(e) => setKind(e.target.value as typeof kind)}
          >
            <option value="onboarding">{t('kinds.onboarding')}</option>
            <option value="offboarding">{t('kinds.offboarding')}</option>
          </select>
        </div>
        <div>
          <label className="label" htmlFor={`cl-start-${employee.id}`}>
            {kind === 'onboarding' ? t('firstDay') : t('lastDay')}
          </label>
          <input
            id={`cl-start-${employee.id}`}
            type="date"
            className="input h-8 w-auto py-0"
            value={start}
            onChange={(e) => setStart(e.target.value)}
          />
        </div>
        {kind === 'offboarding' && admin && employee.user_id != null && (
          <label className="flex items-center gap-2 pb-1 text-body">
            <input type="checkbox" checked={deactivate} onChange={(e) => setDeactivate(e.target.checked)} />
            {t('deactivate')}
          </label>
        )}
        <button className="btn-secondary btn-sm" disabled={begin.isPending} onClick={() => begin.mutate()}>
          {t('start')}
        </button>
      </div>
      {(tasks.data?.items ?? []).length > 0 && (
        <ul className="space-y-1">
          {(tasks.data?.items ?? []).map((task) => (
            <li key={task.id} className="flex items-center gap-2 text-body">
              <input
                type="checkbox"
                checked={task.done_at != null}
                aria-label={task.title}
                onChange={(e) => done.mutate({ id: task.id, value: e.target.checked })}
              />
              <span className={task.done_at ? 'text-steel-400 line-through' : undefined}>{task.title}</span>
              {task.due_date && (
                <span className="text-metadata text-steel-500">
                  <DateDisplay value={task.due_date} />
                </span>
              )}
              {task.assigned_name && <span className="text-metadata text-steel-400">{task.assigned_name}</span>}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
