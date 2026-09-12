'use client';

import { useEffect, type ReactNode } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useAuth } from '@/lib/auth/context';
import { Sidebar } from './Sidebar';
import { LoadingState } from '@/components/ui/LoadingState';

export function AppShell({ children }: { children: ReactNode }) {
  const { isAuthenticated, isLoading, user } = useAuth();
  const router = useRouter();
  const locale = useLocale();
  const tc = useTranslations('common');

  useEffect(() => {
    if (!isLoading && !isAuthenticated) router.replace(`/${locale}/login`);
  }, [isLoading, isAuthenticated, router, locale]);

  // Forced password change gate: block app until changed.
  useEffect(() => {
    if (!isLoading && user?.must_change_password) {
      router.replace(`/${locale}/password`);
    }
  }, [isLoading, user, router, locale]);

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-panel">
        <LoadingState label={tc('loading')} />
      </div>
    );
  }
  if (!isAuthenticated) return null;

  return (
    <div className="flex min-h-screen bg-panel">
      <Sidebar />
      <main className="flex-1 min-w-0">
        <div className="w-full max-w-[1600px] px-6 py-6 space-y-6">{children}</div>
      </main>
    </div>
  );
}
