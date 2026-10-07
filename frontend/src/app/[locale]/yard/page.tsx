'use client';

import { useTranslations } from 'next-intl';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { YardBoard } from '@/components/yard/YardBoard';
import { canAdmin, canChangeStage, useAuth } from '@/lib/auth/context';

export default function YardPage() {
  const t = useTranslations('yard');
  const { user } = useAuth();
  return (
    <AppShell>
      <PageHeader title={t('title')} subtitle={t('subtitle')} />
      <YardBoard canMove={canChangeStage(user)} canConfigure={canAdmin(user)} />
    </AppShell>
  );
}
