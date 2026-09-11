import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';

export default async function SettingsPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'common' });
  return (
    <AppShell>
      <PageHeader title={t('settings')} />
      <EmptyState title="Beállítások" hint="8. fázis: kill switch, rate limit, küldési ablak, nudge-ritmus, értesítések — GET/PUT /settings." />
    </AppShell>
  );
}
