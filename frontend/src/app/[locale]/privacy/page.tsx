import { getTranslations } from 'next-intl/server';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { PrivacyContent, type PrivacyCompany } from '@/components/privacy/PrivacyContent';

function env(name: string): string | undefined {
  const value = process.env[name]?.trim();
  return value ? value : undefined;
}

export default async function PrivacyPage() {
  const t = await getTranslations('privacy');
  const company: PrivacyCompany = {
    name: env('PRIVACY_CONTROLLER_NAME'),
    address: env('PRIVACY_CONTROLLER_ADDRESS'),
    registration: env('PRIVACY_CONTROLLER_REGISTRATION'),
    email: env('PRIVACY_CONTACT_EMAIL'),
    phone: env('PRIVACY_CONTACT_PHONE'),
    noticeUrl: env('PRIVACY_NOTICE_URL'),
  };
  return (
    <AppShell>
      <PageHeader title={t('title')} />
      <PrivacyContent company={company} />
    </AppShell>
  );
}
