'use client';

// Revenue and sales reports only for someone who may see them (0061). The server refuses
// the rest anyway; this keeps an empty error box off the page.

import { canViewBusinessReports, useAuth } from '@/lib/auth/context';

export function BusinessOnly({ children }: { children: React.ReactNode }) {
  const { user } = useAuth();
  return canViewBusinessReports(user) ? <>{children}</> : null;
}
