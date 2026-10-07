'use client';

// The language this partner's invoice and proforma PDFs come in (0049).

import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { partnerExtrasApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';

export const INVOICE_LANGUAGES = ['hu', 'en', 'de'] as const;

export function InvoiceLanguageSelect({
  partnerId,
  value,
  editable,
}: {
  partnerId: number;
  value: string | null | undefined;
  editable: boolean;
}) {
  const t = useTranslations('statement');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const save = useMutation({
    mutationFn: (language: string | null) => partnerExtrasApi.setInvoiceLanguage(partnerId, language),
    onSuccess: () => {
      toast.success(t('languageSaved'));
      void qc.invalidateQueries({ queryKey: qk.partner(partnerId) });
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });
  if (!editable) {
    return <>{value ? t(`languages.${value}`) : t('languageDefault')}</>;
  }
  return (
    <select
      className="input w-auto"
      aria-label={t('invoiceLanguage')}
      value={value ?? ''}
      disabled={save.isPending}
      onChange={(e) => save.mutate(e.target.value || null)}
    >
      <option value="">{t('languageDefault')}</option>
      {INVOICE_LANGUAGES.map((l) => (
        <option key={l} value={l}>
          {t(`languages.${l}`)}
        </option>
      ))}
    </select>
  );
}
