import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';

export default async function DashboardPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  return (
    <AppShell>
      <PageHeader title={t('dashboard')} subtitle="Autotherm belső áttekintése — valós adatokkal a következő fázisokban." />
      <EmptyState
        title="Irányítópult — 1. fázis váza"
        hint="Valós backend-integráció (partnerek, lehetőségek, megrendelések, akadályok, riportok) a 2–8. fázisban érkezik. Üres állapot szándékos: nincs kamu adat."
      />
    </AppShell>
  );
}
