# Logic audit: second sweep

Follow-up to [02-logic-errors.md](02-logic-errors.md) (44 findings, all fixed). This sweep
re-mines the appendix "needs decision" items and open questions from the first two rounds
for entries that turned out to be concrete bugs, not product decisions — plus one fresh
timezone inconsistency on Android.

Branch `logic-audit/fixes`. Spec: [01-behavior-spec.md](01-behavior-spec.md). Open questions:
[00-questions.md](00-questions.md). Integration-audit findings
([../integration-audit/02-findings.md](../integration-audit/02-findings.md)) are not repeated.

Confidence legend (same as 02): **proven** = a test failed on the old code and passes after
the fix; **proven†** = the test targets a helper the fix introduced, so before the fix it
failed to *compile*; **likely** = traced through the code with no failing test.

## Summary

6 findings: 1 high, 3 medium, 2 low. 4 fixed, 2 reported without a code change (genuine
owner decisions). By confidence: 4 proven†, 2 likely.

| ID | Title | Severity | Confidence | Status |
|---|---|---|---|---|
| [SALES-L6](#sales-l6) | Lead update skips the partner/contact checks that create enforces, and both accept archived partners | medium | proven† | fixed |
| [MEDIA-L4](#media-l4) | `PATCH /documents/{id}` wipes every field the request omits, and checks dates against the body only | high | proven† | fixed |
| [ORD-L7](#ord-l7) | Cancelling from intake needs a mileage reading, and `/transitions` offers gated moves as allowed | medium | proven† | fixed |
| [TIME-L2](#time-l2) | Android "quote expired" badge uses the phone's date, not Budapest | low | proven† | fixed |
| [MAIL-L6](#mail-l6) | An office hand-add silently resubscribes an address that opted out | medium | likely | **reported** |
| [REP-L2](#rep-l2) | The workload window allows 63 calendar days while the text says 62 | low | likely | **reported** |

## Verification

- Backend: `SQLX_OFFLINE=true cargo test --lib`: **147 passed, 0 failed**. `cargo test --test
  openapi`, `--test error_codes`: pass (no contract change this round — no regen needed).
- Backend integration tests (`backend/tests/*`) were **not run** (no Postgres here).
  `cargo test --no-run --test leads` compiles the new `tests/leads.rs` (SALES-L6) offline;
  it runs in CI.
- Web: untouched this round.
- Android: `./gradlew :app:testDebugUnitTest`: **62 tests passed, 0 failed** (incl. the new
  quote-expiry case).
- `cargo clippy --lib`: 3 warnings, all pre-existing, none on changed lines.

## Findings

<a id="sales-l6"></a>

### SALES-L6: Lead update skips the partner/contact checks that create enforces, and both accept archived partners
- **Expected:** SALES-33: a lead's contact must belong to the lead's partner, and a contact
  without a partner is refused (`backend/src/service/leads.rs:17-31`, 400 "contact belongs
  to a different partner"). SALES-14/SALES-19: an archived partner takes no new selections;
  orders refuse one (`backend/src/service/orders.rs:52-58`, 400 "partner is archived").
- **Actual:** `PATCH /leads/{id}` merged and wrote any `partner_id`/`contact_id` with no
  check (`backend/src/api/leads.rs:230-244` before the fix), so a lead could point at
  partner B with partner A's contact — or at a contact with no partner at all. Both create
  and update also accepted an archived partner (`service/leads.rs:17-21` checked existence
  only). The web path that produced such pairs was closed in SALES-L2, but the API (and
  any other client) still accepted them.
- **Reproduction:** `POST /api/leads` with partner A + A's contact → `PATCH /api/leads/{id}`
  `{"partner_id": B}` → 200, lead now pairs B with A's contact. Or create/update a lead
  with an archived partner → 200, while `POST /orders` for the same partner says "partner
  is archived".
- **Impact:** quotation letters and conversions work from the wrong company's contact;
  leads accrue on archived partners that orders then refuse to convert for.
- **Severity:** medium
- **Confidence:** proven†
- **Proof:** `lead_relations_are_checked_the_same_on_every_write` in
  `backend/tests/leads.rs` (new file; fails to compile before the fix — no
  `check_relations` — compiles offline, runs in CI).
- **Fix:** `backend/src/service/leads.rs`: new `check_relations` (partner exists + not
  archived, contact exists + belongs), called from `create` and from the update handler
  against the merged pair (`backend/src/api/leads.rs`).

<a id="media-l4"></a>

### MEDIA-L4: `PATCH /documents/{id}` wipes every field the request omits, and checks dates against the body only
- **Expected:** PATCH bodies keep an omitted field and clear it on `null` (docs/API.md:27,
  the API-wide convention; SALES-L3 fixed the same bug on vehicles). SALES-58: `valid_from`
  must not be after `valid_until`.
- **Actual:** `update_document` ran `set_validity` with exactly what the body carried
  (`backend/src/api/media.rs:346-360` before the fix). `PATCH {"issuer":"DEKRA"}` nulled
  `valid_from`, `valid_until` and `vehicle_id` — an ATP certificate lost its validity
  range and its vehicle link in one edit. Worse, the date check compared only dates both
  present in the body, so `PATCH {"valid_until":"2024-06-01"}` on a document valid from
  2025-01-01 was stored, violating the rule the DB check exists for.
- **Reproduction:** `PATCH /api/documents/{id}` with `{"issuer":"DEKRA"}` on a certificate
  with a validity range → the response shows `valid_from`, `valid_until`, `vehicle_id`
  all null.
- **Impact:** silent data loss on evidence metadata; validity ranges the expiry queries
  (SALES-58) depend on disappear, and inverted ranges can be stored.
- **Severity:** high
- **Confidence:** proven†
- **Proof:** `api::media::tests::an_omitted_field_keeps_its_value_while_null_clears_it` and
  `narrowing_one_end_past_the_stored_other_end_is_refused` in
  `backend/src/api/media.rs` (fail to compile before the fix, pass after).
- **Fix:** `DocumentPatch` is PATCH-aware (`Option<Option<_>>`, the shared `patch`
  deserializer); the handler loads the row (still 404 when missing or soft-deleted),
  merges via `merge_validity`, and checks the merged dates.

<a id="ord-l7"></a>

### ORD-L7: Cancelling from intake needs a mileage reading, and `/transitions` offers gated moves as allowed
- **Expected:** ORD-44: leaving `intake` requires `mileage_in`. But exit stages
  (`lost`, `cancelled`) are "reachable from any open stage" and "skip gates"
  (`backend/migrations/0002_partners_leads.sql:60`, `docs/DECISIONS.md:57`). And
  `/transitions` exists so "the UI [presents] targets without reimplementing the
  transition rules" (`backend/src/service/stages.rs:26-29`).
- **Actual:** the intake gate fired for every target except staying
  (`service/stages.rs:135` `to != intake` before the fix), so cancelling an intake order
  needed an odometer reading first — for a van that may never have arrived. Meanwhile
  `gates_met` reflected image gates only, so every target from `intake` showed as allowed
  and then failed with 422 `intake_slip_missing`.
- **Reproduction:** open an order in Átvétel with no slip → Fázisváltás → Törölve: 422
  `intake_slip_missing`. `/orders/{id}/transitions` reports `gates_met: true` for the
  same targets.
- **Impact:** staff cannot cancel a duplicate or no-show order without recording (or
  inventing — polluting evidence) a mileage. Both clients offer moves the server refuses.
- **Severity:** medium
- **Confidence:** proven†
- **Proof:** `service::stages::tests::the_intake_slip_blocks_work_but_never_cancellation`
  and `transitions_hide_gated_targets_but_offer_cancellation` (fail to compile before the
  fix, pass after).
- **Fix:** one shared rule, `intake_gate_blocks` (`service/stages.rs`): in `intake`,
  moving to a non-exit stage without recorded mileage is blocked; staying and exiting
  never are. The move and `/transitions` (`order_transitions` now reads the order's
  mileage) both use it. Leads are unaffected (their keys never match).

<a id="time-l2"></a>

### TIME-L2: Android "quote expired" badge uses the phone's date, not Budapest
- **Expected:** SALES-49: a quote shows "Lejárt" when `quote_valid_until` is before
  today's date in Europe/Budapest — deliberate, so the badge does not flip at UTC
  midnight. TIME-L1 fixed the same device-zone bug for the reports window.
- **Actual:** the phone compared against `LocalDate.now()` in the device zone
  (`android/.../ui/leads/LeadScreens.kt:492-493` before the fix).
- **Reproduction:** set the phone to UTC and open a lead whose quote expires today,
  between 00:00 and 01:00 Budapest time: the web says active, the phone says "Lejárt".
- **Impact:** the two clients disagree about the same quote near midnight; a still-valid
  quote reads expired on the phone.
- **Severity:** low
- **Confidence:** proven†
- **Proof:** `quoteExpiryIsDecidedOnTheBudapestCalendarDay` in
  `SalesLeadConvertTest.kt` (fails to compile before the fix — no `today` parameter;
  passes after).
- **Fix:** `isExpired` defaults `today` to `LocalDate.now(ZoneId.of("Europe/Budapest"))`
  (same one-line shape as the TIME-L1 fix); the parameter keeps it testable.

<a id="mail-l6"></a>

### MAIL-L6: An office hand-add silently resubscribes an address that opted out
- **Expected:** `backend/migrations/0024_newsletter.sql:232-233`: unsubscribing sets
  `unsubscribed_at` rather than deleting, so "the next import [does not] resubscribe
  someone who opted out". `docs/DECISIONS.md`-era rationale: the address must stay known.
- **Actual:** `POST /newsletter/subscriptions` (office hand-add,
  `backend/src/api/newsletter.rs:65`) calls the same `subscribe` as the website, whose
  upsert clears `unsubscribed_at` (`backend/src/repo/newsletter.rs:56-73`,
  "a returning address is welcomed back"). Staff re-adding an opted-out address —
  e.g. re-importing a customer list — silently opts them back in, defeating exactly what
  the tombstone is for.
- **Reproduction:** subscribe `x@example.hu` via website → unsubscribe (link) →
  `POST /newsletter/subscriptions {"email":"x@example.hu"}` as office → the row is
  active again, no consent event anywhere.
- **Impact:** marketing mail to people who opted out — a GDPR consent problem (Grt. 6. §,
  GDPR Art. 7) and the precise failure the schema comment guards against.
- **Severity:** medium
- **Confidence:** likely (traced; no test — the fix direction needs an owner decision)
- **Proof:** none.
- **Fix:** not fixed. Two consistent options: (a) the office endpoint refuses
  re-adding an unsubscribed address with a message pointing at the website resubscribe
  ("coming back is saying yes again" stays website-only); (b) it re-adds but records an
  explicit consent note. Either way the silent path should go. Awaiting product/legal.

<a id="rep-l2"></a>

### REP-L2: The workload window allows 63 calendar days while the text says 62
- **Expected:** "Ranges cap at 62 days" (`backend/src/api/reports.rs:300`) and the 422
  message "workload range caps at 62 days" (`:317`).
- **Actual:** the check is `(to - from).num_days() > 62`, so `from` = Jan 1, `to` = Mar 4
  (62 days apart, 63 calendar days inclusive) passes. The inclusive range exceeds the
  stated cap by one.
- **Reproduction:** `GET /reports/workload?from=2026-01-01&to=2026-03-04` → 200 with 63
  day buckets, despite "caps at 62 days".
- **Impact:** cosmetic boundary disagreement only; no wrong data either way.
- **Severity:** low
- **Confidence:** likely
- **Proof:** none (boundary reading; one line either way).
- **Fix:** not fixed. If the rule is "at most 62 days apart", only the comment/message
  want rewording ("63 calendar days"); if it is "at most 62 calendar days", the check
  wants `>= 62`. Awaiting owner.
