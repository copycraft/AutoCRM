'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import type { VisibilityState } from '@tanstack/react-table';
import { Columns3 } from 'lucide-react';

/** Checkbox menu to show/hide table columns. Labels come from column headers. */
export function ColumnMenu({
  columns,
  visibility,
  onChange,
  onReset,
}: {
  columns: { id: string; label: string }[];
  visibility: VisibilityState;
  onChange: (next: VisibilityState) => void;
  onReset: () => void;
}) {
  const t = useTranslations('qol');
  const [open, setOpen] = useState(false);
  if (columns.length === 0) return null;

  return (
    <div className="relative">
      <button
        type="button"
        className="btn-ghost btn-sm"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        aria-label={t('columnsTitle')}
        title={t('columnsTitle')}
      >
        <Columns3 className="h-4 w-4" aria-hidden />
        {t('columnsTitle')}
      </button>
      {open && (
        <>
          <button
            aria-label={t('close')}
            className="fixed inset-0 z-40 cursor-default bg-transparent"
            onClick={() => setOpen(false)}
            tabIndex={-1}
          />
          <div className="card absolute right-0 z-50 mt-1 w-56 p-2">
            <ul className="max-h-64 space-y-0.5 overflow-y-auto">
              {columns.map((c) => {
                const visible = visibility[c.id] !== false;
                return (
                  <li key={c.id}>
                    <label className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-body hover:bg-steel-200/30">
                      <input
                        type="checkbox"
                        className="rounded border-steel-200 accent-steel-900"
                        checked={visible}
                        onChange={() =>
                          onChange({ ...visibility, [c.id]: !visible })
                        }
                      />
                      <span className="truncate">{c.label}</span>
                    </label>
                  </li>
                );
              })}
            </ul>
            <div className="mt-1 border-t border-steel-200 pt-1 text-right">
              <button type="button" className="btn-ghost btn-sm" onClick={() => { onReset(); setOpen(false); }}>
                {t('columnsReset')}
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
