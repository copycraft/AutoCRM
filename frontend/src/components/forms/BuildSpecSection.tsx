'use client';

import { useTranslations } from 'next-intl';
import type { UseFormRegister } from 'react-hook-form';
import type { OrderFormValues } from './OrderForm';

/**
 * The build specification, shown only when the chosen project type asks for one.
 *
 * Which section appears is configuration, not code: `project_types.spec_form` says
 * `heating`, `cooling`, or nothing at all. A repair job gets no section, and the office can
 * retype a category in settings without a deploy. The backend takes the form from the
 * project type too and ignores whatever the request claims, so the two cannot drift.
 */
export type SpecForm = 'heating' | 'cooling';

export function BuildSpecSection({
  form,
  register,
}: {
  form: SpecForm | null;
  register: UseFormRegister<OrderFormValues>;
}) {
  const t = useTranslations('spec');

  if (!form) return null;

  return (
    <>
      <div className="md:col-span-2 border-t border-steel-200 pt-4">
        <h3 className="text-section font-semibold">
          {form === 'cooling' ? t('coolingTitle') : t('heatingTitle')}
        </h3>
        <p className="mt-1 text-metadata text-steel-500">{t('explanation')}</p>
      </div>

      {/* Shared: a heated body and a cooled body both hold a temperature and are insulated.
          Duplicating these per variant would mean two columns to report on. */}
      <div>
        <label className="label" htmlFor="of-temp">
          {t('targetTemp')}
        </label>
        <input
          id="of-temp"
          inputMode="decimal"
          className="input font-mono"
          placeholder={form === 'cooling' ? '-18' : '20'}
          {...register('target_temp_c')}
        />
      </div>
      <div>
        <label className="label" htmlFor="of-insul">
          {t('insulation')}
        </label>
        <input
          id="of-insul"
          type="number"
          min={0}
          max={500}
          className="input font-mono"
          {...register('insulation_mm')}
        />
      </div>

      {form === 'cooling' ? (
        <>
          <div>
            <label className="label" htmlFor="of-cmake">
              {t('coolingUnitMake')}
            </label>
            <input id="of-cmake" className="input" {...register('cooling_unit_make')} />
          </div>
          <div>
            <label className="label" htmlFor="of-cmodel">
              {t('coolingUnitModel')}
            </label>
            <input id="of-cmodel" className="input" {...register('cooling_unit_model')} />
          </div>
          <div>
            <label className="label" htmlFor="of-atp">
              {t('atpClass')}
            </label>
            <input
              id="of-atp"
              className="input font-mono"
              placeholder="FRC"
              {...register('atp_class')}
            />
          </div>
          <div>
            <label className="label" htmlFor="of-comp">
              {t('compartments')}
            </label>
            <input
              id="of-comp"
              type="number"
              min={1}
              max={5}
              className="input font-mono"
              {...register('compartments')}
            />
          </div>
          <div>
            <label className="label" htmlFor="of-defrost">
              {t('defrost')}
            </label>
            <select id="of-defrost" className="input" {...register('defrost')}>
              <option value="">—</option>
              <option value="automatic">{t('defrostAutomatic')}</option>
              <option value="manual">{t('defrostManual')}</option>
              <option value="hot_gas">{t('defrostHotGas')}</option>
            </select>
          </div>
          <div className="flex items-end">
            <label className="flex items-center gap-2 text-body">
              <input type="checkbox" className="h-4 w-4" {...register('electric_standby')} />
              {t('electricStandby')}
            </label>
          </div>
        </>
      ) : (
        <>
          <div>
            <label className="label" htmlFor="of-hmake">
              {t('heaterMake')}
            </label>
            <input id="of-hmake" className="input" {...register('heater_make')} />
          </div>
          <div>
            <label className="label" htmlFor="of-hmodel">
              {t('heaterModel')}
            </label>
            <input id="of-hmodel" className="input" {...register('heater_model')} />
          </div>
          <div>
            <label className="label" htmlFor="of-kw">
              {t('heatOutput')}
            </label>
            <input
              id="of-kw"
              inputMode="decimal"
              className="input font-mono"
              {...register('heat_output_kw')}
            />
          </div>
          <div>
            <label className="label" htmlFor="of-fuel">
              {t('fuel')}
            </label>
            <select id="of-fuel" className="input" {...register('fuel')}>
              <option value="">—</option>
              <option value="diesel">{t('fuelDiesel')}</option>
              <option value="electric">{t('fuelElectric')}</option>
              <option value="lpg">{t('fuelLpg')}</option>
              <option value="engine_coolant">{t('fuelEngineCoolant')}</option>
            </select>
          </div>
          <div className="flex items-end">
            <label className="flex items-center gap-2 text-body">
              <input type="checkbox" className="h-4 w-4" {...register('thermostat')} />
              {t('thermostat')}
            </label>
          </div>
        </>
      )}

      <div className="md:col-span-2">
        <label className="label" htmlFor="of-specnotes">
          {t('notes')}
        </label>
        <textarea id="of-specnotes" rows={2} className="input" {...register('spec_notes')} />
      </div>
    </>
  );
}
