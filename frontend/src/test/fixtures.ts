// Fixtures for the route smoke tests, typed against the generated contract so a
// backend schema change breaks `tsc` here rather than leaving a test that renders
// a shape the API never returns.
import type {
  AuditEntry,
  Blocker,
  Contact,
  EmailMessage,
  EmailSummary,
  Invoice,
  ItemView,
  Lead,
  LeadDetail,
  LeadSummary,
  Order,
  OrderDetail,
  OrderNote,
  OrderSummary,
  Partner,
  PartnerDetail,
  OrderSpec,
  Proforma,
  ProjectType,
  RawImportView,
  SessionUser,
  Settings,
  StageDefinition,
  StageEntry,
  Vehicle,
  User,
  UserSettings,
} from '@/lib/api/types';

const NOW = '2026-03-02T09:00:00Z';
const DAY = '2026-03-02';

export const sessionUser: SessionUser = {
  id: 1,
  email: 'iroda@autotherm.hu',
  display_name: 'Iroda Ilona',
  role: 'admin',
  must_change_password: false,
  session_kind: 'web',
};

export const user: User = {
  id: 1,
  email: 'iroda@autotherm.hu',
  display_name: 'Iroda Ilona',
  role: 'admin',
  is_active: true,
  must_change_password: false,
  created_at: NOW,
  updated_at: NOW,
};

export const userSettings: UserSettings = {
  user_id: 1,
  density: 'comfortable',
  page_size: 25,
  updated_at: NOW,
};

export const partner: Partner = {
  id: 7,
  kind: 'business',
  name: 'Müller Kühltransporte GmbH',
  tax_number: null,
  eu_tax_number: 'DE123456789',
  country: 'DE',
  default_currency: 'EUR',
  email: 'einkauf@mueller.de',
  phone: '+49 89 111111',
  website: null,
  postal_code: '80331',
  city: 'München',
  address_line: 'Bahnhofstrasse 1',
  notes: null,
  minicrm_id: 4242,
  archived_at: null,
  created_at: NOW,
  updated_at: NOW,
};

export const contact: Contact = {
  id: 11,
  partner_id: 7,
  name: 'Klaus Weber',
  email: 'weber@mueller.de',
  phone: null,
  position: 'Fuhrparkleiter',
  notes: null,
  archived_at: null,
  created_at: NOW,
  updated_at: NOW,
};

export const orderSummary: OrderSummary = {
  id: 3,
  number: 'MC-1001',
  title: 'Sprinter hűtőgép beépítés',
  partner_id: 7,
  partner_name: 'Müller Kühltransporte GmbH',
  project_type_id: 1,
  project_type_label: 'Hűtőgép beépítés',
  currency: 'EUR',
  total_minor: 1250000,
  vehicle_make: 'Mercedes-Benz',
  vehicle_model: 'Sprinter',
  vehicle_plate: 'ABC-123',
  due_date: DAY,
  assigned_to: 1,
  assigned_name: 'Iroda Ilona',
  stage_key: 'production',
  stage_label: 'Gyártás',
  stage_entered_at: NOW,
  stage_is_terminal: false,
  open_blockers: 1,
  created_at: NOW,
  updated_at: NOW,
};

export const order: Order = {
  id: 3,
  number: 'MC-1001',
  title: 'Sprinter hűtőgép beépítés',
  partner_id: 7,
  contact_id: 11,
  lead_id: 5,
  project_type_id: 1,
  currency: 'EUR',
  valuation_date: DAY,
  vehicle_make: 'Mercedes-Benz',
  vehicle_model: 'Sprinter',
  vehicle_plate: 'ABC-123',
  vehicle_vin: 'WDB9066571S123456',
  description: 'Hűtőgép beépítés és ATP vizsgálat.',
  due_date: DAY,
  assigned_to: 1,
  created_by: 1,
  minicrm_id: 1001,
  created_at: NOW,
  updated_at: NOW,
};

