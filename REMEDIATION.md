# Remediation

Work-through of `AUDIT.md` findings, in R0–R9 order, then product work that landed in the same tree. One phase per commit; every phase left `tsc`, `lint` and `build` green.

## Closed findings

- **BLOCKER-01** (R1). Session reads the generated `SessionUser`/`MeResponse` (`data?.user`, `must_change_password`). Verified live: 5 real payloads (`/auth/me`, partners, leads, lead-detail, order-detail) validate against the generated zod schemas. Correction to the audit: the backend sends `id`, not `user_id`; the generated contract agrees, so there was no `user_id` bug to fix. The real drift was the envelope shape and the `password_change_required` naming.
- **BLOCKER-02** (R1). utoipa annotations on all handlers; `openapi/openapi.json` emitted by `autocrm openapi` with uniqueness + freshness tests; `types/api.ts` deleted and replaced by `openapi-typescript` output plus hey-api zod validators (`npm run gen:api`, proven byte-idempotent); every response validated at the fetch boundary (`ContractError` on mismatch).
- **M1** (R1). CI frontend job: regen, `git diff --exit-code` on generated files, typecheck, lint, build.
- **M2** (R1.6). `noUncheckedIndexedAccess` on, `allowJs` removed, `any` 0, non-null assertions on API data 0 (eliminated via `skipToken`, captured ids, fallback constants).
- **MAJOR-01** (R0). Placeholders replaced with honest "not yet available" states; no phase numbers or fake empty states ship to users.
- **MAJOR-02** (R5.5). `orderPatchBody` sends `currency` when changed and the order is item-less.
- **MAJOR-03** (R5.6). Convert dialog currency is a select prefilled from the partner default.
- **MAJOR-04** (R5.2). Traveller reached-state derives from stage history, not position arithmetic; inactive definitions filtered.
- **MAJOR-05** (R5.3, R5.4). `GET /orders+leads/{id}/transitions` returns `manual`/`requires_note`/`gates_met` computed by the same `check_transition` the moves go through (unit-tested, live-verified including 422-without-note/200-with-note). Dialogs consume it; the `'won'` literal and both `needsNote` copies are gone. Overdue is a backend flag in business time.
- **MAJOR-06** (R2.2). `formatMoney` rebuilt on integer arithmetic (Intl grouping only). HUF fillér render when nonzero (backend stores exponent 2; DB check confirmed zero current rows, conversions can produce them).
- **MAJOR-07** (R2.1). Price entry takes major units in user notation, converts exactly (string split, pad/truncate, BigInt combine, range-checked); negatives allowed; field labelled with the order's symbol; 26 runtime checks pass.
- **MAJOR-08** (R4.1). All hardcoded Hungarian moved to `hu.json`, including error codes, table headers, rail/pagination/filter/error states, dialog strings, enum labels, and the `Traveller` literal. Follow-up sweep caught stragglers (contact labels, `createdAt` headers, `Mentés…` states).
- **MAJOR-09** (R3). Signal is blocked/overdue only; cold unused pending MEO; done is completed. Buttons, focus, selection, tabs and links are steel; stage badges neutral steel / done / muted-exit.
- **MAJOR-10** (R7.1). Server-side sort params (strict whitelist, 400 on unknown, static CASE branches keep compile-time checking) plus sortable headers with `aria-sort` (asc/desc/cleared). Sort integration test; all variants live-verified.
- **MAJOR-11** (R6.1). Four dialogs on Radix Dialog (trap, Escape, restoration, labelled Title/Description); order tabs on Radix Tabs (arrow keys, tablist semantics).
- **MAJOR-12** (R7.7). Optimistic stage advancement with snapshot/rollback for orders and leads. Blocker resolution has no UI; skipped per the brief.
- **MAJOR-13** (R1.6). See M2.
- **MAJOR-14** (R5.1). `<DateDisplay>` (date-fns `hu`, relative-under-7-days, absolute on hover) on all 14 call sites; dead formatters deleted.
- **MAJOR-15** (R7.4). Config/entity queries surface loading/error states (filter captions, traveller error slot); stage dialogs already had them.
- **MAJOR-16** (R4.1, R7.6). Error texts come from the `errors` catalogue; `validation` shows the backend's field-level message.
- **MINOR-02** (R5.3). Gone with the transitions endpoint.
- **MINOR-03** (R3.5). Forms plugin on class strategy (no `#2563eb`/`#6b7280`), `text-white` replaced with the surface token. The `rgb(0 0 0 / …)` shadow alphas in `tailwind.config.ts` remain; they were not in R3.5's fix list and the plan bans only excessive shadows.
- **MINOR-04** (R6.3, corrected rule). Spinners, skeletons and hover/focus transitions kept; one global `prefers-reduced-motion` rule suppresses them.
- **MINOR-05** (R8.3). Content mapped to 12.8/16/20/25/31; record titles use the record variant via a `PageHeader` size prop. The 8 arbitrary values are functional geometry (dialog centering, rail markers, 280px traveller) and were kept.
- **MINOR-06** (R8.1, R8.2). Detail sections use ruled rhythm; cards kept for forms, dialogs, traveller, filter bar and tables. Content left-aligned to 1600px; empty/error/items-empty states un-centred.
- **MINOR-07** (R7.2). Money/date columns right-aligned mono tabular via column `align` meta.
- **MINOR-08** (R7.3). No-records vs filtered-no-match vs permission-denied are distinct; empty Pagination renders "0 találat".
- **MINOR-10** (R4.3, R4.4, R4.5). `en` locale removed (hu-only routing); ICU `{min}`/`{max}` fixed; duplicate keys removed (plus a dead duplicate `leads.title` found later).
- **MINOR-11** (R8.4). `TravellerStrip` below `lg` with inline blocker text and responsible parties; heading catalogued.
- **MINOR-12** (R5.7). Project type and assignee resolve to names (admin directory; raw id fallback documented).
- **MINOR-13** (R9). `axios`, `js-cookie`, `@types/js-cookie`, `@tailwindcss/typography` removed (zero imports). `date-fns` now used; `recharts` and `@tanstack/react-virtual` kept for their named next phases; `lucide-react`, `clsx`, `tailwind-merge`, `@hookform/resolvers` recorded as approved in the plan.
- **MINOR-14** (R2.4). Dead helpers deleted; `roleLabel` (zero call sites) removed in cleanup.
- **MINOR-15** (R6.1, R6.2). Radix tabs; partner picker rebuilt as an ARIA combobox (label, listbox, arrows/Enter/Escape).
- **MINOR-16** (R4.1). See MAJOR-16.
- **NOTE-02** (R9). Dead `generateStaticParams` removed; build marks all locale routes dynamic as the comment claims.
- **NOTE-03**, first half (R9). API rewrite target and image hosts come from `AUTOCRM_API_URL`/`AUTOCRM_S3_URL` with documented defaults.

