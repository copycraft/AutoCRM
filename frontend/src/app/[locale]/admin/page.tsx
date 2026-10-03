import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { UsersAdmin } from '@/components/admin/UsersAdmin';

export default async function AdminPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'users' });
  return (
    <AppShell>
      <PageHeader title={t('title')} subtitle={t('subtitle')} />
      <UsersAdmin />
    </AppShell>
  );
}
