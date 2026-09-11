'use client';

import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { OrderForm, orderCreateBody, type OrderFormValues } from '@/components/forms/OrderForm';
import { ordersApi } from '@/lib/api/endpoints';
import { useAuth } from '@/lib/auth/context';

export default function NewOrderPage() {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const { user } = useAuth();

  const create = useMutation({
    mutationFn: (v: OrderFormValues) => ordersApi.create(orderCreateBody(v, user?.id)),
    onSuccess: (order) => {
      void qc.invalidateQueries({ queryKey: ['orders'] });
      router.replace(`/${locale}/orders/${order.id}`);
    },
  });

  return (
    <AppShell>
      <PageHeader title={t('newOrder')} subtitle={t('currencySharedNote')} />
      <OrderForm
        submitLabel={tc('create')}
        onSubmit={(v) => create.mutateAsync(v).then(() => undefined)}
      />
    </AppShell>
  );
}
