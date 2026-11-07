import { WelcomeForm } from '@/components/account/WelcomeForm';

// The page an invitation letter's link opens (0066). No AppShell, no login: the link is the
// proof, and finishing here signs the browser in.
export default async function WelcomePage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  return <WelcomeForm token={token} />;
}
