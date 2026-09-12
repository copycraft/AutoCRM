'use client';

import { useTranslations } from 'next-intl';

export function LoadingState({ label }: { label?: string }) {
  const tc = useTranslations('common');
  return (
    <div className="flex items-center justify-center gap-3 py-10" role="status" aria-live="polite">
      <span className="h-5 w-5 animate-spin rounded-full border-2 border-steel-200 border-t-steel-900" aria-hidden />
      <span className="text-metadata text-steel-500">{label ?? tc('loading')}</span>
    </div>
  );
}

export function TableSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="table-container p-3 space-y-2" aria-hidden>
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="h-10 animate-pulse rounded-lg bg-panel" />
      ))}
    </div>
  );
}

export function DetailSkeleton() {
  return (
    <div className="space-y-4" aria-hidden>
      <div className="h-8 w-1/3 animate-pulse rounded-lg bg-steel-200" />
      <div className="h-40 animate-pulse rounded-xl bg-surface border border-steel-200" />
      <div className="h-24 animate-pulse rounded-xl bg-surface border border-steel-200" />
    </div>
  );
}
