import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { HrWorkspace } from '@/components/hr/HrWorkspace';

export default async function HrPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'hr' });
  return (
    <AppShell>
      <PageHeader title={t('title')} subtitle={t('subtitle')} />
      <HrWorkspace />
    </AppShell>
  );
}
