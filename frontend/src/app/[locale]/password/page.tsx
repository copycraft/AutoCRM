'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { authApi } from '@/lib/api/endpoints';
import { isApiError } from '@/lib/api/errors';

export default function PasswordPage() {
  const t = useTranslations('auth');
  const locale = useLocale();
  const router = useRouter();
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (next.length < 12) {
      setError('A jelszó túl gyenge (min 12 karakter)');
      return;
    }
    setBusy(true);
    try {
      await authApi.changePassword(current, next);
      router.replace(`/${locale}`);
    } catch (err) {
      setError(isApiError(err) ? err.backendMessage : 'Hiba történt.');
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-panel px-4">
      <form className="card w-full max-w-md" onSubmit={submit}>
        <div className="card-header">
          <h1 className="text-section font-semibold">{t('changePassword')}</h1>
          <p className="text-sm text-steel-500">{t('passwordChangeRequired')}</p>
        </div>
        <div className="card-content space-y-4">
          <div>
            <label className="label" htmlFor="cur">Jelenlegi jelszó</label>
            <input id="cur" type="password" className="input" value={current} onChange={(e) => setCurrent(e.target.value)} required />
          </div>
          <div>
            <label className="label" htmlFor="nxt">{t('newPassword')}</label>
            <input id="nxt" type="password" className="input" value={next} onChange={(e) => setNext(e.target.value)} required minLength={12} />
          </div>
          {error && <p className="rounded-lg bg-signal/10 px-3 py-2 text-sm text-signal" role="alert">{error}</p>}
          <button className="btn-primary w-full" disabled={busy}>{busy ? 'Mentés…' : t('changePassword')}</button>
        </div>
      </form>
    </div>
  );
}
