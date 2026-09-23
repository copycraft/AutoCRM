'use client';

// The two recharts plots, split out of WorkloadCharts so the library loads on demand.
//
// recharts is ~100 kB of the reports route's bundle — more than the rest of the page put
// together — and nothing can be plotted until `GET /reports/workload` answers anyway.
// Loading it here, behind next/dynamic, means the download overlaps the request instead of
// blocking the paint, and the three totals above are on screen while it happens.
//
// Keep this file free of anything the parent needs synchronously: everything imported here
// is in the deferred chunk.

import { useTranslations } from 'next-intl';
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import type { WorkloadDay } from './WorkloadCharts';

/** `2026-09-12` → `09.12.` for axis ticks. */
function tick(date: string): string {
  return `${date.slice(5, 7)}.${date.slice(8, 10)}.`;
}

export function WorkloadChartsView({ days }: { days: WorkloadDay[] }) {
  const t = useTranslations('reports');

  return (
    <>
      <div className="card p-4">
        <h3 className="text-body font-semibold">{t('placedPerDay')}</h3>
        <div className="mt-2 h-64" dir="ltr">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={days} margin={{ top: 8, right: 8, bottom: 0, left: -12 }}>
              <CartesianGrid stroke="#D5DBDC" vertical={false} />
              <XAxis
                dataKey="date"
                tickFormatter={tick}
                tick={{ fontSize: 12 }}
                interval="preserveStartEnd"
              />
              <YAxis allowDecimals={false} tick={{ fontSize: 12 }} />
              <Tooltip
                labelFormatter={(d) => String(d)}
                formatter={(value, name) => [value, name]}
              />
              <Legend />
              <Bar dataKey="placed" name={t('placed')} fill="#1B2327" />
              <Bar dataKey="completed" name={t('completed')} fill="#2F7A3E" />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="card p-4">
        <h3 className="text-body font-semibold">{t('inWorkshopPerDay')}</h3>
        <div className="mt-2 h-64" dir="ltr">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={days} margin={{ top: 8, right: 8, bottom: 0, left: -12 }}>
              <CartesianGrid stroke="#D5DBDC" vertical={false} />
              <XAxis
                dataKey="date"
                tickFormatter={tick}
                tick={{ fontSize: 12 }}
                interval="preserveStartEnd"
              />
              <YAxis allowDecimals={false} tick={{ fontSize: 12 }} />
              <Tooltip labelFormatter={(d) => String(d)} />
              <Area
                dataKey="in_workshop"
                name={t('inWorkshop')}
                fill="#0F5C7A"
                fillOpacity={0.15}
                stroke="#0F5C7A"
                strokeWidth={2}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>
    </>
  );
}
