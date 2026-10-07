'use client';

// The own-account cards on the preferences page (0049): the signature put under hand-written
// mail, the private calendar link, and two-factor sign-in with an authenticator app.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CalendarDays, KeyRound, PenLine } from 'lucide-react';
import { accountApi, authApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { CopyButton } from '@/components/ui/CopyButton';
import type { UserSettings } from '@/lib/api/types';

function Alert({ text }: { text: string | null }) {
  if (!text) return null;
  return (
    <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
      {text}
    </p>
  );
}

export function SignatureCard({ initial }: { initial: UserSettings }) {
  const t = useTranslations('account');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [text, setText] = useState(initial.email_signature ?? '');
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: () => authApi.savePreferences({ email_signature: text }),
    onSuccess: (prefs) => {
      setError(null);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
      qc.setQueryData(qk.preferences, prefs);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <section className="card">
      <div className="card-header flex items-center gap-2">
        <PenLine className="h-4 w-4 text-steel-500" aria-hidden />
        <h2 className="text-section">{t('signatureTitle')}</h2>
      </div>
      <div className="card-content space-y-2">
        <p className="text-metadata text-steel-500">{t('signatureIntro')}</p>
        <textarea
          className="input min-h-[120px] font-mono text-body"
          value={text}
          maxLength={2000}
          onChange={(e) => setText(e.target.value)}
          placeholder={t('signaturePlaceholder')}
          aria-label={t('signatureTitle')}
        />
      </div>
      <div className="card-footer flex-wrap justify-between gap-3">
        <span className="text-metadata text-steel-500">{saved ? t('saved') : ''}</span>
        <div className="flex gap-2">
          <Alert text={error} />
          <button className="btn-primary" disabled={save.isPending} onClick={() => save.mutate()}>
            {save.isPending ? tc('saving') : tc('save')}
          </button>
        </div>
      </div>
    </section>
  );
}

export function CalendarCard() {
  const t = useTranslations('account');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const feed = useQuery({ queryKey: ['account', 'calendar'], queryFn: accountApi.calendar });
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const create = useMutation({
    mutationFn: accountApi.createCalendar,
    onSuccess: (data) => {
      setError(null);
      qc.setQueryData(['account', 'calendar'], data);
    },
    onError,
  });
  const remove = useMutation({
    mutationFn: accountApi.deleteCalendar,
    onSuccess: () => {
      setError(null);
      qc.setQueryData(['account', 'calendar'], { url: null });
    },
    onError,
  });
  const url = feed.data?.url ?? null;
  return (
    <section className="card">
      <div className="card-header flex items-center gap-2">
        <CalendarDays className="h-4 w-4 text-steel-500" aria-hidden />
        <h2 className="text-section">{t('calendarTitle')}</h2>
      </div>
      <div className="card-content space-y-3">
        <p className="text-metadata text-steel-500">{t('calendarIntro')}</p>
        {url ? (
          <div className="flex items-center gap-2">
            <input className="input font-mono text-metadata" readOnly value={url} aria-label={t('calendarTitle')} />
            <CopyButton value={url} label={t('copyLink')} />
          </div>
        ) : (
          <p className="text-body text-steel-700">{t('calendarNone')}</p>
        )}
        {url && <p className="text-metadata text-steel-500">{t('calendarSecret')}</p>}
      </div>
      <div className="card-footer flex-wrap justify-end gap-2">
        <Alert text={error} />
        {url && (
          <button className="btn-ghost" disabled={remove.isPending} onClick={() => remove.mutate()}>
            {t('calendarRemove')}
          </button>
        )}
        <button className="btn-secondary" disabled={create.isPending} onClick={() => create.mutate()}>
          {url ? t('calendarRenew') : t('calendarCreate')}
        </button>
      </div>
    </section>
  );
}

/** The secret in groups of four, as authenticator apps print it. */
function grouped(secret: string): string {
  return secret.replace(/(.{4})/g, '$1 ').trim();
}

export function TwoFactorCard() {
  const t = useTranslations('account');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ['account', 'two-factor'], queryFn: accountApi.twoFactor });
  const [password, setPassword] = useState('');
  const [code, setCode] = useState('');
  const [setup, setSetup] = useState<{ secret: string; otpauth_url: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = () => void qc.invalidateQueries({ queryKey: ['account', 'two-factor'] });
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));

  const start = useMutation({
    mutationFn: () => accountApi.setupTwoFactor(password),
    onSuccess: (data) => {
      setError(null);
      setPassword('');
      setSetup(data);
      refresh();
    },
    onError,
  });
  const enable = useMutation({
    mutationFn: () => accountApi.enableTwoFactor(code),
    onSuccess: () => {
      setError(null);
      setCode('');
      setSetup(null);
      refresh();
    },
    onError,
  });
  const disable = useMutation({
    mutationFn: () => accountApi.disableTwoFactor(password),
    onSuccess: () => {
      setError(null);
      setPassword('');
      refresh();
    },
    onError,
  });

  const enabled = status.data?.enabled ?? false;
  return (
    <section className="card">
      <div className="card-header flex items-center gap-2">
        <KeyRound className="h-4 w-4 text-steel-500" aria-hidden />
        <h2 className="text-section">{t('twoFactorTitle')}</h2>
        {enabled && <span className="badge-done ml-auto">{t('twoFactorOn')}</span>}
      </div>
      <div className="card-content space-y-3">
        <p className="text-metadata text-steel-500">{t('twoFactorIntro')}</p>
        {setup ? (
          <div className="space-y-3">
            <ol className="list-decimal space-y-1 pl-5 text-body text-steel-800">
              <li>{t('twoFactorStep1')}</li>
              <li>
                {t('twoFactorStep2')}
                <div className="mt-1 flex items-center gap-2">
                  <code className="rounded bg-steel-100 px-2 py-1 font-mono text-body tracking-wider">
                    {grouped(setup.secret)}
                  </code>
                  <CopyButton value={setup.secret} label={t('copySecret')} />
                </div>
                <a className="mt-1 inline-block text-metadata text-cold underline" href={setup.otpauth_url}>
                  {t('twoFactorOpenApp')}
                </a>
              </li>
              <li>{t('twoFactorStep3')}</li>
            </ol>
            <div className="flex flex-wrap items-end gap-2">
              <div>
                <label className="label" htmlFor="tf-code">{t('twoFactorCode')}</label>
                <input
                  id="tf-code"
                  className="input w-40 font-mono tracking-widest"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  maxLength={7}
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                />
              </div>
              <button className="btn-primary" disabled={code.trim().length < 6 || enable.isPending} onClick={() => enable.mutate()}>
                {t('twoFactorConfirm')}
              </button>
              <button className="btn-ghost" onClick={() => setSetup(null)}>{tc('cancel')}</button>
            </div>
          </div>
        ) : (
          <div className="flex flex-wrap items-end gap-2">
            <div>
              <label className="label" htmlFor="tf-password">{t('currentPassword')}</label>
              <input
                id="tf-password"
                type="password"
                className="input w-64"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
            {enabled ? (
              <button className="btn-danger" disabled={!password || disable.isPending} onClick={() => disable.mutate()}>
                {t('twoFactorDisable')}
              </button>
            ) : (
              <button className="btn-primary" disabled={!password || start.isPending} onClick={() => start.mutate()}>
                {t('twoFactorEnable')}
              </button>
            )}
          </div>
        )}
        <Alert text={error} />
      </div>
    </section>
  );
}
