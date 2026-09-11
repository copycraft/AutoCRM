import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';

export default async function ReportsPage({ params: { locale } }: { params: { locale: string } }) {
  const t = await getTranslations({ locale, namespace: 'navigation' });
  return (
    <AppShell>
      <PageHeader title={t('reports')} />
      <EmptyState title="Nincs elérhető jelentés." hint="7. fázis: forgalom, fázis-időtartamok, átjutás, elakadt, akadály-terhelés, árfolyamok — backend riport-API-val, MNB normalizálással." />
    </AppShell>
  );
}
