# Autotherm CRM — Frontend Plan

> **Provenance.** Assembled from the current specification used for the frontend audit
> (`AUDIT.md`) and the rule corrections issued with the remediation brief. The canonical
> `autotherm-crm-frontend-plan.md` was not available in this workspace when this file was
> written; if it differs, it wins and this file should be replaced by it. Section numbers
> below are what source comments cite.

## 1. Scope

Web client for the Autotherm project lifecycle system: Lead → Order → Design → Production →
MEO documentation → Done. Hungarian is the working language.

Out of scope, and not to be built: inventory or stock, invoicing, billing, payments,
receivables, VAT, bank accounts, cost tracking, margin or profit reporting, purchase orders,
timesheets, customer portal, chat, Android contact sync and calling.

Sources of truth, in order: the Rust backend and `docs/API.md` for behaviour and contract;
`docs/DECISIONS.md` for decisions already made; this document for frontend architecture,
UX and visual design.

## 2. API contract

- Base path `/api`, JSON in and out. Every write goes to the Rust API — no Next.js Server
  Actions or API routes for business mutations.
- Types come from the backend's OpenAPI document, generated in CI; the build fails on drift.
  No hand-written API types. Responses are validated at the client boundary.
- Money is integer minor units plus an explicit currency. Quantities are decimal strings.
- Errors are `{"error": {"code", "message"}}`; codes are handled intentionally (§14).
- Lists return `{"items": [...]}` with `limit` (1–200, default 50) and `offset`.
- PATCH: omit a field to keep it, `null` to clear it.
- Web auth is the httpOnly `autocrm_session` cookie. Nothing sensitive in `localStorage`.
- The backend is the security boundary; the UI hides or disables what a capability forbids.

## 3. Stack

Locked:

```
Next.js (App Router) · TypeScript strict · Tailwind
@tanstack/react-query · @tanstack/react-table · @tanstack/react-virtual
react-hook-form · zod · recharts · next-intl · date-fns
@radix-ui/react-dialog · @radix-ui/react-tabs · @radix-ui/react-select
```

- **Radix (rule correction).** Components are custom-built. Radix is adopted as unstyled
  primitives for dialog and tabs, styled with project tokens. `@radix-ui/react-select` was
  deliberately NOT adopted (R6): the remaining selects are native, labelled and
  keyboard-operable, and installing an unused package would contradict the dependency rule.
  Everything else stays custom. Do not hand-roll focus traps.
- **date-fns (rule correction).** Used by `<DateDisplay>` with the `hu` locale.
- **Approved additions:** `lucide-react` (icons), `clsx` + `tailwind-merge` (`cn()`),
  `@hookform/resolvers` (zod ↔ react-hook-form). Any other runtime dependency needs a reason
  recorded here.
- No state library beyond TanStack Query and React state (no Redux, Zustand, Jotai, MobX,
  Recoil). No GraphQL.

## 4. Hard rules — MUST NOT

| # | Rule |
|---|---|
| N1 | No screens outside §6. No inventory, invoicing, costs, margin, purchase orders, timesheets, customer portal, chat. |
| N2 | No hardcoded stage names — string literals, enums, CSS class names, icon maps, switch statements, sort orders. Stages come from the API; code references `key`, UI shows `label_hu`. Behaviour attached to a stage is a flag on its definition, not a key comparison. |
| N3 | No hardcoded user-facing strings. Everything through i18n. |
| N4 | No money arithmetic in the frontend: no `toFixed`, no float math, no summing amounts, no currency conversion. Amounts arrive as minor units + currency and are displayed; user-entered amounts are converted to minor units with integer/string arithmetic only. |
| N5 | No Next.js Server Actions for mutations. |
| N6 | No default component-library styling. Radix primitives (§3) are unstyled and take project tokens. |
| N7 | No colour outside the token set (§7). No raw hex or arbitrary Tailwind colours in components. |
| N8 | No decorative colour. The three semantic colours encode state only (§7). |
| N9 | No unvirtualised image rendering. Never more than 50 images mounted at once. |
| N10 | **(Corrected.)** Hover/focus colour transitions, loading spinners and skeletons are permitted. All motion is suppressed under `prefers-reduced-motion`. No decorative animation. |
| N11 | No state management library beyond TanStack Query + React state. |
| N12 | No hand-written API types (§2). |

## 5. Hard rules — MUST

| # | Rule |
|---|---|
| M1 | API types regenerated in CI; build fails on drift. |
| M2 | TypeScript `strict` and `noUncheckedIndexedAccess`; no `any`; no non-null assertions on API data. |
| M3 | Every amount through `<Money>`, every date through `<DateDisplay>`. |
| M4 | Hungarian strings written first; layouts measured against Hungarian. |
| M5 | Four states in every data view: loading, empty, error, loaded. Empty distinguishes no records / no match / no permission. Unbuilt features say so; they never pose as empty data. |
| M6 | Table columns sortable, lists filterable. |
| M7 | Keyboard navigation in the gallery lightbox and all modals — arrows, Escape, focus trapping, focus restoration. |
| M8 | Optimistic updates with rollback for stage advancement and blocker resolution. |
| M9 | Intake images immutable in the UI — no delete, replace or crop; visibly marked; capture timestamp shown. |
| M10 | No business logic in components. Transition rules, overdue status and similar decisions come from the API. |

