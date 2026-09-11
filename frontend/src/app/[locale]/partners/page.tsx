import { redirect } from 'next/navigation';

// Consumers are the default partners view; /partners keeps working as an alias.
export default function PartnersPage({ params: { locale } }: { params: { locale: string } }) {
  redirect(`/${locale}/partners/consumers`);
}
