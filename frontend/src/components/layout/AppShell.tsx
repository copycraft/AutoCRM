'use client';

import { AssistantPanel } from '@/components/assistant/AssistantPanel';
import { useEffect, useState, type ReactNode } from 'react';
import dynamic from 'next/dynamic';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useAuth } from '@/lib/auth/context';
import { Sidebar } from './Sidebar';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { OfflineBanner } from './OfflineBanner';
import { ShortcutHelpDialog, useGlobalShortcuts } from '@/hooks/useGlobalShortcuts';

// CommandPalette pulls in radix-dialog + search + debounce. It is only needed
// when the user hits Ctrl+K, so split it out — it was inflating every page's
// JS and delaying first paint for a feature used a few times a day.
const CommandPalette = dynamic(
  () => import('@/components/search/CommandPalette').then((m) => m.CommandPalette),
  { ssr: false },
);

/** Instant shell chrome: sidebar + main skeleton shown while auth resolves.
 *  Previously the whole app was a bare spinner (or blank `null`) until
 *  GET /auth/me returned, so every reload looked frozen. */
function ShellSkeleton() {
  return (
    <div className="flex min-h-screen bg-panel" aria-hidden>
      <div className="w-60 shrink-0 animate-pulse border-r border-steel-200 bg-surface">
        <div className="border-b border-steel-200 px-5 py-4">
          <div className="h-9 w-3/4 rounded-lg bg-panel" />
        </div>
        <div className="space-y-2 p-3">
          {Array.from({ length: 7 }).map((_, i) => (
            <div key={i} className="h-9 rounded-lg bg-panel" />
          ))}
        </div>
      </div>
      <main className="min-w-0 flex-1">
        <div className="w-full max-w-[1600px] space-y-6 px-6 py-6">
          <div className="h-8 w-1/4 animate-pulse rounded-lg bg-steel-200" />
          <DetailSkeleton />
        </div>
      </main>
    </div>
  );
}

export function AppShell({ children }: { children: ReactNode }) {
  const { isAuthenticated, isLoading, user } = useAuth();
  const router = useRouter();
  const locale = useLocale();
  const tq = useTranslations('qol');
  const [helpOpen, setHelpOpen] = useState(false);

  useGlobalShortcuts({ onHelp: () => setHelpOpen(true) });

  useEffect(() => {
    if (!isLoading && !isAuthenticated) router.replace(`/${locale}/login`);
  }, [isLoading, isAuthenticated, router, locale]);

  // Forced password change gate: block app until changed.
  useEffect(() => {
    if (!isLoading && user?.must_change_password) {
      router.replace(`/${locale}/password`);
    }
  }, [isLoading, user, router, locale]);

  // `?` opens help from anywhere, including outside inputs handled above.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const target = e.target as HTMLElement | null;
        const typing =
          target &&
          (target.tagName === 'INPUT' ||
            target.tagName === 'TEXTAREA' ||
            target.tagName === 'SELECT' ||
            target.isContentEditable);
        if (!typing) setHelpOpen(true);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  if (isLoading) {
    // Auth still resolving: paint the shell immediately so the page never
    // looks frozen. Children (with their own skeletons) start fetching in
    // parallel instead of waiting behind /auth/me.
    return <ShellSkeleton />;
  }
  // Redirecting to login: keep the skeleton up instead of flashing blank.
  if (!isAuthenticated) return <ShellSkeleton />;

  return (
    <div className="flex min-h-screen bg-panel">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-[200] focus:rounded-lg focus:bg-surface focus:px-4 focus:py-2 focus:text-body focus:font-medium"
      >
        {tq('skipToContent')}
      </a>
      <Sidebar onHelp={() => setHelpOpen(true)} />
      <main id="main-content" tabIndex={-1} className="flex-1 min-w-0 focus:outline-none">
        <OfflineBanner />
        <div className="w-full max-w-[1600px] px-6 py-6 space-y-6">{children}</div>
      </main>
      <ShortcutHelpDialog open={helpOpen} onClose={() => setHelpOpen(false)} />
      <CommandPalette />
      <AssistantPanel />
    </div>
  );
}
