'use client';

// Sales and marketing reports (0049): the funnel of the period's leads, each salesperson's
// numbers, how fast leads hear back, revenue by country and year, cumulative flow of
// orders through the stages, newsletter trends, and who gets the Monday report.

import { useMemo, useState } from 'react';
import dynamic from 'next/dynamic';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { configApi, leadSourcesApi, salesReportsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';
import { canAdmin, useAuth } from '@/lib/auth/context';
import { cn } from '@/lib/utils/format';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';

// recharts loads on demand, as in WorkloadCharts.
const FlowChart = dynamic(() => import('./SalesFlowChart').then((m) => m.SalesFlowChart), {
  ssr: false,
  loading: () => <div className="h-72 animate-pulse rounded-lg bg-steel-100" />,
});

const PERIODS = [90, 365, 730] as const;

const iso = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

function pct(part: number, whole: number): string {
  return whole === 0 ? '—' : `${Math.round((part / whole) * 100)}%`;
}

function hours(h: number | null | undefined): string {
  if (h == null) return '—';
  if (h < 1) return `${Math.max(1, Math.round(h * 60))} p`;
  if (h < 48) return `${h.toFixed(1)} ó`;
  return `${(h / 24).toFixed(1)} n`;
}

function Section({ id, title, subtitle, children }: { id: string; title: string; subtitle?: string; children: React.ReactNode }) {
  return (
    <section className="mt-10 space-y-3" aria-labelledby={id}>
      <div>
        <h2 id={id} className="text-section font-semibold">{title}</h2>
        {subtitle && <p className="text-metadata text-steel-500">{subtitle}</p>}
      </div>
      {children}
    </section>
  );
}

export function SalesReports() {
  const t = useTranslations('salesReports');
  const [days, setDays] = useState<(typeof PERIODS)[number]>(365);
  const range = useMemo(() => {
    const to = new Date();
    return { from: iso(new Date(to.getTime() - days * 86_400_000)), to: iso(to) };
  }, [days]);

  return (
    <div>
      <div className="mt-10 flex flex-wrap items-center justify-between gap-3 border-t border-steel-200 pt-6">
        <div>
          <h2 className="text-page-title">{t('title')}</h2>
          <p className="text-metadata text-steel-500">{t('subtitle')}</p>
        </div>
        <div role="group" aria-label={t('period')} className="flex gap-1">
          {PERIODS.map((p) => (
            <button
              key={p}
              aria-pressed={days === p}
              onClick={() => setDays(p)}
              className={cn('btn-sm', days === p ? 'btn-primary' : 'btn-ghost')}
            >
              {t('lastDays', { days: p })}
            </button>
          ))}
        </div>
      </div>
      <Funnel range={range} />
      <Salespeople range={range} />
      <FirstResponse range={range} />
      <RevenueByCountry range={range} />
      <CumulativeFlow range={range} />
      <NewsletterTrends range={range} />
      <WeeklyReportRecipients />
    </div>
  );
}

type Range = { from: string; to: string };

function Funnel({ range }: { range: Range }) {
  const t = useTranslations('salesReports');
  const q = useQuery({ queryKey: ['report-funnel', range], queryFn: () => salesReportsApi.funnel(range) });
  return (
    <Section id="funnel-title" title={t('funnel')} subtitle={t('funnelHint')}>
      {q.isLoading ? (
        <LoadingState />
      ) : q.isError ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : (
        <div className="card space-y-4 p-4">
          <div className="flex flex-wrap gap-6 text-body">
            <span>{t('created')}: <b className="font-mono">{q.data!.totals.created}</b></span>
            <span>{t('quoted')}: <b className="font-mono">{q.data!.totals.quoted}</b> ({pct(q.data!.totals.quoted, q.data!.totals.created)})</span>
            <span>{t('won')}: <b className="font-mono">{q.data!.totals.won}</b> ({pct(q.data!.totals.won, q.data!.totals.created)})</span>
            <span>{t('wonValue')}: <Money minor={q.data!.totals.won_huf_minor} currency="HUF" /></span>
          </div>
          <ul className="space-y-1.5">
            {q.data!.stages.map((s) => {
              const width = q.data!.totals.created === 0 ? 0 : Math.round((s.leads / q.data!.totals.created) * 100);
              return (
                <li key={s.key} className="grid grid-cols-[12rem_1fr_5rem] items-center gap-3 text-body">
                  <span className={cn('truncate', s.is_exit && 'text-steel-500')}>{s.label}</span>
                  <span className="h-5 rounded bg-steel-100">
                    <span
                      className={cn('block h-5 rounded', s.is_exit ? 'bg-steel-300' : 'bg-cold')}
                      style={{ width: `${Math.min(100, width)}%` }}
                    />
                  </span>
                  <span className="text-right font-mono">
                    {s.leads} <span className="text-metadata text-steel-500">{pct(s.leads, q.data!.totals.created)}</span>
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </Section>
  );
}

function Salespeople({ range }: { range: Range }) {
  const t = useTranslations('salesReports');
  const q = useQuery({ queryKey: ['report-salespeople', range], queryFn: () => salesReportsApi.salespeople(range) });
  return (
    <Section id="salespeople-title" title={t('salespeople')} subtitle={t('salespeopleHint')}>
      {q.isLoading ? (
        <LoadingState />
      ) : q.isError ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : (
        <div className="table-container">
          <table className="table">
            <thead>
              <tr>
                <th>{t('person')}</th>
                <th className="text-right">{t('leads')}</th>
                <th className="text-right">{t('quoted')}</th>
                <th className="text-right">{t('won')}</th>
                <th className="text-right">{t('winRate')}</th>
                <th className="text-right">{t('lost')}</th>
                <th className="text-right">{t('wonValue')}</th>
                <th className="text-right">{t('medianResponse')}</th>
              </tr>
            </thead>
            <tbody>
              {q.data!.rows.map((r) => (
                <tr key={r.user_id ?? 0}>
                  <td>{r.name}</td>
                  <td className="text-right font-mono">{r.leads}</td>
                  <td className="text-right font-mono">{r.quoted}</td>
                  <td className="text-right font-mono">{r.won}</td>
                  <td className="text-right font-mono">{pct(r.won, r.leads)}</td>
                  <td className="text-right font-mono">{r.lost}</td>
                  <td className="text-right"><Money minor={r.won_huf_minor} currency="HUF" /></td>
                  <td className="text-right font-mono">{hours(r.median_response_hours)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Section>
  );
}

function FirstResponse({ range }: { range: Range }) {
  const t = useTranslations('salesReports');
  const q = useQuery({ queryKey: ['report-first-response', range], queryFn: () => salesReportsApi.firstResponse(range) });
  const sources = useQuery({ queryKey: ['lead-sources', true], queryFn: () => leadSourcesApi.list(true) });
  const label = (key: string) => sources.data?.items.find((x) => x.key === key)?.label ?? key;
  return (
    <Section id="response-title" title={t('firstResponse')} subtitle={t('firstResponseHint')}>
      {q.isLoading ? (
        <LoadingState />
      ) : q.isError ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : (
        <div className="table-container">
          <table className="table">
            <thead>
              <tr>
                <th>{t('source')}</th>
                <th className="text-right">{t('leads')}</th>
                <th className="text-right">{t('answered')}</th>
                <th className="text-right">{t('median')}</th>
                <th className="text-right">{t('p90')}</th>
                <th className="text-right">{t('withinDay')}</th>
              </tr>
            </thead>
            <tbody>
              {q.data!.rows.map((r) => (
                <tr key={r.source} className={r.source === '*' ? 'font-medium' : undefined}>
                  <td>{r.source === '*' ? t('everyone') : r.source ? label(r.source) : t('noSource')}</td>
                  <td className="text-right font-mono">{r.leads}</td>
                  <td className="text-right font-mono">{r.answered}</td>
                  <td className="text-right font-mono">{hours(r.median_hours)}</td>
                  <td className="text-right font-mono">{hours(r.p90_hours)}</td>
                  <td className="text-right font-mono">{pct(r.within_day, r.leads)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Section>
  );
}

function RevenueByCountry({ range }: { range: Range }) {
  const t = useTranslations('salesReports');
  const q = useQuery({ queryKey: ['report-revenue-country', range], queryFn: () => salesReportsApi.revenueByCountry(range) });
  const pivot = useMemo(() => {
    const rows = q.data?.rows ?? [];
    const years = [...new Set(rows.map((r) => r.year))].sort();
    const countries = [...new Set(rows.map((r) => r.country))];
    const total = (c: string) => rows.filter((r) => r.country === c).reduce((a, r) => a + r.huf_minor, 0);
    countries.sort((a, b) => total(b) - total(a));
    return { years, countries, cell: (c: string, y: number) => rows.find((r) => r.country === c && r.year === y) };
  }, [q.data]);
  return (
    <Section id="revenue-title" title={t('revenueByCountry')} subtitle={t('revenueHint')}>
      {q.isLoading ? (
        <LoadingState />
      ) : q.isError ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : pivot.countries.length === 0 ? (
        <p className="text-body text-steel-500">{t('nothing')}</p>
      ) : (
        <div className="table-container">
          <table className="table">
            <thead>
              <tr>
                <th>{t('country')}</th>
                {pivot.years.map((y) => (
                  <th key={y} className="text-right">{y}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {pivot.countries.map((c) => (
                <tr key={c}>
                  <td className="font-mono">{c}</td>
                  {pivot.years.map((y) => {
                    const cell = pivot.cell(c, y);
                    return (
                      <td key={y} className="text-right">
                        {cell ? (
                          <span title={t('ordersCount', { count: cell.orders })}>
                            <Money minor={cell.huf_minor} currency="HUF" />
                            {cell.missing_fx > 0 && <span className="ml-1 text-metadata text-signal">*</span>}
                          </span>
                        ) : (
                          '—'
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Section>
  );
}

function CumulativeFlow({ range }: { range: Range }) {
  const t = useTranslations('salesReports');
  const q = useQuery({ queryKey: ['report-cfd', range], queryFn: () => salesReportsApi.cumulativeFlow(range) });
  const stages = useQuery({ queryKey: qk.stages('order'), queryFn: () => configApi.stages('order') });
  return (
    <Section id="cfd-title" title={t('cumulativeFlow')} subtitle={t('cumulativeFlowHint')}>
      {q.isLoading || stages.isLoading ? (
        <LoadingState />
      ) : q.isError ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : (
        <div className="card p-4">
          <FlowChart
            points={q.data!.points}
            stages={(stages.data?.items ?? []).filter((s) => !s.is_terminal).map((s) => ({ key: s.key, label: s.label_hu }))}
          />
        </div>
      )}
    </Section>
  );
}

function NewsletterTrends({ range }: { range: Range }) {
  const t = useTranslations('salesReports');
  const q = useQuery({ queryKey: ['report-newsletter', range], queryFn: () => salesReportsApi.newsletterTrends(range) });
  return (
    <Section id="newsletter-title" title={t('newsletterTrends')} subtitle={t('newsletterHint')}>
      {q.isLoading ? (
        <LoadingState />
      ) : q.isError ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          <div className="table-container">
            <table className="table">
              <thead>
                <tr>
                  <th>{t('month')}</th>
                  <th className="text-right">{t('joined')}</th>
                  <th className="text-right">{t('left')}</th>
                  <th className="text-right">{t('activeAtEnd')}</th>
                </tr>
              </thead>
              <tbody>
                {q.data!.months.map((m) => (
                  <tr key={m.month}>
                    <td className="font-mono">{m.month}</td>
                    <td className="text-right font-mono">+{m.joined}</td>
                    <td className="text-right font-mono">−{m.left}</td>
                    <td className="text-right font-mono">{m.active_at_end}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="table-container">
            <table className="table">
              <thead>
                <tr>
                  <th>{t('send')}</th>
                  <th className="text-right">{t('sent')}</th>
                  <th className="text-right">{t('openRate')}</th>
                  <th className="text-right">{t('clickRate')}</th>
                </tr>
              </thead>
              <tbody>
                {q.data!.sends.length === 0 && (
                  <tr>
                    <td colSpan={4} className="text-steel-500">{t('noSends')}</td>
                  </tr>
                )}
                {q.data!.sends.map((s) => (
                  <tr key={s.send_id}>
                    <td>
                      <div className="truncate">{s.subject}</div>
                      <div className="text-metadata text-steel-500"><DateDisplay value={s.send_at} /></div>
                    </td>
                    <td className="text-right font-mono">{s.sent}</td>
                    <td className="text-right font-mono">{pct(s.opened, s.sent)}</td>
                    <td className="text-right font-mono">{pct(s.clicked, s.sent)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </Section>
  );
}

function WeeklyReportRecipients() {
  const t = useTranslations('salesReports');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const admin = canAdmin(user);
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ['weekly-recipients'], queryFn: salesReportsApi.weeklyRecipients, enabled: admin });
  const [text, setText] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: () =>
      salesReportsApi.saveWeeklyRecipients(
        (text ?? '')
          .split(/[\s,;]+/)
          .map((x) => x.trim())
          .filter(Boolean),
      ),
    onSuccess: (r) => {
      qc.setQueryData(['weekly-recipients'], r);
      setText(r.recipients.join('\n'));
      setMessage(t('saved'));
    },
    onError: (e) => setMessage(errorMessage(e, ter, ter('unknownError'))),
  });
  if (!admin) return null;
  const value = text ?? (q.data?.recipients ?? []).join('\n');
  return (
    <Section id="weekly-title" title={t('weeklyReport')} subtitle={t('weeklyReportHint')}>
      <div className="card space-y-2 p-4">
        <textarea
          className="input min-h-[90px] font-mono text-body"
          aria-label={t('weeklyReport')}
          placeholder="vezeto@autotherm.hu"
          value={value}
          onChange={(e) => setText(e.target.value)}
        />
        <div className="flex items-center gap-3">
          <button className="btn-primary btn-sm" disabled={save.isPending} onClick={() => save.mutate()}>
            {t('save')}
          </button>
          {message && <span className="text-metadata text-steel-600" role="status">{message}</span>}
        </div>
      </div>
    </Section>
  );
}
