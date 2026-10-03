// One function per backend operation. Response types come from the generated contract and
// every response is validated by the generated zod schema passed to `request`: if the schema's
// output and the declared return type disagree, this file does not compile.

import { request, requestNoContent } from './client';
import * as s from './zod/zod.gen';
import type { components } from './schema.gen';
import type { QueryOf, StageEntity } from './types';

type S = components['schemas'];

// ── Auth ──
export const authApi = {
  login: (body: S['LoginBody']): Promise<S['LoginResponse']> =>
    request('/auth/login', s.zAuthLoginResponse, { method: 'POST', body }),
  logout: (): Promise<void> => requestNoContent('/auth/logout', { method: 'POST' }),
  me: (): Promise<S['MeResponse']> => request('/auth/me', s.zAuthMeResponse),
  changePassword: (body: S['ChangePasswordBody']): Promise<void> =>
    requestNoContent('/auth/password', { method: 'POST', body }),
  sessions: (): Promise<S['Items_SessionView']> => request('/auth/sessions', s.zAuthListSessionsResponse),
  revokeSession: (id: number): Promise<void> =>
    requestNoContent(`/auth/sessions/${id}`, { method: 'DELETE' }),
  preferences: (): Promise<S['UserSettings']> =>
    request('/auth/preferences', s.zAuthGetPreferencesResponse),
  savePreferences: (body: S['PreferencesBody']): Promise<S['UserSettings']> =>
    request('/auth/preferences', s.zAuthPutPreferencesResponse, { method: 'PUT', body }),
};

// ── Users (admin) ──
export const usersApi = {
  list: (): Promise<S['Items_User']> => request('/users', s.zUsersListResponse),
  create: (body: S['CreateUser']): Promise<S['User']> =>
    request('/users', s.zUsersCreateResponse, { method: 'POST', body }),
  update: (id: number, body: S['UpdateUser']): Promise<S['User']> =>
    request(`/users/${id}`, s.zUsersUpdateResponse, { method: 'PATCH', body }),
  resetPassword: (id: number, body: S['ResetPassword']): Promise<void> =>
    requestNoContent(`/users/${id}/password`, { method: 'POST', body }),
  revokeSessions: (id: number): Promise<S['RevokedSessions']> =>
    request(`/users/${id}/revoke-sessions`, s.zUsersRevokeSessionsResponse, { method: 'POST' }),
};

// ── Partners ──
export const partnersApi = {
  list: (search: QueryOf<'partners_search'> = {}): Promise<S['Items_Partner']> =>
    request('/partners', s.zPartnersSearchResponse, { search }),
  get: (id: number): Promise<S['PartnerDetail']> => request(`/partners/${id}`, s.zPartnersDetailResponse),
  create: (body: S['CreatePartner']): Promise<S['Partner']> =>
    request('/partners', s.zPartnersCreateResponse, { method: 'POST', body }),
  patch: (id: number, body: S['PatchPartner']): Promise<S['Partner']> =>
    request(`/partners/${id}`, s.zPartnersUpdateResponse, { method: 'PATCH', body }),
  archive: (id: number): Promise<void> => requestNoContent(`/partners/${id}/archive`, { method: 'POST' }),
  unarchive: (id: number): Promise<void> =>
    requestNoContent(`/partners/${id}/unarchive`, { method: 'POST' }),
  contacts: (partnerId: number, search: QueryOf<'partners_list_contacts'> = {}): Promise<S['Items_Contact']> =>
    request(`/partners/${partnerId}/contacts`, s.zPartnersListContactsResponse, { search }),
  createContact: (partnerId: number, body: S['ContactBody']): Promise<S['Contact']> =>
    request(`/partners/${partnerId}/contacts`, s.zPartnersCreateContactResponse, { method: 'POST', body }),
  patchContact: (id: number, body: S['ContactBody']): Promise<S['Contact']> =>
    request(`/contacts/${id}`, s.zPartnersUpdateContactResponse, { method: 'PATCH', body }),
  archiveContact: (id: number): Promise<void> =>
    requestNoContent(`/contacts/${id}/archive`, { method: 'POST' }),
};

