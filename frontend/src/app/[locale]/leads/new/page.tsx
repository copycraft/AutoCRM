'use client';

import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { LeadForm, leadCreateBody, type LeadFormValues } from '@/components/forms/LeadForm';
import { leadsApi } from '@/lib/api/endpoints';
import { useAuth } from '@/lib/auth/context';

export default function NewLeadPage() {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const { user } = useAuth();

  const create = useMutation({
    mutationFn: (v: LeadFormValues) => leadsApi.create(leadCreateBody(v, user?.id)),
    onSuccess: (lead) => {
      void qc.invalidateQueries({ queryKey: ['leads'] });
      router.replace(`/${locale}/leads/${lead.id}`);
    },
  });

  return (
    <AppShell>
      <PageHeader title={t('newLead')} />
      <LeadForm submitLabel={tc('create')} onSubmit={(v) => create.mutateAsync(v).then(() => undefined)} />
    </AppShell>
  );
}
