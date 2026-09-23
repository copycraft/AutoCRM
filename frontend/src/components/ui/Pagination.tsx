'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';

export function Pagination({
  offset,
  limit,
  loaded,
  onPrev,
  onNext,
  onJump,
}: {
  offset: number;
  limit: number;
  /** Items loaded on this page — fewer than limit means last page. */
  loaded: number;
  onPrev: () => void;
  onNext: () => void;
  /** Jump to an exact 1-based page. No totals exist (backend gap), so the target is unchecked. */
  onJump?: (page: number) => void;
}) {
  const t = useTranslations('pagination');
  const tq = useTranslations('qol');
  const page = Math.floor(offset / limit) + 1;
  const isLast = loaded < limit;
  const [draft, setDraft] = useState('');

  const jump = () => {
    const n = Number.parseInt(draft, 10);
    if (Number.isFinite(n) && n >= 1 && onJump) {
      onJump(Math.floor(n));
      setDraft('');
    }
  };

  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p className="text-metadata text-steel-500 font-mono">
        {loaded === 0 ? t('empty') : t('range', { from: offset + 1, to: offset + loaded, limit })}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <button className="btn-ghost btn-sm" onClick={onPrev} disabled={offset === 0}>
          ← {t('previous')}
        </button>
        <span className="text-metadata font-mono text-steel-500">{page}. {t('page')}</span>
        <button className="btn-ghost btn-sm" onClick={onNext} disabled={isLast}>
          {t('next')} →
        </button>
        {onJump && (
          <span className="inline-flex items-center gap-1">
            <input
              className="input w-16 !px-2 !py-1 font-mono"
              inputMode="numeric"
              value={draft}
              onChange={(e) => setDraft(e.target.value.replace(/[^0-9]/g, ''))}
              onKeyDown={(e) => {
                if (e.key === 'Enter') jump();
              }}
              placeholder={tq('pageJump')}
              aria-label={tq('pageJump')}
            />
            <button className="btn-ghost btn-sm" onClick={jump} disabled={draft === ''}>
              {tq('pageGo')}
            </button>
          </span>
        )}
      </div>
    </div>
  );
}
