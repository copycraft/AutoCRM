import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';

export default async function BlockersPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  const tc = await getTranslations({ locale, namespace: 'emptyStates' });
  return (
    <AppShell>
      <PageHeader title={t('blockers')} />
      <EmptyState title={tc('noBlockers')} hint="4. fázis: akadály lista, létrehozás, megoldás, újranyitás, határidők, felelősség, nudge-állapot — valós backend műveletekkel." />
    </AppShell>
  );
}
