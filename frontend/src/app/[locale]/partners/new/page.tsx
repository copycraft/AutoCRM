'use client';

import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { PartnerForm, partnerCreateBody, type PartnerFormValues } from '@/components/forms/PartnerForm';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';

export default function NewPartnerPage() {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();

  const create = useMutation({
    mutationFn: (v: PartnerFormValues) => partnersApi.create(partnerCreateBody(v)),
    onSuccess: (partner) => {
      void qc.invalidateQueries({ queryKey: ['partners'] });
      router.replace(`/${locale}/partners/${partner.id}`);
    },
  });

  return (
    <AppShell>
      <PageHeader title={t('newPartner')} />
      <PartnerForm submitLabel={tc('create')} onSubmit={(v) => create.mutateAsync(v).then(() => undefined)} />
    </AppShell>
  );
}
