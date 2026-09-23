// Streaming fallback: shown by Next.js while the server prepares the locale
// layout (getMessages + metadata). Previously there was no loading.tsx at all,
// so every navigation was a blank freeze until the server + /auth/me + page
// queries all resolved.
import { DetailSkeleton } from '@/components/ui/LoadingState';

export default function LocaleLoading() {
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
