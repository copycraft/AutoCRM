import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { UnavailableState } from '@/components/ui/UnavailableState';

export default async function BlockersPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'navigation' });
  const tu = await getTranslations({ locale, namespace: 'unavailable' });
  return (
    <AppShell>
      <PageHeader title={t('blockers')} />
      <UnavailableState title={tu('title')} body={tu('body')} />
    </AppShell>
  );
}
