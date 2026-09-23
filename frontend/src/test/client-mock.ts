// Stand-in for `@/lib/api/client` in the route smoke tests.
//
// Mocking here rather than at `@/lib/api/endpoints` keeps endpoints.ts itself in
// the test: the pages exercise the real call sites, only the network and the zod
// validation are replaced. Paths are matched exactly as endpoints.ts builds them.
import * as f from './fixtures';

type Handler = () => unknown;

const EXACT: Record<string, Handler> = {
  '/auth/me': () => ({ user: f.sessionUser }),
  '/auth/preferences': () => f.userSettings,
  '/auth/sessions': () => ({ items: [] }),
  '/users': () => ({ items: [f.user] }),
  '/partners': () => ({ items: [f.partner] }),
  '/leads': () => ({ items: [f.leadSummary] }),
  '/orders': () => ({ items: [f.orderSummary] }),
  '/blockers': () => ({ items: [f.blocker] }),
  '/emails': () => ({ items: [f.emailSummary] }),
  '/email-templates': () => ({ items: [] }),
  '/reports/stalled': () => ({ items: [] }),
  '/email-suppressions': () => ({ items: [] }),
  '/project-types': () => ({
    items: [f.coolingProjectType, f.heatingProjectType, f.plainProjectType],
  }),
  '/settings': () => f.settings,
};

const PATTERNS: [RegExp, Handler][] = [
  [/^\/partners\/\d+$/, () => f.partnerDetail],
  [/^\/partners\/\d+\/contacts$/, () => ({ items: [f.contact] })],
  [/^\/leads\/\d+$/, () => f.leadDetail],
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
  if (override) return override();
  if (path === '/stage-definitions') return stageDefinitions(search);
  const exact = EXACT[path];
  if (exact) return exact();
  for (const [re, handler] of PATTERNS) {
    if (re.test(path)) return handler();
  }
  unhandled.push(path);
  return { items: [] };
}

export function request<T>(
  path: string,
  _schema: unknown,
  opts: { search?: Record<string, unknown> } = {},
): Promise<T> {
  return Promise.resolve(resolve(path, opts.search) as T);
}

export function requestNoContent(_path: string, _opts?: unknown): Promise<void> {
  return Promise.resolve();
}
