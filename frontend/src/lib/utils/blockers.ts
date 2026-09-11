import type { Blocker } from '@/lib/api/types';

/** A blocker is open while resolved_at is null (backend has no flag). */
export function isBlockerOpen(b: Blocker): boolean {
  return b.resolved_at == null;
}

/** Overdue = past due date and still open. Client-side display only. */
export function isBlockerOverdue(b: Blocker, today: string = new Date().toISOString().slice(0, 10)): boolean {
  return b.resolved_at == null && b.due_date != null && b.due_date < today;
}
