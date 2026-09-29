'use client';

import { format, formatDistanceToNow } from 'date-fns';
import { hu } from 'date-fns/locale';
import { cn } from '@/lib/utils/format';

// Dates younger than this render relative ("3 nappal ezelőtt") with the
// absolute date on hover; older dates render absolute.
const RECENT_MS = 7 * 24 * 60 * 60 * 1000;
const DATE_ONLY = /^\d{4}-\d{2}-\d{2}$/;

export function DateDisplay({
  value,
  withTime = false,
  className,
}: {
  value: string;
  withTime?: boolean;
  className?: string;
}) {
  // A calendar date ("2026-09-29": due dates, issue dates) has no time of day, so it is
  // read as local midnight — `new Date` would take UTC midnight and shift it for anyone
  // west of Greenwich — and always shown as a date: "10 órával ezelőtt" means nothing
  // for a due date.
  const dateOnly = DATE_ONLY.test(value);
  const d = new Date(dateOnly ? `${value}T00:00:00` : value);
  if (Number.isNaN(d.getTime())) {
    return <span className={cn('font-mono tabular-nums', className)}>—</span>;
  }
  const absolute = format(d, withTime && !dateOnly ? 'yyyy. MM. dd. HH:mm' : 'yyyy. MM. dd.', {
    locale: hu,
  });
  const age = Date.now() - d.getTime();
  const text =
    !dateOnly && age >= 0 && age < RECENT_MS
      ? formatDistanceToNow(d, { locale: hu, addSuffix: true })
      : absolute;
  return (
    <time dateTime={value} title={absolute} className={cn('font-mono tabular-nums', className)}>
      {text}
    </time>
  );
}
