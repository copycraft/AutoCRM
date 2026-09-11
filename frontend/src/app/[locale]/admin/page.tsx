import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';

export default async function AdminPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  return (
    <AppShell>
      <PageHeader title={t('admin')} />
      <EmptyState title="Adminisztráció" hint="8. fázis: felhasználók, munkamenetek, fázisdefiníciók, projekttípusok, rendszerállapot, jobok, FX-letöltés — admin képességgel." />
    </AppShell>
  );
}
