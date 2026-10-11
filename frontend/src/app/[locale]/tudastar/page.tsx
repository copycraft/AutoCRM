import { Suspense } from 'react';
import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { KnowledgeBase } from '@/components/knowledge/KnowledgeBase';

export default async function KnowledgePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const tNav = await getTranslations({ locale, namespace: 'navigation' });
  const t = await getTranslations({ locale, namespace: 'knowledge' });
  return (
    <AppShell>
      <PageHeader title={tNav('knowledge')} subtitle={t('subtitle')} />
      <Suspense>
        <KnowledgeBase />
      </Suspense>
    </AppShell>
  );
}
