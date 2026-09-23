'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { Snowflake } from 'lucide-react';
import { useAuth } from '@/lib/auth/context';
import { errorMessage } from '@/lib/api/errors';

const schema = z.object({
  email: z.string().email(),
  password: z.string().min(1),
});
type Form = z.infer<typeof schema>;

export default function LoginPage() {
  const t = useTranslations('auth');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const router = useRouter();
  const { login } = useAuth();
  const [serverError, setServerError] = useState<string | null>(null);
  const [nativeSubmit, setNativeSubmit] = useState(false);
  const { register, handleSubmit, formState } = useForm<Form>({ resolver: zodResolver(schema) });

  // A native form submit (JS dead, stale chunks) GETs the credentials into the URL.
  // Scrub the password back out and say so, instead of leaving it in history.
  useEffect(() => {
    try {
      const params = new URLSearchParams(window.location.search);
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
      const res = await login(v.email.trim(), v.password);
      router.replace(res.user.must_change_password ? `/${locale}/password` : `/${locale}`);
    } catch (e) {
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
          {serverError && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{serverError}</p>
          )}
          {nativeSubmit && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{t('nativeSubmit')}</p>
          )}
          <button className="btn-primary w-full" type="submit" disabled={formState.isSubmitting}>
            {formState.isSubmitting ? t('loggingIn') : t('loginButton')}
          </button>
        </form>
      </div>
    </div>
  );
}