// ── Leads ──
export const leadsApi = {
  list: (search: QueryOf<'leads_search'> = {}): Promise<S['Items_LeadSummary']> =>
    request('/leads', s.zLeadsSearchResponse, { search }),
  get: (id: number): Promise<S['LeadDetail']> => request(`/leads/${id}`, s.zLeadsDetailResponse),
  create: (body: S['LeadBody']): Promise<S['Lead']> =>
    request('/leads', s.zLeadsCreateResponse, { method: 'POST', body }),
  patch: (id: number, body: S['LeadBody']): Promise<S['Lead']> =>
    request(`/leads/${id}`, s.zLeadsUpdateResponse, { method: 'PATCH', body }),
  stage: (id: number, body: S['StageBody']): Promise<S['StageChange']> =>
    request(`/leads/${id}/stage`, s.zLeadsChangeStageResponse, { method: 'POST', body }),
  transitions: (id: number): Promise<S['Items_TransitionOption']> =>
    request(`/leads/${id}/transitions`, s.zLeadsTransitionsResponse),
  convert: (id: number, body: S['OrderBody']): Promise<S['Order']> =>
    request(`/leads/${id}/convert`, s.zLeadsConvertResponse, { method: 'POST', body }),
  quotation: (id: number, body: S['QuotationRequest']): Promise<S['QuotationSent']> =>
    request(`/leads/${id}/quotation`, s.zLeadsQuotationResponse, { method: 'POST', body }),
};

// ── Orders ──
export const ordersApi = {
  list: (search: QueryOf<'orders_search'> = {}): Promise<S['Items_OrderSummary']> =>
    request('/orders', s.zOrdersSearchResponse, { search }),
  get: (id: number): Promise<S['OrderDetail']> => request(`/orders/${id}`, s.zOrdersDetailResponse),
  create: (body: S['OrderBody']): Promise<S['Order']> =>
    request('/orders', s.zOrdersCreateResponse, { method: 'POST', body }),
  patch: (id: number, body: S['PatchOrder']): Promise<S['Order']> =>
    request(`/orders/${id}`, s.zOrdersUpdateResponse, { method: 'PATCH', body }),
  stage: (id: number, body: S['StageBody']): Promise<S['StageChange']> =>
    request(`/orders/${id}/stage`, s.zOrdersChangeStageResponse, { method: 'POST', body }),
  transitions: (id: number): Promise<S['Items_TransitionOption']> =>
    request(`/orders/${id}/transitions`, s.zOrdersTransitionsResponse),
  stages: (id: number): Promise<S['Items_StageEntry']> =>
    request(`/orders/${id}/stages`, s.zOrdersStageHistoryResponse),
  audit: (id: number, search: QueryOf<'orders_audit_trail'> = {}): Promise<S['Items_AuditEntry']> =>
    request(`/orders/${id}/audit`, s.zOrdersAuditTrailResponse, { search }),
  notes: (id: number): Promise<S['Items_OrderNote']> =>
    request(`/orders/${id}/notes`, s.zOrdersNotesResponse),
  putSpec: (id: number, body: S['SpecBody']): Promise<S['OrderSpec']> =>
    request(`/orders/${id}/spec`, s.zOrdersPutSpecResponse, { method: 'PUT', body }),
  items: (orderId: number): Promise<S['Items_ItemView']> =>
    request(`/orders/${orderId}/items`, s.zOrdersListItemsResponse),
  createItem: (orderId: number, body: S['AddItem']): Promise<S['ItemView']> =>
    request(`/orders/${orderId}/items`, s.zOrdersAddItemResponse, { method: 'POST', body }),
  patchItem: (id: number, body: S['PatchItem']): Promise<S['ItemView']> =>
    request(`/order-items/${id}`, s.zOrdersUpdateItemResponse, { method: 'PATCH', body }),
  deleteItem: (id: number): Promise<void> => requestNoContent(`/order-items/${id}`, { method: 'DELETE' }),
};

