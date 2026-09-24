'use client';

import { useState } from 'react';
import { useForm, Controller } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { useTranslations } from 'next-intl';
import { skipToken, useQuery } from '@tanstack/react-query';
import { constraintName, errorMessage } from '@/lib/api/errors';
import { configApi, partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { PartnerPicker, type PartnerOption } from './PartnerPicker';
import { AssigneeField } from './AssigneeField';
import { DateQuickPicks } from './DateQuickPicks';
import { lastAssignee, lastUsed } from '@/hooks/useLastUsed';
import { useDirtyGuard } from '@/hooks/useDirtyGuard';
import { useFormDraft } from '@/hooks/useFormDraft';
import { BuildSpecSection, type SpecForm } from './BuildSpecSection';
import { ReturningVehicleWarning } from '@/components/orders/ReturningVehicleWarning';
import type {
  Order,
  OrderBody,
  OrderSpec,
  PatchOrder,
  ProjectType,
  SpecBody,
} from '@/lib/api/types';

/**
 * The numeric ranges the database enforces on `order_specs` (migration 0015).
 *
 * Mirrored here so a wrong value is caught under the field the user is looking at, rather
 * than coming back as a 422 after a round trip. The database keeps its CHECKs — this is a
 * courtesy, not the guarantee — and [`SPEC_CONSTRAINT_FIELD`] catches whatever still gets
 * through so that even then the error lands on the right input.
 */
export const SPEC_LIMITS = {
  target_temp_c: { min: -40, max: 120 },
  insulation_mm: { min: 0, max: 500 },
  compartments: { min: 1, max: 5 },
} as const;

/**
 * A blank optional number, or one inside the database's range.
 *
 * The message is a token, not finished text: zod resolves messages at parse time, where
 * there is no translator in scope. [`fieldErrorText`] turns it into Hungarian at render.
 */
function boundedNumber(min: number, max: number) {
  return z
    .string()
    .trim()
    .optional()
    .refine(
      (s) => {
        if (!s) return true;
        // Hungarian keyboards produce a decimal comma, and the office types it.
        const n = Number(s.replace(',', '.'));
        return Number.isFinite(n) && n >= min && n <= max;
      },
      { message: `range:${min}:${max}` },
    );
}

/** Same, for "must be greater than zero" where naming an upper bound would be noise. */
function positiveNumber() {
  return z
    .string()
    .trim()
    .optional()
    .refine((s) => {
      if (!s) return true;
      const n = Number(s.replace(',', '.'));
      return Number.isFinite(n) && n > 0;
    }, { message: 'positive' });
}

/**
 * A react-hook-form error message as Hungarian text.
 *
 * Three kinds arrive here: a `range:min:max` token from [`boundedNumber`], a bare
 * catalogue key, and a message the server wrote (already Hungarian, or at least already
 * a sentence). Anything unrecognised is shown as-is rather than swallowed — a message
 * the user can read beats a blank space under a red box.
 */
export function fieldErrorText(
  message: string | undefined,
  tv: (key: string, values?: Record<string, string | number>) => string,
): string {
  if (!message) return '';
  const range = /^range:(-?[\d.]+):(-?[\d.]+)$/.exec(message);
  if (range) return tv('range', { min: range[1] ?? '', max: range[2] ?? '' });
  let translated = message;
  try {
    translated = tv(message);
  } catch {
    // Not a catalogue key — a server sentence, or a zod default.
  }
  return translated === message ? message : translated;
}

/**
 * Database constraint → the input that carries the offending value.
 *
 * The last line of defence. Everything in `SPEC_LIMITS` is checked before submit, but a
 * CHECK can be added to a migration without anyone touching this form, and enum columns
 * (`defrost`, `fuel`) are only ever as correct as the select that feeds them. Mapping the
 * name back to a field turns "the data violates the database's rules" — true, and useless —
 * into a red box around the thing to change.
 */
const SPEC_CONSTRAINT_FIELD: Record<string, keyof OrderFormValues> = {
  order_specs_target_temp_c_check: 'target_temp_c',
  order_specs_insulation_mm_check: 'insulation_mm',
  order_specs_compartments_check: 'compartments',
  order_specs_heat_output_kw_check: 'heat_output_kw',
  order_specs_defrost_check: 'defrost',
  order_specs_fuel_check: 'fuel',
  orders_currency_check: 'currency',
};

/**
 * The text a field shows when the database, rather than zod, rejected the value.
 *
 * Deliberately the same token the client-side check would have produced: the user should
 * not be able to tell which side caught it, and two wordings for one rule is how they
 * drift apart.
 */
function constraintMessage(field: keyof OrderFormValues): string {
  const limit = SPEC_LIMITS[field as keyof typeof SPEC_LIMITS];
  if (limit) return `range:${limit.min}:${limit.max}`;
  if (field === 'heat_output_kw') return 'positive';
  return 'invalidValue';
}

const schema = z.object({
  title: z.string().trim().min(1),
  // Partner is required, but enforced in the submit handler (not via
  // .refine) so the field type stays a plain nullable the form can handle.
  partner: z.custom<PartnerOption | null>(() => true),
  contact_id: z.string(),
  project_type_id: z.string(),
  currency: z.enum(['HUF', 'EUR']),
  valuation_date: z.string(),
  vehicle_make: z.string().trim().optional(),
  vehicle_model: z.string().trim().optional(),
  vehicle_plate: z.string().trim().optional(),
  vehicle_vin: z.string().trim().optional(),
  description: z.string().trim().optional(),
  due_date: z.string(),
  assigned_to: z.custom<number | null | 'me'>(() => true),
  // Build specification. Which of these the user sees depends on the chosen project type's
  // spec_form; the ones that do not apply are never sent.
  target_temp_c: boundedNumber(SPEC_LIMITS.target_temp_c.min, SPEC_LIMITS.target_temp_c.max),
  insulation_mm: boundedNumber(SPEC_LIMITS.insulation_mm.min, SPEC_LIMITS.insulation_mm.max),
  cooling_unit_make: z.string().trim().optional(),
  cooling_unit_model: z.string().trim().optional(),
  atp_class: z.string().trim().optional(),
  compartments: boundedNumber(SPEC_LIMITS.compartments.min, SPEC_LIMITS.compartments.max),
  defrost: z.string().trim().optional(),
  electric_standby: z.boolean().optional(),
  heater_make: z.string().trim().optional(),
  heater_model: z.string().trim().optional(),
  heat_output_kw: positiveNumber(),
  fuel: z.string().trim().optional(),
  thermostat: z.boolean().optional(),
  spec_notes: z.string().trim().optional(),
});

export type OrderFormValues = z.infer<typeof schema>;

function todayLocal(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function assignee(v: OrderFormValues['assigned_to'], meId: number | undefined): number | null {
  return v === 'me' ? (meId ?? null) : v;
}

/**
 * The build spec, or undefined when the project type asks for none.
 *
 * Only the chosen variant's fields are sent. The backend blanks the other variant's
 * columns anyway and the database refuses a row that mixes them, but sending a heater
 * make on a cooling order would still be a lie about what the user filled in.
 */
export function orderSpecBody(v: OrderFormValues, form: SpecForm | null): SpecBody | undefined {
  if (!form) return undefined;
  const clean = (x: string | undefined) => (x?.trim() ? x.trim() : undefined);
  const int = (x: string | undefined) => {
    const n = Number(clean(x));
    return clean(x) && Number.isFinite(n) ? Math.trunc(n) : undefined;
  };
  const shared = {
    target_temp_c: clean(v.target_temp_c),
    insulation_mm: int(v.insulation_mm),
    notes: clean(v.spec_notes),
  };
  return form === 'cooling'
    ? {
        ...shared,
        cooling_unit_make: clean(v.cooling_unit_make),
        cooling_unit_model: clean(v.cooling_unit_model),
        atp_class: clean(v.atp_class),
        compartments: int(v.compartments),
        defrost: clean(v.defrost),
        electric_standby: v.electric_standby ?? false,
      }
    : {
        ...shared,
        heater_make: clean(v.heater_make),
        heater_model: clean(v.heater_model),
        heat_output_kw: clean(v.heat_output_kw),
        fuel: clean(v.fuel),
        thermostat: v.thermostat ?? false,
      };
}

/** The spec form a project type asks for, if any. */
export function specFormOf(
  projectTypes: ProjectType[] | undefined,
  projectTypeId: string,
): SpecForm | null {
  if (!projectTypeId) return null;
  const found = projectTypes?.find((p) => p.id === Number(projectTypeId));
  return found?.spec_form === 'heating' || found?.spec_form === 'cooling'
    ? found.spec_form
    : null;
}

export function orderCreateBody(
  v: OrderFormValues,
  meId: number | undefined,
  specForm: SpecForm | null,
): OrderBody {
  const clean = (s: string | undefined) => (s?.trim() ? s.trim() : undefined);
  const date = (s: string) => (s ? s : undefined);
  return {
    title: v.title.trim(),
    // The form requires a partner; the backend rejects a missing partner_id regardless.
    partner_id: v.partner?.id,
    contact_id: v.contact_id ? Number(v.contact_id) : undefined,
    project_type_id: v.project_type_id ? Number(v.project_type_id) : undefined,
    currency: v.currency,
    valuation_date: date(v.valuation_date),
    vehicle_make: clean(v.vehicle_make),
    vehicle_model: clean(v.vehicle_model),
    vehicle_plate: clean(v.vehicle_plate),
    vehicle_vin: clean(v.vehicle_vin),
    description: clean(v.description),
    due_date: date(v.due_date),
    assigned_to: assignee(v.assigned_to, meId),
    spec: orderSpecBody(v, specForm),
    items: [],
  };
}

/**
 * PATCH diff with backend semantics. Currency is sent only when it changed
 * and the order has no items (the backend answers 422 currency_locked
 * otherwise — the control is disabled in that case, so this is a backstop).
 * Changing partner without explicitly picking a contact clears the contact,
 * mirroring the backend rule.
 */
export function orderPatchBody(
  original: Order,
  v: OrderFormValues,
  meId: number | undefined,
  contactDirty: boolean,
  currencyLocked: boolean,
): PatchOrder {
  const body: PatchOrder = {};
  if (v.title.trim() !== original.title) body.title = v.title.trim();
  if (!currencyLocked && v.currency !== original.currency) body.currency = v.currency;
  const newPartner = v.partner?.id ?? null;
  const partnerChanged = newPartner !== original.partner_id;
  if (partnerChanged && newPartner !== null) body.partner_id = newPartner;
  const newContact = v.contact_id ? Number(v.contact_id) : null;
  if (newContact !== (original.contact_id ?? null)) body.contact_id = newContact;
  else if (partnerChanged && contactDirty === false && original.contact_id != null) {
    body.contact_id = null;
  }
  const newPt = v.project_type_id ? Number(v.project_type_id) : null;
  if (newPt !== (original.project_type_id ?? null)) body.project_type_id = newPt;
  if (v.valuation_date && v.valuation_date !== original.valuation_date) {
    body.valuation_date = v.valuation_date;
  }
  for (const f of ['vehicle_make', 'vehicle_model', 'vehicle_plate', 'vehicle_vin', 'description'] as const) {
    const nv = v[f]?.trim() ?? '';
    if (nv !== (original[f] ?? '')) body[f] = nv ? nv : null;
  }
  const newDue = v.due_date ? v.due_date : null;
  if (newDue !== (original.due_date ?? null)) body.due_date = newDue;
  const assigned = assignee(v.assigned_to, meId);
  if (assigned !== (original.assigned_to ?? null)) body.assigned_to = assigned;
  return body;
}

export function OrderForm({
  initial,
  initialPartner,
  initialSpec,
  currencyLocked,
  draftKey,
  onSubmit,
  submitLabel,
}: {
  initial?: Order;
  initialPartner?: PartnerOption | null;
  /** The order's existing build spec, so editing starts from what was recorded. */
  initialSpec?: OrderSpec | null;
  /** True while the order has items — currency select disabled. */
  currencyLocked?: boolean;
  /** Draft autosave slot for the create page; ignored when editing. */
  draftKey?: string;
  onSubmit: (v: OrderFormValues, contactDirty: boolean, specForm: SpecForm | null) => Promise<void>;
  submitLabel: string;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const tq = useTranslations('qol');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const [serverError, setServerError] = useState<string | null>(null);
  const [partnerError, setPartnerError] = useState(false);

  const emptyOrder: OrderFormValues = {
    title: '',
    partner: null,
    contact_id: '',
    project_type_id: '',
    currency: 'HUF',
    valuation_date: todayLocal(),
    vehicle_make: '',
    vehicle_model: '',
    vehicle_plate: '',
    vehicle_vin: '',
    description: '',
    due_date: '',
    assigned_to: null,
    target_temp_c: '',
    insulation_mm: '',
    cooling_unit_make: '',
    cooling_unit_model: '',
    atp_class: '',
    compartments: '',
    defrost: '',
    electric_standby: false,
    heater_make: '',
    heater_model: '',
    heat_output_kw: '',
    fuel: '',
    thermostat: false,
    spec_notes: '',
  };

  const { register, handleSubmit, control, watch, reset, setValue, setError, formState } = useForm<OrderFormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      title: initial?.title ?? '',
      partner: initialPartner ?? null,
      contact_id: initial?.contact_id ? String(initial.contact_id) : '',
      project_type_id:
        initial?.project_type_id ? String(initial.project_type_id) : (lastUsed('ptype') ?? ''),
      currency: initial?.currency ?? 'HUF',
      valuation_date: initial?.valuation_date ?? todayLocal(),
      vehicle_make: initial?.vehicle_make ?? '',
      vehicle_model: initial?.vehicle_model ?? '',
      vehicle_plate: initial?.vehicle_plate ?? '',
      vehicle_vin: initial?.vehicle_vin ?? '',
      description: initial?.description ?? '',
      due_date: initial?.due_date ?? '',
      assigned_to: initial?.assigned_to ?? lastAssignee(),
      target_temp_c: initialSpec?.target_temp_c ?? '',
      insulation_mm: initialSpec?.insulation_mm != null ? String(initialSpec.insulation_mm) : '',
      cooling_unit_make: initialSpec?.cooling_unit_make ?? '',
      cooling_unit_model: initialSpec?.cooling_unit_model ?? '',
      atp_class: initialSpec?.atp_class ?? '',
      compartments: initialSpec?.compartments != null ? String(initialSpec.compartments) : '',
      defrost: initialSpec?.defrost ?? '',
      electric_standby: initialSpec?.electric_standby ?? false,
      heater_make: initialSpec?.heater_make ?? '',
      heater_model: initialSpec?.heater_model ?? '',
      heat_output_kw: initialSpec?.heat_output_kw ?? '',
      fuel: initialSpec?.fuel ?? '',
      thermostat: initialSpec?.thermostat ?? false,
      spec_notes: initialSpec?.notes ?? '',
    },
  });

  const partner = watch('partner');
  const partnerId = partner?.id;
  const draft = useFormDraft({
    key: draftKey && !initial ? draftKey : null,
    watch,
    reset,
    empty: emptyOrder,
  });
  // Spec fields are registered on this same form, so its isDirty covers them too.
  useDirtyGuard(formState.isDirty && !formState.isSubmitSuccessful, tq('unsavedChanges'));
  const contactsQuery = useQuery({
    queryKey: qk.partner(partnerId ?? 0),
    queryFn: partnerId === undefined ? skipToken : () => partnersApi.get(partnerId),
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });
  // The section below the vehicle fields changes as this changes: cooling types show the
  // refrigeration fields, heating types the heater fields, a repair neither.
  const specForm = specFormOf(projectTypes.data?.items, watch('project_type_id'));

  return (
    <form
      className="card"
      noValidate
      onSubmit={handleSubmit(async (v) => {
        setServerError(null);
        if (!v.partner) {
          setPartnerError(true);
          return;
        }
        try {
          await onSubmit(v, !!formState.dirtyFields.contact_id, specForm);
          draft.clear();
        } catch (e) {
          // A rejected write names the constraint it tripped. If that names a field on
          // this form, put the error there and take the user to it; the banner is for
          // everything that genuinely has no field to blame.
          const field = SPEC_CONSTRAINT_FIELD[constraintName(e) ?? ''];
          if (field) {
            setError(field, { message: constraintMessage(field) }, { shouldFocus: true });
          } else {
            setServerError(errorMessage(e, ter, ter('unknownError')));
          }
        }
      })}
    >
      {serverError && (
        <div className="card-content pb-0">
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{serverError}</p>
        </div>
      )}
      {draft.restored && (
        <div className="card-content pb-0">
          <p className="flex flex-wrap items-center gap-2 rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900">
            <span>{tq('draftRestored')}</span>
            <button type="button" className="btn-ghost btn-sm" onClick={draft.discard}>
              {tq('draftDiscard')}
            </button>
          </p>
        </div>
      )}
      <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
        <div className="md:col-span-2">
          <label className="label" htmlFor="of-title">{t('fieldTitle')} *</label>
          <input id="of-title" className="input" {...register('title')} />
          {formState.errors.title && <p className="mt-1 text-metadata text-steel-900">{tv('required')}</p>}
        </div>
        <Controller
          control={control}
          name="partner"
          render={({ field }) => (
            <div>
              <PartnerPicker
                value={field.value}
                onChange={(p) => {
                  field.onChange(p);
                  if (p) setPartnerError(false);
                }}
                label={`${tc('partner')} *`}
                role="customer"
              />
              {partnerError && <p className="mt-1 text-metadata text-steel-900">{tv('required')}</p>}
            </div>
          )}
        />
        <div>
          <label className="label" htmlFor="of-currency">{t('currencyLabel')} *</label>
          <select id="of-currency" className="input" {...register('currency')} disabled={currencyLocked}>
            <option value="HUF">HUF</option>
            <option value="EUR">EUR</option>
          </select>
          {currencyLocked && <p className="mt-1 text-metadata text-steel-500">{t('currencyLocked')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="of-contact">{tc('contact')}</label>
          <select id="of-contact" className="input" {...register('contact_id')} disabled={!partner || contactsQuery.isLoading}>
            <option value="">—</option>
            {contactsQuery.data?.contacts
              .filter((c) => !c.archived_at)
              .map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
          </select>
          {contactsQuery.isError && (
            <p className="mt-1 text-metadata text-steel-900" role="alert">
              {errorMessage(contactsQuery.error, ter, ter('unknownError'))}
            </p>
          )}
          {initial && <p className="mt-1 text-metadata text-steel-500">{t('contactClearedNote')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="of-pt">{t('projectType')}</label>
          <select id="of-pt" className="input" {...register('project_type_id')} disabled={projectTypes.isLoading}>
            <option value="">—</option>
            {(projectTypes.data?.items ?? [])
              .filter((p) => p.is_active)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label_hu}
                </option>
              ))}
          </select>
          {projectTypes.isError && (
            <p className="mt-1 text-metadata text-steel-900" role="alert">
              {errorMessage(projectTypes.error, ter, ter('unknownError'))}
            </p>
          )}
        </div>
        <div>
          <label className="label" htmlFor="of-val">{t('valuationDate')}</label>
          <input id="of-val" type="date" className="input font-mono" {...register('valuation_date')} />
        </div>
        <div>
          <label className="label" htmlFor="of-due">{t('dueDate')}</label>
          <input id="of-due" type="date" className="input font-mono" {...register('due_date')} />
          <div className="mt-1">
            <DateQuickPicks onPick={(iso) => setValue('due_date', iso, { shouldDirty: true, shouldTouch: true })} />
          </div>
        </div>
        <div>
          <label className="label" htmlFor="of-make">{t('vehicleMake')}</label>
          <input id="of-make" className="input" {...register('vehicle_make')} />
        </div>
        <div>
          <label className="label" htmlFor="of-model">{t('vehicleModel')}</label>
          <input id="of-model" className="input" {...register('vehicle_model')} />
        </div>
        <div>
          <label className="label" htmlFor="of-plate">{t('vehiclePlate')}</label>
          <input id="of-plate" className="input font-mono" {...register('vehicle_plate')} />
        </div>
        <div>
          <label className="label" htmlFor="of-vin">{t('vehicleVin')}</label>
          <input id="of-vin" className="input font-mono" {...register('vehicle_vin')} />
        </div>
        <ReturningVehicleWarning
          plate={watch('vehicle_plate') ?? ''}
          vin={watch('vehicle_vin') ?? ''}
          active={initial == null}
        />
        <div className="md:col-span-2">
          <label className="label" htmlFor="of-desc">{t('description')}</label>
          <textarea id="of-desc" rows={3} className="input" {...register('description')} />
        </div>
        <Controller
          control={control}
          name="assigned_to"
          render={({ field }) => (
            <AssigneeField label={t('assignedTo')} value={field.value} onChange={field.onChange} />
          )}
        />

        <BuildSpecSection form={specForm} register={register} errors={formState.errors} />
      </div>
      <div className="card-footer justify-end">
        <button className="btn-primary" type="submit" disabled={formState.isSubmitting}>
          {formState.isSubmitting ? tc('saving') : submitLabel}
        </button>
      </div>
    </form>
  );
}
