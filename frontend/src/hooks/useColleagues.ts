'use client';

// Everyone's name, from `GET /colleagues`: any signed-in user may read it. Assignee pickers
// and "sent by" / "assigned to" labels use this, never the admin-only `GET /users`, so a
// salesperson sees names instead of "#12". The key sits under ['users'], so whatever
// refreshes the user list after an admin edit refreshes the names too.

import { useQuery } from '@tanstack/react-query';
import { usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import type { components } from '@/lib/api/schema.gen';

export type Colleague = components['schemas']['Colleague'];

export function useColleagues(enabled = true) {
  return useQuery({
    queryKey: qk.colleagues,
    queryFn: () => usersApi.colleagues(),
    enabled,
    // Names change when an admin edits an account, not by the minute.
    staleTime: 5 * 60_000,
  });
}

/** A colleague's name, or `#id` while the list loads or for an account no longer listed. */
export function colleagueName(items: Colleague[] | undefined, id: number): string {
  return items?.find((c) => c.id === id)?.display_name ?? `#${id}`;
}