// ── Invoicing ──
// Issuing is asynchronous: POST answers 202 with the invoice in `submitting`, and the
// screen polls `get` until NAV has decided. Proformas are the other shape entirely — no
// NAV call, no status, no chain — and deliberately live under their own paths.
export const invoicesApi = {
  forOrder: (orderId: number): Promise<S['Items_Invoice']> =>
    request(`/orders/${orderId}/invoices`, s.zInvoicesListForOrderResponse),
  // The Számlázó page: every invoice and storno, newest first, across orders.
  all: (search: QueryOf<'invoices_list_all'> = {}): Promise<S['Items_BilledInvoice']> =>
    request('/invoices', s.zInvoicesListAllResponse, { search }),
  get: (id: number): Promise<S['InvoiceDetail']> =>
    request(`/invoices/${id}`, s.zInvoicesDetailResponse),
  create: (orderId: number, body: S['IssueRequest']): Promise<S['Invoice']> =>
    request(`/orders/${orderId}/invoices`, s.zInvoicesCreateResponse, { method: 'POST', body }),
  storno: (id: number, body: S['StornoRequest']): Promise<S['Invoice']> =>
    request(`/invoices/${id}/storno`, s.zInvoicesStornoResponse, { method: 'POST', body }),
  annul: (id: number, body: S['AnnulRequest']): Promise<S['Invoice']> =>
    request(`/invoices/${id}/annul`, s.zInvoicesAnnulResponse, { method: 'POST', body }),
  chain: (id: number): Promise<S['Items_ChainStep']> =>
    request(`/invoices/${id}/chain`, s.zInvoicesChainResponse),
  refetchPdf: (id: number): Promise<S['Invoice']> =>
    request(`/invoices/${id}/pdf`, s.zInvoicesRefetchPdfResponse, { method: 'POST' }),
  proformas: (orderId: number): Promise<S['Items_Proforma']> =>
    request(`/orders/${orderId}/proformas`, s.zInvoicesListProformasResponse),
  // The Számlázó page: every díjbekérő, newest first, across orders.
  proformasAll: (): Promise<S['Items_BilledProforma']> =>
    request('/proformas', s.zInvoicesListAllProformasResponse),
  createProforma: (orderId: number, body: S['ProformaRequest']): Promise<S['ProformaCreated']> =>
    request(`/orders/${orderId}/proformas`, s.zInvoicesCreateProformaResponse, {
      method: 'POST',
      body,
    }),
};

// ── Blockers ──
export const blockersApi = {
  listOpen: (search: QueryOf<'blockers_list_open'> = {}): Promise<S['Items_Blocker']> =>
    request('/blockers', s.zBlockersListOpenResponse, { search }),
  forOrder: (orderId: number): Promise<S['Items_Blocker']> =>
    request(`/orders/${orderId}/blockers`, s.zBlockersListForOrderResponse),
  create: (orderId: number, body: S['BlockerBody']): Promise<S['Blocker']> =>
    request(`/orders/${orderId}/blockers`, s.zBlockersCreateResponse, { method: 'POST', body }),
  patch: (id: number, body: S['BlockerBody']): Promise<S['Blocker']> =>
    request(`/blockers/${id}`, s.zBlockersUpdateResponse, { method: 'PATCH', body }),
  resolve: (id: number, body: S['ResolveBody']): Promise<S['Blocker']> =>
    request(`/blockers/${id}/resolve`, s.zBlockersResolveResponse, { method: 'POST', body }),
  reopen: (id: number): Promise<S['Blocker']> =>
    request(`/blockers/${id}/reopen`, s.zBlockersReopenResponse, { method: 'POST' }),
};

// ── Migration provenance ──
// The MiniCRM source record behind a migrated row. Its own endpoint rather than a field on
// the detail responses: it is the whole source record and most screen loads do not want it.
export const rawImportApi = {
  partner: (id: number): Promise<S['RawImportView']> =>
    request(`/partners/${id}/raw-import`, s.zMigrationPartnerRawImportResponse),
  lead: (id: number): Promise<S['RawImportView']> =>
    request(`/leads/${id}/raw-import`, s.zMigrationLeadRawImportResponse),
  order: (id: number): Promise<S['RawImportView']> =>
    request(`/orders/${id}/raw-import`, s.zMigrationOrderRawImportResponse),
};

