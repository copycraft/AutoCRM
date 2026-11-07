# Frontend Audit

## Summary
- Build: PASS — 0 errors (`next build`, exit 0, 31 static pages generated)
- Typecheck: PASS — 0 errors (`npx tsc --noEmit`, exit 0, no output)
- Lint: PASS — 0 warnings / 0 errors (`next lint`, config `next/core-web-vitals` only)
- Rules: 4 PASS / 15 FAIL / 2 PARTIAL / 1 NOT VERIFIED (2 of the 4 passes are vacuous, see table)
- Screens: 0 built / 4 partial / 5 missing / 6 not in spec
- Blockers: 2  Major: 15  Minor: 16

Command output excerpt (per-locale `/hu`/`/en` sub-rows and the shared-chunk table trimmed; Windows console mangled the check-mark and tree glyphs):

```
> npx tsc --noEmit
(no output)                                                    exit 0

> autocrm-frontend@0.1.0 lint
> next lint
âś” No ESLint warnings or errors                                 exit 0

> autocrm-frontend@0.1.0 build
> next build
  â–˛ Next.js 14.2.35
 âś“ Compiled successfully
   Linting and checking validity of types ...
 âś“ Generating static pages (31/31)
Route (app)                              Size     First Load JS
â”ś â—Ź /[locale]                            192 B           135 kB
â”ś â—Ź /[locale]/admin                      192 B           135 kB
â”ś â—Ź /[locale]/blockers                   192 B           135 kB
â”ś â—Ź /[locale]/emails                     192 B           135 kB
â”ś â—Ź /[locale]/leads                      3.32 kB         151 kB
â”ś Ć’ /[locale]/leads/[id]                 4.66 kB         169 kB
â”ś â—Ź /[locale]/leads/new                  581 B           165 kB
â”ś â—Ź /[locale]/login                      4.42 kB         143 kB
â”ś â—Ź /[locale]/orders                     4.03 kB         152 kB
â”ś Ć’ /[locale]/orders/[id]                6.9 kB          171 kB
â”ś â—Ź /[locale]/orders/new                 600 B           165 kB
â”ś â—Ź /[locale]/partners                   2.71 kB         151 kB
â”ś Ć’ /[locale]/partners/[id]              4.11 kB         167 kB
â”ś â—Ź /[locale]/partners/new               554 B           164 kB
â”ś â—Ź /[locale]/password                   2.91 kB         106 kB
â”ś â—Ź /[locale]/reports                    192 B           135 kB
â”” â—Ź /[locale]/settings                   192 B           135 kB
Ć’ Middleware                             39.8 kB                 exit 0
```

## Findings

### [BLOCKER-01] Session user is read with the wrong shape: every role check and the forced password change are dead
- **Rule:** N12 (hand-written API types), M10
- **Location:** `frontend/src/lib/api/endpoints.ts:42`, `frontend/src/lib/auth/context.tsx:22-26,49-51,74-76`, `frontend/src/app/[locale]/login/page.tsx:30-31`, `frontend/src/components/layout/AppShell.tsx:20-24`
- **Evidence:**
  ```ts
  // endpoints.ts:40-42
  login: (body: LoginRequest) => api.post<{ must_change_password: boolean }>('/auth/login', body),
  me: () => api.get<User>('/auth/me'),
  // context.tsx:49-51
  user: data ?? null,
  isLoading,
  isAuthenticated: !!data,
  // context.tsx:74-76
  export function canEdit(user: User | null): boolean {
    return !!user && (user.role === 'admin' || user.role === 'office');
  ```
  Backend actually returns an envelope, and the inner object is `AuthUser` (field `user_id`, not `id`):
  ```rust
  // backend/src/api/auth.rs:96-98
  async fn me(Auth(user): Auth) -> Json<Value> {
      Json(json!({ "user": user }))
  }
  // backend/src/api/auth.rs:75 (web login)
  Ok((jar.add(cookie), Json(json!({ "user": outcome.user }))))
  // backend/src/service/auth.rs:45-52
  pub struct AuthUser { pub user_id: i64, pub session_id: i64, ... pub display_name: String, pub role: Role, pub must_change_password: bool }
  ```
- **Spec required:** Types generated from the OpenAPI spec; UI hides/disables actions per the capability model; forced password change represented.
- **Impact:** `data` is `{ user: {...} }`, so `user.role`, `user.id`, `user.display_name` and `user.must_change_password` are all `undefined` while `isAuthenticated` is `true`. `canEdit`/`canAdmin`/`canChangeStage` return `false` for every account, including admins: "Új megrendelés", edit, stage change, item editing, contact editing and the Admin/Settings nav entries are never shown. `res.must_change_password` in the login page and `user?.must_change_password` in `AppShell` are always `undefined`, so a user with a temporary password is never routed to `/password`; every API call then fails with `422 password_change_required` rendered as generic error states. The "Csak az enyém" assignee option sends no id (`user?.id` is `undefined`). Sidebar name/e-mail render empty. (Static reading of both sides; not exercised against a running backend.)

### [BLOCKER-02] All API types are hand-written; no OpenAPI source, no generation, no CI drift check
- **Rule:** N12, M1
- **Location:** `frontend/src/types/api.ts:1-501`, `.github/workflows/ci.yml:7-8`
- **Evidence:**
  ```ts
  // types/api.ts:1-2
  // AutoCRM API types — mirrors backend DOCS/API.md + DECISIONS.md.
  // Money: integer minor units + explicit currency. Quantities: decimal strings.
  ```
  ```yaml
  # .github/workflows/ci.yml — the only job
  jobs:
    backend:
  ```
  No `openapi`, `utoipa`, `ts-rs` or `aide` reference exists in `backend/`, and no `openapi*`/`swagger*` file exists in the repository. Further drift already present in the hand-written file:
  ```ts
  // types/api.ts:445-453
  export interface Settings { kill_switch: boolean; rate_limit_per_minute: number; ... }
  ```
  ```rust
  // backend/src/repo/config.rs:185-194
  pub struct Settings { pub automatic_email_enabled: bool, pub max_auto_emails_per_recipient_day: i32, pub send_window_start: NaiveTime, ... pub send_window_weekdays_only: bool, ... pub stage_change_notifications: bool, ... }
  ```
  `AdminStatus` (`types/api.ts:485-492`: `kill_switch`, `fx_coverage_days`) vs `backend/src/api/admin.rs:57-68` (`environment`, `automatic_email_enabled`, `orders_missing_fx_rate`, `latest_eur_rate_day`, …). `EmailMessage` (`types/api.ts:402-411`: `body_preview`, `created_at`) vs `backend/src/repo/emails.rs:43-61` (`queued_at`, `trigger`, `is_automatic`, `error`, no `body_preview`). `apiFetch` casts every response unchecked: `return JSON.parse(text) as T;` (`lib/api/client.ts:43`).
- **Spec required:** Types generated from the OpenAPI spec (N12); regenerated in CI with the build failing on drift (M1).
- **Impact:** The type checker certifies code against a contract that does not exist. BLOCKER-01 is the first live consequence; the settings, admin status and email screens are typed against field names the backend does not send.

