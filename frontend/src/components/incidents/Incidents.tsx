'use client';

// The internal incident log: damage the workshop caused, what it cost and how it was
// settled. Opened by hand or from a "new" damage at kiadás; a rework job can be opened from
// it. Internal only: nothing here reaches a customer.

import Link from 'next/link';
import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { incidentsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canChangeStage, canEditOrders, useAuth } from '@/lib/auth/context';
import { parseMajorToMinor, minorToMajorString } from '@/lib/utils/format';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { Money } from '@/components/ui/Money';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { cn } from '@/lib/utils/format';
import type { Incident } from '@/lib/api/types';

type Currency = 'HUF' | 'EUR';

export function IncidentDialog({
  orderId,
  currency,
  damageId,
  initialTitle,
  incident,
  onClose,
}: {
  orderId: number;
  currency: Currency;
  damageId?: number;
  initialTitle?: string;
  incident?: Incident;
  onClose: () => void;
}) {
  const t = useTranslations('incidents');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { user } = useAuth();
  const [title, setTitle] = useState(incident?.title ?? initialTitle ?? '');
  const [description, setDescription] = useState(incident?.description ?? '');
  const [cost, setCost] = useState(incident?.cost_minor != null ? minorToMajorString(incident.cost_minor) : '');
  const [cur, setCur] = useState<Currency>((incident?.currency as Currency) ?? currency);
  const [responsible, setResponsible] = useState(incident?.responsible ?? '');
  const [rework, setRework] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: async () => {
      const costMinor = cost.trim() ? parseMajorToMinor(cost.trim()) : null;
      if (cost.trim() && costMinor === null) throw new Error(t('badCost'));
      if (incident) {
        return incidentsApi.update(incident.id, {
          title: title.trim(),
          description: description.trim() || null,
          cost_minor: costMinor,
          currency: cur,
          responsible: responsible.trim() || null,
        });
      }
      return incidentsApi.create({
        order_id: orderId,
        damage_id: damageId ?? null,
        title: title.trim(),
        description: description.trim() || null,
        cost_minor: costMinor,
        currency: cur,
        responsible: responsible.trim() || null,
        open_rework: rework,
      });
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['incidents'] });
      void qc.invalidateQueries({ queryKey: ['order'] });
      onClose();
    },
    onError: (e) => setError(e instanceof Error && !('status' in e) ? e.message : errorMessage(e, ter, ter('unknownError'))),
  });

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[94vw] max-w-lg -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <div className="card-header">
            <Dialog.Title className="text-section font-semibold">{incident ? t('edit') : t('new')}</Dialog.Title>
            <Dialog.Description className="text-metadata text-steel-500">{t('internalOnly')}</Dialog.Description>
          </div>
          <form
            className="card-content space-y-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (title.trim()) save.mutate();
            }}
          >
            {error && <p className="text-body text-signal" role="alert">{error}</p>}
            <div>
              <label className="label" htmlFor="inc-title">{t('title')} *</label>
              <input id="inc-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>
            <div>
              <label className="label" htmlFor="inc-desc">{t('description')}</label>
              <textarea id="inc-desc" rows={3} className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>
            <div className="grid grid-cols-3 gap-2">
              <div className="col-span-2">
                <label className="label" htmlFor="inc-cost">{t('cost')}</label>
                <input id="inc-cost" inputMode="decimal" className="input" value={cost} onChange={(e) => setCost(e.target.value)} />
              </div>
              <div>
                <label className="label" htmlFor="inc-cur">{tc('currency')}</label>
                <select id="inc-cur" className="input" value={cur} onChange={(e) => setCur(e.target.value as Currency)}>
                  <option value="HUF">HUF</option>
                  <option value="EUR">EUR</option>
                </select>
              </div>
            </div>
            <div>
              <label className="label" htmlFor="inc-resp">{t('responsible')}</label>
              <input id="inc-resp" className="input" value={responsible} onChange={(e) => setResponsible(e.target.value)} placeholder={t('responsibleHint')} />
            </div>
            {!incident && canEditOrders(user) && (
              <label className="flex items-center gap-2 text-body">
                <input type="checkbox" className="accent-steel-900" checked={rework} onChange={(e) => setRework(e.target.checked)} />
                {t('openRework')}
              </label>
            )}
            <div className="flex justify-end gap-2 pt-2">
              <button type="button" className="btn-ghost" onClick={onClose}>{tc('cancel')}</button>
              <button type="submit" className="btn-primary" disabled={!title.trim() || save.isPending}>
                {save.isPending ? tc('saving') : tc('save')}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ResolveDialog({ incident, onClose }: { incident: Incident; onClose: () => void }) {
  const t = useTranslations('incidents');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [resolution, setResolution] = useState(incident.resolution ?? '');
  const [error, setError] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: () => incidentsApi.update(incident.id, { status: 'resolved', resolution: resolution.trim() || null }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['incidents'] });
      void qc.invalidateQueries({ queryKey: ['order'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 w-[94vw] max-w-md -translate-x-1/2 -translate-y-1/2">
          <div className="card-header">
            <Dialog.Title className="text-section font-semibold">{t('resolveTitle')}</Dialog.Title>
          </div>
          <div className="card-content space-y-3">
            {error && <p className="text-body text-signal" role="alert">{error}</p>}
            <label className="label" htmlFor="inc-res">{t('resolution')}</label>
            <textarea id="inc-res" rows={3} className="input" value={resolution} onChange={(e) => setResolution(e.target.value)} />
          </div>
          <div className="card-footer justify-end">
            <button className="btn-ghost" onClick={onClose}>{tc('cancel')}</button>
            <button className="btn-primary" onClick={() => save.mutate()} disabled={save.isPending}>{t('resolve')}</button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function IncidentRows({ items, showOrder }: { items: Incident[]; showOrder: boolean }) {
  const t = useTranslations('incidents');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const qc = useQueryClient();
  const { user } = useAuth();
  const [editing, setEditing] = useState<Incident | null>(null);
  const [resolving, setResolving] = useState<Incident | null>(null);
  const [error, setError] = useState<string | null>(null);
  const rework = useMutation({
    mutationFn: (id: number) => incidentsApi.openRework(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['incidents'] });
      void qc.invalidateQueries({ queryKey: ['order'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  const reopen = useMutation({
    mutationFn: (id: number) => incidentsApi.update(id, { status: 'open' }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['incidents'] }),
  });
  const edit = canChangeStage(user);
  if (!items.length) return <p className="text-body text-steel-500">{t('empty')}</p>;
  return (
    <>
      {error && <p className="text-body text-signal" role="alert">{error}</p>}
      <ul className="space-y-2">
        {items.map((i) => (
          <li key={i.id} className={cn('rounded-lg border p-3', i.status === 'open' ? 'border-signal/40' : 'border-steel-200')}>
            <div className="flex flex-wrap items-center gap-2">
              <StatusBadge tone={i.status === 'open' ? 'signal' : 'done'}>{t(`status.${i.status}`)}</StatusBadge>
              <span className="text-body font-medium">{i.title}</span>
              {i.cost_minor != null && (
                <span className="font-mono text-metadata">
                  <Money minor={i.cost_minor} currency={i.currency} />
                </span>
              )}
              <span className="flex-1" />
              <span className="text-metadata text-steel-500">
                <DateDisplay value={i.created_at} />
                {i.created_by_name ? ` · ${i.created_by_name}` : ''}
              </span>
            </div>
            <p className="mt-1 flex flex-wrap gap-x-3 text-metadata text-steel-500">
              {showOrder && (
                <Link className="underline" href={`/${locale}/orders/${i.order_id}`}>
                  #{i.order_number} · {i.order_title}
                </Link>
              )}
              {i.responsible && <span>{t('responsible')}: {i.responsible}</span>}
              {i.damage_id && <span>{t('fromInspection')}</span>}
              {i.rework_order_id && (
                <Link className="underline" href={`/${locale}/orders/${i.rework_order_id}`}>
                  {t('reworkJob')} #{i.rework_order_number}
                </Link>
              )}
            </p>
            {i.description && <p className="mt-1 whitespace-pre-wrap text-body">{i.description}</p>}
            {i.resolution && (
              <p className="mt-1 text-metadata text-steel-500">
                {t('resolution')}: {i.resolution}
              </p>
            )}
            {edit && (
              <div className="mt-2 flex flex-wrap gap-2">
                <button type="button" className="btn-ghost btn-sm" onClick={() => setEditing(i)}>{t('editShort')}</button>
                {i.status === 'open' ? (
                  <button type="button" className="btn-secondary btn-sm" onClick={() => setResolving(i)}>{t('resolve')}</button>
                ) : (
                  <button type="button" className="btn-ghost btn-sm" onClick={() => reopen.mutate(i.id)}>{t('reopen')}</button>
                )}
                {!i.rework_order_id && canEditOrders(user) && (
                  <button type="button" className="btn-ghost btn-sm" disabled={rework.isPending} onClick={() => rework.mutate(i.id)}>
                    {t('openRework')}
                  </button>
                )}
              </div>
            )}
          </li>
        ))}
      </ul>
      {editing && (
        <IncidentDialog
          orderId={editing.order_id}
          currency={editing.currency as Currency}
          incident={editing}
          onClose={() => setEditing(null)}
        />
      )}
      {resolving && <ResolveDialog incident={resolving} onClose={() => setResolving(null)} />}
    </>
  );
}

/** An order's incidents, with a button to log one. */
export function OrderIncidents({ orderId, currency }: { orderId: number; currency: Currency }) {
  const t = useTranslations('incidents');
  const { user } = useAuth();
  const [creating, setCreating] = useState(false);
  const query = useQuery({
    queryKey: qk.incidents({ order: orderId }),
    queryFn: () => incidentsApi.list({ order_id: orderId }),
  });
  return (
    <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-section font-semibold">{t('title_section')}</h2>
        {canChangeStage(user) && (
          <button type="button" className="btn-secondary btn-sm" onClick={() => setCreating(true)}>
            {t('new')}
          </button>
        )}
      </div>
      <p className="mt-1 text-metadata text-steel-500">{t('internalOnly')}</p>
      <div className="mt-3">
        {query.isPending ? (
          <LoadingState />
        ) : query.isError ? (
          <ErrorState error={query.error} onRetry={() => void query.refetch()} />
        ) : (
          <IncidentRows items={query.data.items} showOrder={false} />
        )}
      </div>
      {creating && <IncidentDialog orderId={orderId} currency={currency} onClose={() => setCreating(false)} />}
    </section>
  );
}

/** The whole log, for the incidents page. */
export function IncidentLog() {
  const t = useTranslations('incidents');
  const [status, setStatus] = useState<'open' | 'resolved' | 'all'>('open');
  const query = useQuery({
    queryKey: qk.incidents({ status }),
    queryFn: () => incidentsApi.list({ status: status === 'all' ? undefined : status, limit: 200 }),
  });
  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        {(['open', 'resolved', 'all'] as const).map((s) => (
          <button key={s} type="button" className={cn('btn-sm', status === s ? 'btn-primary' : 'btn-ghost')} onClick={() => setStatus(s)}>
            {t(`filter.${s}`)}
          </button>
        ))}
      </div>
      {query.isPending ? (
        <LoadingState />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <IncidentRows items={query.data.items} showOrder />
      )}
    </div>
  );
}