// ── Media ──
export const mediaApi = {
  images: (orderId: number, search: QueryOf<'media_list_images'> = {}): Promise<S['Items_ImageView']> =>
    request(`/orders/${orderId}/images`, s.zMediaListImagesResponse, { search }),
  original: (imageId: number): Promise<S['OriginalImageUrl']> =>
    request(`/images/${imageId}/original`, s.zMediaOriginalUrlResponse),
  deleteImage: (id: number): Promise<void> => requestNoContent(`/images/${id}`, { method: 'DELETE' }),
  documents: (orderId: number): Promise<S['Items_Document']> =>
    request(`/orders/${orderId}/documents`, s.zMediaListDocumentsResponse),
  searchDocuments: (search: QueryOf<'media_search_documents'> = {}): Promise<S['Items_Document']> =>
    request('/documents', s.zMediaSearchDocumentsResponse, { search }),
  downloadDocument: (id: number): Promise<S['DownloadUrl']> =>
    request(`/documents/${id}/download`, s.zMediaDocumentUrlResponse),
  deleteDocument: (id: number): Promise<void> => requestNoContent(`/documents/${id}`, { method: 'DELETE' }),
  requestUpload: (orderId: number, body: S['UploadRequest']): Promise<S['UploadResponse']> =>
    request(`/orders/${orderId}/uploads`, s.zMediaRequestUploadResponse, { method: 'POST', body }),
  completeUpload: (body: S['CompleteBody']): Promise<S['Completed']> =>
    request('/uploads/complete', s.zMediaCompleteUploadResponse, { method: 'POST', body }),
};

// ── Email ──
export const emailApi = {
  list: (search: QueryOf<'email_list'> = {}): Promise<S['Items_EmailSummary']> =>
    request('/emails', s.zEmailListResponse, { search }),
  get: (id: number): Promise<S['EmailMessage']> => request(`/emails/${id}`, s.zEmailDetailResponse),
  preview: (body: S['ComposeRequest']): Promise<S['Preview']> =>
    request('/emails/preview', s.zEmailPreviewResponse, { method: 'POST', body }),
  send: (body: S['ComposeRequest']): Promise<S['EmailMessage']> =>
    request('/emails', s.zEmailSendResponse, { method: 'POST', body }),
  cancel: (id: number): Promise<void> => requestNoContent(`/emails/${id}/cancel`, { method: 'POST' }),
  retry: (id: number): Promise<void> => requestNoContent(`/emails/${id}/retry`, { method: 'POST' }),
  templates: (): Promise<S['Items_EmailTemplate']> => request('/email-templates', s.zEmailListTemplatesResponse),
  createTemplate: (body: S['CreateTemplate']): Promise<S['EmailTemplate']> =>
    request('/email-templates', s.zEmailCreateTemplateResponse, { method: 'POST', body }),
  updateTemplate: (id: number, body: S['PatchTemplate']): Promise<S['EmailTemplate']> =>
    request(`/email-templates/${id}`, s.zEmailUpdateTemplateResponse, { method: 'PATCH', body }),
  variables: (): Promise<S['Items_TemplateVariable']> =>
    request('/email-templates/variables', s.zEmailVariablesResponse),
  suppressions: (): Promise<S['Items_Suppression']> =>
    request('/email-suppressions', s.zEmailListSuppressionsResponse),
  addSuppression: (body: S['AddSuppression']): Promise<void> =>
    requestNoContent('/email-suppressions', { method: 'POST', body }),
  removeSuppression: (email: string): Promise<void> =>
    requestNoContent(`/email-suppressions/${encodeURIComponent(email)}`, { method: 'DELETE' }),
};

// ── Newsletter ──
// One blast, everyone in BCC. Subscriptions are managed here; the website signs up
// through the keyed public endpoint (double opt-in: readers confirm from the link in
// their letter), and readers leave from the unsubscribe page.
export const newsletterApi = {
  subscriptions: (): Promise<S['Items_Subscription']> =>
    request('/newsletter/subscriptions', s.zNewsletterSubscriptionsResponse),
  addSubscription: (body: S['SubscriptionBody']): Promise<S['Subscription']> =>
    request('/newsletter/subscriptions', s.zNewsletterAddSubscriptionResponse, { method: 'POST', body }),
  removeSubscription: (id: number): Promise<void> =>
    requestNoContent(`/newsletter/subscriptions/${id}`, { method: 'DELETE' }),
  send: (body: S['NewsletterRequest']): Promise<S['NewsletterSent']> =>
    request('/newsletter/send', s.zNewsletterSendResponse, { method: 'POST', body }),
  confirm: (token: string): Promise<S['Confirmed']> =>
    request('/newsletter/confirm', s.zNewsletterConfirmResponse, { search: { token } }),
  unsubscribe: (search: { token?: string; email?: string }): Promise<S['Unsubscribed']> =>
    request('/newsletter/unsubscribe', s.zNewsletterUnsubscribeResponse, { search }),
};

