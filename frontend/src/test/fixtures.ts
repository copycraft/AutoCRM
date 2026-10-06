// Fixtures for the route smoke tests, typed against the generated contract so a
// backend schema change breaks `tsc` here rather than leaving a test that renders
// a shape the API never returns.
import type {
  Absence,
  LeaveBalance,
  NotificationList,
  Employee,
  JobApplication,
  JobPosting,
  AuditEntry,
  BilledInvoice,
  BilledProforma,
  Blocker,
  Contact,
  EmailMessage,
  EmailSummary,
  Invoice,
  ItemView,
  Lead,
  LeadDetail,
  LeadSummary,
  LeadRow,
  LeadTag,
  NewsletterTag,
  IncomingInvoice,
  TimelineEvent,
  SubscriberRow,
  Lookups,
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
  hr_access: true,
};

export const user: User = {
  id: 1,
  email: 'iroda@autotherm.hu',
  display_name: 'Iroda Ilona',
  role: 'admin',
  is_active: true,
  must_change_password: false,
  hr_access: false,
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
  tags: [
    { id: 41, market: 'de', label: 'bestattungswagen.at', color: '#c9402a', matched_domain: 'bestattungswagen.at' },
  ],
};

export const leadRow: LeadRow = { ...leadSummary, tags: leadDetail.tags };

