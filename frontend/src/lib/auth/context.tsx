'use client';

import { createContext, useCallback, useContext, useMemo, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { authApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import type { Role, User } from '@/types/api';

interface AuthState {
  user: User | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (email: string, password: string) => Promise<{ must_change_password: boolean }>;
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
    async (email: string, password: string) => {
      const res = await authApi.login({ email, password, client: 'web' });
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

  const value = useMemo<AuthState>(
    () => ({
      user: data ?? null,
      isLoading,
      isAuthenticated: !!data,
      login,
      logout,
      refetch: async () => {
        await refetch();
      },
    }),
    [data, isLoading, login, logout, refetch],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}

// Capability helpers mirror backend domain/role.rs (Role::can).
// UI-only: backend remains the security boundary.
// NOTE: lead stage changes require EditLeads (admin|office) — designers get
// 403 there, unlike order stage changes (ChangeStages). Use canEditLeads.
export function canEdit(user: User | null): boolean {
  return !!user && (user.role === 'admin' || user.role === 'office');
}

export function canEditPartners(user: User | null): boolean {
  return canEdit(user);
}

export function canEditLeads(user: User | null): boolean {
  return canEdit(user);
}

export function canEditOrders(user: User | null): boolean {
  return canEdit(user);
}

export function canChangeStage(user: User | null): boolean {
  return !!user && (user.role === 'admin' || user.role === 'office' || user.role === 'designer');
}

export function canManageBlockers(user: User | null): boolean {
  return canChangeStage(user);
}

export function canUploadMedia(user: User | null): boolean {
  return canChangeStage(user);
}

export function canAdmin(user: User | null): boolean {
  return !!user && user.role === 'admin';
}

export function roleLabel(role: Role, t: (key: string) => string): string {
  return t(`users.roles.${role}`);
}
