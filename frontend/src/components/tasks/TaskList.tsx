'use client';

// Follow-up tasks pinned to one record. Checkbox toggles done, small form adds.
// Optimistic: toggle/add/delete apply instantly and roll back on error, with a
// toast either way. Used on order, lead and partner details with the same
// shape; the dashboard widget reads the caller's open tasks instead.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { tasksApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { DateQuickPicks } from '@/components/forms/DateQuickPicks';
import { useToast } from '@/components/ui/Toasts';

export type TaskEntity = 'order' | 'lead' | 'partner';

interface TaskRow {
  id: number;
  title: string;
  due_date: string | null;
  done_at: string | null;
  [k: string]: unknown;
}

export function TaskList({ entity, id }: { entity: TaskEntity; id: number }) {
  const t = useTranslations('tasks');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const tq = useTranslations('qol');
  const toast = useToast();
  const qc = useQueryClient();
  const [title, setTitle] = useState('');
  const [due, setDue] = useState('');
  const [error, setError] = useState<string | null>(null);

  const list = useQuery({
    queryKey: qk.tasksFor(entity, id),
    queryFn: () => tasksApi.forEntity(entity, id),
  });

  const touch = () => {
    void qc.invalidateQueries({ queryKey: qk.tasksFor(entity, id) });
    void qc.invalidateQueries({ queryKey: qk.tasksMine });
  };

  const fail = (e: unknown) => {
    const msg = errorMessage(e, ter, ter('unknownError'));
    setError(msg);
    toast.error(tq('toastError'), msg);
  };

  const add = useMutation({
    mutationFn: () =>
      tasksApi.create({
        entity_type: entity,
        entity_id: id,
        title: title.trim(),
        due_date: due || null,
        assigned_to: null,
      }),
    onMutate: async () => {
      const draft = title.trim();
      const draftDue = due || null;
      setTitle('');
      setDue('');
      await qc.cancelQueries({ queryKey: qk.tasksFor(entity, id) });
      const prev = qc.getQueryData<{ items: TaskRow[] }>(qk.tasksFor(entity, id));
      if (prev) {
        const temp: TaskRow = {
          id: -Date.now(),
          title: draft,
          due_date: draftDue,
          done_at: null,
        };
        qc.setQueryData(qk.tasksFor(entity, id), { ...prev, items: [...prev.items, temp] });
      }
      return { prev };
    },
    onSuccess: () => {
      setError(null);
      toast.success(tq('taskAdded'));
      touch();
    },
    onError: (e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(qk.tasksFor(entity, id), ctx.prev);
      fail(e);
    },
  });

  const toggle = useMutation({
    mutationFn: ({ taskId, done }: { taskId: number; done: boolean }) =>
      tasksApi.setDone(taskId, done),
    onMutate: async ({ taskId, done }) => {
      await qc.cancelQueries({ queryKey: qk.tasksFor(entity, id) });
      const prev = qc.getQueryData<{ items: TaskRow[] }>(qk.tasksFor(entity, id));
      if (prev) {
        qc.setQueryData(qk.tasksFor(entity, id), {
          ...prev,
          items: prev.items.map((x) =>
            x.id === taskId ? { ...x, done_at: done ? new Date().toISOString() : null } : x,
          ),
        });
      }
      return { prev, done };
    },
    onSuccess: (_d, v) => {
      setError(null);
      toast.success(v.done ? tq('taskDone') : tq('taskUndone'));
      touch();
    },
    onError: (e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(qk.tasksFor(entity, id), ctx.prev);
      fail(e);
    },
  });

  const remove = useMutation({
    mutationFn: (taskId: number) => tasksApi.remove(taskId),
    onMutate: async (taskId) => {
      await qc.cancelQueries({ queryKey: qk.tasksFor(entity, id) });
      const prev = qc.getQueryData<{ items: TaskRow[] }>(qk.tasksFor(entity, id));
      const deleted = prev?.items.find((x) => x.id === taskId) ?? null;
      if (prev) {
        qc.setQueryData(qk.tasksFor(entity, id), {
          ...prev,
          items: prev.items.filter((x) => x.id !== taskId),
        });
      }
      return { prev, deleted };
    },
    onSuccess: (_d, _taskId, ctx) => {
      setError(null);
      const deleted = ctx?.deleted;
      toast.success(tq('taskDeleted'), undefined, deleted ? {
        label: tq('undo'),
        onClick: () => {
          void tasksApi
            .create({
              entity_type: entity,
              entity_id: id,
              title: deleted.title,
              due_date: deleted.due_date,
              assigned_to: null,
            })
            .then(() => {
              toast.success(tq('taskRestored'));
              touch();
            })
            .catch((e: unknown) => fail(e));
        },
      } : undefined);
      touch();
    },
    onError: (e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(qk.tasksFor(entity, id), ctx.prev);
      fail(e);
    },
  });

  const items = list.data?.items ?? [];
  const open = items.filter((x) => !x.done_at);

  return (
    <div className="space-y-2">
      {list.isLoading ? (
        <p className="text-metadata text-steel-500">{tc('loading')}</p>
      ) : (
        <ul className="space-y-1">
          {items.map((x) => (
            <li key={x.id} className="flex items-center gap-2 text-body">
              <input
                type="checkbox"
                checked={!!x.done_at}
                disabled={toggle.isPending}
                onChange={() => toggle.mutate({ taskId: x.id, done: !x.done_at })}
                aria-label={x.title}
                className="h-4 w-4 accent-steel-900"
              />
              <span className={x.done_at ? 'text-steel-500 line-through' : ''}>{x.title}</span>
              {x.due_date && (
                <span className="text-metadata text-steel-500">
                  <DateDisplay value={x.due_date} />
                </span>
              )}
              {!x.done_at && (
                <button
                  className="ml-auto text-metadata text-steel-500 underline"
                  onClick={() => remove.mutate(x.id)}
                >
                  {t('delete')}
                </button>
              )}
            </li>
          ))}
          {items.length === 0 && (
            <li className="text-metadata text-steel-500">{t('empty')}</li>
          )}
        </ul>
      )}
      <div className="flex flex-wrap gap-2">
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && title.trim() && !add.isPending) add.mutate();
          }}
          placeholder={t('newPlaceholder')}
          aria-label={t('newPlaceholder')}
          className="input flex-1 min-w-40"
        />
        <input
          type="date"
          value={due}
          onChange={(e) => setDue(e.target.value)}
          aria-label={t('dueDate')}
          className="input w-auto"
        />
        <DateQuickPicks onPick={setDue} />
        <button
          className="btn-secondary btn-sm"
          disabled={!title.trim() || add.isPending}
          onClick={() => add.mutate()}
        >
          {add.isPending ? tc('saving') : t('add')}
        </button>
      </div>
      {open.length === 0 && items.length > 0 && (
        <p className="text-metadata text-steel-500">{t('allDone')}</p>
      )}
      {error && (
        <p className="text-body text-steel-900" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