export const itemView: ItemView = {
  id: 21,
  order_id: 3,
  position: 1,
  description: 'Hűtőgép',
  quantity: '1',
  unit_price: 1250000,
  currency: 'EUR',
  line_total_minor: 1250000,
  created_at: NOW,
  updated_at: NOW,
};

export const blocker: Blocker = {
  id: 31,
  order_id: 3,
  order_number: 'MC-1001',
  what: 'Fényezés alvállalkozónál',
  responsible_partner_id: 9,
  responsible_partner_name: 'Fényező Kft.',
  responsible_email: 'info@fenyezo.hu',
  due_date: DAY,
  notes: 'Múlt héten egyeztetve.',
  nudge_enabled: true,
  last_nudged_at: NOW,
  nudge_count: 2,
  resolved_at: null,
  resolved_by: null,
  resolution_note: null,
  created_by: 1,
  created_at: NOW,
  updated_at: NOW,
  is_overdue: true,
};

export const stageEntry: StageEntry = {
  id: 41,
  stage_key: 'production',
  label_hu: 'Gyártás',
  entered_at: NOW,
  left_at: null,
  entered_by: 1,
  entered_by_name: 'Iroda Ilona',
  note: null,
};

export const auditEntry: AuditEntry = {
  id: 51,
  at: NOW,
  user_id: 1,
  user_name: 'Iroda Ilona',
  entity: 'order',
  entity_id: 3,
  action: 'item_update',
  changes: { unit_price: { from: 1000000, to: 1250000 } },
};

export const orderNote: OrderNote = {
  id: 71,
  order_id: 3,
  minicrm_id: 9001,
  author_name: 'Nagy Béla',
  body: 'Ügyfél jóváhagyta a tervet.',
  occurred_at: NOW,
};

export const rawImportView: RawImportView = {
  minicrm_id: 1001,
  raw_import: { Id: 1001, Name: 'Sprinter', Rendszam: 'ABC-123' },
};

export const vehicle: Vehicle = {
  id: 91,
  vin: 'WDB9066571S123456',
  plate: 'ABC-123',
  plate_norm: 'ABC123',
  make: 'Mercedes-Benz',
  model: 'Sprinter',
  year: 2019,
  partner_id: 7,
  notes: null,
  created_at: NOW,
  updated_at: NOW,
};

export const orderSpec: OrderSpec = {
  order_id: 3,
  form: 'cooling',
  target_temp_c: '-18.0',
  insulation_mm: 80,
  cooling_unit_make: 'Carrier',
  cooling_unit_model: 'Xarios 600',
  atp_class: 'FRC',
  compartments: 1,
  defrost: 'automatic',
  electric_standby: true,
  heater_make: null,
  heater_model: null,
  heat_output_kw: null,
  fuel: null,
  thermostat: null,
  notes: null,
  created_at: NOW,
  updated_at: NOW,
};

export const orderDetail: OrderDetail = {
  order,
  partner: { id: partner.id, name: partner.name },
  stage: {
    key: 'production',
    label_hu: 'Gyártás',
    entered_at: NOW,
    days_in_stage: 4,
    is_terminal: false,
  },
  items: [itemView],
  value: {
    currency: 'EUR',
    total_minor: 1250000,
    valuation_date: DAY,
    fx_day: DAY,
    fx_rate: '395.12',
    total_huf_minor: 493900000,
  },
  blockers: [blocker],
  related: {
    id: 2,
    number: 'MC-0900',
    title: 'Sprinter hűtőgép beépítés (eredeti)',
    relation: 'warranty',
  },
  vehicles: [vehicle],
  spec: orderSpec,
  image_counts: { intake: 3, completion: 2 },
};

export const lead: Lead = {
  id: 5,
  title: 'Három Sprinter árajánlat',
  partner_id: 7,
  contact_id: 11,
  contact_name: 'Klaus Weber',
  contact_email: 'weber@mueller.de',
  contact_phone: null,
  source: 'E-mail',
  description: 'Három azonos Sprinter hűtőgéppel.',
  assigned_to: 1,
  created_by: 1,
  quoted_value_minor: 4500000,
  currency: "EUR",
  quote_valid_until: "2026-04-30",
  minicrm_id: 900,
  created_at: NOW,
  updated_at: NOW,
};