### [MAJOR-01] Five of nine approved screens are placeholders that present "no data" and roadmap text to users
- **Rule:** M5, Step 3
- **Location:** `frontend/src/app/[locale]/reports/page.tsx:11`, `settings/page.tsx:11`, `emails/page.tsx:12`, `blockers/page.tsx:12`, `admin/page.tsx:11`, `page.tsx:10-14`, `orders/[id]/page.tsx:272`
- **Evidence:**
  ```tsx
  // reports/page.tsx:11
  <EmptyState title="Nincs elérhető jelentés." hint="7. fázis: forgalom, fázis-időtartamok, átjutás, elakadt, akadály-terhelés, árfolyamok — backend riport-API-val, MNB normalizálással." />
  // page.tsx:12-13
  title="Irányítópult — 1. fázis váza"
  hint="Valós backend-integráció (...) a 2–8. fázisban érkezik. Üres állapot szándékos: nincs kamu adat."
  // orders/[id]/page.tsx:272
  <p className="rounded-lg bg-cold/10 px-3 py-2 text-xs text-cold">{t('blockersPhaseNote')}</p>
  ```
  Gallery (no `<img>`/`next/image` anywhere in `src/`), email compose, correspondence, reports and settings have no implementation. `emailApi`, `mediaApi`, `reportsApi`, `adminApi`, `blockersApi` (`lib/api/endpoints.ts:123-194`) have zero call sites.
- **Spec required:** Four real states per data view; the nine screens in Step 3.
- **Impact:** "Nincs elérhető jelentés." and `noEmails`/`noBlockers` state that data is absent when the feature does not exist. Developer phase numbers ship in production UI.

### [MAJOR-02] Changing an order's currency is silently discarded
- **Rule:** M10 (behaviour diverges from the API contract)
- **Location:** `frontend/src/components/forms/OrderForm.tsx:61-97,182`
- **Evidence:**
  ```tsx
  // OrderForm.tsx:61-63
  /**
   * PATCH diff with backend semantics. Currency is never sent here — the
   * caller disables it while items exist (422 currency_locked otherwise).
  // OrderForm.tsx:182
  <select id="of-currency" className="input" {...register('currency')} disabled={currencyLocked}>
  ```
  `orderPatchBody` (lines 67-97) builds no `currency` key under any condition.
- **Spec required:** API: currency is editable, locked only while items exist (`docs/API.md` Orders, `PATCH /orders/{id}`).
- **Impact:** On an order with no items the currency select is enabled, the user changes HUF→EUR, saves, receives success, and the order stays HUF.

### [MAJOR-03] Lead conversion forces the partner's default currency; the user cannot choose
- **Rule:** M10
- **Location:** `frontend/src/components/forms/LeadConvertDialog.tsx:45-48,69,95-104`
- **Evidence:**
  ```tsx
  const defaultCurrency: Currency =
    partnerQuery.data?.partner.default_currency === 'EUR' ? 'EUR' : 'HUF';
  const effectiveCurrency = partnerQuery.data ? defaultCurrency : currency;
  ...
  const canConvert = title.trim() !== '' && partnerId != null && !convert.isPending;
  ...
  {partnerQuery.data ? (
    <p ...>{defaultCurrency} <span className="text-steel-500">(partner)</span></p>
  ) : ( <select id="lc-currency" ...> )}
  ```
- **Spec required:** No business logic in components; backend order body takes `currency: "HUF"|"EUR"` as a caller choice.
- **Impact:** Conversion requires a partner, and once the partner loads the select is replaced by static text. An EUR job for a partner whose default is HUF can only be converted as HUF; with items on the order the currency is then locked.

### [MAJOR-04] The traveller shows cancelled/lost records as having completed every stage
- **Rule:** Step 5.1, M10
- **Location:** `frontend/src/components/ui/StageRail.tsx:19-26,35-42`
- **Evidence:**
  ```tsx
  const ordered = [...stages].sort((a, b) => a.position - b.position);
  const currentIdx = ordered.findIndex((s) => s.key === currentKey);
  ...
  const done = currentIdx >= 0 && i < currentIdx;
  ```
  Seeded order stages: `completed` position 50, `cancelled` position 60 (`is_exit`). No `is_active` filter is applied to `stages`.
- **Spec required:** Traveller driven by API stage definitions and history, showing where the vehicle is and what happened before.
- **Impact:** An order cancelled while in `design` renders `production`, `meo` and `completed` with the green check (`text-done`) as done. Deactivated stage definitions still appear on the rail. The rail ignores stage history entirely; "done" is inferred from position.

### [MAJOR-05] Stage-transition and blocker rules re-implemented in components
- **Rule:** M10
- **Location:** `frontend/src/components/forms/OrderStageDialog.tsx:39-43`, `LeadStageDialog.tsx:29,34-38`, `frontend/src/types/api.ts:352-354`
- **Evidence:**
  ```tsx
  // OrderStageDialog.tsx:40-43 (duplicated in LeadStageDialog.tsx:35-38)
  const needsNote =
    !!targetDef &&
    !!currentDef &&
    (targetDef.position < currentDef.position || currentDef.is_terminal);
  // LeadStageDialog.tsx:29
  const targets = definitions.filter((d) => d.key !== current && d.key !== 'won' && d.is_active);
  // types/api.ts:352
  export function isBlockerOverdue(b: Blocker, today: string = new Date().toISOString().slice(0, 10)): boolean {
  ```
- **Spec required:** Components do formatting and display only; the backend decides transitions.
- **Impact:** The save button is disabled based on a client copy of the note rule; if the backend rule changes, the UI blocks or permits the wrong transitions. `isBlockerOverdue` uses the UTC calendar date, so between 00:00 and 01:00/02:00 Budapest time a blocker due yesterday (local) is shown as not overdue.

### [MAJOR-06] `<Money>` performs float arithmetic and rounds HUF fillér away
- **Rule:** N4
- **Location:** `frontend/src/lib/utils/format.ts:8-26`, `frontend/src/components/ui/Money.tsx:15-17`
- **Evidence:**
  ```ts
  export function formatMoney(minorUnits: number, currency: 'HUF' | 'EUR', locale: 'hu-HU' | 'en-US' = 'hu-HU'): string {
    const majorUnits = minorUnits / 100;
    if (currency === 'HUF') {
      return new Intl.NumberFormat(locale, { style: 'currency', currency: 'HUF', minimumFractionDigits: 0, maximumFractionDigits: 0 }).format(majorUnits);
  ```
- **Spec required:** No float math on currency; amounts displayed, not computed.
- **Impact:** Every rendered amount passes through IEEE-754 division. HUF amounts with non-zero fillér (e.g. a line total of `quantity "2.5"` × odd unit price) are displayed rounded to whole forints, so displayed line totals can disagree with the stored `line_total_minor`.

### [MAJOR-07] Unit price entry requires minor units under a forint-looking placeholder
- **Rule:** N4 (money handling in the frontend)
- **Location:** `frontend/src/components/forms/ItemsSection.tsx:24-27,94`
- **Evidence:**
  ```tsx
  function parseUnitPrice(s: string): number | null {
    const n = Number(s.replace(/\s/g, '').replace(',', '.'));
    return Number.isInteger(n) && n >= 0 ? n : null;
  }
  ...
  <input id="it-price" className="input font-mono" inputMode="numeric" placeholder="Ft / cent" {...register('unit_price')} />
  ```
  The parsed value is sent unchanged as `unit_price` (line 74). Negative lines are rejected client-side (`n >= 0`); the backend schema permits negative `unit_price` (`order_items.unit_price BIGINT NOT NULL`, no sign check).
