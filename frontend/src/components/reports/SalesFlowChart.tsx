'use client';

// The cumulative flow plot, split out so recharts loads on demand (see WorkloadChartsView).

import { useMemo } from 'react';
import { Area, AreaChart, CartesianGrid, Legend, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';

// Stage bands, from the first stage (bottom) up; repeats past ten stages.
const BANDS = ['#0F5C7A', '#3E7C93', '#6B9CAD', '#2F7A3E', '#5B9A63', '#8BB98F', '#B4801E', '#D0A24F', '#7A5C99', '#A28BBA'];

export function SalesFlowChart({
  points,
  stages,
}: {
  points: { day: string; stage_key: string; orders: number }[];
  stages: { key: string; label: string }[];
}) {
  const data = useMemo(() => {
    const byDay = new Map<string, Record<string, number | string>>();
    for (const p of points) {
      const row = byDay.get(p.day) ?? { day: p.day };
      row[p.stage_key] = p.orders;
      byDay.set(p.day, row);
    }
    return [...byDay.values()].sort((a, b) => String(a.day).localeCompare(String(b.day)));
  }, [points]);
  // Only stages that ever held an order, in pipeline order.
  const shown = stages.filter((s) => points.some((p) => p.stage_key === s.key));
  return (
    <div className="h-72" dir="ltr">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={{ top: 8, right: 8, bottom: 0, left: -16 }}>
          <CartesianGrid stroke="#D5DBDC" vertical={false} />
          <XAxis dataKey="day" tickFormatter={(d: string) => `${d.slice(5, 7)}.${d.slice(8, 10)}.`} fontSize={11} />
          <YAxis allowDecimals={false} fontSize={11} />
          <Tooltip />
          <Legend wrapperStyle={{ fontSize: 11 }} />
          {shown.map((s, i) => (
            <Area
              key={s.key}
              type="monotone"
              dataKey={s.key}
              name={s.label}
              stackId="flow"
              stroke={BANDS[i % BANDS.length]}
              fill={BANDS[i % BANDS.length]}
              fillOpacity={0.75}
            />
          ))}
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
