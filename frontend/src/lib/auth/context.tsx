'use client';

import { createContext, useCallback, useContext, useMemo, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { authApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import type { Capability, LoginResponse, SessionUser } from '@/lib/api/types';

interface AuthState {
  user: SessionUser | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (email: string, password: string, totpCode?: string) => Promise<LoginResponse>;
  logout: () => Promise<void>;
  refetch: () => Promise<void>;
}

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const qc = useQueryClient();
  const { data, isLoading, refetch } = useQuery({
    queryKey: qk.me,
    queryFn: () => authApi.me(),
    retry: false,
  });

  const login = useCallback(
    async (email: string, password: string, totpCode?: string) => {
      const res = await authApi.login({ email, password, client: 'web', totp_code: totpCode || undefined });
      await qc.invalidateQueries({ queryKey: qk.me });
      await refetch();
      return res;
    },
    [qc, refetch],
  );

  const logout = useCallback(async () => {
    try {
      await authApi.logout();
    } finally {
      qc.setQueryData(qk.me, null);
      await qc.invalidateQueries({ queryKey: qk.me });
    }
  }, [qc]);

  const value = useMemo<AuthState>(() => {
    const user = data?.user ?? null;
    return {
      user,
      isLoading,
      isAuthenticated: user !== null,
      login,
      logout,
      refetch: async () => {
        await refetch();
      },
    };
  }, [data, isLoading, login, logout, refetch]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}

// Capability helpers. The backend sends the effective set in `capabilities`: the role's
// defaults plus whatever an admin granted this user (domain/role.rs, Capability).
// UI-only: backend remains the security boundary.
// NOTE: lead stage changes require EditLeads — use canEditLeads there, not canChangeStage.
export function hasCapability(user: SessionUser | null, capability: Capability): boolean {
  return !!user && (user.capabilities ?? []).includes(capability);
}

export function canEdit(user: SessionUser | null): boolean {
  return hasCapability(user, 'edit_orders');
}

export function canEditPartners(user: SessionUser | null): boolean {
  return hasCapability(user, 'edit_partners');
}

export function canEditLeads(user: SessionUser | null): boolean {
  return hasCapability(user, 'edit_leads');
}

export function canEditOrders(user: SessionUser | null): boolean {
  return hasCapability(user, 'edit_orders');
}

export function canChangeStage(user: SessionUser | null): boolean {
  return hasCapability(user, 'change_stages');
}

export function canManageBlockers(user: SessionUser | null): boolean {
  return hasCapability(user, 'manage_blockers');
}

export function canUploadMedia(user: SessionUser | null): boolean {
  return hasCapability(user, 'upload_media');
}

/// Sending mail (manual, quotation, newsletter).
export function canSendEmail(user: SessionUser | null): boolean {
  return hasCapability(user, 'send_email');
}

/// User accounts and the role grid. Never granted per user, so in practice the admin role —
/// but asked as a right, like every other gate here, so the screens follow the server.
export function canManageUsers(user: SessionUser | null): boolean {
  return hasCapability(user, 'manage_users');
}

/// System settings: the mail server, sending windows, where alerts go.
export function canManageSettings(user: SessionUser | null): boolean {
  return hasCapability(user, 'manage_settings');
}

/// Configuration: stages, project types, templates, departments, the yard, the questionnaire.
export function canManageConfiguration(user: SessionUser | null): boolean {
  return hasCapability(user, 'manage_configuration');
}

/// Background work: retrying failed mail and jobs, cancelling anyone's queued mail.
export function canOperateSystem(user: SessionUser | null): boolean {
  return hasCapability(user, 'operate_system');
}

/// Technically annulling a NAV data report.
export function canAnnulInvoices(user: SessionUser | null): boolean {
  return hasCapability(user, 'annul_invoices');
}

/// The HR staff directory: personal data, documents, checklists. Admins, and users an admin
/// gave `hr_access` (the backend computes it into the flag, already true for admins).
export function canAccessHr(user: SessionUser | null): boolean {
  return !!user && user.hr_access;
}

/// Leave and absences; the staff come with it by name only. `hr_access` brings it too.
export function canManageLeave(user: SessionUser | null): boolean {
  return hasCapability(user, 'manage_leave');
}

/// Job listings, ads and applicants. `hr_access` brings it too.
export function canManageRecruitment(user: SessionUser | null): boolean {
  return hasCapability(user, 'manage_recruitment');
}

/// Any part of the HR module: the menu item shows, and the page offers the parts one has.
export function canEnterHr(user: SessionUser | null): boolean {
  return canAccessHr(user) || canManageLeave(user) || canManageRecruitment(user);
}

/// Issuing invoices, marking them paid, and recording incoming ones.
export function canIssueInvoices(user: SessionUser | null): boolean {
  return hasCapability(user, 'issue_invoices');
}

/// Comments with mentions.
export function canComment(user: SessionUser | null): boolean {
  return hasCapability(user, 'comment');
}

// Seeing (0061): the server leaves out what these say no to; the screens follow, so nobody
// is shown an empty table or a 403 where a menu item could simply be absent.
export function canViewPrices(user: SessionUser | null): boolean {
  return hasCapability(user, 'view_prices');
}

export function canViewInvoices(user: SessionUser | null): boolean {
  return hasCapability(user, 'view_invoices');
}

export function canViewIncomingInvoices(user: SessionUser | null): boolean {
  return hasCapability(user, 'view_incoming_invoices');
}

export function canViewSales(user: SessionUser | null): boolean {
  return hasCapability(user, 'view_sales');
}

export function canViewContacts(user: SessionUser | null): boolean {
  return hasCapability(user, 'view_contacts');
}

export function canViewBusinessReports(user: SessionUser | null): boolean {
  return hasCapability(user, 'view_business_reports');
}

/// Átvétel and átadás: the walkaround protocols (0063).
export function canHandleHandovers(user: SessionUser | null): boolean {
  return hasCapability(user, 'handle_handovers');
}