- **Spec required:** Amounts as integer minor units + currency; no client-invented money rules.
- **Impact:** Typing `4850000` into a field labelled "Ft" stores 48 500 Ft. The label shows "Ft / cent" for both currencies. Discount lines are impossible.

### [MAJOR-08] 87 source lines carry hardcoded Hungarian UI text outside the catalogues, plus English literals
- **Rule:** N3
- **Location:** e.g. `frontend/src/app/[locale]/partners/page.tsx:25-47`, `components/ui/StageRail.tsx:23,37,39,48-49,61`, `components/ui/ErrorState.tsx:14,18,22`, `components/ui/Pagination.tsx:22,26,28,30`, `components/tables/FilterBar.tsx:12`, `lib/api/errors.ts:21-45`, `orders/[id]/page.tsx:107,337`, `components/forms/OrderStageDialog.tsx:13`, `lib/utils/format.ts:84`
- **Evidence:**
  ```tsx
  // partners/page.tsx:25,40,42
  header: 'Név',
  header: 'Típus',
  cell: ({ getValue }) => (getValue<string>() === 'business' ? 'Vállalkozás' : 'Személy'),
  // orders/[id]/page.tsx:337
  <h2 className="text-sm font-semibold">Traveller</h2>
  // OrderStageDialog.tsx:13
  return `min. ${d.min_images} db ${d.required_image_category} kép`;
  // lib/api/errors.ts:31
  stage_gate: 'A megrendelés nem léphet tovább, mert egy szükséges feltétel még nem teljesült.',
  ```
  Grep for Hungarian accented characters in non-comment lines of `src/**/*.ts(x)` outside `messages/`: 87 lines. `OrderStageDialog.tsx:13` also prints the raw category key (`completion`).
- **Spec required:** Everything through i18n.
- **Impact:** `/en/*` renders Hungarian table headers, pagination, error states, traveller text and all backend error explanations. The gate text shows an untranslated enum value.

### [MAJOR-09] Semantic colours used decoratively
- **Rule:** N8
- **Location:** `frontend/src/lib/utils/stages.ts:4-9`, `frontend/src/app/globals.css:25,29,39`, `orders/page.tsx:86`, `leads/page.tsx:73`, `partners/page.tsx:31`, `orders/[id]/page.tsx:133`, `components/ui/StageRail.tsx:39`, `OrderStageDialog.tsx:84,96`, `LeadStageDialog.tsx:85`
- **Evidence:**
  ```ts
  // stages.ts:5-8
  if (!def) return 'steel';
  if (def.is_exit) return 'signal';
  if (def.is_terminal) return 'done';
  return 'cold';
  ```
  ```css
  /* globals.css:25,29,39 */
  @apply outline-none ring-2 ring-signal ring-offset-2 ring-offset-surface;
  @apply bg-signal/20 text-steel-900;
  @apply btn bg-signal text-white hover:bg-signal/90 active:bg-signal/80;
  ```
  ```tsx
  // orders/page.tsx:86
  <Link href={`./orders/${row.original.id}`} className="font-mono font-medium text-cold hover:underline">
  ```
- **Spec required:** `--signal` blocked/overdue only; `--cold` MEO/certification only; `--done` complete only.
- **Impact:** Every open stage badge (intake, design, production) is `cold`; every record link is `cold`; every primary button, focus ring, text selection, active tab underline and the current-stage marker are `signal`; cancelled/lost render as `signal`. Orange no longer means "blocked" and blue no longer means "MEO".

### [MAJOR-10] No table column is sortable
- **Rule:** M6
- **Location:** `frontend/src/components/tables/DataTable.tsx:25-30,41-45`
- **Evidence:**
  ```tsx
  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getRowId: getRowId ? (row) => getRowId(row) : undefined,
  });
  ```
  No `getSortedRowModel`, `SortingState` or sort query parameter exists anywhere in `src/`. Header cells are plain `<th scope="col">`.
- **Spec required:** Table columns sortable.
- **Impact:** Orders, leads and partners lists cannot be sorted by any column, including due date and total.

### [MAJOR-11] Modals have no focus trap; three of four have no Escape handling or initial focus
- **Rule:** M7
- **Location:** `frontend/src/components/forms/OrderStageDialog.tsx:59`, `LeadStageDialog.tsx:53`, `LeadConvertDialog.tsx:72`, `components/ui/ConfirmDialog.tsx:26-34`
- **Evidence:**
  ```tsx
  // OrderStageDialog.tsx:59
  <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4" role="dialog" aria-modal="true" aria-label={t('changeStage')} onClick={onClose}>
  // ConfirmDialog.tsx:27-32 — the only Escape handler in src/
  confirmRef.current?.focus();
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  };
  ```
  Grep for `focus-trap|inert|tabIndex|onKeyDown|ArrowLeft|ArrowRight|Escape` in `src/`: 1 hit (`ConfirmDialog.tsx:30`). No dialog returns focus to its trigger. No lightbox exists.
- **Spec required:** Keyboard navigation in the lightbox and all modals — arrows, escape, focus trapping.
- **Impact:** In the stage-change and conversion dialogs, Tab moves focus to the page behind the overlay and Escape does nothing.

### [MAJOR-12] No optimistic updates anywhere
- **Rule:** M8
- **Location:** `frontend/src/components/forms/OrderStageDialog.tsx:45-54`
- **Evidence:**
  ```tsx
  const change = useMutation({
    mutationFn: () => ordersApi.stage(orderId, { stage: target, note: note.trim() || undefined }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.order(orderId) });
  ```
  Grep for `onMutate|cancelQueries|setQueryData`: one hit, `lib/auth/context.tsx:42` (logout). Blocker resolution has no UI (`blockersApi.resolve` unused).
- **Spec required:** Optimistic updates with rollback for stage advancement and blocker resolution.
- **Impact:** Stage changes wait for round-trip plus refetch; blocker resolution cannot be performed at all.

### [MAJOR-13] `noUncheckedIndexedAccess` off; non-null assertions on API data
- **Rule:** M2
- **Location:** `frontend/tsconfig.json:3-27`, `app/[locale]/leads/[id]/page.tsx:41,52`, `app/[locale]/orders/[id]/page.tsx:313`, `components/forms/LeadConvertDialog.tsx:37`, `components/forms/LeadForm.tsx:100`, `components/forms/OrderForm.tsx:45,141`
- **Evidence:**
  ```json
  "strict": true,
  ```
  (no `noUncheckedIndexedAccess` key in `compilerOptions`; `"allowJs": true` is set)
  ```tsx
  // leads/[id]/page.tsx:41
  queryFn: () => partnersApi.get(detail.data!.lead.partner_id!),
  // orders/[id]/page.tsx:313
  {audit.data!.items.map((a) => (
  ```
  Counts in `src/`: `any` 0; non-null assertions 7; `as` casts 27 (including `JSON.parse(text) as T` in `lib/api/client.ts:43` and `undefined as T` at lines 40, 42).
- **Spec required:** Strict on, `noUncheckedIndexedAccess` on, no `any`, no non-null assertions on API data.
- **Impact:** Index access on arrays/records is typed as always defined; API-derived values are asserted non-null rather than checked.

