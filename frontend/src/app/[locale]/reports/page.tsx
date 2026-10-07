import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { WorkloadCharts } from '@/components/reports/WorkloadCharts';
import { LeadSourcesReport } from '@/components/reports/LeadSourcesReport';
import { WebsiteConversionReport } from '@/components/reports/WebsiteConversionReport';
import { SalesReports } from '@/components/reports/SalesReports';

export default async function ReportsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'navigation' });
  return (
    <AppShell>
      <PageHeader title={t('reports')} />
      <WorkloadCharts />
      <LeadSourcesReport />
      <WebsiteConversionReport />
      <SalesReports />
    </AppShell>
  );
}
