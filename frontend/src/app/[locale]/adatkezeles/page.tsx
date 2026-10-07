// Public: no login and no CRM shell, because people arrive here from a link in a letter, an
// offer or a website form. The controller's details come from the server environment at
// request time, so they can be changed without a rebuild.

import { getTranslations } from 'next-intl/server';
import type { Metadata } from 'next';
import { PrivacyContent, type PrivacyCompany } from '@/components/privacy/PrivacyContent';

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'privacy' });
  return { title: `${t('title')} | Autotherm` };
}

/** A blank variable counts as not set. */
function env(name: string): string | undefined {
  const value = process.env[name]?.trim();
  return value ? value : undefined;
}

export default function PrivacyPage() {
  const company: PrivacyCompany = {
    name: env('PRIVACY_CONTROLLER_NAME'),
    address: env('PRIVACY_CONTROLLER_ADDRESS'),
    registration: env('PRIVACY_CONTROLLER_REGISTRATION'),
    email: env('PRIVACY_CONTACT_EMAIL'),
    phone: env('PRIVACY_CONTACT_PHONE'),
    noticeUrl: env('PRIVACY_NOTICE_URL'),
  };
  return (
    <div className="min-h-screen bg-panel">
      <PrivacyContent company={company} />
    </div>
  );
}
