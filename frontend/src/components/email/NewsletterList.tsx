'use client';

// Newsletter subscriptions under settings: who the blast reaches. Unsubscribed rows stay
// visible — a deleted address would silently resubscribe on the next import, which is
// exactly how you mail someone who opted out.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { newsletterApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { useAuth, canSendEmail } from '@/lib/auth/context';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { DateDisplay } from '@/components/ui/DateDisplay';

export function NewsletterList() {
  const t = useTranslations('emails');
  const ter = useTranslations('errors');
  const tc = useTranslations('common');
  const { user } = useAuth();
  const qc = useQueryClient();
  const [email, setEmail] = useState('');
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<number | null>(null);

  const list = useQuery({
    queryKey: ['newsletter-subscriptions'],
    queryFn: () => newsletterApi.subscriptions(),
  });

  const refresh = () => void qc.invalidateQueries({ queryKey: ['newsletter-subscriptions'] });

  const add = useMutation({
    mutationFn: () => newsletterApi.addSubscription({ email: email.trim(), name: name.trim() }),
    onSuccess: () => {
      setEmail('');
      setName('');
      setError(null);
      refresh();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const remove = useMutation({
    mutationFn: (id: number) => newsletterApi.removeSubscription(id),
    onSuccess: () => {
      setRemoving(null);
      refresh();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  if (!canSendEmail(user)) return null;
  const items = list.data?.items ?? [];
  const active = items.filter((s) => !s.unsubscribed_at).length;

  return (
    <section aria-label={t('newsletterTitle')} className="mt-10">
      <h2 className="text-section font-semibold">{t('newsletterTitle')}</h2>
      <p className="mt-1 text-metadata text-steel-500">
        {t('newsletterCount', { active, total: items.length })}
      </p>

      <div className="mt-4 flex flex-wrap items-end gap-3">
        <div>
          <label className="label" htmlFor="nl-email">{t('to')}</label>
          <input
            id="nl-email"
            className="input"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="olvaso@example.hu"
          />
        </div>
        <div>
          <label className="label" htmlFor="nl-name">{t('subscriberName')}</label>
          <input
            id="nl-name"
            className="input"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <button
          type="button"
          className="btn-secondary btn-sm"
          disabled={email.trim() === '' || add.isPending}
          onClick={() => add.mutate()}
        >
          {add.isPending ? '…' : t('subscribeAdd')}
        </button>
      </div>

      {error && (
        <p className="mt-3 rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
          {error}
        </p>
      )}

      <ul className="mt-4 space-y-2">
        {items.map((s) => (
          <li key={s.id} className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-3">
            <span className="min-w-0 flex-1 text-body">
              <span className="font-medium">{s.email}</span>
              {s.name && <span className="text-steel-500"> · {s.name}</span>}
              <span className="text-metadata text-steel-500">
                {' '}· {s.source} · <DateDisplay value={s.subscribed_at} />
              </span>
            </span>
            {s.unsubscribed_at ? (
              <StatusBadge tone="muted">{t('unsubscribedBadge')}</StatusBadge>
            ) : (
              <StatusBadge tone="done">{t('subscribedBadge')}</StatusBadge>
            )}
            <button
              type="button"
              className="btn-ghost btn-sm"
              onClick={() => setRemoving(s.id)}
            >
              {tc('delete')}
            </button>
          </li>
        ))}
        {items.length === 0 && !list.isLoading && (
          <li className="text-body text-steel-500">{t('newsletterEmpty')}</li>
        )}
      </ul>

      <ConfirmDialog
        open={removing !== null}
        title={t('unsubscribeRemoveTitle')}
        body={t('unsubscribeRemoveBody')}
        confirmLabel={tc('delete')}
        onClose={() => setRemoving(null)}
        busy={remove.isPending}
        onConfirm={() => removing !== null && remove.mutate(removing)}
      />
    </section>
  );
}