### [MAJOR-14] No `<DateDisplay>`; date formatting called directly from components
- **Rule:** M3
- **Location:** `app/[locale]/orders/page.tsx:135`, `app/[locale]/orders/[id]/page.tsx:160,165,168,184,195,287,319`, `app/[locale]/leads/page.tsx:117`, `app/[locale]/leads/[id]/page.tsx:114,151`, `app/[locale]/partners/[id]/page.tsx:128`, `components/ui/StageRail.tsx:68-69`, `components/forms/OrderForm.tsx:35-38`
- **Evidence:**
  ```tsx
  // orders/page.tsx:135
  <span className="font-mono">{getValue<string | null>() ? formatDate(getValue<string>()) : '—'}</span>
  // OrderForm.tsx:36-37
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  ```
  No component named `DateDisplay` exists. `new Date(` also appears in `types/api.ts:352` and `lib/utils/format.ts:29,38,77`.
- **Spec required:** Every date through `<DateDisplay>`; no scattered format calls in components.
- **Impact:** Date presentation (mono, tabular figures, alignment, relative vs absolute) is decided per call site; 14 call sites across 6 files.

### [MAJOR-15] Configuration queries fail silently; the traveller renders empty on error
- **Rule:** M5
- **Location:** `app/[locale]/orders/[id]/page.tsx:49-52,91,345-350`, `components/forms/OrderStageDialog.tsx:34,68-69`, `app/[locale]/orders/page.tsx:47-54,179,192`, `components/forms/AssigneeField.tsx:27-32,60`, `components/forms/PartnerPicker.tsx:31-35,61-79`
- **Evidence:**
  ```tsx
  // orders/[id]/page.tsx:49-52,91
  const stagesQuery = useQuery({ queryKey: qk.stages('order'), queryFn: () => configApi.stages('order') });
  const defs = stagesQuery.data?.items ?? [];
  // OrderStageDialog.tsx:68-69
  {targets.length === 0 ? (
    <p className="text-sm text-steel-500">{t('noOtherStage')}</p>
  ```
  Per data view:

  | View | Loading | Empty | Error | Loaded |
  |---|---|---|---|---|
  | Orders list | spinner | yes (one message for all causes) | yes | yes |
  | Leads list | spinner | yes (one message for all causes) | yes | yes |
  | Partners list | spinner | yes (one message for all causes) | yes | yes |
  | Order detail | skeleton | n/a | yes | yes |
  | Order → stage history tab | text | yes | yes | yes |
  | Order → audit tab | text | yes | yes | yes |
  | Order → traveller (stage definitions) | none | none | none | yes |
  | Stage filter / project-type filter selects | none | none | none | yes |
  | Stage-change dialog targets | none | "noOtherStage" also on error | none | yes |
  | Assignee select (admin) | disabled only | none | none | yes |
  | Partner picker | text | yes | none | yes |
  | Lead detail → partner name | none | falls back to `#id` | none | yes |
  | Reports / Settings / Emails / Blockers / Admin / Dashboard | — | fake (MAJOR-01) | — | — |
- **Spec required:** Loading, empty, error and loaded states in every data view.
- **Impact:** If `/stage-definitions` fails, the order page shows a blank traveller and the stage dialog claims "Nincs másik választható fázis." with no error.

### [MINOR-01] Screens and tabs outside the approved list
- **Rule:** N1
- **Location:** `frontend/src/components/layout/Sidebar.tsx:22-32`, `app/[locale]/page.tsx`, `app/[locale]/blockers/page.tsx`, `app/[locale]/admin/page.tsx`, `app/[locale]/emails/page.tsx`, `app/[locale]/orders/[id]/page.tsx:93-99`
- **Evidence:**
  ```tsx
  // orders/[id]/page.tsx:93-99
  { key: 'data', label: t('tabsData') },
  { key: 'items', label: `${t('tabsItems')} (${items.length})` },
  { key: 'stages', label: t('tabsStages') },
  { key: 'blockers', label: `${t('tabsBlockers')} (${openBlockers.length})` },
  { key: 'audit', label: t('tabsAudit') },
  ```
  `hu.json:212` `"tabsBlockers": "Akadályok"`.
- **Spec required:** Order detail tabs Adatok, Tervek, Képek, Blokkolók, Levelezés; no screens beyond the nine listed.
- **Impact:** Dashboard, global blockers list, admin and global e-mail routes exist in navigation. Order detail has Tételek/Fázisok/Napló tabs and lacks Tervek/Képek/Levelezés; the blocker tab is labelled "Akadályok", not "Blokkolók". No inventory, invoicing, cost, purchase-order, timesheet, portal or chat screen exists.

### [MINOR-02] One hardcoded stage key literal
- **Rule:** N2
- **Location:** `frontend/src/components/forms/LeadStageDialog.tsx:29`
- **Evidence:**
  ```tsx
  const targets = definitions.filter((d) => d.key !== current && d.key !== 'won' && d.is_active);
  ```
  No other occurrence of `intake|design|production|meo|completed|won|lost|cancelled|new|contacted|quoted` as a stage literal, and no Hungarian stage label literal, in `src/**/*.ts(x)`. (`hu.json:283-285` `images.intake/production/completion` are image-category labels.)
- **Spec required:** No hardcoded stage names including string literals.
- **Impact:** Lead stage filtering depends on a specific key existing with that meaning.

### [MINOR-03] Colours outside the token set reach the rendered page
- **Rule:** N7
- **Location:** `frontend/tailwind.config.ts:36-39,46-49`, `app/globals.css:39`, `components/layout/Sidebar.tsx:44,65`, `app/[locale]/login/page.tsx:41`, checkboxes at `app/[locale]/orders/page.tsx:211-218`, `leads/page.tsx:182-190`, `partners/page.tsx:120-128`
- **Evidence:**
  ```ts
  // tailwind.config.ts:37-38
  'panel': '0 1px 2px 0 rgb(0 0 0 / 0.05)',
  'card': '0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1)',
  // tailwind.config.ts:47
  require('@tailwindcss/forms'),
  ```
  Built CSS (`.next/static/css/b78991f0df6fea4d.css`) contains `#2563eb` 4 times from `@tailwindcss/forms` (`color:#2563eb` on checkboxes, `--tw-ring-color:#2563eb`, `border-color:#2563eb` on focus) and `border-color:#6b7280`. `text-white` used at the locations above.
- **Spec required:** No colour outside the token set.
- **Impact:** Filter checkboxes render Tailwind blue when checked and gray-500 borders; default-palette white and black-alpha shadows are used.

### [MINOR-04] Animations beyond the permitted set; no reduced-motion handling
- **Rule:** N10
- **Location/Evidence (each animation and its trigger):**
  - `app/globals.css:16` `scroll-smooth` — any in-page scroll
  - `app/globals.css:35` `.btn … transition-colors` — hover/active on every button
  - `app/globals.css:63` `.input … transition-colors` — focus on every input/select
  - `app/globals.css:127` `.table tbody tr … transition-colors` — row hover
  - `app/[locale]/orders/[id]/page.tsx:131` `transition-colors` — tab hover/selection
  - `components/layout/Sidebar.tsx:64` `transition-colors` — nav hover
  - `components/ui/LoadingState.tsx:4` `animate-spin` — every loading state
  - `components/ui/LoadingState.tsx:14,23,24,25` `animate-pulse` — table/detail skeletons

  `motion-safe`/`motion-reduce`: 0 occurrences; `prefers-reduced-motion`: 0 in built CSS.
