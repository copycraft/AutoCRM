'use client';

import { useEffect, useState } from 'react';
import type { UseFormReset, UseFormWatch } from 'react-hook-form';

/**
 * Draft autosave for *new-record* forms. Snapshots form values to localStorage
 * on every change, restores them on mount, clears on successful submit.
 * Only active when `key` is set — edit and clone flows never touch drafts.
 */
export function loadDraft<T>(key: string): Partial<T> | null {
  if (typeof window === 'undefined') return null;
  try {
    const raw = window.localStorage.getItem(`autocrm:draft:${key}`);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Partial<T>;
    }
  } catch {
    /* corrupt draft — start blank */
  }
  return null;
}

function saveDraft(key: string, values: unknown): void {
  try {
    window.localStorage.setItem(`autocrm:draft:${key}`, JSON.stringify(values));
  } catch {
    /* storage full/blocked — drafts are best-effort */
  }
}

export function clearDraft(key: string): void {
  try {
    window.localStorage.removeItem(`autocrm:draft:${key}`);
  } catch {
    /* best-effort */
  }
}

export function useFormDraft<TValues extends Record<string, unknown>>(opts: {
  /** Null disables drafting (edit/clone flows). */
  key: string | null;
  watch: UseFormWatch<TValues>;
  reset: UseFormReset<TValues>;
  empty: TValues;
}): { restored: boolean; discard: () => void; clear: () => void } {
  const { key, watch, reset, empty } = opts;
  const [restored, setRestored] = useState(false);

  useEffect(() => {
    if (!key) return;
    // Apply after mount so the first render matches SSR (no hydration mismatch).
    const draft = loadDraft<TValues>(key);
    if (draft) {
      reset({ ...empty, ...draft });
      setRestored(true);
    }
    const sub = watch((values) => {
      saveDraft(key, values);
    });
    return () => sub.unsubscribe();
    // Run once per form instance.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return {
    restored,
    discard: () => {
      if (key) clearDraft(key);
      reset(empty);
      setRestored(false);
    },
    clear: () => {
      if (key) clearDraft(key);
      setRestored(false);
    },
  };
}
