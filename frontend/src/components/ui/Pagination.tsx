'use client';

import { useTranslations } from 'next-intl';

export function Pagination({
  offset,
  limit,
  loaded,
  onPrev,
  onNext,
}: {
  offset: number;
  limit: number;
  /** Items loaded on this page — fewer than limit means last page. */
  loaded: number;
  onPrev: () => void;
  onNext: () => void;
}) {
  const t = useTranslations('pagination');
  const page = Math.floor(offset / limit) + 1;
  const isLast = loaded < limit;
  return (
    <div className="flex items-center justify-between gap-3">
      <p className="text-metadata text-steel-500 font-mono">
        {loaded === 0 ? t('empty') : t('range', { from: offset + 1, to: offset + loaded, limit })}
      </p>
      <div className="flex items-center gap-2">
        <button className="btn-ghost btn-sm" onClick={onPrev} disabled={offset === 0}>
          ← {t('previous')}
        </button>
        <span className="text-metadata font-mono text-steel-500">{page}. {t('page')}</span>
        <button className="btn-ghost btn-sm" onClick={onNext} disabled={isLast}>
          {t('next')} →
        </button>
      </div>
    </div>
  );
}
