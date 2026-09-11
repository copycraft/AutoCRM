// AutoCRM API types — mirrors backend DOCS/API.md + DECISIONS.md.
// Money: integer minor units + explicit currency. Quantities: decimal strings.
// Detail envelopes ({partner,…}, {lead,…}) and archived_at/nullables match
// the Rust structs exactly — do not "flatten" them.

export type Currency = 'HUF' | 'EUR';
export type Role = 'admin' | 'office' | 'designer' | 'viewer';
export type PartnerKind = 'business' | 'person';
export type ImageCategory = 'intake' | 'production' | 'completion' | 'marketing';
export type DocumentKind = 'design' | 'cad' | 'other';
export type EmailStatus = 'queued' | 'sending' | 'sent' | 'failed' | 'cancelled' | 'needs_review';
export type EntityType = 'lead' | 'order';
export type TransitionKind = 'forward' | 'backward' | 'exit' | 'reopen';

export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}

export interface Items<T> {
  items: T[];
}

export interface PaginatedParams {
  limit?: number;
  offset?: number;
}

// ── Auth & users ──────────────────────────────────────────────

export interface User {
  id: number;
  email: string;
  display_name: string;
  role: Role;
  is_active: boolean;
  must_change_password: boolean;
  created_at: string;
  updated_at: string;
}

export interface SessionInfo {
  id: number;
  device_label: string | null;
  created_at: string;
  last_seen_at: string;
  current: boolean;
}

export interface LoginRequest {
  email: string;
  password: string;
  client?: 'web' | 'mobile';
  device_label?: string;
}

export interface CreateUserRequest {
  email: string;
  display_name: string;
  role: User['role'];
  temporary_password: string;
}

// ── Partners & contacts ───────────────────────────────────────

export interface Partner {
  id: number;
  kind: PartnerKind;
  name: string;
  tax_number: string | null;
  eu_tax_number: string | null;
  country: string;
  /** Stored as string ("HUF" | "EUR"). */
  default_currency: string;
  email: string | null;
  phone: string | null;
  website: string | null;
  postal_code: string | null;
  city: string | null;
  address_line: string | null;
  notes: string | null;
  minicrm_id: number | null;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
}

/** GET /partners/{id} envelope — not flattened. */
export interface PartnerDetail {
  partner: Partner;
  contacts: Contact[];
  orders: OrderSummary[];
}

export interface Contact {
  id: number;
  partner_id: number;
  name: string;
  email: string | null;
  phone: string | null;
  position: string | null;
  notes: string | null;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
}

// ── Stages ────────────────────────────────────────────────────

export interface StageDefinition {
  id: number;
  entity: string;
  key: string;
  label_hu: string;
  position: number;
  min_images: number;
  required_image_category: ImageCategory | null;
  is_terminal: boolean;
  is_exit: boolean;
  stall_after_days: number | null;
  is_active: boolean;
}

export interface CurrentStage {
  stage_key: string;
  entered_at: string;
}

export interface StageEntry {
  id: number;
  stage_key: string;
  label_hu: string;
  entered_at: string;
  left_at: string | null;
  entered_by: number | null;
  entered_by_name: string | null;
  note: string | null;
}

export interface StageChange {
  from: string;
  to: string;
  kind: TransitionKind;
}

// ── Leads ─────────────────────────────────────────────────────

export interface Lead {
  id: number;
  title: string;
  partner_id: number | null;
  contact_id: number | null;
  contact_name: string | null;
  contact_email: string | null;
  contact_phone: string | null;
  source: string | null;
  description: string | null;
  assigned_to: number | null;
  created_by: number | null;
  minicrm_id: number | null;
  created_at: string;
  updated_at: string;
}

export interface LeadSummary {
  id: number;
  title: string;
  partner_id: number | null;
  partner_name: string | null;
  contact_name: string | null;
  contact_email: string | null;
  source: string | null;
  assigned_to: number | null;
  assigned_name: string | null;
  stage_key: string;
  stage_label: string;
  stage_entered_at: string;
  order_id: number | null;
  order_number: string | null;
  created_at: string;
}

/** GET /leads/{id} envelope — not flattened. */
export interface LeadDetail {
  lead: Lead;
  stage: CurrentStage | null;
  history: StageEntry[];
  order: OrderRef | null;
}

export interface OrderRef {
  id: number;
  number: string;
}

// ── Orders ────────────────────────────────────────────────────

export interface OrderSummary {
  id: number;
  number: string;
  title: string;
  partner_id: number;
  partner_name: string;
  project_type_id: number | null;
  project_type_label: string | null;
  currency: string;
  total_minor: number;
  vehicle_make: string | null;
  vehicle_model: string | null;
  vehicle_plate: string | null;
  due_date: string | null;
  assigned_to: number | null;
  assigned_name: string | null;
  stage_key: string;
  stage_label: string;
  stage_is_terminal: boolean;
  stage_entered_at: string;
  open_blockers: number;
  created_at: string;
  updated_at: string;
}

/** Raw order row (POST /orders and PATCH /orders/{id} responses). */
export interface OrderRow {
  id: number;
  number: string;
  title: string;
  partner_id: number;
  contact_id: number | null;
  lead_id: number | null;
  project_type_id: number | null;
  currency: string;
  valuation_date: string;
  vehicle_make: string | null;
  vehicle_model: string | null;
  vehicle_plate: string | null;
  vehicle_vin: string | null;
  description: string | null;
  due_date: string | null;
  assigned_to: number | null;
  created_by: number | null;
  minicrm_id: number | null;
  created_at: string;
  updated_at: string;
}

