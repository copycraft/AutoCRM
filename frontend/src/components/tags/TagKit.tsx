'use client';

// Pieces every tag list shares (lead tags, newsletter tags...): ink that reads on a tag's
// colour, the palette, a swatch button with its palette popover, a small icon button,
// and a searchable grouped checklist for putting tags on a record.

import { useMemo, useState } from 'react';
import { useTranslations } from 'next-intl';
import { Check, Globe, Tags } from 'lucide-react';
import { cn } from '@/lib/utils/format';

/** The colours the MiniCRM lists used, plus a few to tell new tags apart. */
export const PALETTE = [
  '#dbe8ff', '#c3d9ff', '#93bcff', '#5b93f5', '#3d6fd6', '#1f3a75',
  '#5aa05a', '#4f8a52', '#2e4a30', '#1e3320', '#f2a33a', '#d9862e',
  '#7e5219', '#5b3a14', '#5a1a12', '#a33122', '#c9402a', '#e8988a',
  '#f7dcb4', '#8b5cf6', '#dde1e6', '#8f99a6', '#6b7685', '#1f2228',
];

/** Black or white, whichever reads better on `hex`. */
export function inkFor(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? '#111827' : '#ffffff';
}

export function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      className="rounded p-1 text-steel-500 hover:bg-steel-200 hover:text-steel-900 disabled:opacity-30"
      title={label}
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/** The tag's colour square; when editable, a click opens the palette. */
export function SwatchButton({
  color,
  editable,
  label,
  onPick,
}: {
  color: string;
  editable: boolean;
  label: string;
  onPick: (color: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button
        type="button"
        className="block h-5 w-5 shrink-0 rounded ring-1 ring-inset ring-black/10 disabled:cursor-default"
        style={{ backgroundColor: color }}
        disabled={!editable}
        title={label}
        aria-label={label}
        onClick={() => setOpen((o) => !o)}
      />
      {open && (
        <>
          <button
            aria-label={label}
            className="fixed inset-0 z-40 cursor-default bg-transparent"
            onClick={() => setOpen(false)}
            tabIndex={-1}
          />
          <div className="card absolute left-0 z-50 mt-1 grid w-48 grid-cols-6 gap-1 p-2">
            {PALETTE.map((c) => (
              <button
                key={c}
                type="button"
                className="h-6 w-6 rounded ring-1 ring-inset ring-black/10"
                style={{
                  backgroundColor: c,
                  outline: c === color ? `2px solid ${inkFor(c)}` : undefined,
                  outlineOffset: -4,
                }}
                aria-label={c}
                onClick={() => {
                  setOpen(false);
                  onPick(c);
                }}
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

/** A coloured chip. `prefix` is a small code in front (the market of a lead tag). */
export function Chip({
  label,
  color,
  prefix,
  globe,
  title,
  onClick,
  onRemove,
}: {
  label: string;
  color: string;
  prefix?: string;
  globe?: boolean;
  title?: string;
  onClick?: () => void;
  onRemove?: () => void;
}) {
  const body = (
    <>
      {prefix && <span className="font-mono text-[10px] uppercase opacity-70">{prefix}</span>}
      {globe && <Globe className="h-3 w-3 shrink-0" aria-hidden />}
      <span className="truncate">{label}</span>
    </>
  );
  return (
    <span
      className="inline-flex max-w-full items-center gap-1 rounded-md px-1.5 py-0.5 text-metadata font-medium ring-1 ring-inset ring-black/10"
      style={{ backgroundColor: color, color: inkFor(color) }}
      title={title}
    >
      {onClick ? (
        <button type="button" className="inline-flex min-w-0 items-center gap-1 hover:underline" onClick={onClick}>
          {body}
        </button>
      ) : (
        body
      )}
      {onRemove && (
        <button type="button" className="-mr-0.5 rounded px-0.5 opacity-70 hover:opacity-100" onClick={onRemove} aria-label={`× ${label}`}>
          ×
        </button>
      )}
    </span>
  );
}

export interface PickerTag {
  id: number;
  label: string;
  color: string;
  /** Extra words the search also matches (domains...). */
  keywords?: string;
  /** A globe after the label: the tag recognises websites. */
  globe?: boolean;
}

/**
 * A button opening a searchable checklist of tags in titled groups. Each click calls
 * `onChange` with the new selection. `value` may hold ids not listed (archived tags the
 * record still has); they are kept.
 */
export function GroupedTagPicker({
  groups,
  value,
  onChange,
  label,
  disabled,
  align = 'left',
}: {
  groups: { title: string; tags: PickerTag[] }[];
  value: number[];
  onChange: (ids: number[]) => void;
  label: string;
  disabled?: boolean;
  align?: 'left' | 'right';
}) {
  const t = useTranslations('tagKit');
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState('');

  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    if (!needle) return groups;
    return groups
      .map((g) => ({
        ...g,
        tags: g.tags.filter((x) =>
          `${x.label} ${x.keywords ?? ''} ${g.title}`.toLowerCase().includes(needle),
        ),
      }))
      .filter((g) => g.tags.length > 0);
  }, [groups, q]);

  const toggle = (id: number) =>
    onChange(value.includes(id) ? value.filter((v) => v !== id) : [...value, id]);

  return (
    <div className="relative">
      <button
        type="button"
        className="btn-secondary btn-sm"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
      >
        <Tags className="h-4 w-4" aria-hidden />
        {label}
      </button>
      {open && (
        <>
          <button
            aria-label={t('done')}
            className="fixed inset-0 z-40 cursor-default bg-transparent"
            onClick={() => setOpen(false)}
            tabIndex={-1}
          />
          <div
            className={cn(
              'card absolute z-50 mt-1 w-80 max-w-[calc(100vw-2rem)] p-2',
              align === 'right' ? 'right-0' : 'left-0',
            )}
            onKeyDown={(e) => e.key === 'Escape' && setOpen(false)}
          >
            <input
              autoFocus
              className="input mb-2"
              placeholder={t('search')}
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
            <div className="max-h-80 overflow-y-auto">
              {shown.length === 0 && <p className="px-2 py-3 text-metadata text-steel-500">{t('noMatch')}</p>}
              {shown.map((g) => (
                <div key={g.title} className="mb-2">
                  <p className="px-2 pb-1 text-metadata font-semibold text-steel-500">{g.title}</p>
                  <ul>
                    {g.tags.map((tag) => {
                      const on = value.includes(tag.id);
                      return (
                        <li key={tag.id}>
                          <button
                            type="button"
                            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-body hover:bg-steel-200/30"
                            onClick={() => toggle(tag.id)}
                            aria-pressed={on}
                          >
                            <span
                              className="flex h-4 w-4 shrink-0 items-center justify-center rounded ring-1 ring-inset ring-black/10"
                              style={{ backgroundColor: tag.color, color: inkFor(tag.color) }}
                            >
                              {on && <Check className="h-3 w-3" aria-hidden />}
                            </span>
                            <span className={cn('truncate', on && 'font-semibold')}>{tag.label}</span>
                            {tag.globe && <Globe className="ml-auto h-3 w-3 shrink-0 text-steel-500" aria-hidden />}
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                </div>
              ))}
            </div>
            <div className="flex justify-end border-t border-steel-200 pt-2">
              <button type="button" className="btn-primary btn-sm" onClick={() => setOpen(false)}>
                {t('done')}
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
