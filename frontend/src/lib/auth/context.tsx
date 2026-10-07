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

/// The admin role itself (user management is never granted per user).
export function canAdmin(user: SessionUser | null): boolean {
  return !!user && user.role === 'admin';
}

/// The HR module: admins, and users an admin granted it (the backend computes it into
/// `hr_access`, which is already true for admins).
export function canAccessHr(user: SessionUser | null): boolean {
  return !!user && user.hr_access;
}

/// Issuing invoices, marking them paid, and recording incoming ones.
export function canIssueInvoices(user: SessionUser | null): boolean {
  return hasCapability(user, 'issue_invoices');
}

/// Comments with mentions.
export function canComment(user: SessionUser | null): boolean {
  return hasCapability(user, 'comment');
}
