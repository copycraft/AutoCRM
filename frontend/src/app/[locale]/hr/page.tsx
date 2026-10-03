import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmployeeDirectory } from '@/components/hr/EmployeeDirectory';

export default async function HrPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'hr' });
  return (
    <AppShell>
      <PageHeader title={t('title')} subtitle={t('subtitle')} />
      <EmployeeDirectory />
    </AppShell>
  );
}
