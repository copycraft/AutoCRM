// Stand-in for `@/lib/api/client` in the route smoke tests.
//
// Mocking here rather than at `@/lib/api/endpoints` keeps endpoints.ts itself in
// the test: the pages exercise the real call sites, only the network and the zod
// validation are replaced. Paths are matched exactly as endpoints.ts builds them.
import * as f from './fixtures';

type Handler = (search?: Record<string, unknown>) => unknown;

const EXACT: Record<string, Handler> = {
  '/auth/me': () => ({ user: f.sessionUser }),
  '/auth/preferences': () => f.userSettings,
  '/auth/sessions': () => ({ items: [] }),
  '/users': () => ({ items: [f.user, f.officeUser] }),
  '/hr/employees': () => ({ items: [f.employee] }),
  '/hr/jobs': () => ({ items: [f.jobPosting] }),
  '/hr/absences': () => ({ items: [f.absence] }),
  '/hr/leave-summary': () => ({ items: [f.leaveBalance] }),
  '/notifications': () => f.notificationList,
  '/partners': () => ({ items: [f.partner] }),
  '/leads': () => ({ items: [f.leadRow] }),
  '/lead-tags': () => ({ items: [f.leadTag] }),
  '/newsletter/tags': () => ({ items: [f.newsletterTag] }),
  '/newsletter/subscribers': () => ({ items: [f.subscriberRow] }),
  '/newsletter/subscribers/counts': () => ({ total: 2, active: 1, pending: 0, unsubscribed: 1, untagged: 0 }),
  '/newsletter/audience': () => ({ recipients: 1 }),
  '/incoming-invoices': () => ({ items: [f.incomingInvoice] }),
  '/incoming-invoices/buckets': () => ({ items: [{ bucket: 'open_invoice', count: 1 }] }),
  '/incoming-invoices/suppliers': () => ({ items: [] }),
  '/invoices/buckets': () => ({ items: [{ bucket: 'issued', count: 1 }] }),
  '/hr/statuses': () => ({ items: [{ id: 10, section: 'Aktív munkavállaló', label: 'Aktív munkavállaló', color: '#1f3a75', position: 2040, ends_employment: false, is_default: true, archived_at: null, employees: 1 }] }),
  '/orders': () => ({ items: [f.orderSummary] }),
  '/blockers': () => ({ items: [f.blocker] }),
  '/emails': () => ({ items: [f.emailSummary] }),
  '/email-templates': () => ({ items: [{ id: 1, key: 'invoice_overdue_1', name: 'Számla fizetési határidő lejárt #1', subject: 'Fizetési emlékeztető', body: 'Tisztelt {{partner.name}}!', locale: 'hu', is_automatic: false, updated_at: '2026-10-01T10:00:00Z', updated_by: null, category: 'billing', folder: 'customer', archived_at: null }] }),
  '/followup-steps': () => ({ items: [{ id: 1, label: '1 hét', delay_days: 7, template_key: 'quote_followup_1', template_name: 'Árajánlat utánkövetés: 1 hét', is_active: true }] }),
  '/reports/stalled': () => ({ items: [] }),
  '/email-suppressions': () => ({ items: [] }),
  '/project-types': () => ({
    items: [f.coolingProjectType, f.heatingProjectType, f.plainProjectType],
  }),
  '/settings': () => f.settings,
  '/config/lookups': () => f.lookups,
  '/invoices': (search) => {
    // The Számlázó filters run on the server; mirror them here.
    let items = [f.billedStorno, f.billedInvoice];
    if (search?.status) items = items.filter((i) => i.status === search.status);
    if (search?.kind) items = items.filter((i) => i.kind === search.kind);
    return { items };
  },
  '/proformas': () => ({ items: [f.billedProforma] }),
};

