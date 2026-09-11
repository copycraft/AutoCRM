'use client';

import type { ReactNode } from 'react';

export function FilterBar({ children, onClear }: { children: ReactNode; onClear?: () => void }) {
  return (
    <div className="card">
      <div className="card-content flex flex-wrap items-end gap-3">
        {children}
        {onClear && (
          <button className="btn-ghost btn-sm" onClick={onClear}>
            Szűrők törlése
          </button>
        )}
      </div>
    </div>
  );
}

export function FilterField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex min-w-44 flex-col gap-1">
      <span className="text-metadata font-medium text-steel-500">{label}</span>
      {children}
    </label>
  );
}
