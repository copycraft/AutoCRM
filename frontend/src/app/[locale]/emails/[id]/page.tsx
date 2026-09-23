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
import { DocumentLink } from '@/components/orders/InvoicesSection';
import { Breadcrumbs, BackToList } from '@/components/ui/Breadcrumbs';
import { CopyButton, CopyLinkButton } from '@/components/ui/CopyButton';
import { emailApi, usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canAdmin, useAuth } from '@/lib/auth/context';
import { useToast } from '@/components/ui/Toasts';
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
      <dd className="text-body">{value}</dd>
    </div>
  );
}

export default function EmailDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('emails');
  const tc = useTranslations('common');
  const tn = useTranslations('navigation');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const [confirm, setConfirm] = useState<'cancel' | 'retry' | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [htmlView, setHtmlView] = useState(true);
  const toast = useToast();
  const tq = useTranslations('qol');

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

  const fail = (e: unknown) => {
    const msg = errorMessage(e, ter, ter('unknownError'));
    setActionError(msg);
    toast.error(tq('toastError'), msg);
  };
  const cancel = useMutation({
    mutationFn: () => emailApi.cancel(id),
    onSuccess: () => {
      setConfirm(null);
      setActionError(null);
      toast.success(t('cancelledNote'));
      invalidate();
    },
    onError: fail,
  });
  const retry = useMutation({
    mutationFn: () => emailApi.retry(id),
    onSuccess: () => {
      setConfirm(null);
      setActionError(null);
      toast.success(t('retry'));
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
          <PageHeader size="record" title={t('title')} />
          <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
        </>
      ) : (
        (() => {
          const email = detail.data;
          return (
            <>
              <BackToList listKey="emails" fallbackHref={`/${locale}/emails`} />
              <div className="flex items-center justify-between gap-3">
                <Breadcrumbs
                  items={[
                    { href: `/${locale}/emails`, label: tn('emails') },
                    { label: email.subject },
                  ]}
                />
                <CopyLinkButton />
              </div>
              <PageHeader size="record"
                title={
                  <span className="inline-flex items-center gap-2">
                    <span>{email.subject}</span>
                    <CopyButton value={email.to_address} label={t('to')} />
                  </span>
                }
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
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                  {actionError}
                </p>
              )}

              {email.status === 'needs_review' && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="note">
                  {t('needsReviewWarning')}
                </p>
              )}
              {email.error && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                  {t('errorLabel')}: {email.error}
                </p>
              )}

              <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
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

              <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                <h2 className="text-section font-semibold">{t('attachments')}</h2>
                <div className="mt-3">
                  {email.attachments.length === 0 ? (
                    <p className="text-metadata text-steel-500">—</p>
                  ) : (
                    <ul className="space-y-1">
                      {email.attachments.map((a) => (
                        <li key={a.document_id} className="text-body">
                          <DocumentLink
                            documentId={a.document_id}
                            label={a.filename ?? `${t('attachments')} #${a.document_id}`}
                          />
                          {a.mode && (
                            <span className="text-metadata text-steel-500"> · {a.mode}</span>
                          )}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </section>

              <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                <div className="flex items-center justify-between gap-3">
                  <h2 className="text-section font-semibold">{t('body')}</h2>
                  <div className="inline-flex rounded-lg bg-steel-200/50 p-0.5 text-metadata" role="group" aria-label={t('body')}>
                    <button
                      type="button"
                      onClick={() => setHtmlView(true)}
                      aria-pressed={htmlView}
                      className={`rounded-md px-2.5 py-1 ${htmlView ? 'bg-surface text-steel-900 shadow-sm' : 'text-steel-500'}`}
                    >
                      {t('viewHtml')}
                    </button>
                    <button
                      type="button"
                      onClick={() => setHtmlView(false)}
                      aria-pressed={!htmlView}
                      className={`rounded-md px-2.5 py-1 ${!htmlView ? 'bg-surface text-steel-900 shadow-sm' : 'text-steel-500'}`}
                    >
                      {t('viewText')}
                    </button>
                  </div>
                </div>
                <div className="mt-3">
                  {htmlView ? (
                    // Sandboxed: the stored HTML renders like a mail client shows it,
                    // with no scripts and no access to the app.
                    <iframe
                      title={t('viewHtml')}
                      sandbox=""
                      srcDoc={email.body_html}
                      className="h-[480px] w-full rounded-lg border border-steel-200 bg-white"
                    />
                  ) : (
                    <pre className="whitespace-pre-wrap font-sans text-body">{email.body_text}</pre>
                  )}
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
