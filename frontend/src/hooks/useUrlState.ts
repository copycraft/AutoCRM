'use client';

import { useCallback, useEffect, useState } from 'react';

/**
 * URL-synced state without Next's useSearchParams (which needs a Suspense
 * boundary). Reads window.location.search LAZILY in the useState initializer
 * so the first render already has the real filter/page/sort — previously the
 * initial value was always the default and an effect corrected it after
 * mount, firing every list query TWICE (once with wrong params, once with
 * real ones). Writes back with history.replaceState so filters survive
 * reload, back/forward and sharing. SSR-safe: falls back to default.
 */
function readUrlParam(key: string, initial: string): string {
  if (typeof window === 'undefined') return initial;
  try {
    const params = new URLSearchParams(window.location.search);
    const found = params.get(key);
    return found ?? initial;
  } catch {
    return initial;
  }
}

export function useUrlState(key: string, initial: string): [string, (v: string) => void] {
  const [value, setValue] = useState(() => readUrlParam(key, initial));

  // Back/forward buttons change the URL without remounting: stay in sync.
  useEffect(() => {
    const onPop = () => {
      try {
        const params = new URLSearchParams(window.location.search);
        setValue(params.get(key) ?? initial);
      } catch {
        /* best-effort */
      }
    };
    window.addEventListener('popstate', onPop);
    return () => window.removeEventListener('popstate', onPop);
  }, [key, initial]);

  const set = useCallback(
    (next: string) => {
      setValue(next);
      try {
        const params = new URLSearchParams(window.location.search);
        if (next === '' || next === initial) {
          params.delete(key);
        } else {
          params.set(key, next);
        }
        const qs = params.toString();
        window.history.replaceState(null, '', `${window.location.pathname}${qs ? `?${qs}` : ''}`);
      } catch {
        /* best-effort */
      }
    },
    [key, initial],
  );

  return [value, set];
}

/** Boolean stored as "1" in the URL. */
export function useUrlFlag(key: string, initial = false): [boolean, (v: boolean) => void] {
  const [raw, setRaw] = useUrlState(key, initial ? '1' : '');
  return [raw === '1', (v: boolean) => setRaw(v ? '1' : '')];
}

/** Integer stored as decimal string in the URL. */
export function useUrlInt(key: string, initial = 0): [number, (v: number) => void] {
  const [raw, setRaw] = useUrlState(key, String(initial));
  const parsed = Number.parseInt(raw, 10);
  const value = Number.isFinite(parsed) && parsed >= 0 ? parsed : initial;
  return [value, (v: number) => setRaw(String(Math.max(0, Math.floor(v))))];
}
