'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { Download } from 'lucide-react';
import { useToast } from '@/components/ui/Toasts';
import { errorMessage } from '@/lib/api/errors';

/**
 * Export button for the current filtered view. The caller fetches (up to 200
 * rows — the API max page) and maps to CSV; this owns pending, errors and
 * the success toast.
 */
export function ExportCsvButton({
  base,
  onExport,
}: {
  base: string;
  onExport: () => Promise<{ header: string[]; rows: (string | number | null | undefined)[][]; count: number } | null>;
}) {
  const t = useTranslations('qol');
  const ter = useTranslations('errors');
  const toast = useToast();
  const [pending, setPending] = useState(false);

  const run = async () => {
    setPending(true);
    try {
      const { downloadCsv, csvFilename } = await import('@/lib/utils/csv');
      const result = await onExport();
      if (!result) return;
      downloadCsv(csvFilename(base), result.header, result.rows);
      toast.success(t('exported', { n: result.count }));
    } catch (e) {
      const msg = errorMessage(e, ter, ter('unknownError'));
      toast.error(t('toastError'), msg);
    } finally {
      setPending(false);
    }
  };

  return (
    <button
      type="button"
      className="btn-ghost btn-sm"
      disabled={pending}
      onClick={() => void run()}
      title={t('exportCsv')}
      aria-label={t('exportCsv')}
    >
      <Download className="h-4 w-4" aria-hidden />
      {pending ? t('exporting') : t('exportCsv')}
    </button>
  );
}
