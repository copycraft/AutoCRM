import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Format integer minor units for display using integer arithmetic only —
 * never divide into floating point. Whole-part grouping comes from Intl;
 * the fractional part is appended as a string.
 *
 * HUF fillér are shown when nonzero (the backend stores HUF with exponent 2
 * and conversions can leave nonzero fillér); whole-forint amounts render
 * the Hungarian way, without decimals.
 */
export function formatMoney(minorUnits: number, currency: 'HUF' | 'EUR', locale: 'hu-HU' | 'en-US' = 'hu-HU'): string {
  const negative = minorUnits < 0;
  const abs = negative ? -minorUnits : minorUnits;
  const frac = abs % 100;
  const whole = (abs - frac) / 100;
  const grouped = new Intl.NumberFormat(locale, { useGrouping: true }).format(whole);
  const sign = negative ? '-' : '';
  if (locale === 'hu-HU') {
    if (currency === 'HUF') {
      const amount = frac === 0 ? grouped : `${grouped},${String(frac).padStart(2, '0')}`;
      return `${sign}${amount} Ft`;
    }
    return `${sign}${grouped},${String(frac).padStart(2, '0')} €`;
  }
  if (currency === 'HUF') {
    const amount = frac === 0 ? grouped : `${grouped}.${String(frac).padStart(2, '0')}`;
    return `${sign}HUF ${amount}`;
  }
  return `${sign}€${grouped}.${String(frac).padStart(2, '0')}`;
}

/**
 * Parse user-typed major units ("4 850 000", "4850000", "12 400,50", "-99,99")
 * to minor units exactly: split on the separator, pad or truncate the
 * fractional part to 2 digits, combine as integers. Never multiply by 100
 * in floating point. Negative values are allowed (discount lines).
 * Returns null when the input is not a number or exceeds safe-integer range.
 */
export function parseMajorToMinor(input: string): number | null {
  const s = input.trim().replace(/\s/g, '').replace(',', '.');
  const m = /^(-)?(?:(\d+)(?:\.(\d*))?|\.(\d+))$/.exec(s);
  if (!m) return null;
  const negative = m[1] === '-';
  const whole = m[2] ?? '0';
  const frac = (m[3] ?? m[4] ?? '').slice(0, 2).padEnd(2, '0');
  const minor = BigInt(whole) * BigInt(100) + BigInt(frac);
  const signed = negative ? -minor : minor;
  if (
    signed > BigInt(Number.MAX_SAFE_INTEGER) ||
    signed < BigInt(-Number.MAX_SAFE_INTEGER)
  ) {
    return null;
  }
  return Number(signed);
}

/** Minor units to an exact major-unit string for editing (4850000 → "48500", 366 → "3,66"). */
export function minorToMajorString(minorUnits: number): string {
  if (!Number.isInteger(minorUnits)) return '';
  const negative = minorUnits < 0;
  const abs = negative ? -minorUnits : minorUnits;
  const frac = abs % 100;
  const whole = (abs - frac) / 100;
  const body = frac === 0 ? String(whole) : `${whole},${String(frac).padStart(2, '0')}`;
  return negative ? `-${body}` : body;
}

export function formatDate(date: string | Date, locale: 'hu-HU' | 'en-US' = 'hu-HU'): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  return new Intl.DateTimeFormat(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(d);
}

export function formatDateTime(date: string | Date, locale: 'hu-HU' | 'en-US' = 'hu-HU'): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  return new Intl.DateTimeFormat(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(d);
}

/** Whole days since an ISO timestamp — lead age, days-in-stage display. */
export function daysSince(iso: string, now: number = Date.now()): number {
  const diff = now - new Date(iso).getTime();
  if (!Number.isFinite(diff) || diff < 0) return 0;
  return Math.floor(diff / 86_400_000);
}

export function truncate(str: string, length: number): string {
  if (str.length <= length) return str;
  return `${str.slice(0, length)}…`;
}