export const leadSummary: LeadSummary = {
  id: 5,
  title: 'Három Sprinter árajánlat',
  partner_id: 7,
  partner_name: 'Müller Kühltransporte GmbH',
  contact_name: 'Klaus Weber',
  contact_email: 'weber@mueller.de',
  source: 'E-mail',
  assigned_to: 1,
  assigned_name: 'Iroda Ilona',
  quoted_value_minor: 485000000,
  currency: 'HUF',
  quote_valid_until: DAY,
  stage_key: 'quoted',
  stage_label: 'Árajánlat kiadva',
  stage_entered_at: NOW,
  order_id: 3,
  order_number: 'MC-1001',
  created_at: NOW,
};

export const leadDetail: LeadDetail = {
  lead,
  stage: { stage_key: 'quoted', entered_at: NOW },
  history: [{ ...stageEntry, stage_key: 'quoted', label_hu: 'Árajánlat kiadva' }],
  orders: [{ id: 3, number: 'MC-1001' }],
  documents: [],
};

export const partnerDetail: PartnerDetail = {
  partner,
  contacts: [contact],
  orders: [orderSummary],
  leads: [leadSummary],
};

export const orderStageDefinitions: StageDefinition[] = [
  {
    id: 101,
    entity: 'order',
    key: 'design',
    label_hu: 'Tervezés',
    position: 1,
    min_images: 0,
    required_image_category: null,
    is_terminal: false,
    is_exit: false,
    stall_after_days: 14,
    is_active: true,
  },
  {
    id: 102,
    entity: 'order',
    key: 'production',
    label_hu: 'Gyártás',
    position: 2,
    min_images: 0,
    required_image_category: null,
    is_terminal: false,
    is_exit: false,
    stall_after_days: 30,
    is_active: true,
  },
  {
    id: 103,
    entity: 'order',
    key: 'completed',
    label_hu: 'Kész',
    position: 3,
    min_images: 1,
    required_image_category: 'completion',
    is_terminal: true,
    is_exit: true,
    stall_after_days: null,
    is_active: true,
  },
];

export const leadStageDefinitions: StageDefinition[] = [
  {
    id: 201,
    entity: 'lead',
    key: 'new',
    label_hu: 'Új',
    position: 1,
    min_images: 0,
    required_image_category: null,
    is_terminal: false,
    is_exit: false,
    stall_after_days: 7,
    is_active: true,
  },
  {
    id: 202,
    entity: 'lead',
    key: 'quoted',
    label_hu: 'Árajánlat kiadva',
    position: 2,
    min_images: 0,
    required_image_category: null,
    is_terminal: false,
    is_exit: false,
    stall_after_days: 14,
    is_active: true,
  },
];

/** A type whose orders carry a refrigeration spec. */
export const coolingProjectType: ProjectType = {
  id: 1,
  key: 'unit_install',
  label_hu: 'Hűtőgép beépítés',
  position: 1,
  is_active: true,
  spec_form: 'cooling',
};

/** A type whose orders carry a heater spec. */
export const heatingProjectType: ProjectType = {
  id: 2,
  key: 'heated_body',
  label_hu: 'Fűtött felépítmény',
  position: 2,
  is_active: true,
  spec_form: 'heating',
};

/** A type with no build spec at all — a repair. */
export const plainProjectType: ProjectType = {
  id: 3,
  key: 'repair',
  label_hu: 'Javítás / átalakítás',
  position: 3,
  is_active: true,
  spec_form: null,
};

export const projectType = coolingProjectType;

export const settings: Settings = {
  automatic_email_enabled: true,
  max_auto_emails_per_recipient_day: 3,
  send_window_start: '08:00',
  send_window_end: '17:00',
  send_window_weekdays_only: true,
  nudge_interval_days: 3,
  nudge_escalate_after: 2,
  stage_change_notifications: false,
  stalled_alert_recipients: ['iroda@autotherm.hu'],
  updated_at: NOW,
  updated_by: 1,
  email_mode: 'smtp',
  smtp_host: 'smtp.example.com',
  smtp_port: 587,
  smtp_security: 'starttls',
  smtp_username: 'autocrm',
  has_password: true,
  smtp_helo_name: null,
  smtp_force_ipv4: false,
  redirect_to: null,
};

