'use client';

// Műszaki tartalom: the order's technical content, one part per department, each in the
// department's colour. Everyone reads every part; the office writes all of them, a
// department's members write their own (the backend says which, per part, in `can_edit`).
//
// "Kiadás a részlegnek" hands a part to its department: the heads get a notification and a
// letter. Written to again after that, the part says so and offers to send the change; a
// plain save never notifies anyone. Each part prints on its own sheet in one click.

import { useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { mediaApi, modulesApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { canEditOrders, useAuth } from '@/lib/auth/context';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { useToast } from '@/components/ui/Toasts';
import type { Department, ModuleView } from '@/lib/api/types';

export const modulesKey = (orderId: number) => ['order', orderId, 'modules'];

/** A department's colour as a soft background behind its part. */
function tint(color: string, alpha = 0.08): string {
  const n = parseInt(color.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

export function DepartmentChip({ name, color }: { name: string; color: string }) {
  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-metadata font-medium"
      style={{ backgroundColor: tint(color, 0.14), color }}
    >
      <span className="h-2 w-2 rounded-full" style={{ backgroundColor: color }} aria-hidden />
      {name}
    </span>
  );
}

export function ModulesSection({ orderId }: { orderId: number }) {
  const t = useTranslations('modules');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const qc = useQueryClient();
  const toast = useToast();
  const query = useQuery({ queryKey: modulesKey(orderId), queryFn: () => modulesApi.forOrder(orderId) });
  const departments = useQuery({ queryKey: ['departments'], queryFn: () => modulesApi.departments() });

  const refresh = () => void qc.invalidateQueries({ queryKey: modulesKey(orderId) });
  const create = useMutation({
    mutationFn: (departmentId: number) => modulesApi.create(orderId, { department_id: departmentId }),
    onSuccess: refresh,
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });

  if (query.isLoading) return <LoadingState />;
  if (query.isError || !query.data) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;

  const modules = query.data.items;
  const taken = new Set(modules.map((m) => m.department_id));
  const startable = (departments.data?.items ?? []).filter(
    (d) => d.is_active && !taken.has(d.id) && query.data.editable_department_ids.includes(d.id),
  );

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-section font-semibold">{t('title')}</h2>
          <p className="text-metadata text-steel-500">{t('hint')}</p>
        </div>
        {modules.length > 0 && (
          <a
            className="btn-secondary btn-sm"
            href={`/${locale}/orders/${orderId}/sheet/all?print=1`}
            target="_blank"
            rel="noreferrer"
          >
            {t('printAll')}
          </a>
        )}
      </div>

      {modules.length === 0 && <p className="text-body text-steel-500">{t('none')}</p>}

      {modules.map((m) => (
        <ModuleCard key={m.id} orderId={orderId} module={m} onChanged={refresh} />
      ))}

      {startable.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-metadata text-steel-500">{t('add')}:</span>
          {startable.map((d: Department) => (
            <button
              key={d.id}
              type="button"
              className="btn-secondary btn-sm"
              style={{ borderColor: d.color, color: d.color }}
              disabled={create.isPending}
              onClick={() => create.mutate(d.id)}
            >
              + {d.name}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function statusLabel(t: ReturnType<typeof useTranslations>, m: ModuleView): string {
  if (m.changed_since_release) return t('statusChanged');
  return t(`status.${m.status}`);
}

function ModuleCard({ orderId, module: m, onChanged }: { orderId: number; module: ModuleView; onChanged: () => void }) {
  const t = useTranslations('modules');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  const toast = useToast();
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(m.content);
  const [picking, setPicking] = useState(false);
  const [removing, setRemoving] = useState(false);
  const onError = (e: unknown) => toast.error(errorMessage(e, ter, ter('unknownError')));

  const save = useMutation({
    mutationFn: () => modulesApi.update(m.id, { content: text }),
    onSuccess: () => {
      setEditing(false);
      onChanged();
    },
    onError,
  });
  const release = useMutation({
    mutationFn: () => modulesApi.release(m.id),
    onSuccess: () => {
      toast.success(t('released', { department: m.department_name }));
      onChanged();
    },
    onError,
  });
  const setStatus = useMutation({
    mutationFn: (status: 'draft' | 'done') => modulesApi.update(m.id, { status }),
    onSuccess: onChanged,
    onError,
  });
  const remove = useMutation({
    mutationFn: () => modulesApi.remove(m.id),
    onSuccess: () => {
      setRemoving(false);
      onChanged();
    },
    onError,
  });
  const removeFile = useMutation({
    mutationFn: (fileId: number) => modulesApi.removeFile(m.id, fileId),
    onSuccess: onChanged,
    onError,
  });

  const needsRelease = m.status === 'draft' || m.changed_since_release;
  return (
    <article
      className="card overflow-hidden border-l-4"
      style={{ borderLeftColor: m.department_color }}
      aria-label={m.department_name}
    >
      <header
        className="flex flex-wrap items-center justify-between gap-2 px-4 py-3"
        style={{ backgroundColor: tint(m.department_color) }}
      >
        <div className="flex flex-wrap items-center gap-2">
          <DepartmentChip name={m.department_name} color={m.department_color} />
          <span
            className={`text-metadata font-medium ${m.changed_since_release ? 'text-signal' : 'text-steel-500'}`}
          >
            {statusLabel(t, m)}
          </span>
          {m.released_at && (
            <span className="text-metadata text-steel-500">
              · {t('releasedAt')} <DateDisplay value={m.released_at} />
              {m.released_by_name ? ` (${m.released_by_name})` : ''}
            </span>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          <a
            className="btn-ghost btn-sm"
            href={`/${locale}/orders/${orderId}/sheet/${m.id}?print=1`}
            target="_blank"
            rel="noreferrer"
          >
            {t('print')}
          </a>
          {m.can_edit && !editing && (
            <button type="button" className="btn-ghost btn-sm" onClick={() => { setText(m.content); setEditing(true); }}>
              {tc('edit')}
            </button>
          )}
          {m.can_edit && needsRelease && !editing && (
            <button
              type="button"
              className="btn-primary btn-sm"
              disabled={release.isPending || !m.content.trim()}
              title={!m.content.trim() ? t('emptyCannotRelease') : t('releaseHint')}
              onClick={() => release.mutate()}
            >
              {m.changed_since_release ? t('sendChange') : t('release')}
            </button>
          )}
          {m.can_edit && m.status === 'released' && !m.changed_since_release && (
            <button type="button" className="btn-secondary btn-sm" onClick={() => setStatus.mutate('done')}>
              {t('markDone')}
            </button>
          )}
          {m.can_edit && m.status === 'done' && (
            <button type="button" className="btn-ghost btn-sm" onClick={() => setStatus.mutate('draft')}>
              {t('reopen')}
            </button>
          )}
        </div>
      </header>

      <div className="space-y-3 px-4 py-3">
        {editing ? (
          <div className="space-y-2">
            <textarea
              className="input min-h-[10rem] font-mono text-body"
              value={text}
              onChange={(e) => setText(e.target.value)}
              aria-label={t('contentLabel', { department: m.department_name })}
              placeholder={t('contentPlaceholder')}
            />
            {m.status !== 'draft' && <p className="text-metadata text-steel-500">{t('editAfterRelease')}</p>}
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-ghost btn-sm" onClick={() => setEditing(false)}>
                {tc('cancel')}
              </button>
              <button type="button" className="btn-primary btn-sm" disabled={save.isPending} onClick={() => save.mutate()}>
                {save.isPending ? tc('saving') : tc('save')}
              </button>
            </div>
          </div>
        ) : m.content.trim() ? (
          <p className="whitespace-pre-wrap text-body">{m.content}</p>
        ) : (
          <p className="text-body text-steel-500">{t('empty')}</p>
        )}

        <div>
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-metadata font-medium text-steel-500">{t('files')}</span>
            {m.can_edit && (
              <button type="button" className="btn-ghost btn-sm" onClick={() => setPicking((v) => !v)}>
                {picking ? tc('close') : t('addFile')}
              </button>
            )}
          </div>
          {m.files.length === 0 ? (
            <p className="text-metadata text-steel-500">{t('noFiles')}</p>
          ) : (
            <ul className="mt-1 flex flex-wrap gap-2">
              {m.files.map((f) => (
                <li key={f.id} className="flex items-center gap-1 rounded-lg bg-steel-200/50 px-2 py-1 text-metadata">
                  {f.image_id ? '🖼' : '📄'} {f.filename ?? t('photo')}
                  {m.can_edit && (
                    <button
                      type="button"
                      className="ml-1 text-steel-500 hover:text-signal"
                      aria-label={t('removeFile')}
                      onClick={() => removeFile.mutate(f.id)}
                    >
                      ×
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
          {picking && <FilePicker orderId={orderId} module={m} onPicked={onChanged} />}
        </div>

        <div className="flex flex-wrap items-center justify-between gap-2 text-metadata text-steel-500">
          <span>
            {m.updated_by_name && (
              <>
                {t('lastEdit')}: {m.updated_by_name} · <DateDisplay value={m.content_updated_at} />
              </>
            )}
          </span>
          {canEditOrders(user) && (
            <button type="button" className="hover:text-signal" onClick={() => setRemoving(true)}>
              {t('remove')}
            </button>
          )}
        </div>
      </div>

      <ConfirmDialog
        open={removing}
        title={t('removeTitle', { department: m.department_name })}
        body={t('removeBody')}
        confirmLabel={t('remove')}
        busy={remove.isPending}
        onConfirm={() => remove.mutate()}
        onClose={() => setRemoving(false)}
      />
    </article>
  );
}

/** The order's drawings and photos, to put one on the part's sheet. */
function FilePicker({ orderId, module: m, onPicked }: { orderId: number; module: ModuleView; onPicked: () => void }) {
  const t = useTranslations('modules');
  const ter = useTranslations('errors');
  const toast = useToast();
  const images = useQuery({ queryKey: ['order', orderId, 'images', undefined], queryFn: () => mediaApi.images(orderId) });
  const documents = useQuery({ queryKey: ['order', orderId, 'documents'], queryFn: () => mediaApi.documents(orderId) });
  const add = useMutation({
    mutationFn: (body: { image_id?: number; document_id?: number }) => modulesApi.addFile(m.id, body),
    onSuccess: onPicked,
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });
  const linkedImages = new Set(m.files.map((f) => f.image_id));
  const linkedDocs = new Set(m.files.map((f) => f.document_id));
  const docs = (documents.data?.items ?? []).filter(
    (d) => !linkedDocs.has(d.id) && !['invoice', 'proforma', 'video'].includes(d.kind),
  );
  const imgs = (images.data?.items ?? []).filter((i) => !linkedImages.has(i.id));

  return (
    <div className="mt-2 space-y-2 rounded-lg border border-steel-200 p-3">
      {docs.length === 0 && imgs.length === 0 && <p className="text-metadata text-steel-500">{t('pickNone')}</p>}
      {docs.length > 0 && (
        <ul className="flex flex-wrap gap-2">
          {docs.map((d) => (
            <li key={d.id}>
              <button type="button" className="btn-secondary btn-sm" onClick={() => add.mutate({ document_id: d.id })}>
                📄 {d.filename}
              </button>
            </li>
          ))}
        </ul>
      )}
      {imgs.length > 0 && (
        <ul className="flex flex-wrap gap-2">
          {imgs.map((i) => (
            <li key={i.id}>
              <button
                type="button"
                className="overflow-hidden rounded-lg border border-steel-200 hover:border-cold"
                title={i.original_filename ?? undefined}
                onClick={() => add.mutate({ image_id: i.id })}
              >
                {i.thumb_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img src={i.thumb_url} alt={i.original_filename ?? t('photo')} className="h-16 w-16 object-cover" />
                ) : (
                  <span className="block h-16 w-16 p-1 text-metadata">{i.original_filename ?? t('photo')}</span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
