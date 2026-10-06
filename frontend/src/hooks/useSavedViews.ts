'use client';

import { useCallback, useEffect, useState } from 'react';

export interface SavedView {
  name: string;
  params: Record<string, string>;
  at: number;
}

function storageKey(listKey: string): string {
  return `autocrm:views:${listKey}`;
}

export function useSavedViews(listKey: string) {
  const [views, setViews] = useState<SavedView[]>([]);

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(storageKey(listKey));
      if (raw) {
        const parsed: unknown = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          setViews(
            parsed
              .filter(
                (v): v is SavedView =>
                  typeof v === 'object' &&
                  v !== null &&
                  typeof (v as SavedView).name === 'string' &&
                  typeof (v as SavedView).params === 'object',
              )
              .slice(0, 20),
          );
        }
      }
    } catch {
      setViews([]);
    }
  }, [listKey]);

  const persist = useCallback(
    (next: SavedView[]) => {
      setViews(next);
      try {
        window.localStorage.setItem(storageKey(listKey), JSON.stringify(next));
      } catch {
        /* best-effort */
      }
    },
    [listKey],
  );

  const save = useCallback(
    (name: string, params: Record<string, string>) => {
      const clean = name.trim();
      if (!clean) return;
      persist(
        [{ name: clean, params, at: Date.now() }, ...views.filter((v) => v.name !== clean)].slice(
          0,
          20,
        ),
      );
    },
    [persist, views],
  );

  const remove = useCallback(
    (name: string) => {
      persist(views.filter((v) => v.name !== name));
    },
    [persist, views],
  );

  return { views, save, remove };
}

/** Snapshot the current URLSearchParams down to the keys the list owns. */
export function snapshotParams(keys: string[]): Record<string, string> {
  const out: Record<string, string> = {};
  try {
    const params = new URLSearchParams(window.location.search);
    for (const k of keys) {
      const v = params.get(k);
      if (v !== null && v !== '') out[k] = v;
    }
  } catch {
    /* best-effort */
  }
  return out;
}

/** Apply a saved snapshot by rewriting the query string, then reload state via navigation. */
export function applyParams(params: Record<string, string>) {
  try {
    const next = new URLSearchParams(window.location.search);
    // Clear owned keys first so stale values don't linger.
    for (const k of Object.keys(params)) next.delete(k);
    for (const [k, v] of Object.entries(params)) {
      if (v !== '') next.set(k, v);
    }
    const qs = next.toString();
    window.location.search = qs ? `?${qs}` : window.location.pathname;
  } catch {
    /* best-effort */
  }
}

/**
 * Save a view of a list from anywhere (the assistant builds filters off the list page).
 * The list's bar reads it the next time the list opens.
 */
export function saveViewTo(listKey: string, name: string, params: Record<string, string>): void {
  const clean = name.trim();
  if (!clean) return;
  try {
    const raw = window.localStorage.getItem(storageKey(listKey));
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    const existing = Array.isArray(parsed) ? (parsed as SavedView[]).filter((v) => v?.name !== clean) : [];
    window.localStorage.setItem(
      storageKey(listKey),
      JSON.stringify([{ name: clean, params, at: Date.now() }, ...existing].slice(0, 20)),
    );
  } catch {
    /* best-effort: storage may be off */
  }
}
