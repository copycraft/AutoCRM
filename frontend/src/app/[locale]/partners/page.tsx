import { redirect } from 'next/navigation';

// Business partners are the partners list; /partners keeps working as an alias.
export default async function PartnersPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  redirect(`/${locale}/partners/business`);
}