export interface OrderStageView {
  key: string;
  label_hu: string;
  entered_at: string;
  days_in_stage: number;
  is_terminal: boolean;
}

/** ItemView: flattened OrderItem + backend-computed line total. */
export interface OrderItemView {
  id: number;
  order_id: number;
  position: number;
  description: string;
  quantity: string;
  unit_price: number;
  currency: string;
  line_total_minor: number;
  created_at: string;
  updated_at: string;
}

export interface OrderValue {
  currency: string;
  total_minor: number;
  valuation_date: string;
  fx_day: string | null;
  fx_rate: string | number | null;
  total_huf_minor: number | null;
}

/** GET /orders/{id} envelope — not flattened. */
export interface OrderDetail {
  order: OrderRow;
  partner: { id: number; name: string };
  stage: OrderStageView;
  items: OrderItemView[];
  value: OrderValue;
  blockers: Blocker[];
  image_counts: Partial<Record<ImageCategory, number>>;
}

export interface AuditEntry {
  id: number;
  at: string;
  user_id: number | null;
  user_name: string | null;
  entity: string;
  entity_id: number;
  action: string;
  changes: unknown;
}

/** POST /orders and POST /leads/{id}/convert body. */
export interface OrderBody {
  title?: string;
  partner_id?: number;
  contact_id?: number;
  project_type_id?: number;
  currency: Currency;
  valuation_date?: string;
  vehicle_make?: string;
  vehicle_model?: string;
  vehicle_plate?: string;
  vehicle_vin?: string;
  description?: string;
  due_date?: string;
  assigned_to?: number;
  items?: {
    description: string;
    quantity: string;
    unit_price: number;
    position?: number;
  }[];
}

export interface Blocker {
  id: number;
  order_id: number;
  order_number: string;
  what: string;
  responsible_partner_id: number | null;
  responsible_partner_name: string | null;
  responsible_email: string | null;
  due_date: string | null;
  notes: string | null;
  nudge_enabled: boolean;
  last_nudged_at: string | null;
  nudge_count: number;
  resolved_at: string | null;
  resolved_by: number | null;
  resolution_note: string | null;
  created_by: number | null;
  created_at: string;
  updated_at: string;
}

/** A blocker is open while resolved_at is null (backend has no flag). */
export function isBlockerOpen(b: Blocker): boolean {
  return b.resolved_at === null;
}

/** Overdue = past due date and still open. Client-side display only. */
export function isBlockerOverdue(b: Blocker, today: string = new Date().toISOString().slice(0, 10)): boolean {
  return b.resolved_at === null && b.due_date !== null && b.due_date < today;
}

// ── Media ─────────────────────────────────────────────────────

export interface OrderImage {
  id: number;
  order_id: number;
  category: ImageCategory;
  filename: string;
  byte_size: number;
  sha256: string;
  thumb_url: string | null;
  display_url: string | null;
  created_at: string;
}

export interface OrderDocument {
  id: number;
  order_id: number;
  kind: DocumentKind;
  filename: string;
  byte_size: number;
  created_at: string;
}

export type UploadTarget =
  | { type: 'image'; category: ImageCategory }
  | { type: 'document'; kind: DocumentKind };

export interface InitiateUploadRequest {
  target: UploadTarget;
  filename: string;
  content_type: string;
  byte_size: number;
  sha256: string;
}

export type InitiateUploadResponse =
  | { status: 'already_uploaded'; image_id?: number; document_id?: number }
  | {
      status: 'upload';
      ticket: string;
      upload: { method: string; url: string; headers: [string, string][] };
      expires_at: string;
    };

// ── Email ─────────────────────────────────────────────────────

export interface EmailMessage {
  id: number;
  order_id: number | null;
  lead_id: number | null;
  to_address: string;
  subject: string;
  body_preview: string;
  status: EmailStatus;
  created_at: string;
}

export interface EmailDetail extends EmailMessage {
  cc: string[];
  body_text: string;
  body_html: string;
  template_key: string | null;
}

export interface EmailPreview {
  subject: string;
  body: string;
  unresolved: string[];
  recipient_suppressed: boolean;
}

export interface EmailTemplate {
  id: number;
  key: string;
  name: string;
  subject: string;
  body: string;
}

// ── Config ────────────────────────────────────────────────────

export interface ProjectType {
  id: number;
  key: string;
  label_hu: string;
  position: number;
  is_active: boolean;
}

export interface Settings {
  kill_switch: boolean;
  rate_limit_per_minute: number;
  send_window_start: string | null;
  send_window_end: string | null;
  nudge_interval_days: number;
  nudge_escalate_after: number;
  stalled_alert_recipients: string[];
}

// ── Reports ───────────────────────────────────────────────────

export interface VolumeRow {
  group: string;
  orders: number;
  huf_minor: number;
  eur_minor: number;
  normalized_huf_minor: number | null;
  missing_fx: number;
}

export interface StageDurationRow {
  stage_key: string;
  visits: number;
  currently_in_stage: number;
  avg_days: number | null;
  median_days: number | null;
  p90_days: number | null;
}

export interface StalledOrder {
  order_id: number;
  order_number: string;
  stage_key: string;
  days_in_stage: number;
  stall_after_days: number;
}

// ── Admin ─────────────────────────────────────────────────────

export interface AdminStatus {
  email_mode: string;
  kill_switch: boolean;
  failed_jobs: number;
  pending_jobs: number;
  emails_needing_attention: number;
  fx_coverage_days: number;
}

export interface Job {
  id: number;
  kind: string;
  state: string;
  payload: unknown;
  attempts: number;
  created_at: string;
}
