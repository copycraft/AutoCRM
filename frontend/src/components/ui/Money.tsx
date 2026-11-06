import { formatMoney } from '@/lib/utils/format';
import type { Currency } from '@/lib/api/types';

export function Money({
  minor,
  currency,
  className,
}: {
  /** Null: the server left the amount out, because this person may not see prices. */
  minor: number | null | undefined;
  currency: Currency;
  className?: string;
}) {
  // Presentation-only. Backend owns totals/FX (docs/history/FRONTEND_PLAN.md §13).
  if (minor == null) {
    return <span className={`text-steel-500 ${className ?? ''}`}>—</span>;
  }
  return (
    <span className={`text-money ${className ?? ''}`} lang="hu">
      {formatMoney(minor, currency)}
    </span>
  );
}