export const leadTag: LeadTag = {
  id: 41,
  market: 'de',
  label: 'bestattungswagen.at',
  color: '#c9402a',
  domains: ['bestattungswagen.at'],
  position: 130,
  archived_at: null,
  open_leads: 1,
  total_leads: 4,
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

export const lookups: Lookups = {
  damage_types: [
    { key: 'scratch', label_hu: 'Karcolás' },
    { key: 'dent', label_hu: 'Horpadás' },
    { key: 'crack', label_hu: 'Repedés' },
    { key: 'chip', label_hu: 'Lepattanás' },
    { key: 'broken', label_hu: 'Törött alkatrész' },
    { key: 'missing', label_hu: 'Hiányzó alkatrész' },
    { key: 'stain', label_hu: 'Folt' },
    { key: 'tear', label_hu: 'Szakadás' },
    { key: 'other', label_hu: 'Egyéb' },
  ],
  severities: [
    { key: 'minor', label_hu: 'Enyhe' },
    { key: 'moderate', label_hu: 'Közepes' },
    { key: 'severe', label_hu: 'Súlyos' },
  ],
  verdicts: [
    { key: 'preexisting', label_hu: 'Már megvolt' },
    { key: 'new', label_hu: 'Új sérülés' },
    { key: 'dismissed', label_hu: 'Nem sérülés' },
  ],
  walkaround_kinds: [
    { key: 'checkout', label_hu: 'Átvétel' },
    { key: 'checkin', label_hu: 'Kiadás' },
  ],
  fuel_levels: [
    { key: 'E', label_hu: 'E' },
    { key: '1/4', label_hu: '1/4' },
    { key: '1/2', label_hu: '1/2' },
    { key: '3/4', label_hu: '3/4' },
    { key: 'F', label_hu: 'F' },
  ],
  heating_fuels: [
    { key: 'diesel', label_hu: 'Dízel' },
    { key: 'electric', label_hu: 'Elektromos' },
    { key: 'lpg', label_hu: 'LPG' },
    { key: 'engine_coolant', label_hu: 'Motorhűtőfolyadék' },
  ],
  defrost_modes: [
    { key: 'automatic', label_hu: 'Automatikus' },
    { key: 'manual', label_hu: 'Kézi' },
    { key: 'hot_gas', label_hu: 'Forrógázas' },
  ],
  order_relations: [
    { key: 'warranty', label_hu: 'Garanciális' },
    { key: 'rework', label_hu: 'Újramunkálás' },
    { key: 'repeat', label_hu: 'Ismételt' },
  ],
  task_entity_types: [
    { key: 'order', label_hu: 'Megrendelés' },
    { key: 'lead', label_hu: 'Érdeklődő' },
    { key: 'partner', label_hu: 'Partner' },
  ],
  currencies: [
    { key: 'HUF', label_hu: 'Forint' },
    { key: 'EUR', label_hu: 'Euró' },
  ],
  invoice_payment_methods: [
    { key: 'TRANSFER', label_hu: 'Átutalás' },
    { key: 'CASH', label_hu: 'Készpénz' },
  ],
  annulment_codes: [
    { key: 'ERRATIC_DATA', label_hu: 'ERRATIC_DATA' },
    { key: 'ERRATIC_INVOICE_NUMBER', label_hu: 'ERRATIC_INVOICE_NUMBER' },
    { key: 'ERRATIC_INVOICE_ISSUE_DATE', label_hu: 'ERRATIC_INVOICE_ISSUE_DATE' },
    { key: 'ERRATIC_ELECTRONIC_HASH_VALUE', label_hu: 'ERRATIC_ELECTRONIC_HASH_VALUE' },
  ],
  image_categories: [
    { key: 'intake', label_hu: 'Bevétel', immutable: true, attachable: false },
    { key: 'production', label_hu: 'Gyártás', immutable: false, attachable: true },
    { key: 'completion', label_hu: 'Átadás/MEO', immutable: false, attachable: false },
    { key: 'marketing', label_hu: 'Marketing', immutable: false, attachable: false },
    { key: 'inspection', label_hu: 'Átvétel', immutable: false, attachable: false },
  ],
  email_themes: [
    {
      key: 'quotation',
      label_hu: 'Árajánlat',
      subject: 'Árajánlatunk',
      hero: 'Megjött az Autotherm árajánlatod!',
      body: 'Tisztelt Címzett!',
    },
    {
      key: 'promo',
      label_hu: 'Akció',
      subject: 'Autotherm akció',
      hero: '',
      body: '# Újdonság',
    },
  ],
  error_texts: [
    { code: 'checkout_open', text_hu: 'Már van nyitott átvételi jegyzőkönyv. Előbb írassa alá vagy dobja el.' },
    { code: 'locked', text_hu: 'Az aláírt jegyzőkönyv már nem módosítható; utólagos megjegyzést lehet hozzáfűzni.' },
  ],
};

export const billedInvoice: BilledInvoice = {
  ...issuedInvoice,
  order_number: 'MC-1001',
  partner_name: 'Müller Kühltransporte GmbH',
  bucket: 'issued',
  paid_at: null,
  reminders_off: false,
  reminders_sent: 0,
};

export const billedStorno: BilledInvoice = {
  ...issuedInvoice,
  id: 504,
  number: 'AT2026-0004',
  kind: 'storno',
  original_invoice_id: 501,
  net_amount: -100_000_000,
  vat_amount: -27_000_000,
  gross_amount: -127_000_000,
  order_number: 'MC-1001',
  partner_name: 'Müller Kühltransporte GmbH',
  bucket: 'storno',
  paid_at: null,
  reminders_off: false,
  reminders_sent: 0,
};

export const billedProforma: BilledProforma = {
  id: 701,
  order_id: 3,
  order_number: 'MC-1001',
  partner_name: 'Müller Kühltransporte GmbH',
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

export const employee: Employee = {
  id: 7,
  full_name: 'Kiss Péter',
  email: 'peter@autotherm.hu',
  company_phone: '+36 30 111 2222',
  personal_phone: '+36 20 333 4444',
  photo_url: null,
  annual_leave_days: 20,
  archived_at: null,
  created_at: NOW,
  updated_at: NOW,
};

export const officeUser: User = {
  ...user,
  id: 2,
  email: 'anna@autotherm.hu',
  display_name: 'Nagy Anna',
  role: 'office',
};

// Leave is shown for the current month, so the fixture follows the calendar.
const monthStart = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
const isoDay = (d: number) =>
  `${monthStart.getFullYear()}-${String(monthStart.getMonth() + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;

export const absence: Absence = {
  id: 31,
  employee_id: 7,
  employee_name: 'Kiss Péter',
  kind: 'annual',
  start_date: isoDay(2),
  end_date: isoDay(4),
  working_days: 3,
  note: 'Nyaralás',
  created_at: NOW,
};

export const leaveBalance: LeaveBalance = {
  employee_id: 7,
  full_name: 'Kiss Péter',
  year: monthStart.getFullYear(),
  allowance_days: 20,
  used_annual: 23,
  remaining: -3,
  sick_days: 2,
  unpaid_days: 0,
  other_days: 0,
};

export const notificationList: NotificationList = {
  items: [
    {
      id: 12,
      kind: 'lead',
      title: 'Új érdeklődés a weboldalról',
      body: 'Kiss Péter: Hűtős átalakítást kérek',
      link: '/leads/5',
      created_at: NOW,
      read_at: null,
    },
    {
      id: 11,
      kind: 'lead',
      title: 'Régi érdeklődés',
      body: null,
      link: null,
      created_at: NOW,
      read_at: NOW,
    },
  ],
  unread: 1,
};

export const jobPosting: JobPosting = {
  id: 3,
  title: 'Hűtős szerelő',
  description: 'Műszakban, hűtőkamrák építése.',
  location: 'Budapest',
  status: 'draft',
  public_url: 'https://crm.autotherm.hu/hu/jobs/00ab12cd34ef5678',
  application_count: 1,
  published_at: null,
  created_at: NOW,
  updated_at: NOW,
};

export const jobApplication: JobApplication = {
  id: 11,
  job_id: 3,
  full_name: 'Tóth Gábor',
  email: 'gabor@example.hu',
  phone: '+36 30 999 8888',
  age: 31,
  city: 'Szeged',
  message: 'Szívesen dolgoznék önöknél.',
  resume_filename: 'Önéletrajz.pdf',
  resume_url: 'https://files.example/resume.pdf',
  notes: null,
  created_at: NOW,
};

export const newsletterTag: NewsletterTag = {
  id: 7,
  section: 'Listák',
  label: 'Pékségek',
  color: '#dbe8ff',
  position: 160,
  archived_at: null,
  active_subscribers: 1,
  total_subscribers: 1,
};

export const subscriberRow: SubscriberRow = {
  id: 3,
  email: 'info@pekseg.hu',
  name: 'Kovács Pékség',
  source: 'import',
  subscribed_at: NOW,
  confirmed_at: NOW,
  unsubscribed_at: null,
  tag_ids: [7],
};

export const incomingInvoice: IncomingInvoice = {
  id: 12,
  kind: 'invoice',
  supplier_name: 'Hűtőgép Kft.',
  supplier_tax_number: '12345678-2-41',
  partner_id: null,
  invoice_number: 'HG-2026/118',
  issue_date: DAY,
  due_date: DAY,
  currency: 'HUF',
  net_amount: 10_000_000,
  vat_amount: 2_700_000,
  gross_amount: 12_700_000,
  payment_method: 'TRANSFER',
  paid_amount: 0,
  paid_on: null,
  booking_only: false,
  notes: null,
  file_name: 'szamla-118.pdf',
  file_type: 'application/pdf',
  file_size: 52_000,
  bucket: 'open_invoice',
  created_at: NOW,
  updated_at: NOW,
};

export const timelineEvent: TimelineEvent = {
  at: NOW,
  kind: 'change',
  action: 'update',
  user_name: 'Iroda Ilona',
  changes: { assigned_to: ['Kiss Péter', 'Busa Ádám'] },
  text: null,
  detail: null,
  file_name: null,
  file_size: null,
  document_id: null,
  image_id: null,
  incoming_invoice_id: null,
  email_id: null,
  order_id: null,
  lead_id: null,
};
