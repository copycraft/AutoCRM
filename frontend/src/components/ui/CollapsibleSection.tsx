'use client';

import { useEffect, useState, type ReactNode } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils/format';

/**
 * Section with a toggleable body. Open state persists per device so long
 * pages (dashboard, order detail) stay the way the user left them.
 */
export function CollapsibleSection({
  storageKey,
  title,
  count,
  defaultOpen = true,
  children,
}: {
  storageKey: string;
  title: string;
  count?: number;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(`autocrm:section:${storageKey}`);
      if (raw === '0') setOpen(false);
      else if (raw === '1') setOpen(true);
    } catch {
      /* best-effort */
    }
  }, [storageKey]);

  const toggle = () => {
    setOpen((prev) => {
      const next = !prev;
      try {
        window.localStorage.setItem(`autocrm:section:${storageKey}`, next ? '1' : '0');
      } catch {
        /* best-effort */
      }
      return next;
    });
  };

  return (
    <section aria-label={title}>
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        className="flex items-center gap-2 text-left"
      >
        <ChevronDown
          className={cn('h-4 w-4 shrink-0 text-steel-500 transition-transform', !open && '-rotate-90')}
          aria-hidden
        />
        <span className="text-section font-semibold">{title}</span>
        {count !== undefined && (
          <span className="rounded-full bg-steel-200/60 px-2 py-0.5 font-mono text-metadata text-steel-900">
            {count}
          </span>
        )}
      </button>
      {open && <div className="mt-3 space-y-2">{children}</div>}
    </section>
  );
}