export const emailSummary: EmailSummary = {
  id: 61,
  order_id: 3,
  lead_id: null,
  partner_id: 7,
  blocker_id: 31,
  template_key: 'blocker_nudge',
  trigger: 'blocker_nudge',
  is_automatic: true,
  sent_by: null,
  sent_by_name: null,
  to_address: 'info@fenyezo.hu',
  subject: 'Emlékeztető: fényezés',
  status: 'sent',
  error: null,
  queued_at: NOW,
  send_after: NOW,
  sent_at: NOW,
};

export const emailMessage: EmailMessage = {
  id: 61,
  order_id: 3,
  lead_id: null,
  partner_id: 7,
  blocker_id: 31,
  template_key: 'blocker_nudge',
  trigger: 'blocker_nudge',
  is_automatic: true,
  sent_by: null,
  to_address: 'info@fenyezo.hu',
  cc: [],
  bcc: [],
  from_address: 'crm@autotherm.hu',
  reply_to: null,
  subject: 'Emlékeztető: fényezés',
  body_html: '<p>Emlekezteto</p>',
  body_text: 'Emlekezteto',
  attachments: [],
  status: 'sent',
  provider_id: null,
  error: null,
  attempts: 1,
  queued_at: NOW,
  send_after: NOW,
  sending_started_at: NOW,
  sent_at: NOW,
  cancelled_at: null,
  cancelled_by: null,
};

// ── Invoicing ──

export const issuedInvoice: Invoice = {
  id: 501,
  order_id: 3,
  number: 'AT2026-0001',
  kind: 'invoice',
  status: 'issued',
  original_invoice_id: null,
  currency: 'HUF',
  issue_date: '2026-09-21',
  delivery_date: '2026-09-21',
  payment_date: '2026-09-29',
  payment_method: 'TRANSFER',
  net_amount: 100_000_000,
  vat_amount: 27_000_000,
  gross_amount: 127_000_000,
  nav_transaction_id: '4Q7ZXY8K2M1N',
  nav_status: 'DONE',
  nav_error_code: null,
  nav_message: null,
  nav_messages: [],
  annulment_transaction_id: null,
  annulment_code: null,
  annulment_reason: null,
  annulled_at: null,
  document_id: 900,
  submitted_at: NOW,
  issued_at: NOW,
  created_by: 1,
  created_at: NOW,
  updated_at: NOW,
};

/** What a NAV rejection looks like on the row: the fault code, kept. */
export const rejectedInvoice: Invoice = {
  ...issuedInvoice,
  id: 502,
  number: 'AT2026-0002',
  status: 'rejected',
  nav_status: 'ABORTED',
  nav_error_code: 'INVOICE_NUMBER_ALREADY_EXISTS',
  nav_message: 'NAV rejected the invoice: INVOICE_NUMBER_ALREADY_EXISTS',
  nav_messages: [
    {
      source: 'business',
      level: 'ERROR',
      code: 'INVOICE_NUMBER_ALREADY_EXISTS',
      message: 'Invoice number already exists',
      path: '/InvoiceData/invoiceNumber',
    },
  ],
  document_id: null,
  issued_at: null,
};

export const submittingInvoice: Invoice = {
  ...issuedInvoice,
  id: 503,
  number: 'AT2026-0003',
  status: 'submitting',
  nav_status: null,
  nav_transaction_id: null,
  document_id: null,
  issued_at: null,
};

export const proforma: Proforma = {
  id: 701,
  order_id: 3,
  number: 'DB2026-0001',
  currency: 'HUF',
  issue_date: '2026-09-21',
  payment_date: '2026-09-29',
  net_amount: 100_000_000,
  vat_amount: 27_000_000,
  gross_amount: 127_000_000,
  document_id: 901,
  created_by: 1,
  created_at: NOW,
};
