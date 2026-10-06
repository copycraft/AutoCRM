'use client';

// Marketing: the newsletter subscribers and the tag lists they are sorted into. Pick a
// tag on the left to list its subscribers; tick rows to tag or untag them together;
// paste a whole list in; and send a newsletter to exactly the tags on screen.

import { useMemo, useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Send, Trash2, Upload, UserPlus } from 'lucide-react';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { Pagination } from '@/components/ui/Pagination';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { ErrorState } from '@/components/ui/ErrorState';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useUrlFlag, useUrlInt, useUrlState } from '@/hooks/useUrlState';
import { newsletterApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { canSendEmail, useAuth } from '@/lib/auth/context';
import { TagSidebar, type Selection } from '@/components/marketing/TagSidebar';
import {
  NewsletterChips,
  NewsletterTagPicker,
  newsletterTagsKey,
  useNewsletterTags,
} from '@/components/marketing/NewsletterTags';
import { ExportMenu, collectAll } from '@/components/tables/ExportCsvButton';
import { SavedViewsBar } from '@/components/tables/SavedViewsBar';
import type { SubscriberRow } from '@/lib/api/types';

const PAGE = 50;
type Status = '' | 'active' | 'pending' | 'unsubscribed';

export default function MarketingPage() {
  const t = useTranslations('marketing');
  const tn = useTranslations('navigation');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  const editable = canSendEmail(user);
  const qc = useQueryClient();

  const [q, setQ] = useUrlState('q', '');
  const [status, setStatus] = useUrlState('status', '');
  const [tagRaw, setTagRaw] = useUrlState('tag', '');
  const [untagged, setUntagged] = useUrlFlag('untagged', false);
  const [page, setPage] = useUrlInt('page', 1);
  const debouncedQ = useDebouncedValue(q);
  const tag = Number(tagRaw) || null;
  const offset = Math.max(0, (page - 1) * PAGE);
  const selection: Selection = tag ? { kind: 'tag', id: tag } : untagged ? { kind: 'untagged' } : { kind: 'all' };

  const [panel, setPanel] = useState<'add' | 'import' | null>(null);
  const [ticked, setTicked] = useState<number[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<SubscriberRow | null>(null);

  const tags = useNewsletterTags();
  const archivedTags = useQuery({ queryKey: newsletterTagsKey(true), queryFn: () => newsletterApi.tags(true) });
  const allTags = useMemo(
    () => [...(tags.data?.items ?? []), ...(archivedTags.data?.items ?? [])],
    [tags.data, archivedTags.data],
  );
  const counts = useQuery({ queryKey: ['newsletter-subscribers', 'counts'], queryFn: () => newsletterApi.counts() });
  const list = useQuery({
    queryKey: ['newsletter-subscribers', { q: debouncedQ, status, tag, untagged, offset }],
    queryFn: () =>
      newsletterApi.subscribers({
        q: debouncedQ || undefined,
        status: (status as Status) || undefined,
        tag: tag ?? undefined,
        untagged: untagged || undefined,
        limit: PAGE,
        offset,
      }),
  });
  const rows = list.data?.items ?? [];

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['newsletter-subscribers'] });
    void qc.invalidateQueries({ queryKey: ['newsletter-tags'] });
    void qc.invalidateQueries({ queryKey: ['newsletter-audience'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));

  const setRowTags = useMutation({
    mutationFn: ({ id, ids }: { id: number; ids: number[] }) => newsletterApi.setTags(id, ids),
    onSuccess: refresh,
    onError,
  });
  const bulk = useMutation({
    mutationFn: (body: { add?: number[]; remove?: number[] }) =>
      newsletterApi.bulkTags({ subscription_ids: ticked, add: body.add ?? [], remove: body.remove ?? [] }),
    onSuccess: refresh,
    onError,
  });
  const remove = useMutation({
    mutationFn: (id: number) => newsletterApi.removeSubscription(id),
    onSuccess: () => {
      setRemoving(null);
      refresh();
    },
    onError,
  });

  const select = (s: Selection) => {
    setTagRaw(s.kind === 'tag' ? String(s.id) : '');
    setUntagged(s.kind === 'untagged');
    setPage(1);
    setTicked([]);
  };
  const selectedTag = tag ? allTags.find((x) => x.id === tag) : undefined;
  const allTicked = rows.length > 0 && rows.every((r) => ticked.includes(r.id));

  return (
    <AppShell>
      <PageHeader
        title={tn('marketing')}
        subtitle={counts.data ? t('summary', { active: counts.data.active, total: counts.data.total }) : undefined}
        actions={
          editable && (
            <>
              <button type="button" className="btn-secondary btn-sm" onClick={() => setPanel(panel === 'add' ? null : 'add')}>
                <UserPlus className="h-4 w-4" aria-hidden />
                {t('addSubscriber')}
              </button>
              <button type="button" className="btn-secondary btn-sm" onClick={() => setPanel(panel === 'import' ? null : 'import')}>
                <Upload className="h-4 w-4" aria-hidden />
                {t('import')}
              </button>
              <Link
                className="btn-primary btn-sm"
                href={`/${locale}/emails/new?audience=newsletter${tag ? `&tags=${tag}` : ''}`}
              >
                <Send className="h-4 w-4" aria-hidden />
                {selectedTag ? t('sendToTag', { tag: selectedTag.label }) : t('sendNewsletter')}
              </Link>
            </>
          )
        }
      />

      {error && (
        <p className="rounded-lg bg-signal/10 px-3 py-2 text-body text-signal" role="alert">
          {error}
        </p>
      )}
      <SavedViewsBar listKey="subscribers" />
      {panel === 'add' && <AddPanel defaultTags={tag ? [tag] : []} onDone={refresh} />}
      {panel === 'import' && <ImportPanel defaultTags={tag ? [tag] : []} onDone={refresh} />}

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[17rem_minmax(0,1fr)]">
        <aside className="card self-start p-4">
          <TagSidebar
            tags={tags.data?.items ?? []}
            counts={counts.data}
            selection={selection}
            onSelect={select}
            editable={editable}
          />
        </aside>

        <div className="min-w-0 space-y-3">
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-[14rem] flex-1">
              <label className="label" htmlFor="mk-q">{tc('search')}</label>
              <input
                id="mk-q"
                className="input"
                placeholder={t('searchPlaceholder')}
                value={q}
                onChange={(e) => {
                  setQ(e.target.value);
                  setPage(1);
                }}
              />
            </div>
            <div>
              <label className="label" htmlFor="mk-status">{t('status')}</label>
              <select
                id="mk-status"
                className="input"
                value={status}
                onChange={(e) => {
                  setStatus(e.target.value);
                  setPage(1);
                }}
              >
                <option value="">{tc('all')}</option>
                <option value="active">{t('statusActive')}</option>
                <option value="pending">{t('statusPending')}</option>
                <option value="unsubscribed">{t('statusUnsubscribed')}</option>
              </select>
            </div>
            <div className="ml-auto pb-1">
              <ExportMenu
                base="feliratkozok"
                onExport={async () => {
                  const all = await collectAll((offset, limit) =>
                    newsletterApi.subscribers({
                      q: debouncedQ || undefined,
                      status: (status as Status) || undefined,
                      tag: tag ?? undefined,
                      untagged: untagged || undefined,
                      limit,
                      offset,
                    }),
                  );
                  const tagName = (id: number) => allTags.find((x) => x.id === id)?.label ?? `#${id}`;
                  return {
                    header: [t('email'), t('name'), t('status'), t('tags'), t('export.source'), t('since'), t('export.confirmed'), t('export.unsubscribed')],
                    rows: all.map((r) => [
                      r.email,
                      r.name,
                      r.unsubscribed_at ? t('statusUnsubscribed') : !r.confirmed_at ? t('statusPending') : t('statusActive'),
                      r.tag_ids.map(tagName).join(', '),
                      r.source,
                      r.subscribed_at,
                      r.confirmed_at,
                      r.unsubscribed_at,
                    ]),
                    count: all.length,
                  };
                }}
              />
            </div>
          </div>

          {selectedTag && (
            <p className="text-body text-steel-500">
              {t('showingTag', { section: selectedTag.section, tag: selectedTag.label })}
            </p>
          )}

          {editable && ticked.length > 0 && (
            <div className="flex flex-wrap items-center gap-2 rounded-lg bg-steel-200/50 px-3 py-2" data-testid="bulk-bar">
              <span className="text-body font-medium">{t('ticked', { count: ticked.length })}</span>
              <NewsletterTagPicker label={t('bulkAdd')} value={[]} onChange={(ids) => bulk.mutate({ add: ids })} />
              <NewsletterTagPicker label={t('bulkRemove')} value={[]} onChange={(ids) => bulk.mutate({ remove: ids })} />
              <button type="button" className="btn-ghost btn-sm" onClick={() => setTicked([])}>
                {t('clearTicks')}
              </button>
            </div>
          )}

          {list.isError ? (
            <ErrorState error={list.error} onRetry={() => void list.refetch()} />
          ) : (
            <div className="card overflow-x-auto">
              <table className="w-full text-body">
                <thead className="border-b border-steel-200 text-left text-metadata text-steel-500">
                  <tr>
                    {editable && (
                      <th className="w-8 px-3 py-2">
                        <input
                          type="checkbox"
                          aria-label={t('tickAll')}
                          className="rounded border-steel-200 accent-steel-900"
                          checked={allTicked}
                          onChange={() =>
                            setTicked(allTicked ? ticked.filter((id) => !rows.some((r) => r.id === id))
                              : Array.from(new Set([...ticked, ...rows.map((r) => r.id)])))
                          }
                        />
                      </th>
                    )}
                    <th className="px-3 py-2">{t('subscriber')}</th>
                    <th className="px-3 py-2">{t('tags')}</th>
                    <th className="px-3 py-2">{t('status')}</th>
                    <th className="px-3 py-2 text-right">{t('since')}</th>
                    {editable && <th className="w-10 px-3 py-2" />}
                  </tr>
                </thead>
                <tbody>
                  {list.isPending && (
                    <tr>
                      <td colSpan={6} className="px-3 py-6 text-center text-steel-500">…</td>
                    </tr>
                  )}
                  {!list.isPending && rows.length === 0 && (
                    <tr>
                      <td colSpan={6} className="px-3 py-6 text-center text-steel-500">{t('empty')}</td>
                    </tr>
                  )}
                  {rows.map((r) => (
                    <tr key={r.id} className="border-b border-steel-200 last:border-b-0 align-top">
                      {editable && (
                        <td className="px-3 py-2">
                          <input
                            type="checkbox"
                            aria-label={r.email}
                            className="rounded border-steel-200 accent-steel-900"
                            checked={ticked.includes(r.id)}
                            onChange={() =>
                              setTicked(ticked.includes(r.id) ? ticked.filter((x) => x !== r.id) : [...ticked, r.id])
                            }
                          />
                        </td>
                      )}
                      <td className="px-3 py-2">
                        <div className="font-medium">{r.email}</div>
                        <div className="text-metadata text-steel-500">
                          {[r.name, r.source].filter(Boolean).join(' · ')}
                        </div>
                      </td>
                      <td className="px-3 py-2">
                        <div className="flex flex-wrap items-center gap-1">
                          <NewsletterChips
                            ids={r.tag_ids}
                            all={allTags}
                            onClick={(id) => select({ kind: 'tag', id })}
                            onRemove={editable ? (id) => setRowTags.mutate({ id: r.id, ids: r.tag_ids.filter((x) => x !== id) }) : undefined}
                          />
                          {editable && (
                            <NewsletterTagPicker
                              label="+"
                              value={r.tag_ids}
                              onChange={(ids) => setRowTags.mutate({ id: r.id, ids })}
                            />
                          )}
                        </div>
                      </td>
                      <td className="px-3 py-2">
                        {r.unsubscribed_at ? (
                          <StatusBadge tone="muted">{t('statusUnsubscribed')}</StatusBadge>
                        ) : !r.confirmed_at ? (
                          <StatusBadge tone="steel">{t('statusPending')}</StatusBadge>
                        ) : (
                          <StatusBadge tone="done">{t('statusActive')}</StatusBadge>
                        )}
                      </td>
                      <td className="px-3 py-2 text-right text-metadata">
                        <DateDisplay value={r.subscribed_at} />
                      </td>
                      {editable && (
                        <td className="px-3 py-2">
                          <button type="button" className="btn-ghost btn-sm" aria-label={tc('delete')} onClick={() => setRemoving(r)}>
                            <Trash2 className="h-4 w-4" aria-hidden />
                          </button>
                        </td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <Pagination
            offset={offset}
            limit={PAGE}
            loaded={rows.length}
            onPrev={() => setPage(Math.max(1, page - 1))}
            onNext={() => setPage(page + 1)}
            onJump={setPage}
          />
        </div>
      </div>

      <ConfirmDialog
        open={removing !== null}
        title={t('removeTitle')}
        body={t('removeBody', { email: removing?.email ?? '' })}
        confirmLabel={tc('delete')}
        onClose={() => setRemoving(null)}
        busy={remove.isPending}
        onConfirm={() => removing && remove.mutate(removing.id)}
      />
    </AppShell>
  );
}

function AddPanel({ defaultTags, onDone }: { defaultTags: number[]; onDone: () => void }) {
  const t = useTranslations('marketing');
  const ter = useTranslations('errors');
  const tags = useNewsletterTags();
  const [email, setEmail] = useState('');
  const [name, setName] = useState('');
  const [tagIds, setTagIds] = useState(defaultTags);
  const [error, setError] = useState<string | null>(null);
  const add = useMutation({
    mutationFn: () => newsletterApi.addSubscription({ email: email.trim(), name: name.trim(), tag_ids: tagIds }),
    onSuccess: () => {
      setEmail('');
      setName('');
      setError(null);
      onDone();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <section className="card p-4">
      <form
        className="flex flex-wrap items-end gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (email.trim()) add.mutate();
        }}
      >
        <div>
          <label className="label" htmlFor="mk-email">{t('email')}</label>
          <input id="mk-email" className="input" type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="olvaso@example.hu" />
        </div>
        <div>
          <label className="label" htmlFor="mk-name">{t('name')}</label>
          <input id="mk-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="flex flex-wrap items-center gap-1 pb-1">
          <NewsletterChips ids={tagIds} all={tags.data?.items ?? []} onRemove={(id) => setTagIds(tagIds.filter((x) => x !== id))} />
          <NewsletterTagPicker label={t('tags')} value={tagIds} onChange={setTagIds} />
        </div>
        <button className="btn-primary btn-sm" type="submit" disabled={!email.trim() || add.isPending}>
          {t('add')}
        </button>
      </form>
      <p className="mt-2 text-metadata text-steel-500">{t('consentNote')}</p>
      {error && <p className="mt-2 text-body text-signal" role="alert">{error}</p>}
    </section>
  );
}

function ImportPanel({ defaultTags, onDone }: { defaultTags: number[]; onDone: () => void }) {
  const t = useTranslations('marketing');
  const ter = useTranslations('errors');
  const tags = useNewsletterTags();
  const [text, setText] = useState('');
  const [tagIds, setTagIds] = useState(defaultTags);
  const [error, setError] = useState<string | null>(null);
  const run = useMutation({
    mutationFn: () => newsletterApi.import({ text, tag_ids: tagIds }),
    onSuccess: () => {
      setError(null);
      onDone();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  const lines = text.split('\n').filter((l) => l.trim()).length;
  return (
    <section className="card space-y-3 p-4">
      <label className="label" htmlFor="mk-import">{t('importLabel')}</label>
      <textarea
        id="mk-import"
        className="input h-40 font-mono text-metadata"
        placeholder={'info@pekseg.hu;Kovács Pékség\nrendeles@hus.hu\tHús Kft.'}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-metadata text-steel-500">{t('importTags')}</span>
        <NewsletterChips ids={tagIds} all={tags.data?.items ?? []} onRemove={(id) => setTagIds(tagIds.filter((x) => x !== id))} />
        <NewsletterTagPicker label={t('tags')} value={tagIds} onChange={setTagIds} />
        <button
          type="button"
          className="btn-primary btn-sm ml-auto"
          disabled={lines === 0 || run.isPending}
          onClick={() => run.mutate()}
        >
          {run.isPending ? '…' : t('importRun', { count: lines })}
        </button>
      </div>
      <p className="text-metadata text-steel-500">{t('consentNote')}</p>
      {error && <p className="text-body text-signal" role="alert">{error}</p>}
      {run.data && (
        <div className="rounded-lg bg-steel-200/50 px-3 py-2 text-body" role="status">
          {t('importResult', { added: run.data.added, existing: run.data.existing, optedOut: run.data.opted_out })}
          {run.data.invalid.length > 0 && (
            <p className="mt-1 text-metadata text-steel-500">
              {t('importInvalid')}: {run.data.invalid.join(' · ')}
            </p>
          )}
        </div>
      )}
    </section>
  );
}
