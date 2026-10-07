import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { IncidentLog } from '@/components/incidents/Incidents';

export default async function IncidentsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'incidents' });
  return (
    <AppShell>
      <PageHeader title={t('pageTitle')} subtitle={t('pageSubtitle')} />
      <IncidentLog />
    </AppShell>
  );
}
