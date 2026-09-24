# Round 4: bug fixes + optimisation pass

Follow-up to [03-second-sweep.md](03-second-sweep.md). Two threads:

1. **Bug fixes** — the two items 03 reported without a code change (MAIL-L6, REP-L2),
   now fixed, plus five new findings from a fresh hunt over leads, documents, stages,
   the partner picker and Android dates.
2. **Optimisations, top to bottom** — frontend → backend/DB → Android, measured first.
   Verdict up front: no layer justified a change (details at the bottom).

Branch `logic-audit/fixes`. Spec: [01-behavior-spec.md](01-behavior-spec.md).

Confidence legend (same as before): **proven** = a test failed on the old code and passes
after the fix; **proven†** = the test targets a helper the fix introduced, so before the
fix it failed to *compile*; **likely** = traced with no failing test.

## Summary

7 fixes (5 new findings + the 2 reported items). By confidence: 1 proven, 6 proven†.
No suspected items remain anywhere in the audit.

| ID | Title | Severity | Confidence | Status |
|---|---|---|---|---|
| [MAIL-L6](#mail-l6) | An office hand-add silently resubscribes an opted-out address (03, reported) | medium | proven† | fixed |
| [REP-L2](#rep-l2) | The workload window allows 63 calendar days while the text says 62 (03, reported) | low | proven† | fixed |
| [SALES-L6](#sales-l6) | Lead update skips the partner/contact checks create enforces; both accept archived partners | medium | proven† | fixed |
| [MEDIA-L4](#media-l4) | `PATCH /documents/{id}` wipes omitted fields and checks dates against the body only | high | proven† | fixed |
| [ORD-L7](#ord-l7) | Cancelling from intake needs a mileage reading; `/transitions` offers gated moves as allowed | medium | proven† | fixed |
| [TIME-L2](#time-l2) | Android "quote expired" badge uses the phone's date, not Budapest | low | proven† | fixed |
| [SALES-L7](#sales-l7) | Customer pickers offer suppliers | medium | proven | fixed |

## Verification

- Backend: `SQLX_OFFLINE=true cargo test --lib`: **149 passed, 0 failed**. `cargo test
  --test openapi`, `--test error_codes`: pass (openapi.json regenerated for the
  `DocumentPatch` shape; no new error codes).
- Backend integration tests: not run (no Postgres). `cargo test --no-run --test leads`
  compiles the new `tests/leads.rs` offline; runs in CI.
- Web: `npx vitest run`: **12 files, 87 tests passed**. `npx tsc --noEmit`: clean.
  `eslint` on changed files: clean.
- Android: `./gradlew :app:testDebugUnitTest`: **62 tests passed, 0 failed**.
- `cargo clippy --lib`: 3 warnings, all pre-existing, none on changed lines.

## Fixes

<a id="mail-l6"></a>

### MAIL-L6: An office hand-add silently resubscribes an opted-out address
- **Expected:** `backend/migrations/0024_newsletter.sql:232-233`: unsubscribing sets
  `unsubscribed_at` rather than deleting, so the next import does not resubscribe
  someone who opted out.
- **Actual:** `POST /newsletter/subscriptions` called the same `subscribe` as the
  website, whose upsert clears `unsubscribed_at` (`backend/src/repo/newsletter.rs:56-73`).
- **Reproduction:** website-subscribe `x@example.hu` → unsubscribe → office
  `POST /newsletter/subscriptions {"email":"x@example.hu"}` → row active again.
- **Impact:** marketing mail to opted-out addresses; defeats the tombstone.
- **Severity:** medium
- **Confidence:** proven†
- **Proof:** `api::newsletter::tests::the_office_cannot_resubscribe_an_opted_out_address`
  in `backend/src/api/newsletter.rs` (fails to compile before the fix, passes after).
- **Fix:** the office endpoint refuses a previously-unsubscribed address with 409
  `duplicate` ("only they can resubscribe via the website form"); website resubscribe
  is untouched. Uses the existing `duplicate` code — no catalog change. The silent path
  is gone; the explicit consent path (the subscriber coming back themselves) stays.

<a id="rep-l2"></a>

### REP-L2: The workload window allows 63 calendar days while the text says 62
- **Expected:** "Ranges cap at 62 days" (`backend/src/api/reports.rs:300`) — the cap
  exists to bound the bar chart.
- **Actual:** `(to - from).num_days() > 62` counts the exclusive span, so Jan 1 → Mar 4
  (63 chart bars) passed.
- **Reproduction:** `GET /reports/workload?from=2026-01-01&to=2026-03-04` → 200 with 63
  day buckets.
- **Impact:** cosmetic; no wrong data. No client requests more than 31 days (web ≤ 31,
  Android 30), so tightening breaks nothing.
- **Severity:** low
- **Confidence:** proven†
- **Proof:** `api::reports::tests::the_workload_cap_counts_calendar_days` (fails to
  compile before the fix, passes after).
- **Fix:** extracted `workload_span_days` (inclusive, `+1`) and capped that at 62.

<a id="sales-l6"></a>

### SALES-L6: Lead update skips the partner/contact checks create enforces; both accept archived partners
- **Expected:** SALES-33 (contact belongs to the lead's partner) and the order-side rule
  SALES-14 (archived partners take no new work, `backend/src/service/orders.rs:52-58`).
- **Actual:** `PATCH /leads/{id}` wrote any merged pair unchecked; create checked
  existence but not archived status.
- **Reproduction:** lead with partner A + A's contact → `PATCH {"partner_id": B}` → 200
  with a cross-company pair; or create/update a lead on an archived partner → 200 while
  `POST /orders` for the same partner refuses.
- **Impact:** quotations/conversions from the wrong company's contact; leads accrue on
  archived partners that later refuse conversion.
- **Severity:** medium
- **Confidence:** proven†
- **Proof:** `lead_relations_are_checked_the_same_on_every_write` in the new
  `backend/tests/leads.rs` (fails to compile before — no `check_relations` — compiles
  offline, runs in CI).
- **Fix:** shared `service::leads::check_relations`, called from `create` and from the
  update handler against the merged pair.

<a id="media-l4"></a>

### MEDIA-L4: `PATCH /documents/{id}` wipes omitted fields and checks dates against the body only
- **Expected:** the API-wide PATCH contract (docs/API.md:27) and SALES-58 (validity order).
- **Actual:** full-replace write; `PATCH {"issuer":"DEKRA"}` nulled validity range and
  vehicle; `PATCH {"valid_until":"2024-06-01"}` stored an inverted range against a stored
  `valid_from` of 2025.
- **Reproduction:** as above on any certificate document.
- **Impact:** silent evidence-metadata loss; inverted ranges stored.
- **Severity:** high
- **Confidence:** proven†
- **Proof:** two `api::media::tests` cases on the new `merge_validity` (fail to compile
  before, pass after).
- **Fix:** PATCH-aware `DocumentPatch`, merge over the loaded row (still 404 when missing
  or soft-deleted), merged-date validation.

<a id="ord-l7"></a>

### ORD-L7: Cancelling from intake needs a mileage reading; `/transitions` offers gated moves as allowed
- **Expected:** exit stages skip gates (`0002_partners_leads.sql:60`); `/transitions` lets
  the UI present targets "without reimplementing the transition rules".
- **Actual:** the slip gate fired for every non-staying target, including `cancelled`;
  `gates_met` ignored the slip gate entirely.
- **Reproduction:** intake order without slip → stage to Törölve → 422; `/transitions`
  said `gates_met: true` for the same move.
- **Impact:** no-show/duplicate orders un-cancellable without recording (or inventing) a
  mileage; both clients offer moves the server refuses.
- **Severity:** medium
- **Confidence:** proven†
- **Proof:** two `service::stages::tests` cases on the shared rule (fail to compile
  before, pass after).
- **Fix:** one shared `intake_gate_blocks` used by the move and by `order_transitions`
  (which now reads the order's mileage — reusing its existing fetch, no extra query).
  Leads unaffected.

<a id="time-l2"></a>

### TIME-L2: Android "quote expired" badge uses the phone's date, not Budapest
- **Expected:** SALES-49 (Budapest calendar day, deliberate).
- **Actual:** `LocalDate.now()` in the device zone.
- **Reproduction:** phone on UTC, quote expiring today, 00:00–01:00 Budapest time: web
  active, phone "Lejárt".
- **Severity:** low
- **Confidence:** proven†
- **Proof:** `quoteExpiryIsDecidedOnTheBudapestCalendarDay` in `SalesLeadConvertTest.kt`
  (fails to compile before — no `today` parameter; passes after).
- **Fix:** default `today` to Budapest, same shape as TIME-L1; parameter keeps it testable.

<a id="sales-l7"></a>

### SALES-L7: Customer pickers offer suppliers
- **Expected:** SALES-17/`0011_relations_quotes_suppliers.sql:27-29`: the role flag exists
  so suppliers stop turning up in the customer picker; `?role=customer` is served.
- **Actual:** `PartnerPicker` never sent `role`, so every lead/order customer search
  listed suppliers.
- **Reproduction:** mark a partner supplier → Leadek → Új lead → type its name → offered
  as the customer.
- **Impact:** jobs filed against suppliers as customers; downstream conversion/invoicing
  confusion.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-partner-picker.test.tsx`: customer picker sends
  `role: 'customer'`; unfiltered picker sends none (both fail before — no `role` prop —
  pass after).
- **Fix:** optional `role` prop on `PartnerPicker`, `role="customer"` on the lead and
  order forms. The orders-page filter stays unfiltered (filtering by a supplier's own
  orders is legitimate), as does anything else that may name a supplier.

## Optimisation pass (top to bottom)

Measured first; the honest result is that no layer justified a change. Scale is an
explicit non-goal (60 leads/month, <20 users), and the code already reflects it.

- **Frontend.** Polling inventory: pickup board 4 queries / 30 s, dashboard mount-only
  queries with 30–60 s staleness, invoice 3 s poll only while `submitting`/freshly
  issued, global 30 s staleTime, no window-focus refetch. All proportionate; no change.
  Bundle: the one heavy widget (workload charts) is already `dynamic`-imported; no
  build-size action without evidence of a problem.
- **Backend/DB.** List endpoints are single queries (the open-blocker count is a
  subselect, not a loop); partner detail is 4 queries; the dashboard fan-out is a
  deliberate no-backend-dashboard design (C10). Workload is O(days × rows) with days ≤
  62. Index audit over all migrations: FK columns, `trgm` search indexes, partial
  indexes and the suppression PK are all present — including `lower(email)` lookups and
  the plate-normalisation index. No missing index found. The one incidental win:
  `order_transitions` reuses its existing order fetch for the mileage check (no added
  query) as part of ORD-L7.
- **Android.** Both workers are network-constrained one-shots with `KEEP`/`REPLACE`
  policies and backoff; lists load on open/pull, no timers. No HTTP cache exists, but
  the only cacheable endpoint (`GET /mobile/orders`, 60 s) serves the currently dead
  picker screen — caching it now would be optimisation for a flow under product review
  (Q-ORD-9). No change.
- **Considered and left:** the exact-multiple pagination dead-end (F3, needs backend
  totals — a known integration gap, not re-reported); merging the board's 4 queries
  into one multi-stage filter (backend contract change for a 30 s TV screen — not worth
  it at this scale).