// ── Configuration ──
export const configApi = {
  stages: (entity?: StageEntity): Promise<S['Items_StageDefinition']> =>
    request('/stage-definitions', s.zConfigurationListStagesResponse, { search: { entity } }),
  createStage: (body: S['CreateStage']): Promise<S['StageDefinition']> =>
    request('/stage-definitions', s.zConfigurationCreateStageResponse, { method: 'POST', body }),
  patchStage: (id: number, body: S['PatchStage']): Promise<S['StageDefinition']> =>
    request(`/stage-definitions/${id}`, s.zConfigurationUpdateStageResponse, { method: 'PATCH', body }),
  projectTypes: (): Promise<S['Items_ProjectType']> =>
    request('/project-types', s.zConfigurationListProjectTypesResponse),
  createProjectType: (body: S['CreateProjectType']): Promise<S['ProjectType']> =>
    request('/project-types', s.zConfigurationCreateProjectTypeResponse, { method: 'POST', body }),
  patchProjectType: (id: number, body: S['PatchProjectType']): Promise<S['ProjectType']> =>
    request(`/project-types/${id}`, s.zConfigurationUpdateProjectTypeResponse, { method: 'PATCH', body }),
  settings: (): Promise<S['Settings']> => request('/settings', s.zConfigurationGetSettingsResponse),
  saveSettings: (body: S['SettingsBody']): Promise<S['Settings']> =>
    request('/settings', s.zConfigurationPutSettingsResponse, { method: 'PUT', body }),
  // Every client-facing enumeration in one document (damage types, fuel levels,
  // currencies, image categories, ...). Selects, chips and labels render from
  // this; nothing in it is duplicated in the clients.
  lookups: (): Promise<S['Lookups']> =>
    request('/config/lookups', s.zConfigurationLookupsResponse),
};

// ── Reports ──
export const reportsApi = {
  volume: (search: QueryOf<'reports_volume'> = {}): Promise<S['VolumeReport']> =>
    request('/reports/volume', s.zReportsVolumeResponse, { search }),
  stageDurations: (search: QueryOf<'reports_stage_durations'> = {}): Promise<S['DurationReport']> =>
    request('/reports/stage-durations', s.zReportsStageDurationsResponse, { search }),
  throughput: (search: QueryOf<'reports_throughput'> = {}): Promise<S['ThroughputReport']> =>
    request('/reports/throughput', s.zReportsThroughputResponse, { search }),
  stalled: (): Promise<S['Items_StalledOrder']> => request('/reports/stalled', s.zReportsStalledResponse),
  blockerLoad: (search: QueryOf<'reports_blocker_load'> = {}): Promise<S['BlockerLoadReport']> =>
    request('/reports/blocker-load', s.zReportsBlockerLoadResponse, { search }),
  fxRates: (search: QueryOf<'reports_fx_rates'> = {}): Promise<S['Items_FxRate']> =>
    request('/reports/fx-rates', s.zReportsFxRatesResponse, { search }),
  workload: (search: QueryOf<'reports_workload'> = {}): Promise<S['WorkloadReport']> =>
    request('/reports/workload', s.zReportsWorkloadResponse, { search }),
};

// ── Global search ──
export const searchApi = {
  global: (q: string, opts?: { signal?: AbortSignal }): Promise<S['GlobalResults']> =>
    request('/search', s.zSearchGlobalResponse, { search: { q }, signal: opts?.signal }),
};

// ── Tasks ──
export const tasksApi = {
  mine: (): Promise<S['Items_Task']> => request('/tasks', s.zTasksMineResponse),
  // `entity` is any task entity type the server lists (`GET /config/lookups`);
  // the server refuses what it does not know.
  forEntity: (entity: string, id: number): Promise<S['Items_Task']> =>
    request(`/tasks/for/${entity}/${id}`, s.zTasksForEntityResponse),
  create: (body: S['TaskBody']): Promise<S['Task']> =>
    request('/tasks', s.zTasksCreateResponse, { method: 'POST', body }),
  setDone: (id: number, done: boolean): Promise<S['Task']> =>
    request(`/tasks/${id}/done`, s.zTasksSetDoneResponse, { method: 'POST', body: { done } }),
  remove: (id: number): Promise<void> => requestNoContent(`/tasks/${id}`, { method: 'DELETE' }),
};

/** The two walkarounds: `checkout` is the first (átvétel, the vehicle arriving), `checkin` the second (kiadás). */
export type ZoneKind = 'checkout' | 'checkin';

