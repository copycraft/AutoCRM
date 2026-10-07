'use client';

// Staff comments on an order or a lead. Typing @ offers the people who can be mentioned;
// whoever is named gets a notification (web bell and phone) that opens this record.

import { useMemo, useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AtSign, Pencil, Trash2 } from 'lucide-react';
import { commentsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useAuth } from '@/lib/auth/context';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { cn } from '@/lib/utils/format';
import type { Comment, Mentionable } from '@/lib/api/types';

/** The ids of the people whose `@Name` is in the text. */
export function mentionedIds(body: string, people: Mentionable[]): number[] {
  return people.filter((p) => body.includes(`@${p.display_name}`)).map((p) => p.id);
}

/** The body with every known `@Name` set apart, for display. */
function Highlighted({ body, names }: { body: string; names: string[] }) {
  if (!names.length) return <>{body}</>;
  const escaped = names
    .slice()
    .sort((a, b) => b.length - a.length)
    .map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const parts = body.split(new RegExp(`(@(?:${escaped.join('|')}))`, 'g'));
  return (
    <>
      {parts.map((part, i) =>
        part.startsWith('@') && names.includes(part.slice(1)) ? (
          <span key={i} className="rounded bg-cold/10 px-0.5 font-medium text-cold">{part}</span>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}

function Composer({
  people,
  initial = '',
  busy,
  submitLabel,
  onSubmit,
  onCancel,
}: {
  people: Mentionable[];
  initial?: string;
  busy: boolean;
  submitLabel: string;
  onSubmit: (body: string, mentionIds: number[]) => void;
  onCancel?: () => void;
}) {
  const t = useTranslations('comments');
  const tc = useTranslations('common');
  const [body, setBody] = useState(initial);
  const [query, setQuery] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const area = useRef<HTMLTextAreaElement>(null);

  const matches = useMemo(() => {
    if (query === null) return [];
    const q = query.toLowerCase();
    return people.filter((p) => p.display_name.toLowerCase().includes(q)).slice(0, 6);
  }, [people, query]);

  const track = (value: string, caret: number) => {
    const before = value.slice(0, caret);
    const m = /(^|\s)@([^\s@]{0,30})$/.exec(before);
    setQuery(m ? (m[2] ?? '') : null);
    setActive(0);
  };

  const pick = (p: Mentionable) => {
    const el = area.current;
    if (!el) return;
    const caret = el.selectionStart;
    const before = body.slice(0, caret).replace(/@([^\s@]{0,30})$/, `@${p.display_name} `);
    const next = before + body.slice(caret);
    setBody(next);
    setQuery(null);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(before.length, before.length);
    });
  };

  const submit = () => {
    const text = body.trim();
    if (!text) return;
    onSubmit(text, mentionedIds(text, people));
  };

  return (
    <div className="relative space-y-2">
      <textarea
        ref={area}
        rows={3}
        className="input"
        value={body}
        placeholder={t('placeholder')}
        aria-label={t('placeholder')}
        onChange={(e) => {
          setBody(e.target.value);
          track(e.target.value, e.target.selectionStart);
        }}
        onKeyDown={(e) => {
          if (matches.length) {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive((a) => (a + 1) % matches.length);
              return;
            }
            if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive((a) => (a - 1 + matches.length) % matches.length);
              return;
            }
            if (e.key === 'Enter' || e.key === 'Tab') {
              e.preventDefault();
              const choice = matches[active];
              if (choice) pick(choice);
              return;
            }
            if (e.key === 'Escape') {
              setQuery(null);
              return;
            }
          }
          if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            submit();
          }
        }}
      />
      {matches.length > 0 && (
        <ul className="absolute left-2 top-full z-20 mt-1 w-64 overflow-hidden rounded-lg border border-steel-200 bg-surface shadow-card" role="listbox">
          {matches.map((p, i) => (
            <li key={p.id} role="option" aria-selected={i === active}>
              <button
                type="button"
                className={cn('flex w-full items-center gap-2 px-3 py-1.5 text-left text-body', i === active ? 'bg-panel' : 'hover:bg-panel')}
                onMouseDown={(e) => {
                  e.preventDefault();
                  pick(p);
                }}
              >
                <AtSign className="h-3 w-3 text-steel-500" aria-hidden /> {p.display_name}
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="flex items-center justify-between gap-2">
        <span className="text-metadata text-steel-500">{t('hint')}</span>
        <div className="flex gap-2">
          {onCancel && (
            <button type="button" className="btn-ghost btn-sm" onClick={onCancel} disabled={busy}>
              {tc('cancel')}
            </button>
          )}
          <button type="button" className="btn-primary btn-sm" onClick={submit} disabled={busy || !body.trim()}>
            {busy ? tc('saving') : submitLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

export function CommentThread({
  entity,
  id,
  canComment,
}: {
  entity: 'order' | 'lead';
  id: number;
  canComment: boolean;
}) {
  const t = useTranslations('comments');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { user } = useAuth();
  const [editing, setEditing] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const list = useQuery({ queryKey: qk.comments(entity, id), queryFn: () => commentsApi.list(entity, id) });
  const people = useQuery({ queryKey: qk.mentionable, queryFn: () => commentsApi.mentionable(), staleTime: 5 * 60_000 });
  const names = (people.data?.items ?? []).map((p) => p.display_name);

  const done = () => {
    setError(null);
    setEditing(null);
    void qc.invalidateQueries({ queryKey: qk.comments(entity, id) });
    void qc.invalidateQueries({ queryKey: entity === 'order' ? qk.order(id) : qk.lead(id) });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const create = useMutation({
    mutationFn: (v: { body: string; mention_ids: number[] }) =>
      commentsApi.create({ entity_type: entity, entity_id: id, ...v }),
    onSuccess: done,
    onError,
  });
  const update = useMutation({
    mutationFn: (v: { id: number; body: string; mention_ids: number[] }) =>
      commentsApi.update(v.id, { body: v.body, mention_ids: v.mention_ids }),
    onSuccess: done,
    onError,
  });
  const remove = useMutation({ mutationFn: (cid: number) => commentsApi.remove(cid), onSuccess: done, onError });

  const mine = (c: Comment) => c.created_by === user?.id || user?.role === 'admin';

  return (
    <div className="space-y-4" id="comments">
      {error && <p className="text-body text-signal" role="alert">{error}</p>}
      {list.isPending ? (
        <LoadingState />
      ) : list.isError ? (
        <ErrorState error={list.error} onRetry={() => void list.refetch()} />
      ) : list.data.items.length === 0 ? (
        <p className="text-body text-steel-500">{t('empty')}</p>
      ) : (
        <ul className="space-y-3">
          {list.data.items.map((c) => (
            <li key={c.id} className="rounded-lg border border-steel-200 p-3">
              <p className="flex flex-wrap items-center gap-2 text-metadata text-steel-500">
                <span className="font-semibold text-steel-900">{c.author_name}</span>
                <DateDisplay withTime value={c.created_at} />
                {c.edited_at && <span>({t('edited')})</span>}
                <span className="flex-1" />
                {canComment && mine(c) && editing !== c.id && (
                  <>
                    <button type="button" className="btn-ghost btn-sm" onClick={() => setEditing(c.id)} aria-label={tc('edit')}>
                      <Pencil className="h-3.5 w-3.5" aria-hidden />
                    </button>
                    <button
                      type="button"
                      className="btn-ghost btn-sm"
                      aria-label={tc('delete')}
                      onClick={() => {
                        if (window.confirm(t('deleteConfirm'))) remove.mutate(c.id);
                      }}
                    >
                      <Trash2 className="h-3.5 w-3.5" aria-hidden />
                    </button>
                  </>
                )}
              </p>
              {editing === c.id ? (
                <div className="mt-2">
                  <Composer
                    people={people.data?.items ?? []}
                    initial={c.body}
                    busy={update.isPending}
                    submitLabel={tc('save')}
                    onSubmit={(body, mention_ids) => update.mutate({ id: c.id, body, mention_ids })}
                    onCancel={() => setEditing(null)}
                  />
                </div>
              ) : (
                <p className="mt-1 whitespace-pre-wrap text-body">
                  <Highlighted body={c.body} names={names} />
                </p>
              )}
            </li>
          ))}
        </ul>
      )}
      {canComment && (
        <Composer
          key={list.data?.items.length ?? 0}
          people={people.data?.items ?? []}
          busy={create.isPending}
          submitLabel={t('send')}
          onSubmit={(body, mention_ids) => create.mutate({ body, mention_ids })}
        />
      )}
    </div>
  );
}
