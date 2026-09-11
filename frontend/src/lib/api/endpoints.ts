import { api } from '@/lib/api/client';
import type {
  AuditEntry,
  Blocker,
  Contact,
  EmailDetail,
  EmailMessage,
  EmailPreview,
  EmailTemplate,
  InitiateUploadRequest,
  InitiateUploadResponse,
  Items,
  Lead,
  LeadDetail,
  LeadSummary,
  LoginRequest,
  OrderBody,
  OrderDetail,
  OrderDocument,
  OrderImage,
  OrderItemView,
  OrderRef,
  OrderRow,
  OrderSummary,
  Partner,
  PartnerDetail,
  ProjectType,
  SessionInfo,
  Settings,
  StageChange,
  StageDefinition,
  StageEntry,
  User,
  AdminStatus,
  Job,
} from '@/types/api';

// ── Auth ──
export const authApi = {
  login: (body: LoginRequest) => api.post<{ must_change_password: boolean }>('/auth/login', body),
  logout: () => api.post<void>('/auth/logout'),
  me: () => api.get<User>('/auth/me'),
  changePassword: (current_password: string, new_password: string) =>
    api.post<void>('/auth/password', { current_password, new_password }),
  sessions: () => api.get<Items<SessionInfo>>('/auth/sessions'),
  deleteSession: (id: number) => api.del<void>(`/auth/sessions/${id}`),
};

// ── Users (admin) ──
export const usersApi = {
  list: () => api.get<Items<User>>('/users'),
  create: (body: { email: string; display_name: string; role: User['role']; temporary_password: string }) =>
    api.post<User>('/users', body),
  patch: (id: number, body: { display_name?: string; role?: User['role']; is_active?: boolean }) =>
    api.patch<User>(`/users/${id}`, body),
  setPassword: (id: number, temporary_password: string) =>
    api.post<void>(`/users/${id}/password`, { temporary_password }),
  revokeSessions: (id: number) => api.post<void>(`/users/${id}/revoke-sessions`),
};

// ── Partners ──
export const partnersApi = {
  list: (params?: {
    q?: string;
    kind?: string;
    include_archived?: boolean;
    limit?: number;
    offset?: number;
  }) => api.get<Items<Partner>>('/partners', params),
  get: (id: number) => api.get<PartnerDetail>(`/partners/${id}`),
  create: (body: Record<string, unknown>) => api.post<Partner>('/partners', body),
  patch: (id: number, body: Record<string, unknown>) => api.patch<Partner>(`/partners/${id}`, body),
  archive: (id: number) => api.post<void>(`/partners/${id}/archive`),
  unarchive: (id: number) => api.post<void>(`/partners/${id}/unarchive`),
  contacts: (partnerId: number) => api.get<Items<Contact>>(`/partners/${partnerId}/contacts`),
  createContact: (partnerId: number, body: Record<string, unknown>) =>
    api.post<Contact>(`/partners/${partnerId}/contacts`, body),
  patchContact: (id: number, body: Record<string, unknown>) => api.patch<Contact>(`/contacts/${id}`, body),
  archiveContact: (id: number) => api.post<void>(`/contacts/${id}/archive`),
};

// ── Leads ──
export const leadsApi = {
  list: (params?: {
    q?: string;
    stage?: string;
    assigned_to?: number;
    open?: boolean;
    limit?: number;
    offset?: number;
  }) => api.get<Items<LeadSummary>>('/leads', params),
  get: (id: number) => api.get<LeadDetail>(`/leads/${id}`),
  create: (body: Record<string, unknown>) => api.post<Lead>('/leads', body),
  patch: (id: number, body: Record<string, unknown>) => api.patch<Lead>(`/leads/${id}`, body),
  stage: (id: number, body: { stage: string; note?: string }) =>
    api.post<StageChange>(`/leads/${id}/stage`, body),
  convert: (id: number, body: OrderBody) => api.post<OrderRef>(`/leads/${id}/convert`, body),
};

// ── Orders ──
export const ordersApi = {
  list: (params?: {
    q?: string; stage?: string; partner_id?: number; project_type_id?: number;
    assigned_to?: number; open?: boolean; limit?: number; offset?: number;
  }) => api.get<Items<OrderSummary>>('/orders', params),
  get: (id: number) => api.get<OrderDetail>(`/orders/${id}`),
  create: (body: Record<string, unknown>) => api.post<OrderRow>('/orders', body),
  patch: (id: number, body: Record<string, unknown>) => api.patch<OrderRow>(`/orders/${id}`, body),
  stage: (id: number, body: { stage: string; note?: string }) =>
    api.post<StageChange>(`/orders/${id}/stage`, body),
  stages: (id: number) => api.get<Items<StageEntry>>(`/orders/${id}/stages`),
  audit: (id: number, params?: { limit?: number }) =>
    api.get<Items<AuditEntry>>(`/orders/${id}/audit`, params),
  items: (orderId: number) => api.get<Items<OrderItemView>>(`/orders/${orderId}/items`),
  createItem: (orderId: number, body: Record<string, unknown>) =>
    api.post<OrderItemView>(`/orders/${orderId}/items`, body),
  patchItem: (id: number, body: Record<string, unknown>) =>
    api.patch<OrderItemView>(`/order-items/${id}`, body),
  deleteItem: (id: number) => api.del<void>(`/order-items/${id}`),
};

