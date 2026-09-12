import type { ReactNode } from 'react';

export function PageHeader({
  title,
  subtitle,
  actions,
  size = 'page',
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  /** Record detail titles use the smaller record scale, not the page scale. */
  size?: 'page' | 'record';
}) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-4">
      <div>
        <h1
          className={`font-semibold tracking-tight ${
            size === 'record' ? 'text-record-title' : 'text-page-title'
          }`}
        >
          {title}
        </h1>
        {subtitle && <p className="mt-1 text-metadata text-steel-500">{subtitle}</p>}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </div>
  );
}
