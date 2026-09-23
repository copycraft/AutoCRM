'use client';

import { useTranslations } from 'next-intl';

export interface FilterChip {
  key: string;
  label: string;
  onRemove: () => void;
}

/** Removable pills for each active filter — one click drops one filter, not all. */
export function ActiveFilterChips({ chips }: { chips: FilterChip[] }) {
  const t = useTranslations('qol');
  if (chips.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-2" aria-label={t('activeFilters')}>
      <span className="text-metadata font-medium text-steel-500">{t('activeFilters')}:</span>
      {chips.map((chip) => (
        <span
          key={chip.key}
          className="inline-flex items-center gap-1.5 rounded-full border border-steel-900/30 bg-surface px-2.5 py-0.5 text-metadata"
        >
          <span className="font-medium">{chip.label}</span>
          <button
            type="button"
            onClick={chip.onRemove}
            aria-label={t('removeFilter', { label: chip.label })}
            className="text-steel-500 hover:text-steel-900"
          >
            ×
          </button>
        </span>
      ))}
    </div>
  );
}
