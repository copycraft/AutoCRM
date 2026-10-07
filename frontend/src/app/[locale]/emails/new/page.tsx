'use client';

import { Suspense } from 'react';

// New email: a direct letter or a newsletter blast, written in text or Markdown with a
// live preview of exactly what will be stored and sent.

import { useRouter, useSearchParams } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { Breadcrumbs, BackToList } from '@/components/ui/Breadcrumbs';
import { ComposeForm } from '@/components/email/ComposeForm';
import { useToast } from '@/components/ui/Toasts';
import { useAuth, canSendEmail } from '@/lib/auth/context';

// useSearchParams needs a Suspense boundary, or the page cannot be prerendered.
export default function NewEmailPage() {
  return (
    <Suspense>
      <NewEmailPageInner />
    </Suspense>
  );
}

function NewEmailPageInner() {
  // order_id, lead_id, partner_id, to; `audience=newsletter` opens the blast and `tags`
  // (comma-separated ids) aims it, from Marketing; answering a received letter (0049):
  // reply_to threads the reply, subject and quote prefill.
  const sp = useSearchParams();
  const resolvedParams = Object.fromEntries(sp.entries()) as {
    order_id?: string;
    lead_id?: string;
    partner_id?: string;
    to?: string;
    audience?: string;
    tags?: string;
    reply_to?: string;
    subject?: string;
    quote?: string;
  };
  const t = useTranslations('emails');
  const tn = useTranslations('navigation');
  const tq = useTranslations('qol');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const toast = useToast();
  const { user } = useAuth();

  const num = (raw: string | undefined): number | undefined => {
    const n = Number(raw);
    return raw != null && Number.isFinite(n) && n > 0 ? Math.floor(n) : undefined;
  };
  // Context from `?order_id=&lead_id=&partner_id=&to=`: the letter is filed against the
  // record, and its documents become attachable.
  const params = resolvedParams;
  const about = {
    ...(num(params.order_id) ? { order_id: num(params.order_id)! } : {}),
    ...(num(params.lead_id) ? { lead_id: num(params.lead_id)! } : {}),
    ...(num(params.partner_id) ? { partner_id: num(params.partner_id)! } : {}),
  };

  if (!canSendEmail(user)) {
    return (
      <AppShell>
        <PageHeader title={t('newEmail')} />
        <p className="mt-4 text-body text-steel-500">{t('noPermission')}</p>
      </AppShell>
    );
  }

  return (
    <AppShell>
      <BackToList listKey="emails" fallbackHref={`/${locale}/emails`} />
      <Breadcrumbs
        items={[
          { href: `/${locale}/emails`, label: tn('emails') },
          { label: t('newEmail') },
        ]}
      />
      <PageHeader title={t('newEmail')} subtitle={t('newEmailHint')} />
      <div className="mt-5">
        <ComposeForm
          about={Object.keys(about).length > 0 ? about : undefined}
          defaultTo={params.to}
          defaultAudience={params.audience === 'newsletter' ? 'newsletter' : undefined}
          defaultTagIds={(params.tags ?? '').split(',').map(Number).filter((n) => Number.isInteger(n) && n > 0)}
          defaultSubject={params.subject}
          replyToInboundId={num(params.reply_to)}
          quote={params.quote}
          onSent={(id, newsletterRecipients) => {
            void qc.invalidateQueries({ queryKey: ['emails'] });
            toast.success(
              newsletterRecipients != null
                ? t('newsletterQueuedNote', { count: newsletterRecipients })
                : t('queuedNote'),
            );
            // A newsletter is a tracked send (one letter per reader), shown on Marketing.
            router.replace(
              newsletterRecipients != null ? `/${locale}/marketing?tab=sends` : `/${locale}/emails/${id}`,
            );
          }}
        />
      </div>
    </AppShell>
  );
}
