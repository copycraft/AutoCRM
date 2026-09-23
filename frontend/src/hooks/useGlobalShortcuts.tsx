'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useLocale, useTranslations } from 'next-intl';
import { usePathname, useRouter } from 'next/navigation';
import { useRestoreFocus } from '@/hooks/useRestoreFocus';

export function ShortcutHelpDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useTranslations('qol');
  useRestoreFocus(open);
  const rows: [string, string][] = [
    ['/', t('scSearch')],
    ['g o', t('scOrders')],
    ['g l', t('scLeads')],
    ['g d', t('scDashboard')],
    ['g e', t('scEmails')],
    ['g b', t('scBoard')],
    ['n', t('scNew')],
    ['?', t('scHelp')],
    ['Esc', t('scClose')],
  ];
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[85vh] w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <div className="card-header">
            <Dialog.Title className="text-section font-semibold">{t('shortcutsTitle')}</Dialog.Title>
          </div>
          <div className="card-content">
            <ul className="space-y-2">
              {rows.map(([k, v]) => (
                <li key={k} className="flex items-center justify-between gap-4 text-body">
                  <span className="text-steel-500">{v}</span>
                  <kbd className="rounded border border-steel-200 bg-panel px-2 py-0.5 font-mono text-metadata">
                    {k}
                  </kbd>
                </li>
              ))}
            </ul>
          </div>
          <div className="card-footer justify-end">
            <button className="btn-ghost" onClick={onClose}>
              {t('close')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/**
 * Global speed keys. `/` is handled by GlobalSearch itself; this owns `g x`
 * sequences, `n` (new record on list pages) and `?` (this help). Never fires
 * while typing in inputs, textareas or contentEditable.
 */
export function useGlobalShortcuts(opts: { onHelp: () => void }) {
  const router = useRouter();
  const pathname = usePathname();
  const locale = useLocale();
  const pendingG = useRef<number>(0);
  const { onHelp } = opts;
  const helpRef = useRef(onHelp);
  helpRef.current = onHelp;

  const go = useCallback(
    (href: string) => {
      router.push(href);
    },
    [router],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing =
        target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable);
      if (typing) return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;

      const now = Date.now();
      if (pendingG.current && now - pendingG.current > 900) pendingG.current = 0;

      if (e.key === '?') {
        e.preventDefault();
        helpRef.current();
        return;
      }
      if (e.key === 'g') {
        pendingG.current = now;
        return;
      }
      if (pendingG.current) {
        const seq = e.key.toLowerCase();
        pendingG.current = 0;
        if (seq === 'o') go(`/${locale}/orders`);
        else if (seq === 'l') go(`/${locale}/leads`);
        else if (seq === 'd' || seq === 'h') go(`/${locale}`);
        else if (seq === 'e') go(`/${locale}/emails`);
        else if (seq === 'b') go(`/${locale}/board`);
        else if (seq === 'r') go(`/${locale}/reports`);
        return;
      }
      if (e.key === 'n') {
        // Contextual "new": order/lead/partner list → its create page.
        if (pathname.includes('/orders')) go(`/${locale}/orders/new`);
        else if (pathname.includes('/leads')) go(`/${locale}/leads/new`);
        else if (pathname.includes('/partners')) go(`/${locale}/partners/new`);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [go, locale, pathname]);
}
