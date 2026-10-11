'use client';

import { useEffect, useState } from 'react';

/**
 * Route id that works on both shapes of Next params:
 * - older / test callers pass `{ id }` directly,
 * - Next 16 passes `Promise<{ id }>` to client components.
 *
 * Returns `null` while a promised id is still resolving (render a skeleton),
 * the id string once known. React 18 has no `use()` hook, so this unwraps
 * the promise in an effect instead.
 */
export function useRouteId(params: { id: string } | Promise<{ id: string }>): string | null {
  const [id, setId] = useState<string | null>(() => {
    if (params && typeof params === 'object' && 'id' in params) {
      const v = (params as { id: unknown }).id;
      return typeof v === 'string' ? v : null;
    }
    return null;
  });

  useEffect(() => {
    let dead = false;
    const p = params as unknown;
    if (p && typeof (p as Promise<{ id: string }>).then === 'function') {
      (p as Promise<{ id: string }>)
        .then((v) => {
          if (!dead) setId(v?.id ?? null);
        })
        .catch(() => {
          if (!dead) setId(null);
        });
    } else if (p && typeof p === 'object' && 'id' in (p as Record<string, unknown>)) {
      const v = (p as Record<string, unknown>).id;
      setId(typeof v === 'string' ? v : null);
    }
    return () => {
      dead = true;
    };
  }, [params]);

  return id;
}