- **Spec required:** No animation beyond panel/upload/stage-transition feedback.
- **Impact:** Hover and loading motion throughout; none suppressed for reduced-motion users.

### [MINOR-05] Type scale bypassed
- **Rule:** Step 4 (type scale)
- **Location:** repository-wide; `components/layout/PageHeader.tsx:15`, `app/[locale]/orders/[id]/page.tsx:105-107`, `components/ui/StageRail.tsx:31,37,39,41`
- **Evidence:** Occurrences in `src/`: `text-sm` 66 (14px), `text-xs` 20 (12px), `text-base` 1, arbitrary `[..px]` 8 (e.g. `h-[19px]`, `left-[9px]`, `w-[280px]`); scale tokens `text-metadata` 23, `text-body` 1, `text-section` 15, `text-page-title` 2, `text-record-title` 0.
  ```tsx
  // PageHeader.tsx:15 — used for the order record title
  <h1 className="text-page-title font-semibold tracking-tight">{title}</h1>
  ```
- **Spec required:** 12.8 / 16 / 20 / 25 / 31 scale.
- **Impact:** Most body and label text is 14px or 12px, not on the scale; the record title renders at 31px instead of 25px.

### [MINOR-06] SaaS-card styling and centred layout
- **Rule:** Step 4 (layout, card styling, left alignment)
- **Location:** `app/globals.css:70-72`, `components/layout/AppShell.tsx:39`, `components/ui/EmptyState.tsx:6`, `components/ui/ErrorState.tsx:16`, `components/forms/ItemsSection.tsx:224`
- **Evidence:**
  ```css
  .card {
    @apply bg-surface rounded-xl border border-steel-200 shadow-card;
  }
  ```
  ```tsx
  <div className="mx-auto max-w-7xl px-6 py-6 space-y-6">{children}</div>
  ```
  `className="card…"` occurrences: 73 (filter bar, every detail section, every form, every dialog, the traveller).
- **Spec required:** No identical rounded shadowed boxes; left-aligned data views.
- **Impact:** Every section is a rounded shadowed card; content column is centred in a 1280px max-width container on wide screens; empty/error states and the empty items row are centred.

### [MINOR-07] Money and date columns in lists are left-aligned; dates lack tabular figures
- **Rule:** Step 4 (money/date columns)
- **Location:** `components/tables/DataTable.tsx:42,53`, `app/[locale]/orders/page.tsx:119-137`, `app/[locale]/leads/page.tsx:102-118`
- **Evidence:**
  ```tsx
  // DataTable.tsx:42,53
  <th key={h.id} scope="col">
  <td key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</td>
  // orders/page.tsx:134-136
  <span className="font-mono">{getValue<string | null>() ? formatDate(getValue<string>()) : '—'}</span>
  ```
  `tabular-nums` appears once (`globals.css:137`, `.text-money`). Only the items tables use `text-right`.
- **Spec required:** Money and date columns mono, tabular figures, right-aligned.
- **Impact:** Order totals, due dates, lead age and created dates are left-aligned in all three list tables.

### [MINOR-08] Empty states do not distinguish no data / no match / no permission
- **Rule:** M5
- **Location:** `components/tables/DataTable.tsx:33`, `messages/hu.json:471-473`, `components/ui/Pagination.tsx:21-23`
- **Evidence:**
  ```tsx
  if (data.length === 0) return <EmptyState title={emptyTitle} />;
  ```
  ```json
  "noOrders": "Még nincsenek megrendelések. Kattintson az \"Új megrendelés\" gombra a kezdéshez.",
  ```
  ```tsx
  {offset + 1}–{offset + loaded} · {limit}/oldal
  ```
  `filterNoResults` and `permissionDenied` keys exist in `hu.json:481-482` with zero call sites.
- **Spec required:** Four distinct states per data view.
- **Impact:** A filter matching nothing says no orders exist and tells viewers (who have no create button) to click "Új megrendelés". An empty page shows "1–0 · 50/oldal".

### [MINOR-09] Assignee filter shows "Nincs kiválasztva" while unfiltered; unassigned cannot be filtered
- **Rule:** M6
- **Location:** `components/forms/AssigneeField.tsx:38-44,59`, `app/[locale]/orders/page.tsx:39,44-45`, `app/[locale]/leads/page.tsx:35,40-41`
- **Evidence:**
  ```tsx
  // orders/page.tsx:44-45
  const assignedTo =
    assignee === 'me' ? (user?.id ?? null) : assignee === 'all' ? undefined : (assignee ?? undefined);
  // AssigneeField.tsx:59 (admin)
  value={value === 'me' || value === 'all' ? value : (value ?? '')}
  // AssigneeField.tsx:40 (non-admin)
  value={String(value)}
  ```
- **Spec required:** Lists filterable.
- **Impact:** Initial state `null` displays the "unassigned" option (admin) or matches no option (`"null"`, non-admin) while no filter is applied; choosing "Nincs kiválasztva" also applies no filter.

### [MINOR-10] English catalogue incomplete; wrong interpolation syntax; duplicate keys
- **Rule:** N3, M4
- **Location:** `messages/en.json`, `messages/hu.json:36,57,487-488`
- **Evidence:** Read-only key comparison: hu 452 keys, en 231; 221 hu keys missing from en; namespaces absent from en: `blockers, images, documents, emails, reports, admin, settings, stageDefinitions, projectTypes, users, validation`; 59 translation calls in components resolve to keys missing in en (e.g. `orders.number` at `orders/page.tsx:83`, `validation.required` at `ContactSection.tsx:66`, `orders.currencyLocked` at `OrderForm.tsx:186`).
  ```json
  "minLength": "Legalább {{min}} karakter szükséges",
  ```
  `common.email` defined twice in `hu.json` (lines 36, 57); `common.save`, `create`, `edit`, `search`, `name` defined twice in `en.json`.
- **Spec required:** Everything through i18n.
- **Impact:** `/en/*` routes show missing-message fallbacks on orders, partners and forms. next-intl uses ICU `{min}`; `{{min}}` renders literally.

### [MINOR-11] Traveller hidden below 1024px; heading in English; blockers only as a count
- **Rule:** Step 3.3, Step 5.1
- **Location:** `app/[locale]/orders/[id]/page.tsx:334,337`, `components/ui/StageRail.tsx:46-51`
- **Evidence:**
  ```tsx
  <aside className="hidden w-[280px] shrink-0 lg:block">
  ...
  <h2 className="text-sm font-semibold">Traveller</h2>
  ```
  ```tsx
  {daysInStage} napja itt
  {openBlockers > 0 && ` · ${openBlockers} nyitott akadály`}
  ```
- **Spec required:** Traveller rail persistent; blockers shown inline.
- **Impact:** Below the `lg` breakpoint the order page has no traveller. Inline blocker information is a number only, with no blocker text or responsible party.

### [MINOR-12] Order detail shows project type and assignee as raw ids
- **Rule:** M5 (loaded state)
- **Location:** `app/[locale]/orders/[id]/page.tsx:158,166`
- **Evidence:**
  ```tsx
  <Info label={t('projectType')} value={order.project_type_id ? `#${order.project_type_id}` : '—'} mono />
  <Info label={t('assignedTo')} value={order.assigned_to ? `#${order.assigned_to}` : '—'} mono />
  ```
- **Spec required:** Loaded state presents the record.
- **Impact:** Users see `#3` instead of the project type label or the assignee's name.

