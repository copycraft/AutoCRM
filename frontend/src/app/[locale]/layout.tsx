import { NextIntlClientProvider } from 'next-intl';
import { getMessages, getTranslations } from 'next-intl/server';
import { notFound } from 'next/navigation';
import { locales, type Locale } from '@/i18n';
import { QueryProvider } from '@/lib/query/provider';
import { AuthProvider } from '@/lib/auth/context';
import { ToastProvider } from '@/components/ui/Toasts';
// Authenticated CRM: every page depends on the session cookie, so nothing
// prerenders. No generateStaticParams: with force-dynamic it would be dead
// configuration suggesting static output that never happens.
export const dynamic = 'force-dynamic';

import type { Metadata } from 'next';

export async function generateMetadata({ params: { locale } }: { params: { locale: string } }): Promise<Metadata> {
  const t = await getTranslations({ locale, namespace: 'common' });
  return { title: `AutoCRM — ${t('order')} / ${t('lead')}` };
}

export default async function LocaleLayout({
  children,
  params: { locale },
}: {
  children: React.ReactNode;
  params: { locale: string };
}) {
  if (!locales.includes(locale as Locale)) notFound();
  const messages = await getMessages();
  return (
    <NextIntlClientProvider messages={messages}>
      <QueryProvider>
        <AuthProvider>
          <ToastProvider>{children}</ToastProvider>
        </AuthProvider>
      </QueryProvider>
    </NextIntlClientProvider>
  );
}
