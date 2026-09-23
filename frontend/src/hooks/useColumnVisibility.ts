'use client';

import { useCallback, useEffect, useState } from 'react';
import type { VisibilityState } from '@tanstack/react-table';

function storageKey(listKey: string): string {
  return `autocrm:cols:${listKey}`;
}

/** Per-list column visibility, persisted per device. Empty = all visible. */
export function useColumnVisibility(listKey: string) {
  const [visibility, setVisibility] = useState<VisibilityState>({});

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(storageKey(listKey));
      if (raw) {
        const parsed: unknown = JSON.parse(raw);
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          setVisibility(parsed as VisibilityState);
        }
      }
    } catch {
      /* best-effort */
    }
  }, [listKey]);

  const onChange = useCallback(
    (next: VisibilityState) => {
      setVisibility(next);
      try {
        window.localStorage.setItem(storageKey(listKey), JSON.stringify(next));
      } catch {
        /* best-effort */
      }
    },
    [listKey],
  );

  const reset = useCallback(() => {
    setVisibility({});
    try {
      window.localStorage.removeItem(storageKey(listKey));
    } catch {
      /* best-effort */
    }
  }, [listKey]);

  return { visibility, onChange, reset };
}
