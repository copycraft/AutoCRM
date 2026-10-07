'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useForm } from 'react-hook-form';
import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { Snowflake } from 'lucide-react';
import { useAuth } from '@/lib/auth/context';
import { ApiError, errorMessage } from '@/lib/api/errors';
import { accountApi } from '@/lib/api/endpoints';

const schema = z.object({
  email: z.string().email(),
  password: z.string().min(1),
});
type Form = z.infer<typeof schema>;

/** Google's multicolour "G", as its sign-in branding asks for. */
function GoogleLogo() {
  return (
    <svg className="h-[18px] w-[18px]" viewBox="0 0 48 48" aria-hidden>
      <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z" />
      <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z" />
      <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z" />
      <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z" />
    </svg>
  );
}

/**
 * "Sign in with Google". A full navigation, not a fetch: the backend redirects on to
 * Google's account chooser and back to `/api/auth/google/callback`. Shown always; until
 * GOOGLE_CLIENT_ID/GOOGLE_CLIENT_SECRET are set on the server it is disabled.
 */
function GoogleButton({ label, enabled }: { label: string; enabled: boolean }) {
  const look =
    'flex h-10 w-full items-center justify-center gap-3 rounded-lg border border-steel-200 bg-white text-body font-medium text-[#1f1f1f]';
  if (!enabled) {
    return (
      <button type="button" className={`${look} cursor-not-allowed opacity-50`} disabled aria-disabled>
        <GoogleLogo />
        {label}
      </button>
    );
  }
  return (
    <a className={`${look} transition-colors hover:bg-steel-100`} href="/api/auth/google/start">
      <GoogleLogo />
      {label}
    </a>
  );
}

export default function LoginPage() {
  const t = useTranslations('auth');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const router = useRouter();
  const { login } = useAuth();
  const [serverError, setServerError] = useState<string | null>(null);
  const [nativeSubmit, setNativeSubmit] = useState(false);
  // Two-factor accounts: the password was right, now the app's code (0049).
  const [needsCode, setNeedsCode] = useState(false);
  const [code, setCode] = useState('');
  const providers = useQuery({ queryKey: ['auth', 'providers'], queryFn: accountApi.providers, retry: false });
  const { register, handleSubmit, formState } = useForm<Form>({ resolver: zodResolver(schema) });

  // A native form submit (JS dead, stale chunks) GETs the credentials into the URL.
  // Scrub the password back out and say so, instead of leaving it in history.
  useEffect(() => {
    try {
      const params = new URLSearchParams(window.location.search);
      const googleError = params.get('error');
      if (googleError?.startsWith('google_')) {
        const key = `googleErrors.${googleError}`;
        const text = t(key);
        setServerError(text === `auth.${key}` ? t('googleErrors.google_failed') : text);
      }
      if (params.has('password') || params.has('email')) {
        params.delete('password');
        params.delete('email');
        const rest = params.toString();
        router.replace(`/${locale}/login${rest ? `?${rest}` : ''}`);
        setNativeSubmit(true);
      }
    } catch {
      /* non-browser — nothing to scrub */
    }
    // Once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onSubmit = handleSubmit(async (v) => {
    setServerError(null);
    try {
      const res = await login(v.email.trim(), v.password, needsCode ? code.trim() : undefined);
      router.replace(res.user.must_change_password ? `/${locale}/password` : `/${locale}`);
    } catch (e) {
      if (e instanceof ApiError && e.code === 'totp_required') {
        setNeedsCode(true);
        setServerError(null);
        return;
      }
      setServerError(errorMessage(e, ter, t('invalidCredentials')));
    }
  });

  return (
    <div className="flex min-h-screen items-center justify-center bg-panel px-4">
      <div className="card w-full max-w-md">
        <div className="card-header flex items-center gap-3">
          <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-steel-900 text-surface">
            <Snowflake className="h-5 w-5" aria-hidden />
          </span>
          <div>
            <h1 className="text-section font-semibold">{t('loginTitle')}</h1>
            <p className="text-metadata text-steel-500">{t('loginSubtitle')}</p>
          </div>
        </div>
        <form className="card-content space-y-4" onSubmit={onSubmit} noValidate>
          <div>
            <label className="label" htmlFor="email">{t('emailLabel')}</label>
            <input id="email" type="email" autoComplete="username" className="input" {...register('email')} />
            {formState.errors.email && <p className="mt-1 text-metadata text-steel-900">{tv('email')}</p>}
          </div>
          <div>
            <label className="label" htmlFor="password">{t('passwordLabel')}</label>
            <input id="password" type="password" autoComplete="current-password" className="input" {...register('password')} />
          </div>
          {needsCode && (
            <div>
              <label className="label" htmlFor="totp">{t('totpCode')}</label>
              <input
                id="totp"
                className="input font-mono tracking-widest"
                inputMode="numeric"
                autoComplete="one-time-code"
                autoFocus
                maxLength={7}
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
              <p className="mt-1 text-metadata text-steel-500">{t('totpHint')}</p>
            </div>
          )}
          {serverError && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{serverError}</p>
          )}
          {nativeSubmit && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{t('nativeSubmit')}</p>
          )}
          <button className="btn-primary w-full" type="submit" disabled={formState.isSubmitting}>
            {formState.isSubmitting ? t('loggingIn') : t('loginButton')}
          </button>
          <div className="flex items-center gap-3 text-metadata text-steel-500">
            <span className="h-px flex-1 bg-steel-200" />
            {t('or')}
            <span className="h-px flex-1 bg-steel-200" />
          </div>
          <GoogleButton label={t('google')} enabled={providers.data?.google === true} />
          {providers.isSuccess && !providers.data.google && (
            <p className="text-center text-metadata text-steel-500">{t('googleNotConfigured')}</p>
          )}
        </form>
      </div>
    </div>
  );
}
