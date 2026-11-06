'use client';

// Repeat order: a new project started as a copy of an earlier one. The data row, the items
// at their old prices, the build spec, each department's technical content and the
// drawings and photos it uses come along; plate, VIN and the earlier job's own records do
// not. Everything copied stays editable on the new project.

import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ordersApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';

type Part = 'items' | 'spec' | 'modules' | 'files';
const PARTS: Part[] = ['items', 'spec', 'modules', 'files'];

export function CopyOrderDialog({
  source,
  onClose,
}: {
  source: { id: number; number: string; title: string };
  onClose: () => void;
}) {
  const t = useTranslations('repeat');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const toast = useToast();
  const [title, setTitle] = useState(source.title);
  const [parts, setParts] = useState<Record<Part, boolean>>({ items: true, spec: true, modules: true, files: true });
  const [error, setError] = useState<string | null>(null);

  const copy = useMutation({
    mutationFn: () => ordersApi.copy(source.id, { title: title.trim() || null, ...parts }),
    onSuccess: (r) => {
      void qc.invalidateQueries({ queryKey: ['orders'] });
      if (r.failed_files.length > 0) {
        toast.error(t('someFilesFailed', { files: r.failed_files.join(', ') }));
      } else {
        toast.success(t('done', { number: r.order.number }));
      }
      onClose();
      router.push(`/${locale}/orders/${r.order.id}`);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  return (
    <Dialog.Root open onOpenChange={(open) => !open && !copy.isPending && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[92vw] max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!copy.isPending) copy.mutate();
            }}
          >
            <div className="card-header">
              <Dialog.Title className="text-section font-semibold">{t('title')}</Dialog.Title>
              <Dialog.Description className="mt-1 text-metadata text-steel-500">
                {t('intro', { number: source.number })}
              </Dialog.Description>
            </div>
            <div className="card-content space-y-4">
              {error && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                  {error}
                </p>
              )}
              <div>
                <label className="label" htmlFor="copy-title">
                  {t('newTitle')}
                </label>
                <input id="copy-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
              </div>
              <fieldset>
                <legend className="label">{t('what')}</legend>
                <ul className="mt-1 space-y-2">
                  {PARTS.map((p) => (
                    <li key={p}>
                      <label className="flex items-center gap-2 text-body">
                        <input
                          type="checkbox"
                          checked={parts[p]}
                          onChange={(e) => setParts((prev) => ({ ...prev, [p]: e.target.checked }))}
                        />
                        {t(`parts.${p}`)}
                      </label>
                    </li>
                  ))}
                </ul>
              </fieldset>
              <p className="text-metadata text-steel-500">{t('notCopied')}</p>
            </div>
            <div className="card-footer justify-end">
              <button type="button" className="btn-ghost" onClick={onClose} disabled={copy.isPending}>
                {tc('cancel')}
              </button>
              <button type="submit" className="btn-primary" disabled={copy.isPending}>
                {copy.isPending ? tc('processing') : t('submit')}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
