import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';

export default async function EmailsPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  const tc = await getTranslations({ locale, namespace: 'emptyStates' });
  return (
    <AppShell>
      <PageHeader title={t('emails')} />
      <EmptyState title={tc('noEmails')} hint="6. fázis: levelezés, előnézet, küldés, sablonok, tiltások, needs_review megkülönböztetéssel." />
    </AppShell>
  );
}
