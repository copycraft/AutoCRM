'use client';

import { useCallback, useEffect, useState } from 'react';
import { useAuth } from '@/lib/auth/context';

const MAX_DEFAULT = 8;

/**
 * The storage key for this person: on a shared workshop PC one person's recent searches
 * and records (an enquiry, a customer) must not show up for the next (0061).
 */
function usePersonalKey(key: string): string {
  const { user } = useAuth();
  return `${key}:u${user?.id ?? 'anon'}`;
}

/** Non-sensitive localStorage list (search queries, visited record refs). */
export function useRecent(baseKey: string, max: number = MAX_DEFAULT) {
  const key = usePersonalKey(baseKey);
  const [items, setItems] = useState<string[]>([]);

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(key);
      if (raw) {
        const parsed: unknown = JSON.parse(raw);
        if (Array.isArray(parsed)) setItems(parsed.filter((x) => typeof x === 'string').slice(0, max));
      }
    } catch {
      setItems([]);
    }
  }, [key, max]);

  const push = useCallback(
    (value: string) => {
      const v = value.trim();
      if (!v) return;
      setItems((prev) => {
        const next = [v, ...prev.filter((x) => x !== v)].slice(0, max);
        try {
          window.localStorage.setItem(key, JSON.stringify(next));
        } catch {
          /* storage full/blocked — recent lists are best-effort */
        }
        return next;
      });
    },
    [key, max],
  );

  const clear = useCallback(() => {
    setItems([]);
    try {
      window.localStorage.removeItem(key);
    } catch {
      /* best-effort */
    }
  }, [key]);

  return { items, push, clear };
}

export interface RecentRecord {
  href: string;
  title: string;
  sub?: string;
  at: number;
}

/** Recently visited records (orders/leads/partners). Titles + ids only, no PII beyond names. */
export function useRecentRecords(max = 6) {
  const key = usePersonalKey('autocrm:recent-records');
  const [records, setRecords] = useState<RecentRecord[]>([]);

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(key);
      if (raw) {
        const parsed: unknown = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          setRecords(
            parsed
              .filter(
                (r): r is RecentRecord =>
                  typeof r === 'object' &&
                  r !== null &&
                  typeof (r as RecentRecord).href === 'string' &&
                  typeof (r as RecentRecord).title === 'string',
              )
              .slice(0, max),
          );
        }
      }
    } catch {
      setRecords([]);
    }
  }, [key, max]);

  const push = useCallback(
    (rec: Omit<RecentRecord, 'at'>) => {
      setRecords((prev) => {
        const next = [{ ...rec, at: Date.now() }, ...prev.filter((x) => x.href !== rec.href)].slice(
          0,
          max,
        );
        try {
          window.localStorage.setItem(key, JSON.stringify(next));
        } catch {
          /* best-effort */
        }
        return next;
      });
    },
    [key, max],
  );

  const clear = useCallback(() => {
    setRecords([]);
    try {
      window.localStorage.removeItem(key);
    } catch {
      /* best-effort */
    }
  }, [key]);

  return { records, push, clear };
}
