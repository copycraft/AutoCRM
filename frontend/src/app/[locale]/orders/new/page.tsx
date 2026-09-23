'use client';

import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { OrderForm, orderCreateBody, type OrderFormValues } from '@/components/forms/OrderForm';
import type { SpecForm } from '@/components/forms/BuildSpecSection';
import { ordersApi } from '@/lib/api/endpoints';
import { useAuth } from '@/lib/auth/context';
import { rememberLastUsed } from '@/hooks/useLastUsed';

export default function NewOrderPage() {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const { user } = useAuth();

  const create = useMutation({
    // The build spec travels with the order so a failed second request cannot leave a
    // build with no specification.
    mutationFn: ({ v, specForm }: { v: OrderFormValues; specForm: SpecForm | null }) =>
      ordersApi.create(orderCreateBody(v, user?.id, specForm)),
    onSuccess: (order) => {
      void qc.invalidateQueries({ queryKey: ['orders'] });
      router.replace(`/${locale}/orders/${order.id}`);
    },
  });

  return (
    <AppShell>
      <PageHeader title={t('newOrder')} subtitle={t('currencySharedNote')} />
      <OrderForm
        draftKey="order-new"
        submitLabel={tc('create')}
        onSubmit={(v, _contactDirty, specForm) => {
          if (v.assigned_to !== null) rememberLastUsed('assignee', String(v.assigned_to));
          if (v.project_type_id) rememberLastUsed('ptype', v.project_type_id);
          return create.mutateAsync({ v, specForm }).then(() => undefined);
        }}
      />
    </AppShell>
  );
}
