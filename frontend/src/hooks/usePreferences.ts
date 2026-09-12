'use client';

import { useQuery, useQueryClient } from '@tanstack/react-query';
import { authApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';

export const FALLBACK_PAGE_SIZE = 50;
export const FALLBACK_DENSITY = 'comfortable';

export function usePreferences() {
  return useQuery({
    queryKey: qk.preferences,
    queryFn: () => authApi.preferences(),
    staleTime: 5 * 60_000,
    retry: false,
  });
}

export function usePageSize(): number {
  const { data } = usePreferences();
  return data?.page_size ?? FALLBACK_PAGE_SIZE;
}

export function useDensity(): string {
  const { data } = usePreferences();
  return data?.density ?? FALLBACK_DENSITY;
}

export function useInvalidateLists() {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: ['partners'] });
    void qc.invalidateQueries({ queryKey: ['leads'] });
    void qc.invalidateQueries({ queryKey: ['orders'] });
  };
}
