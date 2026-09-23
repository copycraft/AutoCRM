'use client';

import { useEffect, useRef } from 'react';

/**
 * Focus restoration for controlled dialogs (no Radix Trigger to return to).
 * Remembers the opener on `open`, gives focus back on close.
 */
export function useRestoreFocus(open: boolean) {
  const prev = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (open) {
      const active = document.activeElement;
      prev.current = active instanceof HTMLElement ? active : null;
    } else if (prev.current && document.contains(prev.current)) {
      prev.current.focus();
      prev.current = null;
    }
  }, [open ]);
}
