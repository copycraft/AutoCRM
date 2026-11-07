'use client';

// Three cards — follow the device, light, dark — each a small picture of the app in that
// look. Choosing one applies it at once, so the person sees what they picked. Used on the
// invitation page and under Beállításaim.

import { Monitor, Moon, Sun } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { cn } from '@/lib/utils/format';
import { applyTheme, THEMES, type Theme } from '@/lib/theme';

const ICONS = { system: Monitor, light: Sun, dark: Moon } as const;

/** A miniature of the app: fixed colours, so each card shows its own look in any theme. */
function Miniature({ dark }: { dark: boolean }) {
  const bg = dark ? '#111619' : '#F7F8F7';
  const card = dark ? '#1B2226' : '#FFFFFF';
  const ink = dark ? '#E6EAEB' : '#1B2327';
  const line = dark ? '#343F45' : '#D5DBDC';
  return (
    <svg viewBox="0 0 120 72" className="h-full w-full" aria-hidden>
      <rect width="120" height="72" fill={bg} />
      <rect width="30" height="72" fill={card} />
      <rect x="6" y="8" width="18" height="4" rx="2" fill={ink} />
      <rect x="6" y="18" width="14" height="3" rx="1.5" fill={line} />
      <rect x="6" y="25" width="16" height="3" rx="1.5" fill={line} />
      <rect x="38" y="8" width="44" height="6" rx="3" fill={ink} />
      <rect x="38" y="20" width="74" height="44" rx="4" fill={card} stroke={line} />
      <rect x="44" y="28" width="40" height="4" rx="2" fill={line} />
      <rect x="44" y="37" width="56" height="4" rx="2" fill={line} />
      <rect x="44" y="50" width="22" height="7" rx="3.5" fill="#E8590C" />
    </svg>
  );
}

export function ThemePicker({ value, onChange }: { value: Theme; onChange: (t: Theme) => void }) {
  const t = useTranslations('theme');
  return (
    <div role="radiogroup" aria-label={t('label')} className="grid grid-cols-1 gap-3 sm:grid-cols-3">
      {THEMES.map((theme) => {
        const Icon = ICONS[theme];
        const selected = value === theme;
        return (
          <button
            key={theme}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => {
              applyTheme(theme);
              onChange(theme);
            }}
            className={cn(
              'flex flex-col overflow-hidden rounded-xl border-2 bg-surface text-left transition-colors',
              selected ? 'border-steel-900' : 'border-steel-200 hover:border-steel-500',
            )}
          >
            <span className="relative block h-20 w-full">
              {theme === 'system' ? (
                // Half and half: whatever the device is set to.
                <span className="absolute inset-0 flex">
                  <span className="h-full w-1/2 overflow-hidden">
                    <span className="block h-full w-[200%]">
                      <Miniature dark={false} />
                    </span>
                  </span>
                  <span className="h-full w-1/2 overflow-hidden">
                    <span className="-ml-[100%] block h-full w-[200%]">
                      <Miniature dark />
                    </span>
                  </span>
                </span>
              ) : (
                <Miniature dark={theme === 'dark'} />
              )}
            </span>
            <span className="flex items-center gap-2 px-3 py-2 text-body font-medium">
              <Icon className="h-4 w-4 shrink-0" aria-hidden />
              {t(theme)}
            </span>
          </button>
        );
      })}
    </div>
  );
}
