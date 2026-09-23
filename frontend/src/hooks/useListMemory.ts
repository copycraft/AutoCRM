'use client';

import { useEffect } from 'react';

/**
 * Remembers the last list URL (path + query, i.e. filters included) in the
 * session so detail pages can link back to the exact filtered list.
 */
export function useRememberList(listKey: string) {
  useEffect(() => {
    try {
      const url = window.location.pathname + window.location.search;
      // Only remember real list URLs, not detail/new pages sharing the hook.
      window.sessionStorage.setItem(`autocrm:last-list:${listKey}`, url);
    } catch {
      /* best-effort */
    }
  });
}

export function lastListUrl(listKey: string, fallback: string): string {
  try {
    return window.sessionStorage.getItem(`autocrm:last-list:${listKey}`) ?? fallback;
  } catch {
    return fallback;
  }
}