const PATTERNS: [RegExp, Handler][] = [
  [/^\/hr\/jobs\/\d+\/applications$/, () => ({ items: [f.jobApplication] })],
  [/^\/partners\/\d+$/, () => f.partnerDetail],
  [/^\/partners\/\d+\/contacts$/, () => ({ items: [f.contact] })],
  [/^\/leads\/\d+$/, () => f.leadDetail],
  [/^\/timeline\/[a-z_]+\/\d+$/, () => ({ items: [f.timelineEvent] })],
  [/^\/leads\/\d+\/followups$/, () => ({ items: [] })],
  [/^\/leads\/\d+\/transitions$/, () => ({ items: [] })],
  [/^\/orders\/\d+$/, () => f.orderDetail],
  [/^\/orders\/\d+\/stages$/, () => ({ items: [f.stageEntry] })],
  [/^\/orders\/\d+\/transitions$/, () => ({ items: [] })],
  [/^\/orders\/\d+\/audit$/, () => ({ items: [f.auditEntry] })],
  [/^\/orders\/\d+\/items$/, () => ({ items: [f.itemView] })],
  [/^\/orders\/\d+\/blockers$/, () => ({ items: [f.blocker] })],
  [/^\/orders\/\d+\/images$/, () => ({ items: [] })],
  [/^\/orders\/\d+\/documents$/, () => ({ items: [] })],
  [/^\/orders\/\d+\/invoices$/, () => ({ items: [] })],
  [/^\/orders\/\d+\/proformas$/, () => ({ items: [] })],
  [/^\/invoices\/\d+\/chain$/, () => ({ items: [] })],
  [/^\/emails\/\d+$/, () => f.emailMessage],
];

function stageDefinitions(search: Record<string, unknown> | undefined): unknown {
  const entity = search?.entity;
  if (entity === 'lead') return { items: f.leadStageDefinitions };
  if (entity === 'order') return { items: f.orderStageDefinitions };
  return { items: [...f.leadStageDefinitions, ...f.orderStageDefinitions] };
}

/**
 * Per-test answers, keyed by exact path, checked before everything else.
 *
 * Some screens are about state the fixtures cannot hold two of at once — an invoice NAV
 * accepted and one it rejected, say. A test sets what it needs here and clears it after.
 */
export const overrides = new Map<string, Handler>();

export function resetOverrides(): void {
  overrides.clear();
}

/** Paths a test asked for that nothing here answers — asserted empty. */
export const unhandled: string[] = [];

export function resetUnhandled(): void {
  unhandled.length = 0;
}

function resolve(path: string, search?: Record<string, unknown>): unknown {
  const override = overrides.get(path);
  if (override) return override(search);
  if (path === '/stage-definitions') return stageDefinitions(search);
  const exact = EXACT[path];
  if (exact) return exact(search);
  for (const [re, handler] of PATTERNS) {
    if (re.test(path)) return handler();
  }
  unhandled.push(path);
  return { items: [] };
}

/** Every write (anything but a GET) the page made, in order, for tests that assert what was sent. */
export const writes: { path: string; method: string; body?: unknown; search?: Record<string, unknown> }[] = [];

export function resetWrites(): void {
  writes.length = 0;
}

interface Opts {
  method?: string;
  body?: unknown;
  formBody?: FormData;
  search?: Record<string, unknown>;
}

function record(path: string, opts: Opts): void {
  const method = opts.method ?? 'GET';
  if (method !== 'GET') {
    writes.push({
      path,
      method,
      ...(opts.body === undefined ? {} : { body: opts.body }),
      // A multipart form is recorded as its fields; a file shows as its name.
      ...(opts.formBody === undefined
        ? {}
        : {
            body: Object.fromEntries(
              [...opts.formBody.entries()].map(([k, v]) => [k, typeof v === 'string' ? v : v.name]),
            ),
          }),
      ...(opts.search === undefined ? {} : { search: opts.search }),
    });
  }
}

export function request<T>(path: string, _schema: unknown, opts: Opts = {}): Promise<T> {
  record(path, opts);
  return Promise.resolve(resolve(path, opts.search) as T);
}

export function requestNoContent(path: string, opts: Opts = {}): Promise<void> {
  record(path, opts);
  return Promise.resolve();
}
