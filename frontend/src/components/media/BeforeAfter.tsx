'use client';

// Two photos of the same spot, one over the other, with a handle to wipe between them:
// the vehicle as it arrived on the left, as it leaves on the right. A new scratch shows up
// as the handle passes over it.

import { useState } from 'react';
import { useTranslations } from 'next-intl';

export function BeforeAfter({
  before,
  after,
  beforeLabel,
  afterLabel,
}: {
  before: string;
  after: string;
  beforeLabel?: string;
  afterLabel?: string;
}) {
  const t = useTranslations('media');
  const [split, setSplit] = useState(50);
  return (
    <figure className="space-y-2">
      <div className="relative w-full select-none overflow-hidden rounded-lg border border-steel-200 bg-steel-900">
        {/* Plain img: presigned S3 URLs never match next/image remotePatterns. */}
        <img src={after} alt={afterLabel ?? t('after')} className="block w-full" draggable={false} />
        <div className="absolute inset-0 overflow-hidden" style={{ clipPath: `inset(0 ${100 - split}% 0 0)` }}>
          <img src={before} alt={beforeLabel ?? t('before')} className="block h-full w-full object-cover" draggable={false} />
        </div>
        <div className="pointer-events-none absolute inset-y-0 w-0.5 bg-surface shadow-card" style={{ left: `${split}%` }} aria-hidden />
        <span className="absolute left-2 top-2 rounded bg-steel-900/70 px-2 py-0.5 text-metadata text-surface">
          {beforeLabel ?? t('before')}
        </span>
        <span className="absolute right-2 top-2 rounded bg-steel-900/70 px-2 py-0.5 text-metadata text-surface">
          {afterLabel ?? t('after')}
        </span>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        value={split}
        onChange={(e) => setSplit(Number(e.target.value))}
        className="w-full accent-steel-900"
        aria-label={t('compareSlider')}
      />
    </figure>
  );
}