### [MINOR-13] Dependencies outside the locked stack; required stack members absent or unused
- **Rule:** Locked stack
- **Location:** `frontend/package.json:12-43`
- **Evidence:** Not in stack: `axios` (0 imports), `js-cookie` (0 imports), `lucide-react`, `clsx`, `tailwind-merge`, `@hookform/resolvers`, `@tailwindcss/forms`, `@tailwindcss/typography` (registered, `prose` 0 uses). In stack but unused: `recharts` (0 imports), `date-fns` (0 imports). In stack but absent: `@tanstack/react-virtual` (not installed), shadcn/ui (no copied components, no `@radix-ui/*`).
- **Spec required:** Only the locked stack.
- **Impact:** Two unused HTTP/cookie libraries ship as dependencies; the virtualisation library the gallery depends on is not installed.

### [MINOR-14] Dead helpers containing N4-pattern money code
- **Rule:** N4 (grep targets)
- **Location:** `frontend/src/lib/utils/format.ts:48-73,87-107`, `frontend/src/hooks/useI18n.ts:7-24`
- **Evidence:**
  ```ts
  export function parseDecimalString(value: string): number {
    return parseFloat(value.replace(',', '.'));
  }
  export function toDecimalString(value: number): string {
    return value.toFixed(2).replace('.', ',');
  }
  ```
  `formatNumber`, `parseDecimalString`, `toDecimalString`, `generateId`, `debounce`, `getInitials`, `truncate`, `isEmpty`, `useLocaleSwitcher`, `useT`, `roleLabel`: 0 call sites.
- **Spec required:** No `toFixed`/`parseFloat` on amounts.
- **Impact:** Float decimal helpers are exported from the shared formatting module.

### [MINOR-15] Tabs and partner picker not programmatically wired
- **Rule:** M7 / accessibility
- **Location:** `app/[locale]/orders/[id]/page.tsx:124-139`, `components/forms/PartnerPicker.tsx:39,49-58`
- **Evidence:**
  ```tsx
  <button key={tb.key} role="tab" aria-selected={tab === tb.key} onClick={() => setTab(tb.key)} ...>
  ```
  ```tsx
  <span className="label">{label}</span>
  ...
  <input className="input" placeholder={t('searchPlaceholder')} value={q} ...
  ```
- **Spec required:** Keyboard navigation and accessible controls.
- **Impact:** Tabs have no `aria-controls`, no `role="tabpanel"` and no arrow-key handling. The partner search input has no associated label; its result list has no listbox semantics or keyboard selection.

### [MINOR-16] Backend validation detail replaced by a generic message
- **Rule:** M5 (error state)
- **Location:** `frontend/src/lib/api/errors.ts:22,47-49,61`
- **Evidence:**
  ```ts
  validation: 'Érvényesítési hiba. Ellenőrizze a megadott adatokat.',
  ...
  return HUNGARIAN_MESSAGES[code] ?? fallback;
  ...
  return new ApiError(code, res.status, hungarianMessage(code, message));
  ```
- **Spec required:** Error states that say what went wrong.
- **Impact:** Every 400 (`validation`) — e.g. "password must be at least 12 characters", "recipient 'x' is invalid" — is shown as the same generic sentence; the field-level reason is discarded. The property is still named `backendMessage`.

### [NOTE-01] Route protection is client-side only
- **Location:** `components/layout/AppShell.tsx:15-17`, `src/middleware.ts:4`
- **Evidence:** `if (!isLoading && !isAuthenticated) router.replace(`/${locale}/login`);`; middleware is `createMiddleware(routing)` (locale only).
- **Impact:** Page bundles load for unauthenticated visitors before redirect. The backend remains the enforcement point.

### [NOTE-02] `force-dynamic` declared but routes are prerendered
- **Location:** `app/[locale]/layout.tsx:7-8,17-19`
- **Evidence:** `export const dynamic = 'force-dynamic';` alongside `generateStaticParams()`; build output marks `/[locale]/orders`, `/leads`, `/partners`, `/login` etc. as `● (SSG)` with `/hu/*` and `/en/*` generated.
- **Impact:** The comment "never prerender" does not describe the build result.

### [NOTE-03] All API traffic proxied through the Next.js server to a hardcoded host; upload flow not implemented
- **Location:** `frontend/next.config.js:24-31`, `lib/api/endpoints.ts:143-145`
- **Evidence:** `destination: 'http://localhost:8080/api/:path*'`; `initiateUpload`/`completeUpload` have 0 call sites; no fetch to a presigned URL exists.
- **Impact:** Upload behaviour (presigned direct-to-S3 vs proxy) cannot be assessed; the rewrite target is not environment-driven.

## Spec drift — built to the superseded brief
- **Cost and invoice strings.** `messages/hu.json:87` `navigation.costs: "Költségek"`, `hu.json:247-254` `orders.tabs.{costs: "Költségek", invoices: "Számlák", plans, images, blockers, data}`, `en.json:91` `navigation.costs`. Zero references from code. Entanglement: none (catalogue only).
- **Superseded brief in the repository root.** `FRONTEND_PLAN.md` §64-65 prescribes order tabs `[Költség]`/`Költségek` and `[Számlák]`, §52 maps `done → completed / paid`, §87 lists "invoice/business rules" among backend-owned concerns, §99 discusses VAT on line items, §75-82 define Android scope. It is the document the code cites (`// FRONTEND_PLAN §89`, `§26`, `§92` in `errors.ts:3`, `StageRail.tsx:1`). Entanglement: documentation, but it remains the brief an agent reading the repo will follow.
- **Global blocker list page** (`app/[locale]/blockers/page.tsx`) — old Phase 4 "blocker list". Placeholder only. Entangled with `Sidebar.tsx:27` and the link at `orders/[id]/page.tsx:267-269`.
- **Admin operations page** (`app/[locale]/admin/page.tsx`, `adminApi` at `endpoints.ts:188-194`, `AdminStatus`/`Job` types at `types/api.ts:485-501`) — old Phase 8 "admin status, jobs, FX fetch, operational jobs"; not among the current Settings items. Placeholder + unused typed wrappers. Entangled with `Sidebar.tsx:30`.
- **Dashboard** (`app/[locale]/page.tsx`) — not in either the current screen list or as a screen in the old plan's phase list; placeholder. Entangled with `Sidebar.tsx:23` and the logo link `Sidebar.tsx:43`.
- **Global e-mail page** (`app/[locale]/emails/page.tsx`) — old Phase 6 "email history"; current spec has per-record correspondence. Placeholder. Entangled with `Sidebar.tsx:28`.
- **Items/Stages/Audit tabs** in order detail (`orders/[id]/page.tsx:93-99`) follow the old plan's Phase 3 list ("line items, stage history, audit") rather than the current tab set.
- Not found: invoicing, billing, VAT fields, cost tracking, inventory, Android contact sync or calling (`tel:` 0 occurrences; `OrderBody.items` has no `vat_rate`).

