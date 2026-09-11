'use client';

import { useEffect, useRef } from 'react';
import { useTranslations } from 'next-intl';

export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onClose,
  busy = false,
}: {
  open: boolean;
  title: string;
  body?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onClose: () => void;
  busy?: boolean;
}) {
  const tc = useTranslations('common');
  const okLabel = confirmLabel ?? tc('confirm');
  const noLabel = cancelLabel ?? tc('cancel');
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    confirmRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      onClick={onClose}
    >
      <div className="card w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <div className="card-header">
          <h2 className="text-section font-semibold">{title}</h2>
        </div>
        {body && (
          <div className="card-content">
            <p className="text-sm">{body}</p>
          </div>
        )}
        <div className="card-footer justify-end">
          <button className="btn-ghost" onClick={onClose} disabled={busy}>
            {noLabel}
          </button>
          <button ref={confirmRef} className="btn-primary" onClick={onConfirm} disabled={busy}>
            {busy ? tc('processing') : okLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
