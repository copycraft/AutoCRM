'use client';

// New email: a direct letter or a newsletter blast, written in text or Markdown with a
// live preview of exactly what will be stored and sent.

import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { Breadcrumbs, BackToList } from '@/components/ui/Breadcrumbs';
import { ComposeForm } from '@/components/email/ComposeForm';
import { useToast } from '@/components/ui/Toasts';
import { useAuth, canSendEmail } from '@/lib/auth/context';

export default function NewEmailPage({
  searchParams,
}: {
  searchParams?: {
    order_id?: string;
    lead_id?: string;
    partner_id?: string;
    to?: string;
    // `newsletter` opens the blast; `tags` (comma-separated ids) aims it, from Marketing.
    audience?: string;
    tags?: string;
  };
}) {
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
  const params = searchParams ?? {};
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
          onSent={(id, newsletterRecipients) => {
            void qc.invalidateQueries({ queryKey: ['emails'] });
            toast.success(
              newsletterRecipients != null
                ? t('newsletterQueuedNote', { count: newsletterRecipients })
                : t('queuedNote'),
            );
            router.replace(`/${locale}/emails/${id}`);
          }}
        />
      </div>
    </AppShell>
  );
}
