'use client';

import { useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { ApiError } from '@/lib/api/errors';
import { canAccessHr, canManageLeave, canManageRecruitment, useAuth } from '@/lib/auth/context';
import { cn } from '@/lib/utils/format';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { Directory } from '@/components/hr/EmployeeDirectory';
import { LeaveSection } from '@/components/hr/LeaveSection';
import { RecruitmentSection } from '@/components/hr/RecruitmentSection';
import { ChecklistSettings } from '@/components/hr/HrChecklists';

type Tab = 'people' | 'leave' | 'recruitment' | 'checklists';
type User = ReturnType<typeof useAuth>['user'];

/**
 * Each tab and the right behind it, the same one the server checks. The directory and the
 * checklists are HR proper (`hr_access`); leave and recruitment can be granted on their own,
 * so a workshop head may keep leave and an office hand may run the hiring.
 */
const TABS: { key: Tab; label: string; can: (u: User) => boolean }[] = [
  { key: 'people', label: 'tabPeople', can: canAccessHr },
  { key: 'leave', label: 'tabLeave', can: canManageLeave },
  { key: 'recruitment', label: 'tabRecruitment', can: canManageRecruitment },
  { key: 'checklists', label: 'tabChecklists', can: canAccessHr },
];

/** The HR module: the staff directory, leave and recruitment — the parts one has a right to. */
export function HrWorkspace() {
  const t = useTranslations('hr');
  const { user, isLoading } = useAuth();
  // A notification can open a tab directly (`?tab=recruitment` for a new applicant).
  const requested = useSearchParams()?.get('tab') ?? null;
  const [chosen, setChosen] = useState<string | null>(requested);
  if (isLoading) return <LoadingState />;

  const tabs = TABS.filter((x) => x.can(user));
  const tab = tabs.find((x) => x.key === chosen)?.key ?? tabs[0]?.key;
  // The backend refuses everyone else too; this only spares them a request and a 403.
  if (!tab) return <ErrorState error={new ApiError('forbidden', 403, 'forbidden')} />;

  return (
    <div>
      {tabs.length > 1 && (
        <div role="tablist" className="mt-6 flex gap-1 border-b border-steel-200">
          {tabs.map((x) => (
            <button
              key={x.key}
              role="tab"
              aria-selected={tab === x.key}
              onClick={() => setChosen(x.key)}
              className={cn(
                '-mb-px border-b-2 px-4 py-2 text-body font-medium transition-colors',
                tab === x.key ? 'border-steel-900 text-steel-900' : 'border-transparent text-steel-500 hover:text-steel-900',
              )}
            >
              {t(x.label)}
            </button>
          ))}
        </div>
      )}
      {tab === 'people' && <Directory />}
      {tab === 'leave' && <LeaveSection />}
      {tab === 'recruitment' && <RecruitmentSection />}
      {tab === 'checklists' && <ChecklistSettings />}
    </div>
  );
}
