import type { Blocker } from '@/lib/api/types';

/** A blocker is open while resolved_at is null. */
export function isBlockerOpen(b: Blocker): boolean {
  return b.resolved_at == null;
}

// Overdue comes from the API (`Blocker.is_overdue`, measured in business
// time) — never recompute it from the UTC calendar date here.