// ── Handover inspections (átadás-átvétel) ──
// Created on the phone (guided walkaround); the web reads history, reviews
// check-in verdicts, annotates locked inspections and configures zone templates.
export const inspectionsApi = {
  list: (search: QueryOf<'inspections_list'> = {}): Promise<S['Items_Inspection']> =>
    request('/inspections', s.zInspectionsListResponse, { search }),
  get: (id: number): Promise<S['InspectionDetail']> =>
    request(`/inspections/${id}`, s.zInspectionsDetailResponse),
  comparison: (id: number): Promise<S['Comparison']> =>
    request(`/inspections/${id}/comparison`, s.zInspectionsComparisonResponse),
  verdict: (id: number, body: S['VerdictBody']): Promise<S['InspectionVerdict']> =>
    request(`/inspections/${id}/verdicts`, s.zInspectionsSetVerdictResponse, {
      method: 'POST',
      body,
    }),
  note: (id: number, body: S['NoteBody']): Promise<S['InspectionNote']> =>
    request(`/inspections/${id}/notes`, s.zInspectionsAddNoteResponse, { method: 'POST', body }),
  // The zone list a vehicle kind is walked with for one walkaround: its own list, else the
  // general one (items then carry project_type_id null).
  templates: (projectTypeId: number | null, kind: ZoneKind): Promise<S['Items_ZoneTemplate']> =>
    request('/inspections/templates', s.zInspectionsTemplatesResponse, {
      search: { project_type_id: projectTypeId ?? undefined, kind },
    }),
  saveTemplates: (body: S['ReplaceTemplatesBody']): Promise<S['Items_ZoneTemplate']> =>
    request('/inspections/templates', s.zInspectionsReplaceTemplatesResponse, {
      method: 'PUT',
      body,
    }),
  // Removes a vehicle kind's own list; it is then served the general one again.
  deleteTemplates: (projectTypeId: number, kind: ZoneKind): Promise<void> =>
    requestNoContent('/inspections/templates', {
      method: 'DELETE',
      search: { project_type_id: projectTypeId, kind },
    }),
};

// ── Admin ──
export const adminApi = {
  status: (): Promise<S['AdminStatus']> => request('/admin/status', s.zAdminStatusResponse),
  jobs: (search: QueryOf<'admin_list_jobs'> = {}): Promise<S['Items_Job']> =>
    request('/admin/jobs', s.zAdminListJobsResponse, { search }),
  retryJob: (id: number): Promise<void> => requestNoContent(`/admin/jobs/${id}/retry`, { method: 'POST' }),
  fetchFx: (body: S['FxFetch']): Promise<S['JobQueued']> =>
    request('/admin/fx/fetch', s.zAdminFetchFxResponse, { method: 'POST', body }),
  run: (kind: 'nudge_blockers' | 'stalled_orders'): Promise<S['JobQueued']> =>
    request(`/admin/run/${kind}`, s.zAdminRunNowResponse, { method: 'POST' }),
  testEmail: (body: S['EmailTestBody']): Promise<S['EmailTestResult']> =>
    request('/admin/email/test', s.zAdminTestEmailResponse, { method: 'POST', body }),
};

// ── HR (admins and users with HR access) ──
export const hrApi = {
  list: (search: { q?: string; include_archived?: boolean } = {}): Promise<S['Items_Employee']> =>
    request('/hr/employees', s.zHrListResponse, { search }),
  create: (body: S['EmployeeBody']): Promise<S['Employee']> =>
    request('/hr/employees', s.zHrCreateResponse, { method: 'POST', body }),
  update: (id: number, body: S['EmployeeBody']): Promise<S['Employee']> =>
    request(`/hr/employees/${id}`, s.zHrUpdateResponse, { method: 'PATCH', body }),
  archive: (id: number): Promise<S['Employee']> =>
    request(`/hr/employees/${id}/archive`, s.zHrArchiveResponse, { method: 'POST' }),
  unarchive: (id: number): Promise<S['Employee']> =>
    request(`/hr/employees/${id}/unarchive`, s.zHrUnarchiveResponse, { method: 'POST' }),
  setPhoto: (id: number, file: Blob): Promise<S['Employee']> =>
    request(`/hr/employees/${id}/photo`, s.zHrSetPhotoResponse, { method: 'PUT', rawBody: file }),
  removePhoto: (id: number): Promise<S['Employee']> =>
    request(`/hr/employees/${id}/photo`, s.zHrRemovePhotoResponse, { method: 'DELETE' }),
};
