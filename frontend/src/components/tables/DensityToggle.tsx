'use client';

import { useEffect, useState } from 'react';
import { useTranslations } from 'next-intl';

/**
 * Per-device density override. Wins over the server preference when set,
 * cleared back to the server value on second toggle. Stored locally only.
 */
export function useDensityWithOverride(server: string): {
  density: string;
  toggle: () => void;
  overridden: boolean;
} {
  const [override, setOverride] = useState<string | null>(null);

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem('autocrm:density');
      if (raw === 'compact' || raw === 'comfortable') setOverride(raw);
    } catch {
      /* best-effort */
    }
  }, []);

  const toggle = () => {
    setOverride((prev) => {
      const effective = prev ?? server;
      const next = effective === 'compact' ? 'comfortable' : 'compact';
      // Toggling back to the server value clears the override.
      const stored = next === server ? null : next;
      try {
        if (stored === null) window.localStorage.removeItem('autocrm:density');
        else window.localStorage.setItem('autocrm:density', stored);
      } catch {
        /* best-effort */
      }
      return stored;
    });
  };

  return { density: override ?? server, toggle, overridden: override !== null };
}

export function DensityToggle({
  density,
  overridden,
  onToggle,
}: {
  density: string;
  overridden: boolean;
  onToggle: () => void;
}) {
  const t = useTranslations('qol');
  return (
    <button
      type="button"
      className="btn-ghost btn-sm"
      onClick={onToggle}
      title={t('densityToggle')}
      aria-label={t('densityToggle')}
      aria-pressed={density === 'compact'}
    >
      {t(density === 'compact' ? 'densityComfortable' : 'densityCompact')}
      {overridden ? ' ●' : ''}
    </button>
  );
}