## 6. Screens (exhaustive)

1. **Leads** — table; filters: source, assignee, age, stage; quick-add; convert to order.
2. **Orders** — filterable table, saved views, density toggle.
3. **Order detail** — tabs: Adatok, Tervek, Képek, Blokkolók, Levelezés. Traveller rail
   persistent (§12).
4. **Partners** — table; detail with contacts, order history, correspondence.
5. **Gallery** — virtualised, category-grouped, intake pinned and immutable, lightbox, bulk zip.
6. **Email compose** — template picker, variable preview, attach documents from the record.
7. **Correspondence** — per-record email history, read-only, delivery status.
8. **Reports** — order value by month, volume by partner/type, stage duration, stalled
   orders, blocker load; CSV export on each.
9. **Settings** — users/roles, stage definitions (rename, reorder, image gate), email
   templates, suppression list, kill switch.

Authentication screens (login, forced password change) are prerequisites, not additions.

## 7. Colour

Tokens only:

```
--panel      #F7F8F7      --steel-900  #1B2327
--surface    #FFFFFF      --steel-500  #6B767C
                          --steel-200  #D5DBDC
--signal     #E8590C      blocked / overdue — nothing else
--cold       #0F5C7A      MEO / certification — nothing else
--done       #2F7A3E      completed — nothing else
```

- Chrome is steel: primary buttons, focus rings, text selection, active tab, links use
  `steel-900`/`steel-500` with weight, underline or tint for emphasis. No accent colour on
  controls.
- Stage badges are neutral steel. Terminal (completed) → `done`. Exit stages (cancelled, lost)
  → muted steel, never `signal`. The current stage is marked by weight and a filled marker, not
  colour.
- Colour is never the only carrier of information.

## 8. Typography

IBM Plex Sans for UI; IBM Plex Mono for money, dates, identifiers, plates and technical
numbers, with tabular figures. Scale, and nothing else:

| Token | Size | Use |
|---|---|---|
| `text-metadata` | 12.8px | labels, metadata |
| `text-body` | 16px | body, table cells, controls |
| `text-section` | 20px | section headings |
| `text-record-title` | 25px | record titles |
| `text-page-title` | 31px | page titles, key metrics |

No arbitrary `[..px]` sizes.

## 9. Layout

- Left navigation rail; list → detail; content left-aligned, up to 1600px wide; no centred
  content column; empty and error states left-aligned.
- Sections separated by rules and spacing, not by identical rounded, shadowed cards. Borders are
  reserved for genuinely bounded content such as the traveller and dialogs.
- Money and date columns: mono, `tabular-nums`, right-aligned.
- Desktop first; below 1024px the traveller renders as a horizontal strip above the tabs.

## 10. Motion

Permitted: hover/focus colour transitions, loading spinners and skeletons, panel/dialog
open-close, upload progress, stage-transition feedback. Everything is disabled under
`prefers-reduced-motion`.

## 11. Components

`<AppShell>`, `<Sidebar>`, `<PageHeader>`, `<DataTable>`, `<FilterBar>`, `<Money>`,
`<DateDisplay>`, `<StageRail>`, `<StatusBadge>`, `<EmptyState>`, `<ErrorState>`,
`<LoadingState>`, `<ConfirmDialog>`, `<ImageGrid>`, `<ImageLightbox>`, `<Uploader>`.

- `<Money>` takes minor units + currency and formats with integer arithmetic; it never divides.
- `<DateDisplay>` uses date-fns with the `hu` locale: mono, tabular figures, Hungarian format,
  relative for recent dates with the absolute date on hover.
- Components format and display; they do not decide business outcomes (M10).

## 12. Stages and the traveller

- Stage definitions (`key`, `label_hu`, `position`, gates, flags) are fetched from the API and
  editable in Settings. Renaming a label never breaks code.
- The traveller (`<StageRail>`) is the signature component. It answers: where is the vehicle,
  how long has it been there, what is blocking it, can it move, what happened before.
- It is driven by stage history, not position arithmetic. Inactive definitions are hidden. An
  exit stage terminates the rail; stages after it are never-reached, not done.
- Days in stage, blockers (text and responsible party) and gate requirements show inline.
- Allowed transitions and their requirements (note required, gates) come from the API.

## 13. Money

- Display: `<Money>` only (§11). HUF minor units are fillér and are displayed faithfully, never
  silently rounded.
- Entry: users type major units in their own notation (`4 850 000`, `4850000`, `12 400,50`);
  conversion to minor units is exact string/integer arithmetic. Negative values are allowed
  (discount lines). Inputs are labelled with the order's currency.
- The backend computes line totals, order totals and HUF normalisation.

## 14. Errors and states

- Backend error codes map to Hungarian explanations in the catalogue. For `validation` the
  backend's message is shown, because it carries the field-level reason.
- `401` returns to login; `403` renders a permission state; configuration queries (stage
  definitions, project types, users) have loading and error states like any other data.
- Mutations invalidate the affected TanStack Query keys; stage advancement is optimistic with
  rollback.

## 15. Open questions

Real stage names and project types; GDPR/retention stance for immutable intake images and
the email log; historical invoice handling. None of these may be baked into components.
