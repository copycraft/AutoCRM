import { redirect } from 'next/navigation';

// Business partners are the partners list; /partners keeps working as an alias.
export default function PartnersPage({ params: { locale } }: { params: { locale: string } }) {
  redirect(`/${locale}/partners/business`);
}
