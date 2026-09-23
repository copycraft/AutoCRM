'use client';

import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { authApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';

export const FALLBACK_PAGE_SIZE = 50;
export const FALLBACK_DENSITY = 'comfortable';

export function usePreferences() {
  const query = useQuery({
    queryKey: qk.preferences,
    queryFn: () => authApi.preferences(),
    staleTime: 5 * 60_000,
    gcTime: 30 * 60_000,
    retry: false,
  });
  // Seed localStorage so the next reload can use the real page size on the
  // very first render instead of the fallback (which would refetch the list
  // the moment preferences arrive).
  const data = query.data;
  useEffect(() => {
    if (!data) return;
    try {
      window.localStorage.setItem(
        'autocrm:prefs',
        JSON.stringify({ page_size: data.page_size, density: data.density }),
      );
    } catch {
      /* best-effort */
    }
  }, [data]);
  return query;
}

function cachedPref(): { page_size?: number; density?: string } | null {
  if (typeof window === 'undefined') return null;
  try {
    const raw = window.localStorage.getItem('autocrm:prefs');
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === 'object') return parsed as { page_size?: number; density?: string };
    return null;
  } catch {
    return null;
  }
}

export function usePageSize(): number {
  const { data } = usePreferences();
  if (typeof data?.page_size === 'number') return data.page_size;
  // Synchronous seed: first paint already uses the last known size, so the
  // list query key is stable and doesn't refetch when preferences land.
  return cachedPref()?.page_size ?? FALLBACK_PAGE_SIZE;
}

export function useDensity(): string {
  const { data } = usePreferences();
  if (typeof data?.density === 'string') return data.density;
  return cachedPref()?.density ?? FALLBACK_DENSITY;
}

export function useInvalidateLists() {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: ['partners'] });
    void qc.invalidateQueries({ queryKey: ['leads'] });
    void qc.invalidateQueries({ queryKey: ['orders'] });
  };
}