## Rule compliance table
| Rule | Status | Evidence |
|---|---|---|
| N1 | FAIL | Dashboard, `/blockers`, `/admin`, `/emails` routes; order tabs Tételek/Fázisok/Napló (MINOR-01) |
| N2 | PARTIAL | Stage lists, labels, badges API-driven; one literal `'won'` at `LeadStageDialog.tsx:29` (MINOR-02) |
| N3 | FAIL | 87 lines of Hungarian literals outside catalogues; `Traveller` English literal; en catalogue missing 221 keys (MAJOR-08, MINOR-10) |
| N4 | FAIL | `minorUnits / 100` in `format.ts:9`; minor-unit price entry `ItemsSection.tsx:24-27`; `toFixed`/`parseFloat` helpers (MAJOR-06, MAJOR-07, MINOR-14) |
| N5 | PASS | `use server` 0 occurrences; all writes via `apiFetch` to `/api` (`lib/api/client.ts:30-44`) |
| N6 | PASS (vacuous) | No shadcn/ui components or `@radix-ui` packages exist, so no default shadcn styling ships |
| N7 | FAIL | `@tailwindcss/forms` `#2563eb`/`#6b7280` in built CSS; `text-white`; rgb shadows in config (MINOR-03). Tokens themselves defined with exact spec hex (`tailwind.config.ts:12-19`) |
| N8 | FAIL | `cold` for all open stages and links; `signal` for buttons, focus ring, selection, active tab, current stage, exit stages (MAJOR-09) |
| N9 | PASS (vacuous) | No image rendering exists (`<img`, `next/image` 0 occurrences); gallery not built |
| N10 | FAIL | spin/pulse loaders, hover transitions, `scroll-smooth`; no reduced-motion (MINOR-04) |
| N11 | PASS | Dependencies contain no Redux/Zustand/Jotai/MobX/Recoil; state is TanStack Query + React state/context |
| N12 | FAIL | `types/api.ts` hand-written, no OpenAPI anywhere; live drift on `/auth/me`, login, settings, admin status, e-mails (BLOCKER-01, BLOCKER-02) |
| M1 | FAIL | `.github/workflows/ci.yml` has only a `backend` job; no type generation step |
| M2 | FAIL | `strict: true`; `noUncheckedIndexedAccess` absent; `any` 0; 7 non-null assertions incl. on API data; unchecked `JSON.parse(text) as T` (MAJOR-13) |
| M3 | FAIL | `<Money>` used for all displayed amounts; no `<DateDisplay>`, 14 direct `formatDate`/`formatDateTime` calls (MAJOR-14) |
| M4 | NOT VERIFIED | `hu` is default locale and the fuller catalogue; layout against Hungarian lengths not measured (see Not verified) |
| M5 | PARTIAL | Lists and detail have loading/error/empty; config queries silent; empty states undifferentiated; placeholder screens fake empty (MAJOR-01, MAJOR-15, MINOR-08) |
| M6 | FAIL | No sorting in `DataTable.tsx`; filters exist server-side; assignee filter mis-mapped (MAJOR-10, MINOR-09) |
| M7 | FAIL | No lightbox; no focus trap in any dialog; Escape only in `ConfirmDialog` (MAJOR-11, MINOR-15) |
| M8 | FAIL | No `onMutate`/rollback; blocker resolution absent (MAJOR-12) |
| M9 | FAIL | No image UI exists; no intake marking or capture timestamp display |
| M10 | FAIL | Transition rules, `won` filter, overdue computation, conversion currency, traveller "done" inference in components (MAJOR-02..05) |

## Screen inventory
| Screen | Status | Path | Notes |
|---|---|---|---|
| 1. Leads | partial | `app/[locale]/leads/page.tsx` (207 LOC), `leads/[id]/page.tsx` (176), `leads/new/page.tsx` (30) | Table, stage filter, assignee filter (MINOR-09), convert dialog. No source filter, no age filter (age is a column only), no quick-add (full-page form) |
| 2. Orders | partial | `app/[locale]/orders/page.tsx` (235), `orders/new/page.tsx` (33) | Server-side filters (q, stage, partner, project type, assignee, open) + pagination. No sorting, no saved views, no density toggle |
| 3. Order detail | partial | `app/[locale]/orders/[id]/page.tsx` (353) | Tabs Adatok/Tételek/Fázisok/Akadályok/Napló. Tervek, Képek, Levelezés missing; blockers read-only; traveller `lg` only (MINOR-11) |
| 4. Partners | partial | `app/[locale]/partners/page.tsx` (147), `partners/[id]/page.tsx` (185), `partners/new/page.tsx` (29) | Table, kind/archived filters, detail with contacts and order history. No correspondence |
| 5. Gallery | missing | — | No component, no image rendering, `@tanstack/react-virtual` not installed |
| 6. Email compose | missing | — | `emailApi.preview/send` unused |
| 7. Correspondence | missing | `app/[locale]/emails/page.tsx` (14) is a placeholder | No per-record history on order/partner/lead |
| 8. Reports | missing | `app/[locale]/reports/page.tsx` (13) placeholder | No charts, no CSV export |
| 9. Settings | missing | `app/[locale]/settings/page.tsx` (13) placeholder | No users/roles, stage definitions, templates, suppressions, kill switch UI |
| Dashboard | not in spec | `app/[locale]/page.tsx` (16) | Placeholder with roadmap text |
| Blockers (global) | not in spec | `app/[locale]/blockers/page.tsx` (14) | Placeholder |
| Admin | not in spec | `app/[locale]/admin/page.tsx` (13) | Placeholder |
| E-mails (global) | not in spec | `app/[locale]/emails/page.tsx` (14) | Placeholder |
| Login | not in spec (auth prerequisite) | `app/[locale]/login/page.tsx` (64) | NOTE severity |
| Password change | not in spec (auth prerequisite) | `app/[locale]/password/page.tsx` (54) | NOTE severity; unreachable via forced-change gate (BLOCKER-01) |

Component inventory (all custom-built; shadcn-derived: none; third-party UI components: none, icons from `lucide-react`):

| Group | Files (LOC) |
|---|---|
| Layout | `layout/AppShell.tsx` (37), `layout/Sidebar.tsx` (81), `layout/PageHeader.tsx` (20) |
| Tables | `tables/DataTable.tsx` (57), `tables/FilterBar.tsx` (24) |
| UI | `ui/StageRail.tsx` (73), `ui/Money.tsx` (18), `ui/StatusBadge.tsx` (17), `ui/EmptyState.tsx` (12), `ui/ErrorState.tsx` (26), `ui/LoadingState.tsx` (26), `ui/ConfirmDialog.tsx` (61), `ui/Pagination.tsx` (34) |
| Forms/dialogs | `forms/OrderForm.tsx` (248), `forms/ItemsSection.tsx` (233), `forms/ContactSection.tsx` (215), `forms/PartnerForm.tsx` (191), `forms/LeadForm.tsx` (174), `forms/LeadConvertDialog.tsx` (129), `forms/OrderStageDialog.tsx` (103), `forms/LeadStageDialog.tsx` (93), `forms/PartnerPicker.tsx` (89), `forms/AssigneeField.tsx` (74) |
| Lib/infra | `types/api.ts` (445), `lib/api/endpoints.ts` (183), `lib/utils/format.ts` (93), `lib/auth/context.tsx` (90), `lib/api/errors.ts` (59), `lib/query/provider.tsx` (50), `lib/api/client.ts` (47), `lib/utils/stages.ts` (8), `hooks/useI18n.ts` (18), `hooks/useDebouncedValue.ts` (10), `i18n.ts` (18), `routing.ts` (7), `middleware.ts` (6), `app/globals.css` (128), `app/layout.tsx` (26), `app/[locale]/layout.tsx` (33) |
| Catalogues | `messages/hu.json` (500), `messages/en.json` (254) |

