'use client';

// Every client-facing enumeration in one document, from `GET /config/lookups`.
// Selects, chips and labels render from this hook; nothing in it is duplicated
// in the web client. Unknown keys read as themselves (spaced out), the same
// fallback the phone uses, so a value added server-side never renders blank.

import { useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { configApi } from '@/lib/api/endpoints';
import { setServerErrorTexts } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';
import type { components } from '@/lib/api/schema.gen';

export type Lookups = components['schemas']['Lookups'];
export type LookupItem = components['schemas']['LookupItem'];

export function humanizeKey(key: string): string {
  const spaced = key.replace(/_/g, ' ').trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

export function lookupLabel(items: LookupItem[] | undefined, key: string): string {
  const label = items?.find((i) => i.key === key)?.label_hu?.trim();
  return label ? label : humanizeKey(key);
}

export function useLookups() {
  // Enumerations change with server deploys, not by the minute: cache hard.
  const query = useQuery({
    queryKey: qk.lookups,
    queryFn: () => configApi.lookups(),
    staleTime: 10 * 60_000,
  });
  // Side channel: fresh error texts layer over the `errors` catalogue in
  // `errorMessage`, so a reworded message needs no web deploy.
  useEffect(() => {
    if (query.data) setServerErrorTexts(query.data.error_texts);
  }, [query.data]);
  return query;
}
