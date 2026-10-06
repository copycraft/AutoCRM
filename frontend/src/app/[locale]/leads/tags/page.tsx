'use client';

import { useLocale, useTranslations } from 'next-intl';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { Breadcrumbs } from '@/components/ui/Breadcrumbs';
import { LeadTagManager } from '@/components/leads/LeadTagManager';
import { LostReasonEditor } from '@/components/leads/LostReasonEditor';
import { useAuth } from '@/lib/auth/context';

export default function LeadTagsPage() {
  const t = useTranslations('leadTags');
  const tn = useTranslations('navigation');
  const locale = useLocale();
  const { user } = useAuth();
  return (
    <AppShell>
      <Breadcrumbs items={[{ href: `/${locale}/leads`, label: tn('leads') }, { label: t('title') }]} />
      <PageHeader title={t('title')} />
      <LeadTagManager />
      <LostReasonEditor editable={user?.role === 'admin' || user?.role === 'office'} />
    </AppShell>
  );
}
