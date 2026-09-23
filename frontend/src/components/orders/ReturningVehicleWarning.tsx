'use client';

// Returning-vehicle warning on order creation: while the plate or VIN is typed, a
// debounced search asks whether this van has been here before. A hit lists the past
// orders with links — retyping a returning van as a stranger loses its history and,
// for warranty jobs, the only proof of what was done last time.

import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { DateDisplay } from '@/components/ui/DateDisplay';

export function ReturningVehicleWarning({
  plate,
  vin,
  active,
}: {
  plate: string;
  vin: string;
  /** Only on creation: editing an order trivially "finds" itself. */
  active: boolean;
}) {
  const t = useTranslations('orders');
  const locale = useLocale();
  const needle = useDebouncedValue((plate.trim() || vin.trim()).trim(), 400);
  const ready = active && needle.length >= 3;

  const past = useQuery({
    queryKey: qk.orders({ returning: needle }),
    queryFn: () => ordersApi.list({ q: needle, limit: 5 }),
    enabled: ready,
  });

  const hits = ready ? (past.data?.items ?? []) : [];
  if (!ready || past.isLoading || hits.length === 0) return null;

  return (
    <div className="md:col-span-2 rounded-lg border border-steel-900 bg-panel p-4" role="status">
      <p className="text-body font-semibold">{t('returningTitle')}</p>
      <ul className="mt-2 space-y-1">
        {hits.map((o) => (
          <li key={o.id} className="text-body">
            <Link href={`/${locale}/orders/${o.id}`} className="font-medium underline">
              #{o.number} · {o.title}
            </Link>{' '}
            <span className="text-steel-500">
              {o.stage_label} · <DateDisplay value={o.created_at} />
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
