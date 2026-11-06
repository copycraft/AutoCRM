'use client';

// The EU VAT number field: checked against VIES on blur, with the verdict as a badge.
// It also drives the domestic Hungarian tax number field: 12345678-2-06 is looked up in
// VIES as HU12345678 (its közösségi adószám), and a valid answer fills the EU field too.
// A valid number fills the company's empty name/address fields from the EU registry;
// fields that already hold something different are only offered (with a diff), never
// overwritten silently. An invalid number never blocks saving: the CRM holds prospects.

import { useEffect, useRef, useState } from 'react';
import type { UseFormReturn } from 'react-hook-form';
import { useTranslations } from 'next-intl';
import { ChevronDown, ChevronUp, Loader2, RefreshCw } from 'lucide-react';
import { vatApi } from '@/lib/api/endpoints';
import type { components } from '@/lib/api/schema.gen';
import { CopyButton } from '@/components/ui/CopyButton';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { cn } from '@/lib/utils/format';
import type { PartnerFormValues } from '@/components/forms/PartnerForm';

type VatCheckResult = components['schemas']['VatCheckResult'];
type VatCheck = components['schemas']['VatCheck'];

/** The form fields VIES can fill. */
const FILLABLE = ['name', 'postal_code', 'city', 'address_line'] as const;
type Fillable = (typeof FILLABLE)[number];

/** Close enough to the server's normalisation to tell "same number" apart. */
function compact(v: string): string {
  return v.toUpperCase().replace(/[\s.\-/_,]/g, '');
}

function viesValues(r: VatCheckResult): Partial<Record<Fillable, string>> {
  const out: Partial<Record<Fillable, string>> = {};
  if (r.name) out.name = r.name;
  const p = r.address_parts;
  if (p?.postal_code) out.postal_code = p.postal_code;
  if (p?.city) out.city = p.city;
  if (p?.address_line) out.address_line = p.address_line;
  return out;
}

const sameText = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

