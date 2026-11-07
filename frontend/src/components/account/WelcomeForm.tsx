'use client';

// Setting up one's account from the invitation letter (0066): pick how the app looks, set
// a password, and land on the dashboard signed in. The look applies the moment it is
// picked, so the rest of the page is already in it.

import Link from 'next/link';
import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Snowflake } from 'lucide-react';
import { onboardingApi } from '@/lib/api/endpoints';
import { ApiError, errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';
import { LoadingState } from '@/components/ui/LoadingState';
import { ThemePicker } from '@/components/account/ThemePicker';
import type { Theme } from '@/lib/theme';

/** Matches MIN_PASSWORD_CHARS in service/auth.rs; the server checks it again. */
const MIN_PASSWORD = 12;

function Frame({ title, subtitle, children }: { title: string; subtitle?: string; children: React.ReactNode }) {
  return (
    <div className="flex min-h-screen items-start justify-center bg-panel px-4 py-10 sm:items-center">
      <div className="card w-full max-w-2xl">
        <div className="card-header flex items-center gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-steel-900 text-surface">
            <Snowflake className="h-5 w-5" aria-hidden />
          </span>
          <div>
            <h1 className="text-section font-semibold">{title}</h1>
            {subtitle && <p className="text-metadata text-steel-500">{subtitle}</p>}
          </div>
        </div>
        {children}
      </div>
    </div>
  );
}

export function WelcomeForm({ token }: { token: string }) {
  const t = useTranslations('welcome');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const info = useQuery({ queryKey: ['onboarding', token], queryFn: () => onboardingApi.info(token), retry: false });
  const [theme, setTheme] = useState<Theme>('system');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [touched, setTouched] = useState(false);

  const accept = useMutation({
    mutationFn: () => onboardingApi.accept(token, { password, theme }),
    onSuccess: (res) => {
      // Signed in by the cookie the answer set: the app opens without a login.
      qc.setQueryData(qk.me, { user: res.user });
      void qc.invalidateQueries({ queryKey: qk.preferences });
      router.replace(`/${locale}`);
    },
  });

  if (info.isLoading) {
    return (
      <Frame title={t('title')}>
        <div className="card-content">
          <LoadingState />
        </div>
      </Frame>
    );
  }

  if (info.isError || !info.data) {
    const code = info.error instanceof ApiError ? info.error.code : null;
    const message =
      code === 'invite_used' ? t('used') : code === 'invite_expired' ? t('expired') : t('invalid');
    return (
      <Frame title={t('deadTitle')}>
        <div className="card-content space-y-4">
          <p className="text-body">{message}</p>
          <Link className="btn-secondary" href={`/${locale}/login`}>
            {t('toLogin')}
          </Link>
        </div>
      </Frame>
    );
  }

  const short = password.length > 0 && password.length < MIN_PASSWORD;
  const mismatch = confirm.length > 0 && confirm !== password;
  const ready = password.length >= MIN_PASSWORD && confirm === password;
  return (
    <Frame title={t('greeting', { name: info.data.display_name })} subtitle={t('subtitle', { email: info.data.email })}>
      <form
        className="card-content space-y-6"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          setTouched(true);
          if (ready) accept.mutate();
        }}
      >
        <section className="space-y-3">
          <div>
            <h2 className="text-body font-semibold">{t('stepTheme')}</h2>
            <p className="text-metadata text-steel-500">{t('stepThemeHint')}</p>
          </div>
          <ThemePicker value={theme} onChange={setTheme} />
        </section>

        <section className="space-y-3">
          <div>
            <h2 className="text-body font-semibold">{t('stepPassword')}</h2>
            <p className="text-metadata text-steel-500">{t('stepPasswordHint', { n: MIN_PASSWORD })}</p>
          </div>
          {/* For the browser's password manager: which account this password belongs to. */}
          <input type="email" autoComplete="username" value={info.data.email} readOnly hidden />
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <label className="label" htmlFor="welcome-password">{t('password')}</label>
              <input
                id="welcome-password"
                type="password"
                autoComplete="new-password"
                className="input"
                value={password}
                aria-invalid={touched && short ? true : undefined}
                onChange={(e) => setPassword(e.target.value)}
              />
              {touched && short && (
                <p className="mt-1 text-metadata text-steel-900">{t('tooShort', { n: MIN_PASSWORD })}</p>
              )}
            </div>
            <div>
              <label className="label" htmlFor="welcome-confirm">{t('confirm')}</label>
              <input
                id="welcome-confirm"
                type="password"
                autoComplete="new-password"
                className="input"
                value={confirm}
                aria-invalid={mismatch ? true : undefined}
                onChange={(e) => setConfirm(e.target.value)}
              />
              {mismatch && <p className="mt-1 text-metadata text-steel-900">{t('mismatch')}</p>}
            </div>
          </div>
        </section>

        {accept.isError && (
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
            {accept.error instanceof ApiError && accept.error.code === 'invite_used'
              ? t('used')
              : accept.error instanceof ApiError && accept.error.code === 'invite_expired'
                ? t('expired')
                : errorMessage(accept.error, ter, ter('unknownError'))}
          </p>
        )}

        <button className="btn-primary w-full" type="submit" disabled={accept.isPending || (touched && !ready)}>
          {accept.isPending ? t('saving') : t('submit')}
        </button>
        <p className="text-center text-metadata text-steel-500">{t('expires', { date: new Date(info.data.expires_at).toLocaleDateString('hu-HU') })}</p>
      </form>
    </Frame>
  );
}
