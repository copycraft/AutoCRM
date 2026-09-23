'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useQueryClient } from '@tanstack/react-query';
import { authApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';

export default function PasswordPage() {
  const t = useTranslations('auth');
  const tc = useTranslations('common');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (next.length < 12) {
      setError(tv('weakPassword'));
      return;
    }
    setBusy(true);
    try {
      await authApi.changePassword({ current_password: current, new_password: next });
      // The AppShell gate reads cached qk.me: without invalidation it still sees
      // must_change_password and bounces straight back here (redirect loop).
      await qc.invalidateQueries({ queryKey: qk.me });
      router.replace(`/${locale}`);
    } catch (err) {
      setError(errorMessage(err, ter, ter('unknownError')));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-panel px-4">
      <form className="card w-full max-w-md" onSubmit={submit}>
        <div className="card-header">
          <h1 className="text-section font-semibold">{t('changePassword')}</h1>
          <p className="text-metadata text-steel-500">{t('passwordChangeRequired')}</p>
        </div>
        <div className="card-content space-y-4">
          <div>
            <label className="label" htmlFor="cur">{t('currentPassword')}</label>
            <input id="cur" type="password" className="input" value={current} onChange={(e) => setCurrent(e.target.value)} required />
          </div>
          <div>
            <label className="label" htmlFor="nxt">{t('newPassword')}</label>
            <input id="nxt" type="password" className="input" value={next} onChange={(e) => setNext(e.target.value)} required minLength={12} />
          </div>
          {error && <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{error}</p>}
          <button className="btn-primary w-full" disabled={busy}>{busy ? tc('saving') : t('changePassword')}</button>
        </div>
      </form>
    </div>
  );
}
