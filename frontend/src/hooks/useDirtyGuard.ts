'use client';

import { useEffect, useRef } from 'react';

/**
 * Unsaved-changes guard for the big record forms. Covers reload/tab close
 * (beforeunload) and in-app link clicks (capture phase, same-origin only).
 * Programmatic router.push (e.g. after a successful save) is untouched, and
 * the guard is driven off `dirty`, so it goes quiet once the save lands.
 */
export function useDirtyGuard(dirty: boolean, message: string) {
  const ref = useRef({ dirty, message });
  ref.current = { dirty, message };

  useEffect(() => {
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      if (!ref.current.dirty) return;
      e.preventDefault();
    };
    const onClick = (e: MouseEvent) => {
      if (!ref.current.dirty) return;
      if (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey || e.button !== 0) return;
      const el = e.target as HTMLElement | null;
      const anchor = el?.closest?.('a[href]');
      if (!anchor) return;
      const href = anchor.getAttribute('href');
      if (!href || href.startsWith('#')) return;
      let url: URL;
      try {
        url = new URL(href, window.location.href);
      } catch {
        return;
      }
      if (url.origin !== window.location.origin) return;
      if (!window.confirm(ref.current.message)) {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    window.addEventListener('beforeunload', onBeforeUnload);
    document.addEventListener('click', onClick, true);
    return () => {
      window.removeEventListener('beforeunload', onBeforeUnload);
      document.removeEventListener('click', onClick, true);
    };
  }, []);
}
