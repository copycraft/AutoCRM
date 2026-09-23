/** CSV export of the current filtered view. Values are quoted per RFC 4180;
 *  a BOM is prepended so Hungarian Excel opens UTF-8 directly. */

export function csvCell(value: string | number | null | undefined): string {
  const s = value === null || value === undefined ? '' : String(value);
  return `"${s.replace(/"/g, '""')}"`;
}

export const CSV_BOM = '﻿';

export function downloadCsv(
  filename: string,
  header: string[],
  rows: (string | number | null | undefined)[][],
): void {
  const lines = [
    header.map((h) => csvCell(h)).join(';'),
    ...rows.map((r) => r.map(csvCell).join(';')),
  ];
  const blob = new Blob([CSV_BOM + lines.join('\r\n')], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

/** `orders-2026-09-13.csv` style filename in the Budapest calendar. */
export function csvFilename(base: string): string {
  const day = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Budapest' }).format(new Date());
  return `${base}-${day}.csv`;
}
