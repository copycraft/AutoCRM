import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { NotificationList } from '@/components/notifications/NotificationList';

export default async function NotificationsPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'notifications' });
  return (
    <AppShell>
      <PageHeader title={t('title')} subtitle={t('subtitle')} />
      <NotificationList />
    </AppShell>
  );
}
