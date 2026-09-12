'use client';

import { TriangleAlert } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { ApiError, errorMessage } from '@/lib/api/errors';

export function ErrorState({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry?: () => void;
}) {
  const ter = useTranslations('errors');
  const te = useTranslations('emptyStates');
  // A 403 on a read is "you may not view this", not a generic failure.
  const message =
    error instanceof ApiError && error.code === 'forbidden'
      ? te('permissionDenied')
      : errorMessage(error, ter, ter('unknownError'));
  return (
    <div className="flex flex-col items-start justify-center gap-2 rounded-xl border border-steel-200 bg-panel px-6 py-12 text-left" role="alert">
      <TriangleAlert className="h-8 w-8 text-steel-900" aria-hidden />
      <p className="text-body font-medium">{ter('title')}</p>
      <p className="text-body text-steel-900 max-w-md">{message}</p>
      {onRetry && (
        <button className="btn-secondary btn-sm mt-3" onClick={onRetry}>
          {ter('retry')}
        </button>
      )}
    </div>
  );
}