Not present from the specified component set: `<ImageGrid>`, `<ImageLightbox>`, `<Uploader>`, `<DateDisplay>`.

## Dependencies
| Package | Version (declared / installed) | In locked stack? | Used for |
|---|---|---|---|
| next | ^14.2.0 / 14.2.35 | yes | App Router framework |
| react, react-dom | ^18.3.0 / 18.3.1 | implied | runtime |
| typescript (dev) | ^5.4.0 / 5.9.3 | yes | typecheck |
| tailwindcss (dev) | ^3.4.0 / 3.4.19 | yes | styling |
| @tanstack/react-query | ^5.28.0 / 5.102.8 | yes | server state, auth `me` query |
| @tanstack/react-table | ^8.16.0 / 8.21.3 | yes | `DataTable` (core row model only) |
| react-hook-form | ^7.51.0 / 7.87.0 | yes | forms |
| zod | ^3.22.0 / 3.25.76 | yes | form schemas |
| next-intl | ^3.11.0 / 3.26.5 | yes | i18n, locale middleware |
| recharts | ^2.12.0 / 2.15.4 | yes | unused (0 imports) |
| date-fns | ^3.6.0 / 3.6.0 | yes | unused (0 imports) |
| @tanstack/react-virtual | — / not installed | yes | absent |
| shadcn/ui | — / not present | yes | absent |
| @hookform/resolvers | ^3.3.0 / 3.10.0 | no | `zodResolver` |
| clsx | ^2.1.0 / 2.1.1 | no | `cn()` in `format.ts` |
| tailwind-merge | ^2.2.0 / 2.6.1 | no | `cn()` in `format.ts` |
| lucide-react | ^0.372.0 / 0.372.0 | no | icons (Sidebar, StageRail, EmptyState, ErrorState, login) |
| js-cookie | ^3.0.5 / 3.0.8 | no | unused (0 imports) |
| axios | ^1.6.0 / 1.20.0 | no | unused (0 imports; client uses `fetch`) |
| @tailwindcss/forms (dev) | ^0.5.0 / 0.5.11 | no | base form styles (source of `#2563eb`) |
| @tailwindcss/typography (dev) | ^0.5.0 / 0.5.20 | no | registered; `prose` unused |
| @types/node, @types/react, @types/react-dom, @types/js-cookie (dev) | ^20.12.0, ^18.3.0, ^18.3.0, ^3.0.6 / not checked | no | type packages |
| postcss, autoprefixer (dev) | ^8.4.0, ^10.4.0 / not checked | no | Tailwind pipeline |
| eslint, eslint-config-next (dev) | ^8.57.0 / 8.57.1, ^14.2.0 / not checked | no | lint |

## What's actually good
- `npm run build`, `npx tsc --noEmit`, `npm run lint` all exit 0 with no errors or warnings.
- `any` occurs 0 times in `src/`.
- Web session token is not accessible to JavaScript: at runtime on `/hu/login`, `document.cookie` was `NEXT_LOCALE=hu` and `localStorage` had no keys; no `localStorage`/`sessionStorage`/`document.cookie`/`js-cookie` usage in `src/`; requests use `credentials: 'include'` (`client.ts:34`).
- No Server Actions; every mutation is a `fetch` to `/api` (`client.ts:30-44`).
- Stage filter options, stage badges and stage-dialog targets are rendered from `/stage-definitions` `label_hu`; badge tone derives from `is_exit`/`is_terminal` flags, not names (`stages.ts:4-9`); rail order comes from API `position`.
- PATCH bodies for partners, contacts, leads and orders are built as diffs that omit unchanged fields and send `null` to clear (`PartnerForm.tsx:51-70`, `LeadForm.tsx:49-63`, `ContactSection.tsx:129-140`, `OrderForm.tsx:67-97` — currency excepted, MAJOR-02).
- Line totals and order totals are displayed from backend `line_total_minor`/`value.total_minor`/`total_huf_minor`; quantity is kept as a decimal string (`ItemsSection.tsx:66-72`); no amount addition or currency conversion in the frontend.
- List pages use backend `limit`/`offset` pagination at 50 (`orders/page.tsx:26,73-74`).
- Mutations invalidate TanStack Query keys; no timer-based refetching.
- Backend error envelope parsed in one place (`errors.ts:51-62`) with Hungarian text for every code in `docs/API.md`.
- Currency select is disabled while items exist (`orders/[id]/page.tsx:147`, `OrderForm.tsx:182`).
- IBM Plex Sans loaded via `next/font/google` with `latin-ext` (`app/layout.tsx:5-17`; built CSS includes `u+0100-02ba`). Runtime on `/hu/login`: body font `__IBM_Plex_Sans_f04d51`, weights 400/500/600 loaded, a 12.8px `text-metadata` probe reading "Őrült űrhajós — 4 850 000 Ft" rendered in Plex Sans with ő and ű drawn correctly, `document.fonts.check('12.8px …', 'őű')` returned `true`.
- Colour tokens exist with the exact spec values (`tailwind.config.ts:12-19`) and no raw hex appears in components.

## Not verified
- **Authenticated screens at runtime.** The backend was not running and no account was created, so no list, detail, form or dialog was rendered against real data. BLOCKER-01, MAJOR-02/03/04, the four-state behaviour and the per-view table in MAJOR-15 are established by reading frontend and backend source, not by observation.
- **Keyboard tab-through of a modal and the lightbox (Step 5.8).** Dialogs require authentication and data; the lightbox does not exist. MAJOR-11 is static evidence only.
- **Gallery DOM node count with 120 images (Step 5.2).** No gallery exists.
- **Hungarian layout fit (M4).** String-length behaviour on data screens not measured.
- **ő/ű in IBM Plex Mono.** The Mono family is declared and resolves (`__IBM_Plex_Mono_aa1656`) but was not loaded on the login page, so Mono glyph rendering at 12.8px was not observed.
- **Contrast ratios** of token combinations (e.g. `text-signal` on `bg-signal/10`, `text-steel-500` on `bg-panel`).
- **Responsive behaviour** below `lg` beyond reading class names.
- **CI behaviour.** Commands were run locally on Windows (Next 14.2.35, Node 24.19.0); no CI run exists for the frontend.
- **Backend contract beyond the handlers read** (`api/auth.rs`, `api/admin.rs`, `repo/config.rs`, `repo/emails.rs`, `api/media.rs`, `service/auth.rs`, `docs/API.md`). Other response shapes in `types/api.ts` (reports, jobs, images, documents, sessions) were not compared field by field.
- **`theme('colors.steel.200')` in `globals.css:146,159`.** The theme path does not match the flat `steel-200` token key; the `.scrollbar-thin` utility is unused and absent from the built CSS, so its behaviour when used was not observed.
- **Lint coverage.** `next lint` runs `next/core-web-vitals` only; the zero-warning result says nothing about i18n, colour or accessibility rules.