## Deferred

- **MINOR-01** (missing screens/tabs). By design: the brief forbids building missing features in this session. Gallery, email compose, correspondence, reports and admin user/stage/template screens remain stubs or absent; order tabs remain Adatok/Tételek/Fázisok/Blokkolók/Napló. Dashboard, blockers route (unlinked, still reachable from order detail), admin and reports stubs remain.
- **MINOR-09**, second half. Display now separates "all" from emptiness, but filtering to assignees-with-nobody has no API support; that needs a new query param, which is new backend scope.
- **MINOR-03 remainder.** Shadow alphas, as above.
- **NOTE-01.** Unchanged: route protection is a client redirect; the backend enforces. Acknowledged, not a violation.
- **NOTE-03**, second half. Upload flow still unimplemented; belongs to the gallery phase.
- **Suppression test flake.** `suppressed_recipients_never_get_automatic_mail` failed once in a full parallel run, then passed solo, in-file, and in a repeat full run (3 consecutive greens). Unreproduced; recorded, not "fixed".

## Added to FOLLOWUP.md

- Residual `~28 as` casts on API-adjacent data (assertions are at zero; the casts were not introduced to silence R1.6).
- Next.js 14.2.35 → 16.x migration: latest 14.x is installed; the major jump needs `middleware.ts` → `proxy.ts`, async route params (~10 files), an ESLint 9 replacement for the removed `next lint`, and React 19 re-verification with no frontend test net. Dedicated task on a green tree.
- Converted leads still receive reopen targets from the transitions endpoint; unreachable in practice (the UI never opens the dialog for converted leads).
- The suppression flake above.
- Pre-existing entries (canonical plan doc, SMTP baseline test) were already there.

## Non-finding work in the same tree

Product direction taken during the session, verified the same way (trio + live API checks): Leadek naming; consumers/business partner menus (consumers menu later removed per direction; `/hu/partners` redirects to business); real email inbox (list, detail, cancel/retry) with dry-run demo mail; SMTP transport inside the global settings row with worker hot-reload and a test-send endpoint; per-user preferences (density, page size) honored by all lists. Dev-database demo rows (smoke partner/order/lead, demo emails) exist only in the local database, not the repo. SMTP password is stored in the clear in PostgreSQL; PostgreSQL access is the trust boundary (migration comment says so).

## Verification output (final)

- `npx tsc --noEmit`: exit 0, 0 errors.
- `npx eslint src`: exit 0, 0 warnings/errors.
- `npm run build`: compiled successfully; all `[locale]` routes dynamic, 3 static framework pages, middleware 39.8 kB.
- Backend: `cargo fmt --check` clean; `cargo clippy --all-targets` clean (two justified `allow`s: filter-arity queries, transport builder); `cargo test` 98 lib tests pass, all integration suites pass except the pre-existing `smtp.rs::unreachable_server_fails_the_connection_test_with_a_hint` environmental failure already in FOLLOWUP; OpenAPI uniqueness + freshness tests pass.
- Live: sort variants and directions plus the 400 path; transitions truthfulness (gate-blocked `completed`, note enforcement 422/200); overdue true/false lifecycle; settings PUT roundtrip with redaction; preferences roundtrip; test-send dry-run; cancel 204; contract re-validation of 5 endpoints after regen.
