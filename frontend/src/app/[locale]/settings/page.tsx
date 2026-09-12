'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { configApi, adminApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import type { EmailTransportBody, Settings } from '@/lib/api/types';

type Mode = 'inherit' | 'dry_run' | 'smtp';

function toMode(emailMode: string | null | undefined): Mode {
  return emailMode === 'dry_run' || emailMode === 'smtp' ? emailMode : 'inherit';
}

export default function SettingsPage() {
  const t = useTranslations('settings');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const query = useQuery({ queryKey: qk.settings, queryFn: () => configApi.settings() });

  return (
    <AppShell>
      <PageHeader title={t('title')} />
      {query.isLoading ? (
        <DetailSkeleton />
      ) : query.isError || !query.data ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <SettingsForm key={query.data.updated_at} initial={query.data} />
      )}
    </AppShell>
  );
}

function SettingsForm({ initial }: { initial: Settings }) {
  const t = useTranslations('settings');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();

  const [automatic, setAutomatic] = useState(initial.automatic_email_enabled);
  const [maxPerDay, setMaxPerDay] = useState(String(initial.max_auto_emails_per_recipient_day));
  const [windowStart, setWindowStart] = useState(initial.send_window_start.slice(0, 5));
  const [windowEnd, setWindowEnd] = useState(initial.send_window_end.slice(0, 5));
  const [weekdays, setWeekdays] = useState(initial.send_window_weekdays_only);
  const [nudgeInterval, setNudgeInterval] = useState(String(initial.nudge_interval_days));
  const [nudgeEscalate, setNudgeEscalate] = useState(String(initial.nudge_escalate_after));
  const [notifyStage, setNotifyStage] = useState(initial.stage_change_notifications);
  const [stalledTo, setStalledTo] = useState(initial.stalled_alert_recipients.join(', '));

  const [mode, setMode] = useState<Mode>(toMode(initial.email_mode));
  const [host, setHost] = useState(initial.smtp_host ?? '');
  const [port, setPort] = useState(initial.smtp_port != null ? String(initial.smtp_port) : '');
  const [security, setSecurity] = useState(initial.smtp_security ?? 'starttls');
  const [username, setUsername] = useState(initial.smtp_username ?? '');
  // Empty keeps the saved secret; the checkbox clears it explicitly.
  const [password, setPassword] = useState('');
  const [clearPassword, setClearPassword] = useState(false);
  const [helo, setHelo] = useState(initial.smtp_helo_name ?? '');
  const [ipv4, setIpv4] = useState(initial.smtp_force_ipv4 ?? false);
  const [redirect, setRedirect] = useState(initial.redirect_to ?? '');

  const [saveError, setSaveError] = useState<string | null>(null);
  const [savedTick, setSavedTick] = useState(false);

  const [testTo, setTestTo] = useState('');
  const [testResult, setTestResult] = useState<{ ok: boolean; detail: string } | null>(null);

  const save = useMutation({
    mutationFn: () => {
      const clean = (s: string) => (s.trim() ? s.trim() : null);
      const num = (s: string) => {
        const n = Number(s);
        if (!Number.isInteger(n)) throw new Error('numeric');
        return n;
      };
      const email: EmailTransportBody =
        mode === 'inherit'
          ? {
              mode: null,
              smtp_host: null,
              smtp_port: null,
              smtp_security: null,
              smtp_username: null,
              smtp_password: null,
              smtp_helo_name: null,
              smtp_force_ipv4: null,
              redirect_to: null,
            }
          : {
              mode,
              smtp_host: clean(host),
              smtp_port: port.trim() ? num(port) : null,
              smtp_security: security || null,
              smtp_username: clean(username),
              smtp_password: clearPassword ? null : password ? password : undefined,
              smtp_helo_name: clean(helo),
              smtp_force_ipv4: ipv4,
              redirect_to: clean(redirect),
            };
      return configApi.saveSettings({
        automatic_email_enabled: automatic,
        max_auto_emails_per_recipient_day: num(maxPerDay),
        send_window_start: windowStart,
        send_window_end: windowEnd,
        send_window_weekdays_only: weekdays,
        nudge_interval_days: num(nudgeInterval),
        nudge_escalate_after: num(nudgeEscalate),
        stage_change_notifications: notifyStage,
        stalled_alert_recipients: stalledTo
          .split(',')
          .map((r) => r.trim())
          .filter((r) => r !== ''),
        email,
      });
    },
    onSuccess: () => {
      setSaveError(null);
      setSavedTick(true);
      setTimeout(() => setSavedTick(false), 3000);
      void qc.invalidateQueries({ queryKey: qk.settings });
      void qc.invalidateQueries({ queryKey: qk.adminStatus });
    },
    onError: (e) => setSaveError(errorMessage(e, ter, ter('unknownError'))),
  });

  const test = useMutation({
    mutationFn: () => {
      const clean = (s: string) => (s.trim() ? s.trim() : undefined);
      return adminApi.testEmail({
        to: testTo.trim(),
        config:
          mode === 'smtp'
            ? {
                mode: 'smtp',
                smtp_host: clean(host),
                smtp_port: port.trim() ? Number(port) : undefined,
                smtp_security: security || undefined,
                smtp_username: clean(username),
                smtp_password: clearPassword ? undefined : password ? password : undefined,
                smtp_helo_name: clean(helo),
                smtp_force_ipv4: ipv4,
                redirect_to: clean(redirect),
              }
            : undefined,
      });
    },
    onSuccess: (r) => setTestResult(r),
    onError: (e) => setTestResult({ ok: false, detail: errorMessage(e, ter, ter('unknownError')) }),
  });

  return (
    <div className="space-y-6">
      <section className="card">
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('title')}</h2>
        </div>
        <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" className="rounded border-steel-200 accent-steel-900" checked={automatic} onChange={(e) => setAutomatic(e.target.checked)} />
            {t('enabled')}
          </label>
          <div>
            <label className="label" htmlFor="set-maxday">{t('maxPerDay')}</label>
            <input id="set-maxday" className="input font-mono" inputMode="numeric" value={maxPerDay} onChange={(e) => setMaxPerDay(e.target.value)} />
          </div>
          <div>
            <label className="label" htmlFor="set-wstart">{t('windowStart')}</label>
            <input id="set-wstart" type="time" className="input font-mono" value={windowStart} onChange={(e) => setWindowStart(e.target.value)} />
          </div>
          <div>
            <label className="label" htmlFor="set-wend">{t('windowEnd')}</label>
            <input id="set-wend" type="time" className="input font-mono" value={windowEnd} onChange={(e) => setWindowEnd(e.target.value)} />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" className="rounded border-steel-200 accent-steel-900" checked={weekdays} onChange={(e) => setWeekdays(e.target.checked)} />
            {t('weekdaysOnly')}
          </label>
          <div>
            <label className="label" htmlFor="set-nudge">{t('nudgeInterval')}</label>
            <input id="set-nudge" className="input font-mono" inputMode="numeric" value={nudgeInterval} onChange={(e) => setNudgeInterval(e.target.value)} />
          </div>
          <div>
            <label className="label" htmlFor="set-escalate">{t('nudgeEscalate')}</label>
            <input id="set-escalate" className="input font-mono" inputMode="numeric" value={nudgeEscalate} onChange={(e) => setNudgeEscalate(e.target.value)} />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" className="rounded border-steel-200 accent-steel-900" checked={notifyStage} onChange={(e) => setNotifyStage(e.target.checked)} />
            {t('notifyStage')}
          </label>
          <div className="md:col-span-2">
            <label className="label" htmlFor="set-stalled">{t('stalledTo')}</label>
            <input id="set-stalled" className="input" value={stalledTo} onChange={(e) => setStalledTo(e.target.value)} />
          </div>
        </div>
      </section>

      <section className="card">
        <div className="card-header flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-section font-semibold">{t('transportTitle')}</h2>
          <StatusBadge tone={initial.email_mode == null ? 'steel' : 'cold'}>
            {t('transportSource')}: {initial.email_mode == null ? t('sourceEnv') : t('sourceDb')}
          </StatusBadge>
        </div>
        <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
          <div className="md:col-span-2 flex flex-wrap gap-4" role="radiogroup" aria-label={t('modeLabel')}>
            {(['inherit', 'dry_run', 'smtp'] as const).map((m) => (
              <label key={m} className="flex items-center gap-2 text-sm">
                <input
                  type="radio"
                  name="email-mode"
                  className="accent-steel-900"
                  checked={mode === m}
                  onChange={() => setMode(m)}
                />
                {m === 'inherit' ? t('inheritMode') : m === 'dry_run' ? t('dryRunMode') : t('smtpMode')}
              </label>
            ))}
          </div>
          {mode === 'smtp' && (
            <>
              <div>
                <label className="label" htmlFor="set-host">{t('host')} *</label>
                <input id="set-host" className="input font-mono" value={host} onChange={(e) => setHost(e.target.value)} />
              </div>
              <div>
                <label className="label" htmlFor="set-port">{t('port')}</label>
                <input id="set-port" className="input font-mono" inputMode="numeric" placeholder="587" value={port} onChange={(e) => setPort(e.target.value)} />
              </div>
              <div>
                <label className="label" htmlFor="set-sec">{t('security')}</label>
                <select id="set-sec" className="input" value={security} onChange={(e) => setSecurity(e.target.value)}>
                  <option value="starttls">starttls</option>
                  <option value="tls">tls</option>
                  <option value="none">none</option>
                </select>
              </div>
              <div>
                <label className="label" htmlFor="set-user">{t('username')}</label>
                <input id="set-user" className="input" autoComplete="off" value={username} onChange={(e) => setUsername(e.target.value)} />
              </div>
              <div>
                <label className="label" htmlFor="set-pass">{t('password')}</label>
                <input id="set-pass" type="password" className="input" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
                {initial.has_password && !password && !clearPassword && (
                  <p className="mt-1 text-xs text-steel-500">{t('passwordSaved')}</p>
                )}
                {initial.has_password && (
                  <label className="mt-1 flex items-center gap-2 text-xs">
                    <input type="checkbox" className="rounded border-steel-200 accent-steel-900" checked={clearPassword} onChange={(e) => setClearPassword(e.target.checked)} />
                    {t('passwordClear')}
                  </label>
                )}
              </div>
              <div>
                <label className="label" htmlFor="set-helo">{t('heloName')}</label>
                <input id="set-helo" className="input font-mono" value={helo} onChange={(e) => setHelo(e.target.value)} />
              </div>
              <label className="flex items-center gap-2 text-sm">
                <input type="checkbox" className="rounded border-steel-200 accent-steel-900" checked={ipv4} onChange={(e) => setIpv4(e.target.checked)} />
                {t('forceIpv4')}
              </label>
              <div>
                <label className="label" htmlFor="set-redirect">{t('redirectTo')}</label>
                <input id="set-redirect" type="email" className="input" value={redirect} onChange={(e) => setRedirect(e.target.value)} />
              </div>
            </>
          )}
        </div>
        <div className="card-footer flex-wrap justify-between gap-3">
          <span className="text-sm text-steel-500">{savedTick ? t('saved') : ''}</span>
          <div className="flex gap-2">
            {saveError && (
              <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">{saveError}</p>
            )}
            <button className="btn-primary" disabled={save.isPending} onClick={() => save.mutate()}>
              {save.isPending ? tc('saving') : tc('save')}
            </button>
          </div>
        </div>
      </section>

      <section className="card">
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('testTitle')}</h2>
        </div>
        <div className="card-content space-y-3">
          <div>
            <label className="label" htmlFor="set-testto">{t('testTo')}</label>
            <input id="set-testto" type="email" className="input" value={testTo} onChange={(e) => setTestTo(e.target.value)} />
          </div>
          {testResult && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="status">
              <StatusBadge tone={testResult.ok ? 'done' : 'signal'}>
                {testResult.ok ? t('testOk') : t('testFail')}
              </StatusBadge>{' '}
              {testResult.detail}
            </p>
          )}
          <div className="flex justify-end">
            <button className="btn-secondary" disabled={test.isPending || !testTo.trim()} onClick={() => test.mutate()}>
              {t('testSend')}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}