export function VatNumberField({
  form,
  partnerId,
  field: fieldName = 'eu_tax_number',
}: {
  form: UseFormReturn<PartnerFormValues>;
  /** Set when editing: the verdict is filed on the partner and the history is shown. */
  partnerId?: number;
  /** `tax_number`: the domestic Hungarian adószám, checked through its EU form. */
  field?: 'eu_tax_number' | 'tax_number';
}) {
  const domestic = fieldName === 'tax_number';
  const t = useTranslations('vat');
  const tp = useTranslations('partners');
  const { register, getValues, setValue } = form;
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<VatCheckResult | null>(null);
  const [formatInvalid, setFormatInvalid] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [filled, setFilled] = useState<Fillable[]>([]);
  const [offer, setOffer] = useState<Partial<Record<Fillable, string>>>({});
  const [history, setHistory] = useState<VatCheck[] | null>(null);
  const [filledEu, setFilledEu] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const lastChecked = useRef<string>('');
  const fieldLabel: Record<Fillable, string> = {
    name: t('registeredName'),
    postal_code: tp('postalCode'),
    city: tp('city'),
    address_line: tp('addressLine'),
  };

  const loadHistory = async () => {
    if (!partnerId || domestic) return;
    try {
      setHistory((await vatApi.history(partnerId)).items);
    } catch {
      setHistory([]);
    }
  };

  // Editing: show the latest verdict on file when it is about the number in the field.
  useEffect(() => {
    if (!partnerId || domestic) return;
    let live = true;
    vatApi
      .history(partnerId)
      .then(({ items }) => {
        if (!live) return;
        setHistory(items);
        const latest = items[0];
        const current = compact(getValues(fieldName) ?? '');
        if (latest && current && compact(latest.country_code + latest.vat_number) === current) {
          lastChecked.current = current;
          setResult({
            status: latest.status as VatCheckResult['status'],
            source: latest.source,
            country_code: latest.country_code,
            vat_number: latest.vat_number,
            formatted: latest.country_code + latest.vat_number,
            hu_domestic_tax_number: latest.hu_domestic_tax_number ?? null,
            name: latest.name ?? null,
            address: latest.address ?? null,
            address_parts: null,
            request_identifier: latest.request_identifier ?? null,
            request_date: latest.request_date ?? null,
            checked_at: latest.created_at,
            error_code: latest.error_code ?? null,
            cached: true,
            check_id: latest.id,
            hints: latest.hints,
          });
        }
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
    // Once per partner: later checks update the state themselves.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [partnerId]);

  const run = async (force: boolean) => {
    const input = (getValues(fieldName) ?? '').trim();
    if (!input) {
      setResult(null);
      setFormatInvalid(false);
      lastChecked.current = '';
      return;
    }
    // The domestic field only goes to VIES as a full Hungarian adószám; anything else is
    // left to the server's own tax-number validation on save.
    if (domestic && !/^(HU)?d{11}$/.test(compact(input))) {
      setResult(null);
      setFormatInvalid(false);
      lastChecked.current = compact(input);
      return;
    }
    setBusy(true);
    setFormatInvalid(false);
    setFilled([]);
    setOffer({});
    setFilledEu(false);
    try {
      const r = await vatApi.check({
        input,
        default_country: getValues('country') || null,
        subject: partnerId ? { type: 'partner', id: partnerId } : null,
        force,
      });
      if (!r) {
        setResult(null);
        setFormatInvalid(true);
        lastChecked.current = compact(input);
        return;
      }
      lastChecked.current = r.formatted;
      setResult(r);
      lastChecked.current = domestic ? compact(input) : r.formatted;
      if (!domestic && r.formatted !== input) setValue('eu_tax_number', r.formatted, { shouldDirty: true });
      if (r.status === 'VALID') {
        applyVies(r);
        // A Hungarian company with a közösségi adószám: that is its EU VAT number.
        if (domestic && !(getValues('eu_tax_number') ?? '').trim()) {
          setValue('eu_tax_number', r.formatted, { shouldDirty: true });
          setFilledEu(true);
        }
      }
      void loadHistory();
    } catch {
      setResult(null);
    } finally {
      setBusy(false);
    }
  };

  // Empty fields are filled; differing ones are offered.
  const applyVies = (r: VatCheckResult) => {
    const vies = viesValues(r);
    const nowFilled: Fillable[] = [];
    const differs: Partial<Record<Fillable, string>> = {};
    for (const f of FILLABLE) {
      const v = vies[f];
      if (!v) continue;
      const current = (getValues(f) ?? '').trim();
      if (!current) {
        setValue(f, v, { shouldDirty: true, shouldValidate: true });
        nowFilled.push(f);
      } else if (!sameText(current, v)) {
        differs[f] = v;
      }
    }
    // The country field starts as HU; unless the user set it, the registry's country wins.
    const iso = r.country_code === 'EL' ? 'GR' : r.country_code === 'XI' ? 'GB' : r.country_code;
    const country = (getValues('country') ?? '').trim().toUpperCase();
    if (iso && country !== iso && (!country || !form.getFieldState('country').isDirty)) {
      setValue('country', iso, { shouldDirty: true, shouldValidate: true });
    }
    setFilled(nowFilled);
    setOffer(differs);
  };

  const takeOffer = () => {
    for (const [f, v] of Object.entries(offer) as [Fillable, string][]) {
      setValue(f, v, { shouldDirty: true, shouldValidate: true });
    }
    setFilled((prev) => [...prev, ...(Object.keys(offer) as Fillable[])]);
    setOffer({});
  };

  const field = register(fieldName, {
    onBlur: () => {
      const v = compact(getValues(fieldName) ?? '');
      if (v !== lastChecked.current) void run(false);
    },
  });

  const isHu = result?.country_code === 'HU';
  const offerKeys = Object.keys(offer) as Fillable[];

  return (
    <div>
      <label className="label" htmlFor={fieldName}>
        {domestic ? tp('taxNumber') : tp('euTaxNumber')}
      </label>
      <div className="flex items-center gap-2">
        <input
          id={fieldName}
          className={cn('input font-mono', formatInvalid && 'border-signal')}
          placeholder={domestic ? '12345678-1-23' : 'ATU12345678'}
          aria-invalid={formatInvalid || undefined}
          aria-describedby={`${fieldName}_status`}
          {...field}
        />
        {result && !busy && (
          <button
            type="button"
            className="btn-ghost btn-sm shrink-0"
            onClick={() => void run(true)}
            title={t('recheck')}
            aria-label={t('recheck')}
          >
            <RefreshCw className="h-4 w-4" aria-hidden />
          </button>
        )}
      </div>

      <div id={`${fieldName}_status`} className="mt-1 space-y-2 text-metadata" aria-live="polite">
        {busy && (
          <p className="flex items-center gap-1.5 text-steel-500">
            <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden /> {t('checking')}
          </p>
        )}
        {!busy && !result && !formatInvalid && (
          <p className="text-steel-500">{domestic ? t('domesticLookupHint') : t('lookupHint')}</p>
        )}
        {!busy && formatInvalid && <p className="text-signal">{t('formatInvalid')}</p>}

        {!busy && result && (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <VerdictBadge result={result} />
              {result.status === 'ERROR' && (
                <button type="button" className="btn-ghost btn-sm" onClick={() => void run(true)}>
                  {t('retry')}
                </button>
              )}
              {(result.status === 'VALID' || result.status === 'INVALID') && (
                <span className="text-steel-500">
                  <DateDisplay value={result.checked_at} withTime />
                  {result.cached && ` · ${t('cached')}`}
                </span>
              )}
              {result.status === 'VALID' && (result.name || result.address || result.request_identifier) && (
                <button
                  type="button"
                  className="btn-ghost btn-sm"
                  onClick={() => setExpanded((e) => !e)}
                  aria-expanded={expanded}
                >
                  {t('details')}
                  {expanded ? <ChevronUp className="ml-1 h-3.5 w-3.5" /> : <ChevronDown className="ml-1 h-3.5 w-3.5" />}
                </button>
              )}
            </div>

            {result.hu_domestic_tax_number && (
              <p className="font-mono text-steel-900">
                {t('domestic')}: {result.hu_domestic_tax_number.replace(/^(\d{8})(\d)(\d{2})$/, '$1-$2-$3')} ·{' '}
                {t('community')}: {result.formatted}
              </p>
            )}

            {expanded && result.status === 'VALID' && (
              <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-lg bg-steel-200/40 px-3 py-2 text-steel-900">
                {result.name && (
                  <>
                    <dt className="text-steel-500">{t('registeredName')}</dt>
                    <dd>{result.name}</dd>
                  </>
                )}
                {result.address && (
                  <>
                    <dt className="text-steel-500">{t('registeredAddress')}</dt>
                    <dd className="whitespace-pre-line">{result.address}</dd>
                  </>
                )}
                {result.request_identifier && (
                  <>
                    <dt className="text-steel-500">{t('consultationNo')}</dt>
                    <dd className="flex items-center gap-1 font-mono">
                      {result.request_identifier}
                      <CopyButton value={result.request_identifier} label={t('consultationNo')} />
                    </dd>
                  </>
                )}
              </dl>
            )}

            {filledEu && (
              <p className="text-done">
                {t('filledEu', { number: result.formatted })}
              </p>
            )}

            {filled.length > 0 && (
              <p className="text-done">
                {t('filledFromVies')} ({filled.map((f) => fieldLabel[f]).join(', ')})
              </p>
            )}

            {offerKeys.length > 0 && (
              <div className="rounded-lg border border-steel-200 px-3 py-2 text-steel-900">
                <p className="mb-1">{t('differs')}</p>
                <table className="w-full text-left">
                  <thead className="text-steel-500">
                    <tr>
                      <th className="pr-3 font-normal" />
                      <th className="pr-3 font-normal">{t('current')}</th>
                      <th className="font-normal">{t('fromVies')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {offerKeys.map((f) => (
                      <tr key={f}>
                        <td className="pr-3 text-steel-500">{fieldLabel[f]}</td>
                        <td className="pr-3 line-through decoration-steel-500/60">{getValues(f)}</td>
                        <td className="font-medium">{offer[f]}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <button type="button" className="btn-secondary btn-sm mt-2" onClick={takeOffer}>
                  {t('useViesData')}
                </button>
              </div>
            )}

            {result.status === 'INVALID' && isHu && <p className="text-steel-500">{t('huHint')}</p>}
            {result.hints.includes('HU_VAT_GROUP_MEMBER') && <p className="text-steel-500">{t('huGroupHint')}</p>}
            {result.hints.includes('REQUESTER_REJECTED') && <p className="text-steel-500">{t('requesterRejected')}</p>}
          </>
        )}

        {partnerId && history && history.length > 0 && (
          <div>
            <button
              type="button"
              className="btn-ghost btn-sm -ml-2"
              onClick={() => setShowHistory((s) => !s)}
              aria-expanded={showHistory}
            >
              {t('history')} ({history.length})
              {showHistory ? <ChevronUp className="ml-1 h-3.5 w-3.5" /> : <ChevronDown className="ml-1 h-3.5 w-3.5" />}
            </button>
            {showHistory && (
              <ul className="mt-1 divide-y divide-steel-200 rounded-lg border border-steel-200">
                {history.map((h) => (
                  <li key={h.id} className="flex flex-wrap items-center gap-2 px-3 py-1.5">
                    <VerdictBadge
                      result={{ status: h.status as VatCheckResult['status'], country_code: h.country_code, error_code: h.error_code ?? null }}
                    />
                    <span className="font-mono">{h.country_code + h.vat_number}</span>
                    <span className="text-steel-500">
                      <DateDisplay value={h.created_at} withTime />
                    </span>
                    {h.request_identifier && (
                      <span className="font-mono text-steel-500">
                        {t('consultationNo')}: {h.request_identifier}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function VerdictBadge({
  result,
}: {
  result: Pick<VatCheckResult, 'status' | 'country_code' | 'error_code'>;
}) {
  const t = useTranslations('vat');
  switch (result.status) {
    case 'VALID':
      return <span className="badge bg-done/10 text-done">{t('valid')}</span>;
    case 'INVALID':
      return <span className="badge bg-signal/10 text-signal">{t('invalid')}</span>;
    case 'UNAVAILABLE':
      // Never red: VIES not answering says nothing about the number.
      return (
        <span className="badge bg-amber-100 text-amber-800">
          {t('unavailable', { country: result.country_code })}
        </span>
      );
    case 'ERROR':
      return <span className="badge badge-muted">{t('error', { code: result.error_code ?? '?' })}</span>;
    default:
      return null;
  }
}
