import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { UnavailableState } from '@/components/ui/UnavailableState';

export default async function DashboardPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  const tu = await getTranslations({ locale, namespace: 'unavailable' });
  return (
    <AppShell>
      <PageHeader title={t('dashboard')} />
      <UnavailableState title={tu('title')} body={tu('body')} />
    </AppShell>
  );
}
