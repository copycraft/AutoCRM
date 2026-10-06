'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { ClipboardCopy, Download, FileSpreadsheet, FileText, Sheet } from 'lucide-react';
import { useToast } from '@/components/ui/Toasts';
import { errorMessage } from '@/lib/api/errors';

export type ExportRows = {
  header: string[];
  rows: (string | number | null | undefined)[][];
  count: number;
};

/**
 * Every row of a paged list: asks for pages of 200 until one comes back short. `max`
 * guards against exporting a runaway list by accident.
 */
export async function collectAll<T>(
  page: (offset: number, limit: number) => Promise<{ items: T[] }>,
  max = 20_000,
): Promise<T[]> {
  const out: T[] = [];
  for (let offset = 0; offset < max; offset += 200) {
    const { items } = await page(offset, 200);
    out.push(...items);
    if (items.length < 200) break;
  }
  return out;
}

type Format = 'xlsx' | 'csv' | 'sheets' | 'copy';

function download(name: string, blob: Blob) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

/**
 * Export the current filtered view as Excel, CSV, into Google Sheets (copied, then a new
 * sheet opens to paste into), or to the clipboard. The caller fetches the rows; this owns
 * the menu, pending state, errors and the toast.
 */
export function ExportMenu({
  base,
  onExport,
}: {
  base: string;
  onExport: () => Promise<ExportRows | null>;
}) {
  const t = useTranslations('qol');
  const ter = useTranslations('errors');
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);

  const run = async (format: Format) => {
    setOpen(false);
    // Opened before the await, or the browser treats the tab as an unrequested popup.
    const sheetsTab = format === 'sheets' ? window.open('about:blank', '_blank') : null;
    setPending(true);
    try {
      const result = await onExport();
      if (!result) {
        sheetsTab?.close();
        return;
      }
      const { csvFilename, downloadCsv } = await import('@/lib/utils/csv');
      const { buildXlsx, toTsv } = await import('@/lib/utils/xlsx');
      const stem = csvFilename(base).replace(/\.csv$/, '');
      if (format === 'csv') {
        downloadCsv(`${stem}.csv`, result.header, result.rows);
        toast.success(t('exported', { n: result.count }));
      } else if (format === 'xlsx') {
        const bytes = buildXlsx(base, result.header, result.rows);
        download(
          `${stem}.xlsx`,
          new Blob([bytes as Uint8Array<ArrayBuffer>], { type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' }),
        );
        toast.success(t('exported', { n: result.count }));
      } else {
        await navigator.clipboard.writeText(toTsv(result.header, result.rows));
        if (sheetsTab) {
          sheetsTab.location.href = 'https://sheets.new';
          toast.success(t('exportSheets', { n: result.count }));
        } else {
          toast.success(t('exportCopied', { n: result.count }));
        }
      }
    } catch (e) {
      sheetsTab?.close();
      toast.error(t('toastError'), errorMessage(e, ter, ter('unknownError')));
    } finally {
      setPending(false);
    }
  };

  const items: { format: Format; label: string; Icon: typeof Download }[] = [
    { format: 'xlsx', label: t('exportXlsx'), Icon: FileSpreadsheet },
    { format: 'csv', label: t('exportCsvFile'), Icon: FileText },
    { format: 'sheets', label: t('exportGoogle'), Icon: Sheet },
    { format: 'copy', label: t('exportCopy'), Icon: ClipboardCopy },
  ];

  return (
    <div className="relative">
      <button
        type="button"
        className="btn-ghost btn-sm"
        disabled={pending}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <Download className="h-4 w-4" aria-hidden />
        {pending ? t('exporting') : t('export')}
      </button>
      {open && (
        <>
          <button
            aria-label={t('close')}
            className="fixed inset-0 z-40 cursor-default bg-transparent"
            onClick={() => setOpen(false)}
            tabIndex={-1}
          />
          <ul className="card absolute right-0 z-50 mt-1 w-56 p-1" role="menu">
            {items.map(({ format, label, Icon }) => (
              <li key={format}>
                <button
                  type="button"
                  role="menuitem"
                  className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-body hover:bg-steel-200/40"
                  onClick={() => void run(format)}
                >
                  <Icon className="h-4 w-4 text-steel-500" aria-hidden />
                  {label}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/** The old name: every list that had the CSV button now gets the whole menu. */
export const ExportCsvButton = ExportMenu;
