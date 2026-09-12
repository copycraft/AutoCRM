'use client';

import Link from 'next/link';
import { useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { emailApi, usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canAdmin, useAuth } from '@/lib/auth/context';
import type { EmailStatus } from '@/lib/api/types';

function statusTone(status: EmailStatus): StatusTone {
  switch (status) {
    case 'failed':
    case 'needs_review':
      return 'signal';
    case 'sent':
      return 'done';
    case 'cancelled':
      return 'muted';
    default:
      return 'steel';
  }
}

const STATUS_KEYS: Record<EmailStatus, string> = {
  queued: 'queued',
  sending: 'sending',
  sent: 'sent',
  failed: 'failed',
  cancelled: 'cancelled',
  needs_review: 'needsReview',
};

function Info({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className="text-sm">{value}</dd>
    </div>
  );
}

export default function EmailDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('emails');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const [confirm, setConfirm] = useState<'cancel' | 'retry' | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const detail = useQuery({ queryKey: qk.email(id), queryFn: () => emailApi.get(id) });
  // GET /users is admin-only; non-admins see the raw id (backend gap).
  const usersQuery = useQuery({
    queryKey: qk.users,
    queryFn: () => usersApi.list(),
    enabled: canAdmin(user) && (detail.data?.sent_by ?? null) !== null,
    retry: false,
  });
  const sentByName = detail.data?.sent_by
    ? (usersQuery.data?.items.find((u) => u.id === detail.data?.sent_by)?.display_name ??
      `#${detail.data.sent_by}`)
    : '—';

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: qk.email(id) });
    void qc.invalidateQueries({ queryKey: ['emails'] });
  };

  const fail = (e: unknown) => setActionError(errorMessage(e, ter, ter('unknownError')));
  const cancel = useMutation({
    mutationFn: () => emailApi.cancel(id),
    onSuccess: () => {
      setConfirm(null);
      setActionError(null);
      invalidate();
    },
    onError: fail,
  });
  const retry = useMutation({
    mutationFn: () => emailApi.retry(id),
    onSuccess: () => {
      setConfirm(null);
      setActionError(null);
      invalidate();
    },
    onError: fail,
  });

  // Backend rules: own queued mail is cancellable (any queued mail with
  // system powers); retry is failed/needs-review and admin-only. The backend
  // enforces; the UI only hides what it knows is unavailable.
  const m = detail.data;
  const mine = !!m && !!user && m.sent_by === user.id;
  const canCancel = !!m && m.status === 'queued' && (mine || canAdmin(user));
  const canRetry =
    !!m && canAdmin(user) && (m.status === 'failed' || m.status === 'needs_review');

  return (
    <AppShell>
      {detail.isLoading ? (
        <DetailSkeleton />
      ) : detail.isError || !detail.data ? (
        <>
          <PageHeader title={t('title')} />
          <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
        </>
      ) : (
        (() => {
          const email = detail.data;
          return (
            <>
              <PageHeader
                title={email.subject}
                subtitle={email.to_address}
                actions={
                  <>
                    {canCancel && (
                      <button className="btn-secondary btn-sm" onClick={() => { setActionError(null); setConfirm('cancel'); }}>
                        {t('cancel')}
                      </button>
                    )}
                    {canRetry && (
                      <button className="btn-secondary btn-sm" onClick={() => { setActionError(null); setConfirm('retry'); }}>
                        {t('retry')}
                      </button>
                    )}
                  </>
                }
              />
              <div className="flex flex-wrap items-center gap-2">
                <StatusBadge tone={statusTone(email.status)}>
                  {t(`status.${STATUS_KEYS[email.status]}`)}
                </StatusBadge>
                {email.is_automatic ? (
                  <StatusBadge tone="steel">{t('autoBadge')}</StatusBadge>
                ) : (
                  <StatusBadge tone="steel">{t('manualBadge')}</StatusBadge>
                )}
              </div>
              {actionError && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">
                  {actionError}
                </p>
              )}

              {email.status === 'needs_review' && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="note">
                  {t('needsReviewWarning')}
                </p>
              )}
              {email.error && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">
                  {t('errorLabel')}: {email.error}
                </p>
              )}

              <section className="card">
                <div className="card-content grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <Info label={t('to')} value={email.to_address} />
                  <Info label={t('cc')} value={email.cc.length > 0 ? email.cc.join(', ') : '—'} />
                  <Info label={t('sentBy')} value={sentByName} />
                  <Info
                    label={t('orderLink')}
                    value={
                      email.order_id ? (
                        <Link href={`/${locale}/orders/${email.order_id}`} className="text-steel-900 underline">
                          #{email.order_id}
                        </Link>
                      ) : (
                        '—'
                      )
                    }
                  />
                  <Info label={t('queuedAt')} value={<DateDisplay withTime value={email.queued_at} />} />
                  <Info
                    label={t('sentAt')}
                    value={email.sent_at ? <DateDisplay withTime value={email.sent_at} /> : '—'}
                  />
                </div>
              </section>

              <section className="card">
                <div className="card-header">
                  <h2 className="text-section font-semibold">{t('body')}</h2>
                </div>
                <div className="card-content">
                  <pre className="whitespace-pre-wrap font-sans text-sm">{email.body_text}</pre>
                </div>
              </section>

              <ConfirmDialog
                open={confirm !== null}
                title={confirm === 'retry' ? t('retry') : t('cancel')}
                body={confirm === 'retry' ? t('retryConfirm') : t('cancelConfirm')}
                confirmLabel={confirm === 'retry' ? t('retry') : t('cancel')}
                onClose={() => setConfirm(null)}
                busy={cancel.isPending || retry.isPending}
                onConfirm={() => (confirm === 'retry' ? retry.mutate() : cancel.mutate())}
              />
            </>
          );
        })()
      )}
    </AppShell>
  );
}
