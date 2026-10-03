import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { BillingPage } from '@/components/billing/BillingPage';

export default async function InvoicesPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  return (
    <AppShell>
      <PageHeader title={t('invoices')} />
      <BillingPage />
    </AppShell>
  );
}
