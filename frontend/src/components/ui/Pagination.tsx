'use client';

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
  const page = Math.floor(offset / limit) + 1;
  const isLast = loaded < limit;
  return (
    <div className="flex items-center justify-between gap-3">
      <p className="text-metadata text-steel-500 font-mono">
        {offset + 1}–{offset + loaded} · {limit}/oldal
      </p>
      <div className="flex items-center gap-2">
        <button className="btn-ghost btn-sm" onClick={onPrev} disabled={offset === 0}>
          ← Előző
        </button>
        <span className="text-metadata font-mono text-steel-500">{page}. oldal</span>
        <button className="btn-ghost btn-sm" onClick={onNext} disabled={isLast}>
          Következő →
        </button>
      </div>
    </div>
  );
}