// ── Blockers ──
export const blockersApi = {
  list: (params?: { responsible_partner_id?: number }) =>
    api.get<Items<Blocker>>('/blockers', params),
  forOrder: (orderId: number) => api.get<Items<Blocker>>(`/orders/${orderId}/blockers`),
  create: (orderId: number, body: Record<string, unknown>) =>
    api.post<Blocker>(`/orders/${orderId}/blockers`, body),
  patch: (id: number, body: Record<string, unknown>) => api.patch<Blocker>(`/blockers/${id}`, body),
  resolve: (id: number, note?: string) => api.post<Blocker>(`/blockers/${id}/resolve`, { note }),
  reopen: (id: number) => api.post<Blocker>(`/blockers/${id}/reopen`),
};

// ── Media ──
export const mediaApi = {
  images: (orderId: number, category?: string) =>
    api.get<Items<OrderImage>>(`/orders/${orderId}/images`, { category }),
  original: (imageId: number) => api.get<{ url: string; sha256: string }>(`/images/${imageId}/original`),
  deleteImage: (id: number) => api.del<void>(`/images/${id}`),
  documents: (orderId: number) => api.get<Items<OrderDocument>>(`/orders/${orderId}/documents`),
  downloadDocument: (id: number) => api.get<{ url: string }>(`/documents/${id}/download`),
  deleteDocument: (id: number) => api.del<void>(`/documents/${id}`),
  initiateUpload: (orderId: number, body: InitiateUploadRequest) =>
    api.post<InitiateUploadResponse>(`/orders/${orderId}/uploads`, body),
  completeUpload: (ticket: string) => api.post<{ image_id?: number; document_id?: number }>('/uploads/complete', { ticket }),
};

// ── Email ──
export const emailApi = {
  list: (params?: { order_id?: number; lead_id?: number; partner_id?: number; status?: string; attention?: boolean }) =>
    api.get<Items<EmailMessage>>('/emails', params),
  get: (id: number) => api.get<EmailDetail>(`/emails/${id}`),
  preview: (body: Record<string, unknown>) => api.post<EmailPreview>('/emails/preview', body),
  send: (body: Record<string, unknown>) => api.post<EmailMessage>('/emails', body),
  cancel: (id: number) => api.post<void>(`/emails/${id}/cancel`),
  retry: (id: number) => api.post<void>(`/emails/${id}/retry`),
  templates: () => api.get<Items<EmailTemplate>>('/email-templates'),
  updateTemplate: (id: number, body: Record<string, unknown>) =>
    api.patch<EmailTemplate>(`/email-templates/${id}`, body),
};

// ── Config ──
export const configApi = {
  stages: (entity?: string) => api.get<Items<StageDefinition>>('/stage-definitions', { entity }),
  createStage: (body: Record<string, unknown>) => api.post<StageDefinition>('/stage-definitions', body),
  patchStage: (id: number, body: Record<string, unknown>) =>
    api.patch<StageDefinition>(`/stage-definitions/${id}`, body),
  projectTypes: () => api.get<Items<ProjectType>>('/project-types'),
  patchProjectType: (id: number, body: Record<string, unknown>) =>
    api.patch<ProjectType>(`/project-types/${id}`, body),
  settings: () => api.get<Settings>('/settings'),
  saveSettings: (body: Settings) => api.put<Settings>('/settings', body),
};

// ── Reports / Admin ──
export const reportsApi = {
  volume: (params?: Record<string, string | number | boolean | undefined>) =>
    api.get<Record<string, unknown>>('/reports/volume', params),
  stageDurations: (params?: Record<string, string | number | undefined>) =>
    api.get<Record<string, unknown>>('/reports/stage-durations', params),
  throughput: (params?: Record<string, string | undefined>) =>
    api.get<Record<string, unknown>>('/reports/throughput', params),
  stalled: () => api.get<Items<Record<string, unknown>>>('/reports/stalled'),
  blockerLoad: () => api.get<Items<Record<string, unknown>>>('/reports/blocker-load'),
  fxRates: (params?: { base?: string }) => api.get<Record<string, unknown>>('/reports/fx-rates', params),
};

export const adminApi = {
  status: () => api.get<AdminStatus>('/admin/status'),
  jobs: (state?: string) => api.get<Items<Job>>('/admin/jobs', { state }),
  retryJob: (id: number) => api.post<void>(`/admin/jobs/${id}/retry`),
  fetchFx: (from: string, to: string) => api.post<void>('/admin/fx/fetch', { from, to }),
  run: (kind: 'nudge_blockers' | 'stalled_orders') => api.post<void>(`/admin/run/${kind}`),
};
