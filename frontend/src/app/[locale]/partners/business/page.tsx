'use client';

import { useTranslations } from 'next-intl';
import { PartnerList } from '@/components/partners/PartnerList';

export default function BusinessPage() {
  const t = useTranslations('navigation');
  const tp = useTranslations('partners');
  return (
    <PartnerList
      kind="business"
      title={t('business')}
      newLabel={tp('newBusiness')}
      emptyTitle={tp('noBusiness')}
    />
  );
}
