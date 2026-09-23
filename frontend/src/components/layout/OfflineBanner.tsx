'use client';

import { useEffect, useState } from 'react';
import { onlineManager, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { WifiOff } from 'lucide-react';

/** Offline banner: TanStack pauses queries while offline; this says so + retries. */
export function OfflineBanner() {
  const t = useTranslations('qol');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [online, setOnline] = useState(true);

  useEffect(() => {
    setOnline(onlineManager.isOnline());
    const unsub = onlineManager.subscribe(setOnline);
    return unsub;
  }, []);

  if (online) return null;

  return (
    <div
      role="alert"
      className="sticky top-0 z-[90] flex items-center gap-2 bg-steel-900 px-4 py-2 text-surface"
    >
      <WifiOff className="h-4 w-4 shrink-0" aria-hidden />
      <p className="flex-1 text-body">{t('offline')}</p>
      <button
        type="button"
        className="btn-secondary btn-sm"
        onClick={() => {
          void qc.invalidateQueries();
        }}
      >
        {ter('retry')}
      </button>
    </div>
  );
}
