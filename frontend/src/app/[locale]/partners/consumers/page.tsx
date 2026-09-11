'use client';

import { useTranslations } from 'next-intl';
import { PartnerList } from '@/components/partners/PartnerList';

export default function ConsumersPage() {
  const t = useTranslations('navigation');
  const tp = useTranslations('partners');
  return (
    <PartnerList
      kind="person"
      title={t('consumers')}
      newLabel={tp('newConsumer')}
      emptyTitle={tp('noConsumers')}
    />
  );
}
