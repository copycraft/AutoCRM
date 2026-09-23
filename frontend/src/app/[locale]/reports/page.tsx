import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { WorkloadCharts } from '@/components/reports/WorkloadCharts';

export default async function ReportsPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  return (
    <AppShell>
      <PageHeader title={t('reports')} />
      <WorkloadCharts />
    </AppShell>
  );
}
