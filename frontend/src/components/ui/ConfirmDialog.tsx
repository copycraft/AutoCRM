'use client';

import * as Dialog from '@radix-ui/react-dialog';
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
            <Dialog.Title className="text-section font-semibold">{title}</Dialog.Title>
          </div>
          {body && (
            <Dialog.Description asChild>
              <div className="card-content">
                <p className="text-body">{body}</p>
              </div>
            </Dialog.Description>
          )}
          <div className="card-footer justify-end">
            <button className="btn-ghost" onClick={onClose} disabled={busy}>
              {noLabel}
            </button>
            <button className="btn-primary" onClick={onConfirm} disabled={busy}>
              {busy ? tc('processing') : okLabel}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
