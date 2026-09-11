'use client';

import { format, formatDistanceToNow } from 'date-fns';
import { hu } from 'date-fns/locale';
import { cn } from '@/lib/utils/format';

// Dates younger than this render relative ("3 nappal ezelőtt") with the
// absolute date on hover; older dates render absolute.
const RECENT_MS = 7 * 24 * 60 * 60 * 1000;

export function DateDisplay({
  value,
  withTime = false,
  className,
}: {
  value: string;
  withTime?: boolean;
  className?: string;
}) {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) {
    return <span className={cn('font-mono tabular-nums', className)}>—</span>;
  }
  const absolute = format(d, withTime ? 'yyyy. MM. dd. HH:mm' : 'yyyy. MM. dd.', { locale: hu });
  const age = Date.now() - d.getTime();
  const text =
    age >= 0 && age < RECENT_MS
      ? formatDistanceToNow(d, { locale: hu, addSuffix: true })
      : absolute;
  return (
    <time dateTime={value} title={absolute} className={cn('font-mono tabular-nums', className)}>
      {text}
    </time>
  );
}
