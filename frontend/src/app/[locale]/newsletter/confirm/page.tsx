'use client';

// Public confirmation (double opt-in): no login, no shell — readers arrive from the link
// in the confirmation letter. Opening the link confirms; the address joins the list only
// here, never at signup.

import React, { useEffect, useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation } from '@tanstack/react-query';
import { newsletterApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';

export default function ConfirmPage({
  searchParams,
}: {
  searchParams: Promise<{ token?: string }>;
}) {
  const { token: rawToken } = React.use(searchParams);
  const t = useTranslations('emails');
  const tp = useTranslations('privacy');
  const locale = useLocale();
  const ter = useTranslations('errors');
  const [confirmed, setConfirmed] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const token = rawToken ?? '';

  const run = useMutation({
    mutationFn: (tok: string) => newsletterApi.confirm(tok),
    onSuccess: (r) => {
      setConfirmed(r.confirmed);
      setError(null);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  useEffect(() => {
    if (token && confirmed === null && !run.isPending) run.mutate(token);
    // Once: the link acts on open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-panel px-4">
      <div className="card w-full max-w-md">
        <div className="card-header">
          <h1 className="text-section font-semibold">{t('confirmTitle')}</h1>
          <p className="text-metadata text-steel-500">AUTOTHERM</p>
        </div>
        <div className="card-content space-y-4">
          {!token && <p className="text-body">{t('confirmMissing')}</p>}
          {token && confirmed === null && !error && <p className="text-body">…</p>}
          {confirmed === true && <p className="text-body">{t('confirmedDone')}</p>}
          {confirmed === false && <p className="text-body">{t('confirmInvalid')}</p>}
          {error && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
              {error}
            </p>
          )}
        </div>
        <div className="card-footer">
          <a className="text-metadata text-steel-500 underline" href={`/${locale}/adatkezeles`}>
            {tp('link')}
          </a>
        </div>
      </div>
    </div>
  );
}
