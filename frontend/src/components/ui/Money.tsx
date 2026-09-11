import { formatMoney } from '@/lib/utils/format';
import type { Currency } from '@/types/api';

export function Money({
  minor,
  currency,
  className,
}: {
  minor: number;
  currency: Currency;
  className?: string;
}) {
  // Presentation-only. Backend owns totals/FX (§22, §87).
  return (
    <span className={`text-money ${className ?? ''}`} lang="hu">
      {formatMoney(minor, currency)}
    </span>
  );
}
