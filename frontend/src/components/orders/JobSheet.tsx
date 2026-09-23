'use client';

// The paper job sheet: plate + job number big enough to read across the yard, customer,
// vehicle, items with totals, open blockers, sign-off lines. Print-only (`hidden
// print:block`); the screen keeps its tabs. Global print CSS hides the sidebar, nav and
// every button/input, so this needs no print-specific layout beyond its own content.

import { useTranslations } from 'next-intl';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { Currency, ItemView, Blocker, Order, OrderValue, PartnerRef, StageView, Vehicle } from '@/lib/api/types';

export function JobSheet({
  order,
  partner,
  vehicles,
  items,
  value,
  blockers,
  stage,
  assigneeName,
}: {
  order: Order;
  partner: PartnerRef;
  vehicles: Vehicle[];
  items: ItemView[];
  value: OrderValue;
  blockers: Blocker[];
  stage: StageView;
  assigneeName: string;
}) {
  const t = useTranslations('orders');
  const currency = order.currency as Currency;
  const plate =
    vehicles.map((v) => v.plate).find(Boolean) ?? order.vehicle_plate ?? '—';
  const openBlockers = blockers.filter((b) => !b.resolved_at);

  return (
    <div className="hidden print:block">
      <div className="flex items-start justify-between border-b-2 border-steel-900 pb-3">
        <div>
          <p className="text-metadata font-medium tracking-wide">AUTOTHERM · {t('workSheet')}</p>
          <p className="font-mono text-record font-semibold">{plate}</p>
          <p className="text-section font-semibold">
            #{order.number} · {order.title}
          </p>
        </div>
        <div className="text-right text-metadata text-steel-900">
          <p>
            {t('sheetDate')}: <DateDisplay value={new Date().toISOString()} />
          </p>
          <p>{stage.label_hu}</p>
          <p>
            {t('assignedTo')}: {assigneeName}
          </p>
        </div>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-4 text-body">
        <div>
          <p className="text-metadata font-medium text-steel-500">{t('sheetCustomer')}</p>
          <p className="font-medium">{partner.name}</p>
        </div>
        <div>
          <p className="text-metadata font-medium text-steel-500">{t('sheetVehicle')}</p>
          {vehicles.length > 0 ? (
            vehicles.map((v) => (
              <p key={v.id} className="font-mono">
                {[v.plate, v.vin].filter(Boolean).join(' · ')}
                {[v.make, v.model, v.year].filter(Boolean).length > 0 && (
                  <span className="font-sans">
                    {' '}
                    ({[v.make, v.model, v.year].filter(Boolean).join(' ')})
                  </span>
                )}
              </p>
            ))
          ) : (
            <p className="font-mono">
              {[order.vehicle_plate, order.vehicle_vin].filter(Boolean).join(' · ') || '—'}
            </p>
          )}
          {order.due_date && (
            <p className="mt-1">
              {t('dueDate')}: <DateDisplay value={order.due_date} />
            </p>
          )}
        </div>
      </div>

      {(order.mileage_in != null ||
        order.intake_condition ||
        order.fuel_level ||
        order.key_count != null ||
        order.valuables_declared != null) && (
        <p className="mt-3 text-body">
          <span className="text-metadata text-steel-500">{t('intakeTitle')}: </span>
          {/* Joined rather than laid out: this is one line on a sheet that gets signed,
              and every part of it is something a customer later disputes. A recorded
              "no valuables" prints as such — an omission and a denial must not look
              the same on paper. */}
          {[
            order.mileage_in != null
              ? `${order.mileage_in.toLocaleString('hu-HU')} km`
              : null,
            order.fuel_level ? `${t('intakeFuel')}: ${order.fuel_level}` : null,
            order.key_count != null ? `${t('intakeKeys')}: ${order.key_count}` : null,
            order.valuables_declared == null
              ? null
              : `${t('intakeValuables')}: ${
                  order.valuables_declared
                    ? (order.valuables ?? t('intakeValuablesUnlisted'))
                    : t('intakeValuablesNone')
                }`,
            order.intake_condition,
          ]
            .filter(Boolean)
            .join(' · ')}
        </p>
      )}
      {order.description && <p className="mt-3 text-body">{order.description}</p>}

      {items.length > 0 && (
        <table className="mt-4 w-full text-body">
          <thead>
            <tr className="border-b border-steel-900 text-left text-metadata">
              <th className="py-1 font-medium">{t('description')}</th>
              <th className="py-1 text-right font-medium">{t('quantity')}</th>
              <th className="py-1 text-right font-medium">{t('unitPrice')}</th>
              <th className="py-1 text-right font-medium">{t('lineTotal')}</th>
            </tr>
          </thead>
          <tbody>
            {items.map((it) => (
              <tr key={it.id} className="border-b border-steel-200">
                <td className="py-1">{it.description}</td>
                <td className="py-1 text-right font-mono">{it.quantity}</td>
                <td className="py-1 text-right">
                  <Money minor={it.unit_price} currency={it.currency as Currency} />
                </td>
                <td className="py-1 text-right">
                  <Money minor={it.line_total_minor} currency={it.currency as Currency} />
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr className="font-semibold">
              <td className="py-1" colSpan={3}>
                {t('total')} ({currency})
              </td>
              <td className="py-1 text-right">
                <Money minor={value.total_minor} currency={currency} />
              </td>
            </tr>
          </tfoot>
        </table>
      )}

      {openBlockers.length > 0 && (
        <div className="mt-4">
          <p className="text-metadata font-medium text-steel-500">{t('sheetBlockers')}</p>
          <ul className="mt-1 space-y-1 text-body">
            {openBlockers.map((b) => (
              <li key={b.id}>
                <span className="font-medium">{b.what}</span>
                {b.responsible_partner_name && ` · ${b.responsible_partner_name}`}
                {b.due_date && (
                  <>
                    {' '}· <DateDisplay value={b.due_date} />
                  </>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="mt-8 grid grid-cols-2 gap-8 text-body">
        <div>
          <p className="text-metadata text-steel-500">{t('sheetSignDone')}</p>
          <p className="mt-10 border-t border-steel-900 pt-1"> </p>
        </div>
        <div>
          <p className="text-metadata text-steel-500">{t('sheetSignReceived')}</p>
          <p className="mt-10 border-t border-steel-900 pt-1"> </p>
        </div>
      </div>
    </div>
  );
}
