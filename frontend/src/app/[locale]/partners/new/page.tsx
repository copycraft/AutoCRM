'use client';

import { useRouter, useSearchParams } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { PartnerForm, partnerCreateBody, type PartnerFormValues } from '@/components/forms/PartnerForm';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import type { PartnerKind } from '@/lib/api/types';

function kindParam(value: string | null): PartnerKind | undefined {
  return value === 'business' || value === 'person' ? value : undefined;
}

export default function NewPartnerPage() {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const searchParams = useSearchParams();
  const initialKind = kindParam(searchParams.get('kind'));

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
      <PartnerForm
        initialKind={initialKind}
        submitLabel={tc('create')}
        onSubmit={(v) => create.mutateAsync(v).then(() => undefined)}
      />
    </AppShell>
  );
}
