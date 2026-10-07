'use client';

// Public unsubscribe: no login, no shell — readers arrive from a mail link. A token link
// unsubscribes in one click; without one, the address can be typed in.

import React, { useEffect, useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation } from '@tanstack/react-query';
import { newsletterApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';

export default function UnsubscribePage({
  searchParams,
}: {
  searchParams: Promise<{ token?: string }>;
}) {
  const { token: rawToken } = React.use(searchParams);
  const t = useTranslations('emails');
  const tp = useTranslations('privacy');
  const locale = useLocale();
  const ter = useTranslations('errors');
  const [email, setEmail] = useState('');
  const [done, setDone] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const token = rawToken ?? '';

  const run = useMutation({
    mutationFn: (search: { token?: string; email?: string }) => newsletterApi.unsubscribe(search),
    onSuccess: (r) => {
      setDone(r.unsubscribed);
      setError(null);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  useEffect(() => {
    if (token && done === null && !run.isPending) run.mutate({ token });
    // Once: a token link acts on open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-panel px-4">
      <div className="card w-full max-w-md">
        <div className="card-header">
          <h1 className="text-section font-semibold">{t('unsubscribeTitle')}</h1>
          <p className="text-metadata text-steel-500">AUTOTHERM</p>
        </div>
        <div className="card-content space-y-4">
          {done === true && <p className="text-body">{t('unsubscribedDone')}</p>}
          {done === false && <p className="text-body">{t('unsubscribedAlready')}</p>}
          {done === null && !token && (
            <>
              <p className="text-body text-steel-500">{t('unsubscribeHint')}</p>
              <div>
                <label className="label" htmlFor="unsub-email">{t('to')}</label>
                <input
                  id="unsub-email"
                  type="email"
                  className="input"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="olvaso@example.hu"
                />
              </div>
              <button
                type="button"
                className="btn-primary w-full"
                disabled={email.trim() === '' || run.isPending}
                onClick={() => run.mutate({ email: email.trim() })}
              >
                {run.isPending ? '…' : t('unsubscribeGo')}
              </button>
            </>
          )}
          {done === null && token && run.isPending && <p className="text-body">…</p>}
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
