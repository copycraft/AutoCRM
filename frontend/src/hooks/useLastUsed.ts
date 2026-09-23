'use client';

/**
 * Last-used create defaults (assignee, project type). Written on successful
 * create, read as the default on the next blank form. Per device, no PII
 * beyond a user id the user already sees.
 */

export function rememberLastUsed(key: 'assignee' | 'ptype', value: string): void {
  if (!value) return;
  try {
    window.localStorage.setItem(`autocrm:last:${key}`, value);
  } catch {
    /* best-effort */
  }
}

export function lastUsed(key: 'assignee' | 'ptype'): string | null {
  if (typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(`autocrm:last:${key}`);
  } catch {
    return null;
  }
}

/** Stored assignee ('me', a numeric id, or null) back to form shape. */
export function lastAssignee(): number | 'me' | null {
  const raw = lastUsed('assignee');
  if (raw === 'me') return 'me';
  if (raw !== null && /^\d+$/.test(raw)) return Number(raw);
  return null;
}
