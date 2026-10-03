'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { ApiError } from '@/lib/api/errors';
import { canAccessHr, useAuth } from '@/lib/auth/context';
import { cn } from '@/lib/utils/format';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { Directory } from '@/components/hr/EmployeeDirectory';
import { LeaveSection } from '@/components/hr/LeaveSection';

type Tab = 'people' | 'leave';

/** The HR module: the staff directory and leave. Gated here once for both tabs. */
export function HrWorkspace() {
  const t = useTranslations('hr');
  const { user, isLoading } = useAuth();
  const [tab, setTab] = useState<Tab>('people');
  if (isLoading) return <LoadingState />;
  // The backend refuses everyone else too; this only spares them a request and a 403.
  if (!canAccessHr(user)) return <ErrorState error={new ApiError('forbidden', 403, 'forbidden')} />;

  const tabs: { key: Tab; label: string }[] = [
    { key: 'people', label: t('tabPeople') },
    { key: 'leave', label: t('tabLeave') },
  ];
  return (
    <div>
      <div role="tablist" className="mt-6 flex gap-1 border-b border-steel-200">
        {tabs.map((x) => (
          <button
            key={x.key}
            role="tab"
            aria-selected={tab === x.key}
            onClick={() => setTab(x.key)}
            className={cn(
              '-mb-px border-b-2 px-4 py-2 text-body font-medium transition-colors',
              tab === x.key ? 'border-steel-900 text-steel-900' : 'border-transparent text-steel-500 hover:text-steel-900',
            )}
          >
            {x.label}
          </button>
        ))}
      </div>
      {tab === 'people' ? <Directory /> : <LeaveSection />}
    </div>
  );
}
