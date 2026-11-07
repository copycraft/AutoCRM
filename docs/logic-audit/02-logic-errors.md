# Logic audit: logic errors found and fixed

Branch `logic-audit/fixes`. Spec: [01-behavior-spec.md](01-behavior-spec.md). Open questions: [00-questions.md](00-questions.md). Integration-audit findings ([../integration-audit/02-findings.md](../integration-audit/02-findings.md)) are not repeated here.

Findings are sorted by severity, then confidence. **Confidence** legend:

- **proven**: a test failed on the old code, and passes after the fix
- **proven†**: the test targets a helper that the fix introduced, so before the fix it failed to *compile* rather than failing an assertion. The bug was confirmed by reading the code.
- **likely**: traced through the code, but there is no failing test, usually because the proof needs a live Postgres, which was not available
- **suspected**: a plausible timing or race window that has not been reproduced

## Summary

44 findings: 1 critical, 7 high, 20 medium, 16 low. 44 fixed, 0 not fixed. By confidence: 32 proven, 6 proven†, 6 likely, 0 suspected.

| ID | Title | Severity | Confidence | Status |
|---|---|---|---|---|
| [MAIL-L1](#mail-l1) | Test-mail redirect does not strip BCC, so a newsletter blast reaches every real subscriber | critical | proven | fixed |
| [SALES-L3](#sales-l3) | `PATCH /vehicles/{id}` wipes every field the request omits | high | proven | fixed |
| [ORD-L1](#ord-l1) | Reopening a cancelled order straight to `completed` skipped the MEO photo gate | high | proven | fixed |
| [MEDIA-L1](#media-l1) | Production photos starve behind 20+ queued inspection photos | high | proven | fixed |
| [INSP-L3](#insp-l3) | The sync pushes and signs inspections that were never signed off on the phone | high | proven | fixed |
| [SALES-L4](#sales-l4) | Android converts every lead to a HUF order | high | proven† | fixed |
| [INV-L5](#inv-l5) | two concurrent "issue invoice" requests for one order can both pass the one-live-invoice check | high | likely | fixed |
| [INV-L8](#inv-l8) | a retry after a timeout re-files the CREATE, and NAV's duplicate-number answer marks a stored invoice as rejected | high | proven† | fixed |
| [AUTH-L1](#auth-l1) | Web does not return to login when the session dies mid-use | medium | proven | fixed |
| [AUTH-L3](#auth-l3) | Android keeps a dead session after any 401 and cannot get back to login | medium | proven | fixed |
| [SALES-L1](#sales-l1) | Partner update audit silently drops `role` changes | medium | proven | fixed |
| [SALES-L2](#sales-l2) | Web lead form sends a contact that does not belong to the chosen partner | medium | proven | fixed |
| [ORD-L2](#ord-l2) | The pickup board's "in the workshop" list dropped older in-work cars | medium | proven | fixed |
| [ORD-L4](#ord-l4) | Emptying a field on the Android order edit screen did not clear it | medium | proven | fixed |
| [INSP-L2](#insp-l2) | Web "new" / "dismissed" verdicts are linked to the suggested check-out damage | medium | proven | fixed |
| [INSP-L4](#insp-l4) | An unfinished damage (no type or severity) makes the draft unsyncable, or a check-in unsignable | medium | proven | fixed |
| [INSP-L6](#insp-l6) | Discarding a synced draft leaves the server draft behind, blocking new check-outs | medium | proven | fixed |
| [INV-L1](#inv-l1) | NAV messages recorded "for the screen" are never shown unless the invoice is rejected | medium | proven | fixed |
| [INV-L2](#inv-l2) | the invoice list stops polling before the PDF is attached, so the PDF link never appears without a reload | medium | proven | fixed |
| [MAIL-L2](#mail-l2) | A document attached and embedded, or picked twice, makes the letter fail as "deleted before sending" | medium | proven† | fixed |
| [ORD-L5](#ord-l5) | The pickup board's "ready" list selects by creation date, not by completion | medium | proven | fixed |
| [INSP-L7](#insp-l7) | "Újra" (retake) drops earlier kept photos of the same zone and orphans their queue rows | medium | likely | fixed |
| [INSP-L9](#insp-l9) | A check-in started offline can never load its comparison | medium | likely | fixed |
| [INV-L6](#inv-l6) | proforma number lock is released before the proforma exists, so concurrent proformas draw the same number and one PDF is filed as an orphan | medium | likely | fixed |
| [INV-L9](#inv-l9) | a PDF that failed to store "can be fetched again", but nothing can fetch it | medium | proven | fixed |
| [INV-L10](#inv-l10) | an invoice whose request cannot be built at submit time is retried to death and stays `submitting` forever | medium | proven† | fixed |
| [MAIL-L4](#mail-l4) | A dead-lettered `send_email` job leaves the email "queued" forever, invisible and not retryable | medium | proven | fixed |
| [INSP-L10](#insp-l10) | A retried check-out create after a lost response becomes a permanent `checkout_open` | medium | proven | fixed |
| [AUTH-L2](#auth-l2) | Web session cookie expires 7 days after login even for active users (30-day absolute lifetime never applies) | low | proven | fixed |
| [ORD-L3](#ord-l3) | The pickup board rendered two nested app shells | low | proven | fixed |
| [INSP-L1](#insp-l1) | Web offers verdict buttons on a signed (locked) check-in | low | proven | fixed |
| [MEDIA-L2](#media-l2) | A failed "complete" throws away the ticket and re-uploads the photo | low | proven | fixed |
| [INSP-L5](#insp-l5) | A sync that dies after the server sign fails the draft forever with "locked" | low | proven | fixed |
| [MEDIA-L3](#media-l3) | Every system-camera photo leaves an orphan copy in the queue directory | low | proven | fixed |
| [INV-L3](#inv-l3) | "Sztornó" is still offered while a storno of that invoice is being reported, and a second click fails with a generic "duplicate" error | low | proven | fixed |
| [INV-L4](#inv-l4) | proforma payment due date printed as a raw ISO string | low | proven | fixed |
| [MAIL-L3](#mail-l3) | Template validation lists the same unknown variable several times | low | proven | fixed |
| [IMP-L1](#imp-l1) | MiniCRM timestamps inside the spring-forward DST hour were dropped | low | proven | fixed |
| [TIME-L1](#time-l1) | Android Reports window used the phone's time zone for "today" | low | proven | fixed |
| [SALES-L5](#sales-l5) | Android offers "Fázisváltás" on a converted lead | low | proven† | fixed |
| [ORD-L6](#ord-l6) | The intake-slip gate is enforced but the phone cannot record the slip | low | proven† | fixed |
| [INSP-L8](#insp-l8) | An optional zone (roof) cannot be skipped in the walkaround | low | likely | fixed |
| [INV-L7](#inv-l7) | retries of a technical annulment are never recorded on the invoice | low | likely | fixed |
| [MAIL-L5](#mail-l5) | Web newsletter send discards the server's recipient count; the pre-send count includes suppressed addresses | low | proven | fixed |

## Verification

These are the final runs on the combined tree, after every agent finished:

- Backend: `SQLX_OFFLINE=true cargo test --lib`: **143 passed, 0 failed**. `cargo test --test openapi`: **2 passed** (the committed `openapi/openapi.json` was regenerated for the new `client_key` field and the `POST /invoices/{id}/pdf` route). `cargo test --test error_codes`: **3 passed** (new `not_issued`/`pdf_unavailable` codes catalogued). `cargo clippy --lib` shows 3 warnings, none of them on changed lines.
- Backend integration tests (`backend/tests/*`) were **not run**, because there was no Postgres and Docker Desktop would not start from the session. With `SQLX_OFFLINE=true`, `tests/newsletter.rs` and `tests/migration.rs` do not compile: their queries are missing from the `.sqlx` cache. Those files are untouched and the cache is unchanged, so this was already the case before the audit. The new `tests/invoicing.rs` cases (INV-L8 × 2, INV-L10, INV-L9 guards) and the two new `tests/email_and_jobs.rs` cases (MAIL-L4, MAIL-L2) compile offline (`cargo test --no-run`) but need a live Postgres (+ sidecar/MinIO where noted) in CI.
- Web: `npx vitest run`: **11 files, 85 tests passed**. `npx tsc --noEmit`: clean. `eslint` on the changed files: 0 errors (1 existing `<img>` warning).
- Android: `./gradlew :app:testDebugUnitTest` (with `JAVA_HOME=C:/Users/vasta/android-tools/jdk`): **61 tests passed, 0 failed**.
- Sidecar: `npm test` in `nav-sidecar/`: **20 passed, 0 failed**.

## Findings

<a id="mail-l1"></a>

### MAIL-L1: Test-mail redirect does not strip BCC, so a newsletter blast reaches every real subscriber
- **Expected:** MAIL-84: with a redirect active, every message reaches only the redirect address (`backend/src/integrations/email.rs:242` "Rewrites a message so it reaches only `redirect_to`"; `docs/email-google-workspace.md:106-110` "Every message … is delivered to that one inbox"). MAIL-83: "Test mail must never reach real customers" (`backend/src/config.rs:198-199`).
- **Actual:** `apply_redirect` rewrote `to` and cleared `cc` but left `bcc` untouched (`backend/src/integrations/email.rs:243-266` before the fix). `build_message` then added every BCC address (`:341-343`). A newsletter row carries its whole audience in `bcc` (`backend/src/service/email.rs:917-920`).
- **Reproduction:** Run in staging with `EMAIL_MODE=smtp` and `EMAIL_REDIRECT_TO=iroda@autotherm.hu` (the only legal SMTP setup outside production), or set a redirect in Settings. Add two subscribers, then send a newsletter from /emails/new → "Hírlevél". Result: the redirect inbox receives the "[TESZT – …]" copy, and both subscribers also receive it as BCC.
- **Impact:** A staging or parallel run, where the redirect is the documented safety net, mails the entire real newsletter list. Customers receive test content marked "TESZT", which is a reputational problem and, for unsubscribed-in-prod data copies, a GDPR problem.
- **Severity:** critical
- **Confidence:** proven
- **Proof:** `integrations::email::tests::redirect_reaches_only_the_redirect_address_even_with_bcc` in `backend/src/integrations/email.rs` (failed before the fix at the `bcc.is_empty()` assertion, passes after)
- **Fix:** `backend/src/integrations/email.rs`: `apply_redirect` now clears `bcc`, and the TESZT notice states the BCC count ("bcc: N") without listing the addresses.
- **Journey:** Email & communications

<a id="sales-l3"></a>

### SALES-L3: `PATCH /vehicles/{id}` wipes every field the request omits
- **Expected:** PATCH bodies keep an omitted field and clear it on `null` (docs/API.md:27, the API-wide convention). SALES-27/28: a stored vehicle is never overwritten with less information, and its owner (`partner_id`) is kept (backend/src/repo/vehicles.rs:120-122; backend/migrations/0010_vehicles.sql:23-25).
- **Actual:** `update` ran the create-time `fields()` and wrote all seven columns (backend/src/api/vehicles.rs:175-188 and repo/vehicles.rs:103-104 before the fix). A body such as `{"notes":"…"}` was refused with "a vehicle needs a plate or a VIN". A body such as `{"plate":"ABC-124"}`, meant to correct the plate (the use case the module doc names, api/vehicles.rs:4-6), nulled VIN, make, model, year, owner partner and notes.
- **Reproduction:** `PATCH /api/vehicles/3` with `{"plate":"ABC-124"}` on a vehicle that has a VIN, make, model and owner. The response shows `vin`, `make`, `model`, `year` and `partner_id` all null.
- **Impact:** Silent data loss on the vehicle entity: the VIN (the identity that survives a plate change), the owner link to the partner, and notes. "Has this van been here before" matching then degrades.
- **Severity:** high
- **Confidence:** proven. The test also asserts the old path (`fields()`) refuses a notes-only body.
- **Proof:** `api::vehicles::tests::vehicle_patch_keeps_omitted_fields`, `vehicle_patch_null_clears_but_never_both_identifiers` and `vehicle_patch_uppercases_a_new_plate` in backend/src/api/vehicles.rs. `merged()` did not exist before, so the "before" state is shown by the `fields()` assertion.
- **Fix:** backend/src/api/vehicles.rs: `VehicleBody` fields are PATCH-aware (`Option<Option<_>>` with the shared `patch` deserializer). `update` loads the stored vehicle and merges (`merged()`); create keeps its behavior (`fields()`). The OpenAPI schema is unchanged (the `committed_openapi_document_is_current` test passes).
- **Journey:** Sales pipeline

<a id="ord-l1"></a>

### ORD-L1: Reopening a cancelled order straight to `completed` skipped the MEO photo gate
- **Expected:** ORD-38 says that forward moves may skip stages, but every image gate up to the target must be met, "so skipping MEO cannot dodge the MEO photo requirement" (backend/src/domain/stage.rs:7-8; docs/DECISIONS.md:54-55). ORD-43 says leaving a terminal stage is a reopen that needs a note (stage.rs:11). ORD-50 says entering `completed` sends the customer the "ready for pickup" letter.
- **Actual:** `check_transition` returned `Reopen` for any target as soon as a note was present, and never looked at gates (backend/src/domain/stage.rs:122-128 before the fix). The following path therefore worked and reached `completed` with zero `completion` photos:
  1. any open stage → `cancelled` (exit, no gates)
  2. `cancelled` → `completed` (reopen with a note)
  The service then queued `order_ready_for_pickup` (backend/src/service/stages.rs:159-160).
- **Reproduction:**
  1. Web or phone: take an order in `design`, record the intake slip, and move it to Törölve (cancelled).
  2. Open Fázisváltás again, choose Kész (completed), and type any note.
  3. Before the fix: the order becomes Kész and the customer is sent the pickup email, with no MEO photo. `/transitions` also reported `gates_met: true` for this target.
- **Impact:** This bypasses the only evidence gate in the system (docs/history/VIABILITY.md:277). A car could be reported to the customer as ready with no completion photos on file.
- **Severity:** high
- **Confidence:** proven
- **Proof:** `domain::stage::tests::reopening_past_a_gate_still_requires_its_images` (backend/src/domain/stage.rs). It fails before the fix at the first assertion and passes after.
- **Fix:** backend/src/domain/stage.rs. The gate loop is now a shared `check_gates` helper. A reopen to a non-exit target must meet every active image gate positioned before the target (the path is unknown). Reopening to an earlier stage, or to the exit stage, stays ungated, and the note rule is checked first. `/transitions` inherits the same answer because it calls the same function (service/stages.rs:56-62), so both clients grey the option out.
- **Journey:** Order lifecycle

<a id="media-l1"></a>

### MEDIA-L1: Production photos starve behind 20+ queued inspection photos
- **Expected:** MEDIA-10 says the upload worker processes the due rows oldest first (A:data/db/PendingUploadDao.kt:29-43). INSP-51 says inspection rows belong to the inspection sync (A:data/upload/UploadWorker.kt:44-48).
- **Actual:** `dueForUpload` took the 20 oldest due rows (`LIMIT 20`), and the worker removed inspection rows from them only afterwards (A:data/upload/UploadWorker.kt:44-48 before the fix). Inspection rows stay `pending` with `next_attempt_at = 0` until their draft syncs. With 20 or more of them queued, every batch came back empty after the filter, the worker stopped, and nothing else uploaded.
- **Reproduction:** Android: do a walkaround (15+ zones plus dashboard and close-ups, so at least 20 photos) and don't sign it yet, or sign it while offline. Then take production photos on any order ("Kamera"/"Galéria"). Queue screen: the production photos stay "Várakozik" indefinitely, even online.
- **Impact:** Shop-floor photos, including the MEO completion evidence the stage gate needs, never reach the server while any inspection is pending on the phone. There is no visible error.
- **Severity:** high
- **Confidence:** proven
- **Proof:** `production photos are due even behind twenty older inspection photos` in android/app/src/test/java/hu/autotherm/autocrm/InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:data/db/PendingUploadDao.kt: `dueForUpload` excludes `category = 'inspection'` in SQL, before the LIMIT. A:data/upload/UploadWorker.kt: the now-redundant post-filter is removed and the comment updated.
- **Journey:** Inspection & media

<a id="insp-l3"></a>

### INSP-L3: The sync pushes and signs inspections that were never signed off on the phone
- **Expected:** INSP-39/INSP-40: the phone's sign-off requires an overview photo for every non-optional zone, both signatures with names, and a verdict for every check-in damage. Only then does "the sync worker perform the server sign" (A:ui/inspection/WalkaroundViewModel.kt:502-545; A:data/db/InspectionDraft.kt:12-21).
- **Actual:** `InspectionSyncWorker` runs at every app start and syncs every draft in `draft`/`syncing` state (A:data/inspection/InspectionSyncWorker.kt:30-46; A:AutoCrmApp.kt:60). `syncDraft` never checked `payload.signed`: it created the server inspection, pushed damages, photos and signatures, and then called `/sign` (A:data/inspection/InspectionSync.kt:43-230 before the fix). The server checks only for two signatures and for verdicts (backend/src/api/inspections.rs:615-638). So the phone-only overview-per-zone rule was bypassed for any draft whose two signatures had been drawn. A walk still in progress also got a server draft. On a check-out that draft then blocks new check-outs with `checkout_open`.
- **Reproduction:** Android: start a "Kiadás", photograph 2 zones, go to signing via "Összesítő", draw both signatures but don't tap "Aláírás és lezárás" (or tap it and get "hiányzó zónafotó"). Leave the screen and restart the app while online. The server now holds a signed, locked check-out with 13 zones missing. Without signatures you instead get a server draft plus a draft error "both inspector and customer signatures are required".
- **Impact:** A legally significant handover record gets signed and locked with incomplete evidence, without the phone's completeness checks. Partial server drafts appear during walks.
- **Severity:** high
- **Confidence:** proven
- **Proof:** `an unsigned draft is not pushed or signed by the sync` in InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:data/inspection/InspectionSync.kt: `syncDraft` returns the new `SyncResult.NotReady` for drafts with `signed = false` without touching the server. A:data/inspection/InspectionSyncWorker.kt handles `NotReady`.
- **Journey:** Inspection & media

<a id="sales-l4"></a>

### SALES-L4: Android converts every lead to a HUF order
- **Expected:** SALES-69: the conversion currency is pre-filled from the partner's `default_currency` and stays changeable, because an EUR job for an EUR partner must be possible (frontend/src/components/forms/LeadConvertDialog.tsx:52-55; docs/history/REMEDIATION.md:13; docs/history/AUDIT.md:140-156 MAJOR-03). Line items share the order currency, and the currency is locked once items exist (docs/DECISIONS.md:22-24).
- **Actual:** The Android view model always sent `currency = "HUF"` (android/.../ui/leads/LeadScreens.kt:307 before the fix), and the dialog said so ("…HUF pénznemmel jön létre", :506). The web fix for MAJOR-03 was never applied to the phone.
- **Reproduction:** Phone → Leadek → a lead whose partner has default currency EUR (for example an Austrian customer) → Megrendeléssé → Létrehozás → the new order shows HUF.
- **Impact:** Orders for EUR customers are created in the wrong currency. Once items are added the currency is locked (`currency_locked`), so the order must be recreated. Reports normalise the value at the wrong currency.
- **Severity:** high
- **Confidence:** proven† (compile-only). proven. The test failed to compile before the fix: there was no way to pass a currency.
- **Proof:** android/app/src/test/java/hu/autotherm/autocrm/SalesLeadConvertTest.kt: `conversionCurrencyFollowsTheEurPartnerDefault`, `conversionCurrencyFallsBackToHufWithoutAPartnerDefault`. The post-fix run is blocked by another agent's uncompilable SessionExpiryTest.kt (see Test runs).
- **Fix:** LeadScreens.kt: `convertBody(title, currency)`. `openConvert()` loads the partner's `default_currency`, and HUF/EUR chips make it changeable. The hint text is updated. Shared file: Dto.kt `Partner` gains `default_currency` (a field already in the OpenAPI `Partner` schema).
- **Journey:** Sales pipeline

<a id="inv-l5"></a>

### INV-L5: two concurrent "issue invoice" requests for one order can both pass the one-live-invoice check
- **Expected:** INV-11 / INV-12: an order has at most one invoice in flight or live. "Refuses rather than duplicating … reporting the same job twice is not an accident that fixes itself" (`backend/src/service/invoicing.rs:327-331`).
- **Actual:** `create_invoice` read the order with a plain `orders::find` and checked existing invoices with an unlocked `SELECT` (`backend/src/service/invoicing.rs:351-379` before the fix). The only lock was the invoice-number advisory lock, taken **after** the check (`backend/src/repo/invoices.rs:134`). Two requests (two tabs, two users, or a retried POST) could both see no invoice, then draw consecutive numbers and insert two `submitting` invoices. Both are then reported to NAV.
- **Reproduction:** two office users open the same order → Számlák → "Számla kiállítása" and confirm at the same moment (or replay the POST twice in parallel). Two invoices, AT…-000n and AT…-000n+1, appear for the order, and both reach NAV.
- **Impact:** the customer is invoiced twice, and both invoices are reported to the tax authority. Undoing it needs a storno, a second letter and an explanation.
- **Severity:** high
- **Confidence:** likely (race condition; the DB-backed suite cannot run here, and a deterministic race test needs two connections to Postgres)
- **Proof:** none executable here. The code path is shown above: the check-then-insert has no lock covering it.
- **Fix:** `backend/src/service/invoicing.rs`: `create_invoice` takes the order row with `orders::lock` (`SELECT … FOR UPDATE`, an existing repo function) inside the issuing transaction, so a second request waits and then sees the first one's `submitting` row (409 `invoice_in_flight`).
- **Journey:** Invoicing & money

<a id="inv-l8"></a>

### INV-L8: a retry after a timeout re-files the CREATE, and NAV's duplicate-number answer marks a stored invoice as rejected
- **Expected:** INV-67: a timeout "is not proof that nothing was reported … The caller must treat it as 'unknown'" (`backend/src/integrations/nav.rs:409-413`). "Saying 'rejected' here would be a guess, and the wrong one costs an invoice" (`backend/src/service/invoicing.rs:709-710`). Also: "a retried report is a duplicate invoice, not a repeated read" (`backend/src/integrations/nav.rs:248-250`).
- **Actual:** on a retryable failure the job fails and the queue re-runs `submit_invoice`, which sends a fresh `POST /invoices` (CREATE) for the same number (`backend/src/service/invoicing.rs:677-683`). If the first attempt reached NAV, NAV answers `INVOICE_NUMBER_ALREADY_EXISTS`. That is a non-retryable `nav_rejected`, so the row is marked `rejected` (`:714-725`) although NAV holds the invoice. The UI then re-enables "Számla kiállítása" (no live invoice), and a second invoice for the same job gets reported.
- **Reproduction:** set `NAV_SIDECAR_TIMEOUT_SECONDS` below the sidecar's NAV polling time (or have the network drop after submission), then issue an invoice. The retry comes back "Elutasítva — INVOICE_NUMBER_ALREADY_EXISTS", yet the invoice is in the Online Számla portal.
- **Impact:** a reported invoice is shown as rejected. Its number is treated as spent, and the office issues a second invoice for the same work, so two invoices reach NAV.
- **Severity:** high
- **Confidence:** proven†
- **Proof:** `service::invoicing::tests::a_retry_is_adopted_when_nav_holds_our_exact_document` + 4 refusal/parse cases in `backend/src/service/invoicing.rs` (unit; fail to compile before the fix, pass after). End to end: `a_retry_answered_duplicate_number_adopts_our_stored_report` and `a_retry_is_rejected_when_nav_holds_a_different_document` in `backend/tests/invoicing.rs` run a stub sidecar (the mock can never emit this fault): the CREATE is answered `INVOICE_NUMBER_ALREADY_EXISTS` and the read-back serves our document, or a foreign one. Both fail on the old code (adopted→rejected; refusal note missing). They need a live Postgres, so they compile here and run in CI.
- **Fix:** on a non-retryable `INVOICE_NUMBER_ALREADY_EXISTS` for an invoice, the job reads back what NAV holds (`NavSidecar::fetch_invoice`, the round-2 `GET /invoices/:number`) and adopts it when number, issue date and totals (compared as minor units, never floats) match the stored row: same `issued` as a fresh success, PDF and letter included, with `nav_transaction_id` NULL and the row saying why (the timed-out attempt's transaction id is unknowable). Anything else — nothing there, a different document — stays `rejected`, with the refusal reason on the row. A read-back that is itself of unknown outcome (unreachable) stays `submitting` and retries. Stornos are excluded: their totals' sign convention at NAV is unverified, so a duplicate storno still marks rejected.
- **Journey:** Invoicing & money

<a id="auth-l1"></a>

### AUTH-L1: Web does not return to login when the session dies mid-use
- **Expected:** AUTH-64: an unauthenticated user on an app page is sent to `/{locale}/login`, and "`401` returns to login" (docs/history/FRONTEND_PLAN.md:193, `frontend/src/components/layout/AppShell.tsx:56-58`). Sessions end on idle expiry, admin revoke, deactivation, role change or password reset (AUTH-17, AUTH-55, AUTH-56, AUTH-57).
- **Actual:** only the `/auth/me` query drives `isAuthenticated` (`frontend/src/lib/auth/context.tsx:22-26,47-52`). That query is cached (staleTime 30 s, `refetchOnWindowFocus: false`, `frontend/src/lib/query/provider.tsx`) and never re-runs on its own, so every later 401 from another query or mutation just shows the inline error "Nincs bejelentkezve…" (`frontend/src/components/ui/ErrorState.tsx:16-20`) and the user stays on the page until they reload.
- **Reproduction:** sign in on web as office. As admin, `PATCH /api/users/{id} {"role":"viewer"}` (or `POST /users/{id}/revoke-sessions`). Back in the office tab, open another list or save a form: an error box appears, but the sidebar and page stay and no redirect to `/hu/login` happens.
- **Impact:** after a revoke, role change or expiry, staff keep working in a dead UI. Every save fails, and they may lose typed work without knowing they need to sign in again. It also leaves the previous user's cached screens visible on shared PCs.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `frontend/src/test/auth-session-expiry.test.tsx`: "a 401 from any query signs the user out so the shell returns to login" and "a 401 from a mutation signs the user out too" (both failed before, pass after). "a 403 does not sign the user out" is the guard.
- **Fix:** `frontend/src/lib/query/provider.tsx`: `QueryCache`/`MutationCache` `onError` sets `qk.me` to `null` on any `ApiError` with status 401, so the existing AppShell gate redirects to login.
- **Journey:** Auth & access

<a id="auth-l3"></a>

### AUTH-L3: Android keeps a dead session after any 401 and cannot get back to login
- **Expected:** AUTH-83 / AUTH-75: a 401 means "the session is gone; the only cure is signing in again" (`android/app/src/main/java/hu/autotherm/autocrm/data/api/ApiError.kt:18`, UI text "A munkamenet lejárt. Jelentkezz be újra." `ui/common/Errors.kt:27`). The app shows the login screen exactly when no account is stored (`MainActivity.kt`, `account == null -> LoginScreen`). The backend kills sessions on revoke, deactivation, role change and admin password reset (AUTH-17, AUTH-55, AUTH-56, AUTH-57).
- **Actual:** nothing clears the stored token on 401 (`data/api/AutoCrmApi.kt` `execute`/`errorFor` only map 401 to `ApiException.Unauthenticated`). Only the drawer logout and a server-address change clear it (`MainActivity.kt` logout item, `ui/server/ServerSetupScreen.kt:125`). Every screen therefore shows "A munkamenet lejárt. Jelentkezz be újra." forever, and the user has to find the drawer's two-tap logout on their own. Worse: after an admin password reset (`must_change_password=true` plus all sessions revoked), a phone sitting on ChangePasswordScreen gets a 401 and shows "Hibás e-mail vagy jelszó." (`ui/login/ChangePasswordScreen.kt:105`) on a screen with no email field.
- **Reproduction:** sign in on the phone as a designer. As admin, `POST /api/users/{id}/revoke-sessions` (or `PATCH` the role). On the phone, pull to refresh the orders list: an error line appears, and the login screen never does.
- **Impact:** fitters are stuck in a UI where nothing works, which looks like "the app is broken". Uploads silently stop too, because every call 401s.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `android/app/src/test/java/hu/autotherm/autocrm/SessionExpiryTest.kt`. With only the `clearIfToken` line in `AutoCrmApi.execute` temporarily reverted, "a 401 on a signed-in request signs the phone out" FAILED (AssertionError, the account was still stored; 3 tests, 1 failed). With the line restored, all 3 pass, including the guards "a 401 for an old token does not sign out a newer session" and "a 403 keeps the session".
- **Fix:** `android/.../data/auth/SessionStore.kt`: new `clearIfToken(token)`. `android/.../data/api/AutoCrmApi.kt` (`execute`): on a 401 for a request that carried a token, call `sessionStore.clearIfToken(token)`, so the account flow drops to LoginScreen. The upload queue is left alone, as `SessionStore.clear` documents, and resumes after the next sign-in (the workers already no-op without a token).
- **Journey:** Auth & access

<a id="sales-l1"></a>

### SALES-L1: Partner update audit silently drops `role` changes
- **Expected:** SALES-11: each partner update writes a field-by-field diff of every changed column (backend/src/api/partners.rs:324-358, the diff lists every other column). SALES-20: a role change is audited like any other field. The supplier flag is "set deliberately" (backend/migrations/0011_relations_quotes_suppliers.sql:33).
- **Actual:** `PATCH /partners/{id}` writes `role` (backend/src/api/partners.rs:319, repo/partners.rs:158), but the audit diff lists every column except `role` (backend/src/api/partners.rs:324-358 before the fix). Reclassifying a customer as a supplier leaves either an empty `{}` update entry or no role entry at all.
- **Reproduction:** Android → Partner → Edit → set "Beszállító" → Save. Then read `audit_log` for entity `partner`: the changes JSON has no `role`.
- **Impact:** Nobody can tell who turned a customer into a supplier, or when, although it decides whether the partner shows up in customer pickers.
- **Severity:** medium
- **Confidence:** proven (the test asserts on the extracted diff function; see Test runs for the compile status)
- **Proof:** `api::partners::tests::partner_update_audit_records_a_role_change` in backend/src/api/partners.rs
- **Fix:** backend/src/api/partners.rs: moved the diff into `partner_changes()` and added `("role", …)`.
- **Journey:** Sales pipeline

<a id="sales-l2"></a>

### SALES-L2: Web lead form sends a contact that does not belong to the chosen partner
- **Expected:** SALES-33: a lead's contact must belong to the lead's partner. A contact without a partner is refused: 400 "contact belongs to a different partner" (backend/src/service/leads.rs:22-31). SALES-23: the contact select lists only the chosen partner's contacts and is disabled without a partner (frontend/src/components/forms/LeadForm.tsx:214-217).
- **Actual:** Changing or clearing the partner left the previous `contact_id` in the form state. The select is disabled or re-populated, so the stale value is hidden. `leadCreateBody`/`leadPatchBody` still sent it (LeadForm.tsx:61,80 before the fix). Create failed with an error the user cannot act on, because the offending field is invisible. On edit, a backend PATCH has no such check (see Needs decision N1), so the lead was saved with partner B and partner A's contact, or with a contact and no partner.
- **Reproduction:** Leadek → Új lead → pick partner A → pick contact → clear the partner (or pick partner B) → Save → the banner shows "contact belongs to a different partner". On an existing lead, remove the partner and save: `contact_id` stays set.
- **Impact:** Confusing create failures. Inconsistent partner/contact pairs on edited leads, so quotation letters and later conversion work from a contact of the wrong company.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** frontend/src/test/sales-lead-form.test.ts: "is not sent on create when no partner is chosen" and "is cleared on edit when the partner is removed". Both failed before the fix and pass after.
- **Fix:** frontend/src/components/forms/LeadForm.tsx: the builders send a contact only when a partner is set (`contactOf`), and changing the partner in the picker resets `contact_id`.
- **Journey:** Sales pipeline

<a id="ord-l2"></a>

### ORD-L2: The pickup board's "in the workshop" list dropped older in-work cars
- **Expected:** ORD-59 says the board's lower half lists the open orders currently in the workshop (design/production/meo), refreshed every 30 s (frontend/src/components/board/PickupBoard.tsx:3-6,19).
- **Actual:** The board fetched `GET /orders?open=true&limit=100`. The server sorts that by `-created_at` (backend/src/repo/orders.rs:301). The board then filtered to the workshop stages on the client (PickupBoard.tsx:31-45 before the fix). Once 100 newer open orders exist (most of them in `intake`), older cars in design/production/meo fall off the page and silently vanish from the wall display.
- **Reproduction:** Have more than 100 open orders, then create a few new intake orders. Open `/hu/board`. A car that has been in Gyártás (production) for weeks is missing from "working", although `/hu/orders?stage=production` lists it.
- **Impact:** The shop-wall display under-reports work in progress, and the longest-running jobs (the ones most in need of attention) are hidden first.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `pickup board > lists an in-work car even when newer open orders fill the first page` (frontend/src/test/logicfix-orders-board.test.tsx). It fails before the fix and passes after.
- **Fix:** frontend/src/components/board/PickupBoard.tsx now runs one server-filtered query per workshop stage (`stage=<key>`, limit 200) instead of client-filtering the first page of all open orders. The stage keys are still hardcoded (B2).
- **Journey:** Order lifecycle

<a id="ord-l4"></a>

### ORD-L4: Emptying a field on the Android order edit screen did not clear it
- **Expected:** ORD-18 says a PATCH keeps an omitted field and clears one sent as `null` (docs/API.md:25; backend/src/api/mod.rs:162-186). The edit screen shows the current values in text fields, so emptying a field and saving is the user's way of clearing it (implied).
- **Actual:**
  - OrderEditScreen built an `OrderBody` with `blankToNull(...)` for the plate, VIN, make, model, description and due date (android/.../ui/orders/OrderEditScreen.kt:101-111 before the fix).
  - The shared serializer uses `explicitNulls = false` (android/.../data/api/AutoCrmApi.kt:41), so every `null` was dropped from the wire.
  - The server therefore kept the old value, and the screen reported a successful save.
- **Reproduction:** Phone → Munkák → open an order with a due date and a plate → edit → empty "Határidő" and "Rendszám" → save. The order detail still shows the old due date and plate. On the web, the same clear works.
- **Impact:**
  - Wrong data can't be removed from the phone.
  - A due date the fitter believes is cleared stays, and keeps driving overdue lists and sorting.
  - The web and Android disagree on the same action.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `OrderPatchBodyTest.emptied_fields_are_sent_as_explicit_nulls_so_the_server_clears_them` (android/app/src/test/java/hu/autotherm/autocrm/OrderPatchBodyTest.kt).
  - With the fix hunk temporarily reverted (emptied fields omitted, as the old nulls-dropping body did), it fails with `AssertionError` at OrderPatchBodyTest.kt:37.
  - With the fix restored, it passes.
- **Fix:**
  - android/.../ui/orders/OrderEditScreen.kt: the new `orderPatchJson(state)` builds a `JsonObject` with explicit `JsonNull` for emptied fields, and `save()` uses it for edits (creates are unchanged).
  - android/.../data/api/AutoCrmApi.kt (shared file): a `patchOrder(id, JsonObject)` overload sends `JsonObject.toString()`, so the nulls reach the wire.
- **Journey:** Order lifecycle

<a id="insp-l2"></a>

### INSP-L2: Web "new" / "dismissed" verdicts are linked to the suggested check-out damage
- **Expected:** INSP-31/INSP-33: a verdict "may point at the pre-existing check-out damage it matches, or stand alone as new" (backend/migrations/0026_inspections.sql:73-74). The phone sends `checkout_damage_id` only with "Megvolt" and sends null for "Új" / "Nem sérülés" (A:ui/inspection/InspectionFinishScreens.kt:164-173).
- **Actual:** The web sent `suggestion.checkout_damage_id` with every verdict value (InspectionSection.tsx:321 before the fix). A damage judged "new" was stored as linked to an old check-out damage. The same rule was implemented differently on web and Android.
- **Reproduction:** Web → draft check-in whose damage matches a check-out damage (same zone and type) → click "Új sérülés" → the stored verdict has `verdict='new'` and a non-null `checkout_damage_id`.
- **Impact:** A contradictory damage record: the verdict says "new damage, the customer is liable", but the link says it was already there at check-out.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `a "new" verdict does not link the suggested check-out damage` in frontend/src/test/inspection-verdicts.test.tsx (failed before the fix, passes after).
- **Fix:** InspectionSection.tsx: `checkout_damage_id` is sent only when `verdict === 'preexisting'`.
- **Journey:** Inspection & media

<a id="insp-l4"></a>

### INSP-L4: An unfinished damage (no type or severity) makes the draft unsyncable, or a check-in unsignable
- **Expected:** INSP-19/INSP-20: a damage needs a type and a severity, and the damage step refuses "Kész" without both (A:ui/inspection/WalkaroundScreen.kt:524-535). The comparison screen and the zone list only show damages with a type (A:ui/inspection/InspectionFinishScreens.kt:98; WalkaroundScreen.kt:304).
- **Actual:** `startDamage` stores a blank damage in the draft right away (A:ui/inspection/WalkaroundViewModel.kt:356-367). If the user leaves the damage step without "Eldobás" (system back, or the app is killed), the blank damage stays. The sync posted it and the server answered 400 "damage_type must be one of …", which permanently failed the draft (A:data/inspection/InspectionSync.kt:91-111 before the fix). On a check-in, `signNow` counted it as needing a verdict, but it can never get one because the comparison screen doesn't list it. That meant "1 sérüléshez még kell döntés" forever (WalkaroundViewModel.kt:527-538 before the fix).
- **Reproduction:** Android: in a zone tap "Igen" (damage), cancel the camera, press system back. Resume the draft and finish and sign the walk. Check-out: the draft shows an error after sync. Check-in: signing is blocked, and no verdict chip exists for the blank damage.
- **Impact:** A completed handover can never reach the server (the only way out is discarding the whole walk), or a check-in can never be signed.
- **Severity:** medium
- **Confidence:** proven (sync path). The `signNow` path is likely (no unit test for the ViewModel).
- **Proof:** `an unfinished damage without type is not sent to the server` in InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:data/inspection/InspectionSync.kt: only complete damages (`isComplete()`) are synced; close-ups of an abandoned damage attach without a damage link. A:ui/inspection/WalkaroundViewModel.kt `signNow`: pending verdicts count only damages with type and severity.
- **Journey:** Inspection & media

<a id="insp-l6"></a>

### INSP-L6: Discarding a synced draft leaves the server draft behind, blocking new check-outs
- **Expected:** INSP-14 says a draft can be discarded. INSP-08 says one open check-out per order, and the user is told "sign or discard it first" (backend/src/api/inspections.rs:214-218). INSP-52 says discarding a local draft removes it (A:data/inspection/InspectionSyncWorker.kt:77-98).
- **Actual:** Local discard deleted only the phone's copy. If the draft had already reached the server (`server_id` set, then a later step failed, e.g. a verdict 400), the server draft check-out stayed. Neither client has a discard button for server drafts (web: no create/delete, InspectionSection.tsx:3-6; Android history cards are read-only). Every later "Kiadás" on that order then failed with `checkout_open`.
- **Reproduction:** Android: a check-out sync fails after creation (e.g. an invalid value in a later step), the user taps "Eldobás" on the draft, then starts a new "Kiadás" and signs it. The sync fails with "this order already has an open check-out; sign or discard it first".
- **Impact:** The order can no longer get a check-out from the phone. Fixing it needs a DBA or API call.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `discarding a synced draft discards the server draft too` in InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:data/inspection/InspectionSyncWorker.kt `discard`: when the draft has a `serverId`, it first calls `DELETE /inspections/{id}` (best effort; the server refuses signed rows).
- **Journey:** Inspection & media

<a id="inv-l1"></a>

### INV-L1: NAV messages recorded "for the screen" are never shown unless the invoice is rejected
- **Expected:** INV-65 / INV-104: a failed attempt that the queue retries is noted on the row so the screen can show it: "The screen shows this while the queue keeps trying, so 'nothing is happening' is never the whole story a user gets" (`backend/src/service/invoicing.rs:752-755`). When NAV refuses an annulment, the invoice keeps its status and "the refusal is recorded where the screen can show it" (`backend/src/service/invoicing.rs:898-899`).
- **Actual:** the web renders `nav_message`/`nav_error_code` only inside the `status === 'rejected'` block (`frontend/src/components/orders/InvoicesSection.tsx:258-266` before the fix). A `submitting` row only ever shows the static hint "Bejelentés a NAV felé folyamatban…" (`:239`). An `issued` row whose annulment NAV refused shows nothing, so the admin sees an invoice that is still "Kiállítva" with no hint that the annulment failed.
- **Reproduction:** Order → Számlák tab. (a) Stop the nav-sidecar and issue an invoice: the row stays "Bejelentés folyamatban" indefinitely, and the reason (`the NAV sidecar is unreachable: …`, stored in `invoices.nav_message`) is never shown. (b) As admin, choose "Technikai érvénytelenítés" on an issued invoice that NAV refuses to annul: the row stays "Kiállítva" with no message.
- **Impact:** the office cannot tell a stalled report or a refused annulment from normal processing. For an annulment, the admin believes it was filed.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-invoicing.test.tsx`: "shows the note of a failed attempt while the queue keeps retrying" and "shows NAV refusing an annulment on an invoice that stays issued" (both failed before the fix, pass after).
- **Fix:** `frontend/src/components/orders/InvoicesSection.tsx`: a non-rejected row shows `nav_error_code — nav_message` when present. The backend clears both on success (`backend/src/repo/invoices.rs:307`), so a stored invoice shows nothing extra.
- **Journey:** Invoicing & money

<a id="inv-l2"></a>

### INV-L2: the invoice list stops polling before the PDF is attached, so the PDF link never appears without a reload
- **Expected:** INV-71 / INV-80: while NAV decides, "the answer arrives on its own rather than on a reload" (`frontend/src/components/orders/InvoicesSection.tsx:80`). Once NAV stores the report, the PDF is filed with the order and linked on the row (`backend/src/service/invoicing.rs:813-831`).
- **Actual:** `record_success` commits `status = 'issued'` first (`backend/src/service/invoicing.rs:774-786`) and only then fetches the PDF from the sidecar (a NAV `queryInvoiceData` plus render) and sets `document_id` (`:791`, `:829`). The web stopped polling as soon as no row was `submitting` (`InvoicesSection.tsx:81-82` before the fix), so the last poll typically showed the invoice issued with `document_id = null`. The "PDF" link and the order's document list stayed stale until a manual reload. The documents query was never invalidated when the PDF arrived.
- **Reproduction:** issue an invoice and watch the row turn "Kiállítva": no "PDF" link appears. Reload the page and it appears.
- **Impact:** the office thinks the PDF is missing, or downloads and sends it late. The Dokumentumok list also lacks the invoice until reload.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-invoicing.test.tsx`: `invoicePollInterval` › "keeps polling until the PDF of a just-issued invoice is attached" (failed before: no such behaviour or export), plus the controls "stops polling once every invoice is decided and filed" and "does not poll forever for a PDF that never arrived".
- **Fix:** `InvoicesSection.tsx`: new exported `invoicePollInterval`. It keeps the 3 s poll while an invoice is `submitting`, or `issued` without `document_id` within 2 minutes of `issued_at`. It also invalidates `qk.documents(orderId)` when the set of filed PDFs changes.
- **Journey:** Invoicing & money

<a id="mail-l2"></a>

### MAIL-L2: A document attached and embedded, or picked twice, makes the letter fail as "deleted before sending"
- **Expected:** MAIL-18/MAIL-19/MAIL-20: existing documents may be attached, and images may be embedded inline as `cid:doc-ID` (`backend/src/service/email.rs:505-541, 1147-1169`). MAIL-72: a letter fails with "an attached document was deleted before sending" only when a document really was deleted (`backend/src/service/email.rs:1122-1141`).
- **Actual:** `documents::find_many` returns each id once (`backend/src/repo/documents.rs:136-148`, `WHERE id = ANY($1)`). Every caller then compared the number of rows found with the number of requested ids, duplicates included. `deliver` built `ids` from every attachment ref (`service/email.rs:1142` before the fix). An image both attached and embedded is stored as two refs with the same id, so `docs.len() != ids.len()` and the email was set to `failed` "an attached document was deleted before sending". At compose time, the same document picked from the order's list and from the library produced 422 "attachments must be existing documents" (`:528-529`, and likewise `:745`, `:890`, `:402` for embeds).
- **Reproduction:** Web → /emails/new (or compose from an order) → "Könyvtár" → on an image, click both "Csatolás" and "Beágyazás" → Küldés. The send is accepted (202). The detail page later shows "Sikertelen" with the error "an attached document was deleted before sending". Variant: tick a document in the order's attachment list, also attach it from the library → the preview and send fail with "attachments must be existing documents".
- **Impact:** The letter is never delivered, with a false error message. The office thinks a file was deleted and loses time.
- **Severity:** medium
- **Confidence:** proven† (compile-only). proven (helper-level). The unit test failed to compile before the fix because the id-dedup did not exist. A DB-level delivery test was not possible because no Postgres was available.
- **Proof:** `service::email::tests::a_document_attached_and_embedded_is_looked_up_once` in `backend/src/service/email.rs`. DB-level: `a_document_attached_and_embedded_is_not_reported_deleted` in `backend/tests/email_and_jobs.rs` (needs a live Postgres; compiles, not executed here).
- **Fix:** `backend/src/service/email.rs`: new `distinct_ids` / `requested_document_ids` helpers. `deliver`, `prepare` (attachments), `apply_embeds`, `send_quotation` and `send_newsletter` now look up and count each document once. An image that is both attached and embedded is still sent both ways, as requested.
- **Journey:** Email & communications

<a id="ord-l5"></a>

### ORD-L5: The pickup board's "ready" list selects by creation date, not by completion
- **Expected:** ORD-59 says the top section shows the cars "ready for collection" (PickupBoard.tsx:3-4), ordered by when they entered `completed` (the client sorts by `stage_entered_at`).
- **Actual:** The board fetches `GET /orders?stage=completed&limit=30` with no sort. The server applies `-created_at` (backend/src/repo/orders.rs:301), so the board receives the 30 most recently *created* completed orders and only then re-sorts them. Completed orders never leave `completed`, so they accumulate. A car completed today whose order was opened before the 30 newest completed orders never appears, while long-collected newer orders fill the wall.
- **Reproduction:** Have 30 or more completed orders. Take an order created earlier than all of them and move it to Kész today. Open `/hu/board`: the car is missing from "ready".
- **Impact:** The shop display can hide exactly the car that has just become ready.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-orders-board.test.tsx`: "asks the server for the most recently completed cars, not created ones" (fails before: the board sent no `sort`; passes after). Backend: `repo::orders::tests::orders_can_be_sorted_by_when_they_entered_their_stage` asserts `parse_sort` accepts `-stage_entered_at` and the SQL orders by `cs.entered_at` (the full end-to-end sort needs a live Postgres).
- **Fix:** backend `search` accepts `stage_entered_at` in `ORDER_SORTS` with `ORDER BY cs.entered_at` branches (`backend/src/repo/orders.rs`; runtime-checked SQL so no `.sqlx` cache regen was needed). The board (`frontend/src/components/board/PickupBoard.tsx`) and the dashboard "Átvehető" section (`frontend/src/components/dashboard/DashboardView.tsx`) now request `sort: '-stage_entered_at'`; the client-side re-sort stays only as a tiebreak.
- **Also affects:** the web dashboard "Átvehető" section read the same `stage=completed, limit 30` list; it now requests the same sort (fixed together). The reports agent reported this separately as REP-L1; it is merged here. Whether "ready" should exclude collected cars is still Q-ORD-10 (there is no "picked up" stage).
- **Journey:** Order lifecycle

<a id="insp-l7"></a>

### INSP-L7: "Újra" (retake) drops earlier kept photos of the same zone and orphans their queue rows
- **Expected:** INSP-26: retake replaces the shot under review, deleting its file and queue row (A:ui/inspection/WalkaroundViewModel.kt:309-346). Multiple dashboard shots and extra close-ups are allowed ("Újabb műszerfal-fotó", "Még egy közeli"; WalkaroundScreen.kt:220-227, 449).
- **Actual:** The payload filter removed every unattached photo with the same zone, purpose and damage (WalkaroundViewModel.kt:333-338 before the fix), but only the reviewed file and its queue row were deleted. Earlier kept close-ups vanished from the record. Their inspection queue rows stayed forever: the upload worker ignores inspection rows, the sync only handles photos in the payload, and the queue screen gives no discard for them.
- **Reproduction:** Android: take 2 close-ups of one damage ("Megtartom" both), take a third and press "Újra". Only the new retake remains in the draft. The queue screen keeps 2 "Átvétel" rows at "Várakozik" forever.
- **Impact:** Evidence photos silently leave the inspection, and the queue carries rows that never clear.
- **Severity:** medium
- **Confidence:** likely (the ViewModel has no unit-test harness: it needs a live AutoCrmApp plus a camera round trip)
- **Proof:** none (reasoned from the code). Covered by the Android suite compiling and passing.
- **Fix:** A:ui/inspection/WalkaroundViewModel.kt `retakePhoto`: only the photo whose `fileName` equals the reviewed file is removed.
- **Journey:** Inspection & media

<a id="insp-l9"></a>

### INSP-L9: A check-in started offline can never load its comparison
- **Expected:** INSP-33/INSP-47: the comparison needs signal, and the sync resolves the check-out link for check-ins started offline (A:ui/inspection/WalkaroundViewModel.kt:176-186; A:ui/inspection/InspectionFinishScreens.kt:64-72 "Újrapróbálás").
- **Actual:** `loadComparison` returned immediately when `checkoutServerId` was null (WalkaroundViewModel.kt:437-439 before the fix). Nothing resolved the link later on the phone, so "Újrapróbálás" did nothing, even with signal, and damages could never get verdict chips. Because `signNow` needs a verdict for every damage, the check-in could not be signed.
- **Reproduction:** Android: start a "Visszavétel" in airplane mode and record a damage. Turn networking on, reach the Comparison step, tap "Újrapróbálás": nothing happens, and signing says the damages still need decisions.
- **Impact:** A check-in with damages that started without signal can never be completed. It has to be discarded and walked again.
- **Severity:** medium
- **Confidence:** likely (no ViewModel test harness)
- **Proof:** none.
- **Fix:** A:ui/inspection/WalkaroundViewModel.kt `loadComparison`: when the link is missing it fetches the order's latest signed check-out (the same rule the server applies), persists it, then loads it. With no signed check-out it shows "nincs lezárt átadás ehhez az összehasonlításhoz".
- **Journey:** Inspection & media

<a id="inv-l6"></a>

### INV-L6: proforma number lock is released before the proforma exists, so concurrent proformas draw the same number and one PDF is filed as an orphan
- **Expected:** INV-54: proformas have their own series and "its own lock" (`backend/src/repo/invoices.rs:152-153`). The lock exists so that "two … issued in the same second must not draw the same number" (`backend/src/repo/invoices.rs:130-132`).
- **Actual:** `next_proforma_number` computes `max(existing) + 1` over **stored** rows. `create_proforma` drew the number in a transaction it committed immediately, which released the advisory lock (`backend/src/service/invoicing.rs:989-995` before the fix). It then rendered, stored the PDF as an order document, and inserted the row in a second transaction. Two concurrent requests both get e.g. `DB2026-0007`. The second renders a PDF with that number, files it as a document of its order (`store_pdf` runs before the insert), and then fails on `proformas.number UNIQUE` with a generic 409 "duplicate".
- **Reproduction:** two users click "Díjbekérő készítése" on two orders at the same moment. One gets an error, but its order's Dokumentumok list gains a `DB2026-0007.pdf` that belongs to no proforma and carries another customer's proforma number.
- **Impact:** duplicate payment-request numbers on paper, an orphan document, and a failed request the user retries.
- **Severity:** medium
- **Confidence:** likely (race condition; no DB here)
- **Proof:** none executable here.
- **Fix:** `backend/src/service/invoicing.rs`: one transaction now covers number, render, PDF, row, audit and letter. The series lock is held until the row is committed, and a failed render still rolls back without a gap. The customer is validated before the lock is taken, and the read connection is released before the transaction opens.
- **Journey:** Invoicing & money

<a id="inv-l9"></a>

### INV-L9: a PDF that failed to store "can be fetched again", but nothing can fetch it
- **Expected:** INV-81: "A missing PDF is visible on the row and can be fetched again" (`backend/src/service/invoicing.rs:788-790`).
- **Actual:** `store_invoice_pdf` has exactly one caller, `record_success` (`backend/src/service/invoicing.rs:791`). There is no endpoint, job or admin action that retries it, so an invoice whose PDF store failed never gets a PDF or an attached letter.
- **Reproduction:** make object storage fail briefly while an invoice is being reported. The invoice becomes "Kiállítva" with no "PDF" link and nothing in the UI or API brings it back.
- **Impact:** no PDF for an issued invoice, and a letter without its attachment.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-invoicing.test.tsx`: "offers to fetch the PDF again for an issued invoice filed without one" (no such button before; passes after) and "offers no PDF refetch once the invoice already has its PDF". Backend guards: `refetching_a_pdf_needs_an_issued_invoice_missing_its_file` in `backend/tests/invoicing.rs` (`not_issued`, `duplicate`, `not_found`; compiles here, runs in CI).
- **Fix:** new `POST /invoices/{id}/pdf` (`IssueInvoices` gate): re-renders the PDF from what NAV holds and files it with the order's documents. Refused with `not_issued` (422) when the invoice is not issued, `duplicate` (409) when the PDF is already filed, and `pdf_unavailable` (422, retryable) when the fetch fails. Both codes are in the error catalog (`backend/src/error.rs`, `docs/error-codes.md`, web `hu.json`, Android `Errors.kt`). The web shows "PDF újratöltése" on an issued row without a file. The letter queued at issue time is deliberately left alone: it may already have gone out without the attachment, and re-sending mail is the office's call.
- **Journey:** Invoicing & money

<a id="inv-l10"></a>

### INV-L10: an invoice whose request cannot be built at submit time is retried to death and stays `submitting` forever
- **Expected:** "Only a failure to get an answer at all is an error, and the queue retries that" (`backend/src/service/invoicing.rs:660-665`). The screen must be able to show why (INV-65).
- **Actual:** `build_request` re-reads the partner and the FX rate when the job runs (`backend/src/service/invoicing.rs:1225-1249`). If the partner was edited in between (e.g. the address cleared) it returns a rule error. That becomes `anyhow!("building the invoice: …")`, the job fails, and it is retried 5 times and dead-lettered. The row stays `submitting` indefinitely, blocks any new invoice or storno on the order (`:358-366`), and the web polls it every 3 s forever.
- **Reproduction:** issue an invoice while the sidecar is down, clear the partner's address, then restore the sidecar.
- **Impact:** an order stuck with a phantom in-flight invoice and no UI path out.
- **Severity:** medium
- **Confidence:** proven†
- **Proof:** `service::invoicing::tests::only_unbuildable_submit_failures_are_terminal_on_dead_letter` (unit predicate; fail to compile before, passes after). End to end: `an_unbuildable_submit_is_rejected_when_the_queue_gives_up` in `backend/tests/invoicing.rs` wipes the partner address mid-flight, submits (marked error, row still `submitting`), runs the dead-letter step, and asserts `rejected` + "nothing was sent to NAV" + a new invoice issuable for the order. Fails on the old code (no such function; row stuck). Compiles here, runs in CI.
- **Fix:** a build failure now carries an `invoice_unbuildable:` marker (and is noted on the row while retrying, so the screen shows the cause during the retry window). When the queue gives up, `jobs::run_one` recognises the marker and calls the new `invoicing::job_dead_lettered_unbuildable`, which locks the row and — only if still `submitting` — marks it `rejected` with the local code `unbuildable` and an audit entry. The message is explicit that NAV never saw the invoice. On Q-INV-9 (does a non-report spend a number?): yes, like any other rejection the number stays spent — a gap the row explains, instead of an order that can never be invoiced again. Unreachable/timeout failures are untouched: still unknown, still retried, never terminal.
- **Journey:** Invoicing & money

<a id="mail-l4"></a>

### MAIL-L4: A dead-lettered `send_email` job leaves the email "queued" forever, invisible and not retryable
- **Expected:** MAIL-08/MAIL-07: mail needing attention is `failed` + `needs_review`, surfaced in `/admin/status` and the attention filter (`backend/migrations/0005_email.sql:118-119`). MAIL-26: the office's recovery path is "retry a failed or needs-review email" (`backend/src/service/email.rs:1287-1298`). JOB-03: after 5 errors a job is dead-lettered (`backend/src/jobs/mod.rs:105-107`).
- **Actual:** When `deliver` returns `Err` before `mark_sending` (for example storage `get_bytes`/`presign_get` failing, `service/email.rs:1159, 1180, 1198`, or a DB error), the transaction rolls back and the email stays `queued`. After 5 job attempts `run_one` dead-letters the job (`jobs/mod.rs:105-107`) without touching the email. The email then shows "Várakozik" forever, is not counted in `emails_needing_attention`, and `POST /emails/{id}/retry` refuses it with 409 `not_retryable`.
- **Reproduction:** Queue a letter with an attachment while object storage is unreachable for ~1h (5 attempts with backoff). The job appears under /admin/jobs?state=failed, while the email detail still says "Várakozik" and has no Retry button.
- **Impact:** A customer letter silently never goes out, and the email log says it is still pending.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** `service::email::tests::a_dead_lettered_send_leaves_queued_and_never_claims_failure_when_unknown` in `backend/src/service/email.rs` (unit: `queued → failed`, `sending → needs_review`, settled rows untouched). DB-level: `a_dead_lettered_send_job_makes_the_email_failed_and_retryable` in `backend/tests/email_and_jobs.rs` (needs a live Postgres; compiles, not executed here).
- **Fix:** `backend/src/jobs/mod.rs` (`run_one`): when a `send_email` job is dead-lettered, it calls the new `service::email::job_dead_lettered`, which locks the email row and moves `queued → failed` ("the delivery job gave up; nothing was sent") or `sending → needs_review` (SMTP may already have accepted it — never claim failure when the outcome is unknown). Settled rows are untouched. The row then appears under "needs attention" and retries through the normal `POST /emails/{id}/retry` path.
- **Journey:** Email & communications

<a id="insp-l10"></a>

### INSP-L10: A retried check-out create after a lost response becomes a permanent `checkout_open`
- **Expected:** INSP-46 says every sync step is idempotent. INSP-08 allows one draft check-out per order.
- **Actual:** `createInspection` has no idempotency key. If the create response is lost, or the process dies before `setServerId` (A:data/inspection/InspectionSync.kt:71-87), the retry creates again. For a check-out the server answers 422 `checkout_open` (backend/src/api/inspections.rs:210-222), and the draft fails forever against its own orphan. (INSP-L6 now lets the user discard the local draft, but the orphan has no local `serverId` to delete.)
- **Reproduction:** Android: sign a check-out while the network drops just after `POST /inspections` reaches the server. The next sync fails with "this order already has an open check-out".
- **Impact:** The order is stuck until an admin removes the orphan server draft.
- **Severity:** medium
- **Confidence:** proven
- **Proof:** backend `api::inspections::tests::client_key_is_optional_trimmed_and_bounded` and `a_key_replay_must_name_the_same_order_and_kind` in `backend/src/api/inspections.rs` (fail before: no `client_key` concept; pass after). Android `the inspection create carries the draft uuid as its idempotency key` in `InspectionLogicTest.kt` (MockWebServer: asserts the create body contains `"client_key":"<draft uuid>"`; fails before, passes after).
- **Fix:** new nullable `inspections.client_key` with a partial unique index (`backend/migrations/0027_inspection_client_key.sql`; old rows and key-less callers keep the old behaviour). `POST /inspections` accepts an optional `client_key` (blank = none, >100 chars = 400): a replay with the same key returns the existing row with 200 instead of a second row or `checkout_open`; a key reused for a different order/kind answers 409 `duplicate`; a lost race between two retries resolves to the same row. The phone sends its stable local draft UUID as the key (`InspectionSync.kt`; `InspectionBody.clientKey`).
- **Journey:** Inspection & media

<a id="auth-l2"></a>

### AUTH-L2: Web session cookie expires 7 days after login even for active users (30-day absolute lifetime never applies)
- **Expected:** AUTH-18/AUTH-19: web sessions have a 7-day idle and a 30-day absolute lifetime. Activity slides `expires_at` forward, capped at `created_at + 30 days` (`docs/DECISIONS.md:135-136`, `backend/src/service/auth.rs:26-33,195-206`).
- **Actual:** the cookie is set only at login, with `Max-Age = expires_at - now` = the first 7-day idle window (`backend/src/api/auth.rs:134-142`). The server renews `expires_at` on activity but never re-issues the cookie, so the browser deletes it exactly 7 days after login, however active the user is. In practice web sessions have a 7-day absolute lifetime.
- **Reproduction:** sign in on web and use the app every day. At the same time of day, 7 days after that login, the next request carries no cookie, `/auth/me` returns 401 and the user lands on `/hu/login`, while the server-side session (visible via `GET /auth/sessions` from a new login) was valid for up to 7 more days.
- **Impact:** office staff are forced out mid-work once a week, contrary to the documented policy, and a live but unusable session row stays behind.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `backend/src/service/auth.rs` unit test `web_cookie_outlives_the_idle_window_until_the_absolute_cap`. With the handler's existing formula (`session_expiry - now`) it failed (`left: 604800, right: 2592000`); it passes after the fix.
- **Fix:** `backend/src/service/auth.rs`: new `session_absolute_expiry` and `cookie_max_age_secs` (Max-Age runs to the absolute cap). `backend/src/api/auth.rs` (web login): the cookie `Max-Age` now uses it. Idle expiry is still enforced server-side through `expires_at` (AUTH-17).
- **Journey:** Auth & access

<a id="ord-l3"></a>

### ORD-L3: The pickup board rendered two nested app shells
- **Expected:** ORD-59 says the board is a single display; "this page IS the display" (PickupBoard.tsx:3-6). Every other route renders exactly one `AppShell`.
- **Actual:** `app/[locale]/board/page.tsx:5-8` wraps `<PickupBoard/>` in `AppShell`, and `PickupBoard` wrapped its own content in a second `AppShell`. The result was nested navigation and chrome on the wall display.
- **Reproduction:** Open `/hu/board`. Before the fix, the sidebar, header and navigation appear twice, nested.
- **Impact:** Wasted space and a confusing, broken-looking display on the shop TV.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `pickup board > renders the board inside exactly one app shell` (frontend/src/test/logicfix-orders-board.test.tsx). It fails with 2 shells before the fix and passes with 1 after.
- **Fix:** frontend/src/components/board/PickupBoard.tsx no longer wraps itself in `AppShell`; the route supplies it.
- **Journey:** Order lifecycle

<a id="insp-l1"></a>

### INSP-L1: Web offers verdict buttons on a signed (locked) check-in
- **Expected:** INSP-15: every mutation on a signed inspection is refused with 422 `locked` (backend/src/api/inspections.rs:76-90, 759; backend/migrations/0026_inspections.sql:8-9). The UI should not offer an action the lock always refuses.
- **Actual:** The review buttons were gated only on `canChangeStage(user)` (frontend/src/components/inspections/InspectionSection.tsx:249 before the fix). A signed check-in showed "Már megvolt / Új sérülés / Nem sérülés", and every click came back with a 422 error.
- **Reproduction:** Web → order → Átvételi lap → expand a signed "Visszavétel" → the verdict buttons are shown → click one → error "a signed inspection cannot be changed…".
- **Impact:** Office staff think verdicts on a signed record are still editable, and every attempt ends in an error.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `a signed check-in offers no verdict buttons` in frontend/src/test/inspection-verdicts.test.tsx (failed before the fix, passes after).
- **Fix:** frontend/src/components/inspections/InspectionSection.tsx: `review` now also requires `checkin.inspection.status === 'draft'`.
- **Journey:** Inspection & media

<a id="media-l2"></a>

### MEDIA-L2: A failed "complete" throws away the ticket and re-uploads the photo
- **Expected:** MEDIA-15 says a still-valid ticket is reused so already-PUT bytes go straight to complete (A:data/db/PendingUpload.kt:49-55, 97-98; A:data/upload/Uploader.kt:57-60).
- **Actual:** The ticket is stored only after a successful PUT (Uploader.kt:82-86), but `markRetryable` cleared it (A:data/db/PendingUploadDao.kt:84-94 before the fix). So when `complete` hit a 5xx or network error, every retry asked for a new ticket and PUT the whole file again. Ticket reuse only worked after a process death.
- **Reproduction:** Android: queue a photo, let the PUT succeed and `/uploads/complete` fail (server restart, or losing signal at the wrong moment). The next attempt uploads the full file again: the request log shows a second `POST /orders/{id}/uploads` and a second PUT.
- **Impact:** Several MB of mobile data wasted per retry, on the poor-signal yards this queue was built for. Slower drains.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `a retry after a failed complete reuses the ticket instead of uploading again` in InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:data/db/PendingUploadDao.kt: `markRetryable` keeps the ticket. The manual `retryAll` / `retryOne` now clear it, so "Újra" starts clean. The stale comment in A:data/upload/Uploader.kt:125-127 is corrected.
- **Journey:** Inspection & media

<a id="insp-l5"></a>

### INSP-L5: A sync that dies after the server sign fails the draft forever with "locked"
- **Expected:** INSP-46: every sync step is idempotent and a dead battery mid-sync "resumes cleanly" (A:data/inspection/InspectionSync.kt:38-42; A:data/inspection/InspectionSyncWorker.kt:22-25).
- **Actual:** If the process died between `signInspection` succeeding and the local draft being deleted (InspectionSync.kt:225-230), the next sync hit 422 `locked` at the first mutating step and recorded "a signed inspection cannot be changed…" as a permanent draft error (InspectionSync.kt:231-237 before the fix).
- **Reproduction:** Android: sign a walk, and kill the app right as the sync finishes (or lose the response to `/sign`). On the next start the draft shows the "locked" error and "aláírva, feltöltésre vár" forever. The server record is fine.
- **Impact:** A stale error on a completed handover. Users may retry or discard, and are confused about whether the record reached the server.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `a draft already signed on the server finishes as done` in InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:data/inspection/InspectionSync.kt: on a `locked` rule error the sync checks the server row. If it is signed, the draft is deleted and the result is `Done`.
- **Journey:** Inspection & media

<a id="media-l3"></a>

### MEDIA-L3: Every system-camera photo leaves an orphan copy in the queue directory
- **Expected:** MEDIA-04 says the captured file is moved, not copied: "the camera writes straight into app-private storage" (A:data/upload/UploadQueue.kt:44-52). A:data/db/PendingUploadDao.kt:119-121 says "a file without a row is orphaned bytes".
- **Actual:** `cameraResult` passed the camera's content URI to `enqueueFromUri`, which copies the bytes into a staged file and then into the queue. The camera target `files/pending/camera-*.jpg` was never deleted (A:ui/photos/OrderPhotoSection.kt:121-128 before the fix; UploadQueue.kt:119-136).
- **Reproduction:** Android: order → "Kamera" → take a photo → let it upload. `files/pending/` still holds `camera-<ts>.jpg` after the queued copy is swept.
- **Impact:** A full-resolution JPEG (several MB) leaks per camera photo, and phone storage fills up over time.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `a camera photo leaves no orphan copy in the queue directory` in InspectionLogicTest.kt (failed before the fix, passes after).
- **Fix:** A:ui/photos/OrderPhotoSection.kt `cameraResult` now calls `queue.enqueue(file, …)`, which moves the file, and shows the same queued/duplicate message.
- **Journey:** Inspection & media

<a id="inv-l3"></a>

### INV-L3: "Sztornó" is still offered while a storno of that invoice is being reported, and a second click fails with a generic "duplicate" error
- **Expected:** INV-90 / INV-93: an invoice has at most one live (non-rejected) storno (`backend/migrations/0021_invoicing.sql:75-79`). A refusal is a named rule error (`not_stornoable`, 422, `backend/src/api/invoices.rs:143`).
- **Actual:** the original stays `issued` until NAV stores the storno (`backend/src/service/invoicing.rs:783-786`). So the web keeps showing "Sztornó" on it (`InvoicesSection.tsx:246` shows it for any issued invoice), and the backend's only guard was the status check (`backend/src/service/invoicing.rs:506`). A second storno draws a number and hits the unique index, and the user sees `409 duplicate` "a record with these values already exists (invoices_one_storno_per_invoice)" (`backend/src/error.rs:115-122`).
- **Reproduction:** on an issued invoice, click "Sztornó", confirm, then click "Sztornó" again on the same invoice while the storno row is still "Bejelentés folyamatban".
- **Impact:** a confusing technical error on a legal document flow. It invites a retry or a support call. No duplicate reaches NAV (the index holds).
- **Severity:** low
- **Confidence:** proven (web); likely (backend, no DB here)
- **Proof:** `frontend/src/test/logicfix-invoicing.test.tsx`: "does not offer a storno for an invoice whose storno is already being reported" (failed before, passes after). Control: "offers a storno once the earlier storno attempt was rejected".
- **Fix:** web hides "Sztornó" on an invoice that has a non-rejected storno (`InvoicesSection.tsx`). Backend `create_storno` now refuses with `not_stornoable` "a storno of {number} already exists ({storno number}, {status})" before drawing a number (`backend/src/service/invoicing.rs`, pure helper `blocks_another_storno` with 3 unit tests: `a_storno_being_reported_blocks_another_storno_of_the_same_invoice`, `a_rejected_storno_attempt_does_not_block_the_next_one`, `only_stornos_of_this_invoice_count`).
- **Journey:** Invoicing & money

<a id="inv-l4"></a>

### INV-L4: proforma payment due date printed as a raw ISO string
- **Expected:** every date on the web goes through `<DateDisplay>` (`docs/history/FRONTEND_PLAN.md:81`, M3). The invoice rows follow this (`InvoicesSection.tsx:229`).
- **Actual:** `ProformasSection.tsx:97-100` printed `{proforma.payment_date}` verbatim, e.g. "Fizetési határidő: 2026-09-29", next to the issue date rendered as "2026. 09. 21.".
- **Reproduction:** Order → Számlák → Díjbekérők: compare the issue date and the payment due date on a row.
- **Impact:** inconsistent date format on a payment-request screen. Cosmetic, but it is the date the customer must pay by.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-invoicing.test.tsx`: "renders the payment due date like every other date".
- **Fix:** `frontend/src/components/orders/ProformasSection.tsx`: the due date renders through `<DateDisplay>`.
- **Journey:** Invoicing & money

<a id="mail-l3"></a>

### MAIL-L3: Template validation lists the same unknown variable several times
- **Expected:** MAIL-38: a template edit with unknown variables is refused with 422 "unknown template variables: …" (`backend/src/api/email.rs:209-220`). The existing test `unknown_variables_are_flagged_not_evaluated` (`backend/src/domain/template.rs:369-371`) expects each unknown name to appear once.
- **Actual:** `unknown_variables` used `Vec::dedup`, which only removes adjacent repeats (`backend/src/domain/template.rs:102` before the fix). `check_variables` also concatenated the subject and body lists without deduplicating. `{{nope}} {{x}} {{nope}}` produced "nope, x, nope".
- **Reproduction:** `PATCH /email-templates/{id}` with body "{{nope}} {{order.number}} {{nope}}" and subject "{{nope}}" → the message lists `nope` three times.
- **Impact:** A confusing error message only. No wrong data.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `domain::template::tests::unknown_variables_are_listed_once_each` in `backend/src/domain/template.rs` (failed before, passes after)
- **Fix:** `backend/src/domain/template.rs`: first-seen-order dedup over all names. `backend/src/api/email.rs`: `check_variables` merges the subject and body lists without repeats.
- **Journey:** Email & communications

<a id="imp-l1"></a>

### IMP-L1: MiniCRM timestamps inside the spring-forward DST hour were dropped
- **Expected:** REP-11 / TIME-01: an order's valuation date is its creation day in Budapest, never "today" at report or load time (`docs/DECISIONS.md:25-28`; `backend/src/migration/load.rs:925-931`).
- **Actual:** `parse_timestamp` used `tz.from_local_datetime(..).earliest()` (`backend/src/migration/load.rs:316-320`), which returns `None` for local times that don't exist. So a `CreatedAt` like `2019-03-31 02:30:00` counted as "no parseable CreatedAt":
  - the valuation date became the load day;
  - `created_at` was lost, which shifts the workload "placed" day;
  - `StatusUpdatedAt` and to-do dates were affected the same way.
- **Reproduction:** migrate a MiniCRM project whose `CreatedAt` falls between 02:00 and 03:00 on the last Sunday of March. The load report shows "valuation date set to today", and reports put the order in the wrong month.
- **Impact:** historical orders counted in the wrong month or year in volume reports, and wrong FX normalisation.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `migration::load::tests::timestamps_in_the_spring_forward_gap_still_parse_to_that_day` in `backend/src/migration/load.rs`. It failed before the fix ("gap time must parse") and passes after.
- **Fix:** `backend/src/migration/load.rs`: a time inside the DST gap is now read as the clock after the jump (+1 hour), so 02:30 local becomes 01:30 UTC on the same day.
- **Journey:** Reports, search & time

<a id="time-l1"></a>

### TIME-L1: Android Reports window used the phone's time zone for "today"
- **Expected:** TIME-01: "today" is the Europe/Budapest calendar day (`backend/src/service/mod.rs:15-17`). The web's workload window says a non-Budapest "today" queries the wrong window around midnight (`frontend/src/components/reports/WorkloadCharts.tsx:46-48`).
- **Actual:** `ReportsViewModel.load` used `LocalDate.now()` in the device zone (`android/.../ui/reports/ReportsScreen.kt:71-72`, before the fix). This is not listed under A5, which says Android displays in Budapest time.
- **Reproduction:** set the phone to UTC (or be abroad) and open Jelentések between 00:00 and 02:00 Budapest time. "Utolsó 30 nap" ends yesterday, so today's placed and completed orders are missing, and the totals differ from the web's month range.
- **Impact:** the phone and the web show different numbers for the same window.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `hu.autotherm.autocrm.ReportsWindowTest.lastThirtyDaysEndOnTheBudapestCalendarDayNotTheDeviceDay` (`android/app/src/test/java/hu/autotherm/autocrm/ReportsWindowTest.kt`). It passes after the fix. I couldn't run it before the fix: another agent's broken `SessionExpiryTest.kt` and concurrent Gradle builds blocked compilation. As a substitute, jshell showed the old logic gives `2026-09-22` in UTC where Budapest is `2026-09-23`.
- **Fix:** `ReportsScreen.kt`: moved the window into `workloadWindow(now)`, which uses `ZoneId.of("Europe/Budapest")`.
- **Journey:** Reports, search & time

<a id="sales-l5"></a>

### SALES-L5: Android offers "Fázisváltás" on a converted lead
- **Expected:** SALES-44: once a lead has an order, any stage change is refused with 422 `lead_converted` (backend/src/service/stages.rs:191-201; docs/DECISIONS.md:61). The web shows the stage button only while `orders.length === 0` (frontend/src/app/[locale]/leads/[id]/page.tsx:147-156).
- **Actual:** Android shows the button for every editable lead (android/.../ui/leads/LeadScreens.kt:385-397 before the fix). Transitions still list reopen targets (docs/history/REMEDIATION.md:57), so the user can pick one, write a note and only then gets refused.
- **Reproduction:** Phone → a converted lead (the Megrendelések section is not empty) → Fázisváltás → choose "Elveszett" → Mentés → error "this lead was converted to order …".
- **Impact:** A dead-end action and inconsistent behavior between clients. No data damage, because the backend refuses.
- **Severity:** low
- **Confidence:** proven† (compile-only). proven (the test did not compile before the fix)
- **Proof:** SalesLeadConvertTest.kt `convertedLeadOffersNoStageChange`
- **Fix:** LeadScreens.kt: `canChangeLeadStage(orders)` hides the button once the lead has orders. The Convert button stays, because repeat conversion is allowed (V2.7).
- **Journey:** Sales pipeline

<a id="ord-l6"></a>

### ORD-L6: The intake-slip gate is enforced but the phone cannot record the slip
- **Expected:**
  - ORD-44: leaving `intake` requires `mileage_in` (backend/src/service/stages.rs:131-145).
  - ORD-31: the intake slip is where that error points (frontend/src/components/orders/IntakeSlipSection.tsx:3-5).
  - ORD-05: office staff and designers change stages on Android.
- **Actual:**
  - Android has no mileage, fuel, key or valuables field anywhere: no Kotlin file mentions `mileage`, and `OrderBody` has no intake fields (android/.../data/api/Dto.kt:533-545).
  - A fitter moving an order out of Átvétel gets the untranslated backend text "leaving intake requires the intake slip (mileage_in); record it on the order first" (android/.../ui/common/Errors.kt:30 shows `e.detail`) and cannot fix it on the phone.
- **Reproduction:** Phone → open a new order (in Átvétel) → Fázisváltás → Tervezés. An English error appears, and no screen offers the slip.
- **Impact:** Work stalls on the phone until someone at a desk records the mileage.
- **Severity:** low
- **Confidence:** proven†
- **Proof:** `IntakeSlipJsonTest` (`android/.../IntakeSlipJsonTest.kt`): full slip, explicit nulls on cleared fields, nulls surviving the wire serialisation. Fail to compile before the fix (no builder); all pass after (61 Android tests green).
- **Fix:** the phone now has the slip: an "Átvételi lap" card on the order detail shows the recorded values (or that none are recorded) with a Rögzítés/Szerkesztés action for `canEdit` users, opening `IntakeSlipDialog` (mileage required, fuel gauge chips E–F, keys, condition, valuables checkbox+text with the web's always-definite semantics). Saving PATCHes through the explicit-nulls path (ORD-L4), so cleared fields clear. The `Order` DTO carries the six slip fields. Designers remain unable to record it: that needs `EditOrders` (admin/office) server-side, which is the role matrix as documented (see Q-ORD-2).
- **Journey:** Order lifecycle

<a id="insp-l8"></a>

### INSP-L8: An optional zone (roof) cannot be skipped in the walkaround
- **Expected:** INSP-16/INSP-18: `roof` is optional ("Tető (ha elérhető)"), and sign-off requires overviews only of non-optional zones (backend/migrations/0026_inspections.sql:132-141; A:ui/inspection/WalkaroundViewModel.kt:506-510).
- **Actual:** Without an overview the zone step offers only "Fotó készítése". "Nem" / next-zone appears only after an overview exists (A:ui/inspection/WalkaroundScreen.kt:328-383 before the fix). An unreachable roof had to be "photographed" to continue.
- **Reproduction:** Android walkaround → zone 9/15 "Tető": cancel the camera. There is no way forward except taking a photo.
- **Impact:** A meaningless photo is forced into the evidence record, and the optional flag has no effect.
- **Severity:** low
- **Confidence:** likely (Compose UI, no unit-test harness)
- **Proof:** none.
- **Fix:** A:ui/inspection/WalkaroundScreen.kt: optional zones without an overview show a "Kihagyás" button that advances to the next zone.
- **Journey:** Inspection & media

<a id="inv-l7"></a>

### INV-L7: retries of a technical annulment are never recorded on the invoice
- **Expected:** INV-104 / INV-65: while the queue retries, the attempt is noted so "nothing is happening" is never the whole story (`backend/src/service/invoicing.rs:752-755`).
- **Actual:** `annul_job` reused `note_attempt`, whose SQL is `… WHERE id = $1 AND status = 'submitting'` (`backend/src/service/invoicing.rs:756-765`). An invoice being annulled is always `issued` or `stornoed` (`:600-603`), so the update matched nothing, and a sidecar outage during annulment left no trace on the row.
- **Reproduction:** as admin, request "Technikai érvénytelenítés" while the sidecar is down. The row keeps showing "Kiállítva" with no message for the whole retry period (up to 5 attempts with backoff).
- **Impact:** the admin cannot tell whether the annulment was filed.
- **Severity:** low
- **Confidence:** likely (SQL-level; no DB here)
- **Proof:** none executable here. The status filter is visible at `backend/src/service/invoicing.rs:758`.
- **Fix:** `backend/src/service/invoicing.rs`: the annul retry branch writes `nav_message = "annulment not yet filed: …"` on any non-annulled row, using a runtime `sqlx::query` (no new offline query cache entry). The web shows it (INV-L1) and hides it once the row is `annulled`.
- **Journey:** Invoicing & money

<a id="mail-l5"></a>

### MAIL-L5: Web newsletter send discards the server's recipient count; the pre-send count includes suppressed addresses
- **Expected:** NEWS-05: "The answer says how many addresses made the list, so the office knows what 'sent' meant" (`backend/src/api/newsletter.rs:86-92`). NEWS-02: suppressed addresses never make the BCC list (`backend/src/service/email.rs:835-841`).
- **Actual:** `ComposeForm` returns only `sent.email_id` and drops `recipients` (`frontend/src/components/email/ComposeForm.tsx:218-227`). The page shows the generic "queued" toast (`frontend/src/app/[locale]/emails/new/page.tsx:66-69`). The only count the office ever sees is "{count} feliratkozó kapja meg BCC-ben", computed client-side from every non-unsubscribed row, suppressed addresses included (`ComposeForm.tsx:147-151, 267-270`). The detail page shows the company's own mailbox as "Címzett" and no BCC.
- **Reproduction:** Subscribe 3 addresses and suppress 1 of them. Compose a newsletter: the form says "3 feliratkozó". Send it: the toast says only "queued", while the API answered `recipients: 2`.
- **Impact:** The office overestimates the audience and cannot confirm what was sent.
- **Severity:** low
- **Confidence:** proven
- **Proof:** `frontend/src/test/logicfix-newsletter.test.tsx`: "excludes unsubscribed and suppressed addresses from the pre-send count" (old code showed 3, not 2), "counts every active subscriber when nothing is suppressed", "hands the server recipient count to onSent instead of just the email id" (old `onSent` received no count). All fail before, pass after.
- **Fix:** `ComposeForm` reads the suppression list (`GET /email-suppressions`, already `SendEmail`-gated) and excludes suppressed addresses (case-insensitive) from the audience count; `newsletterApi.send`'s `recipients` answer is passed to `onSent` and toasted as "Hírlevél sorba állítva, {count} címzettnek elküldve." (new `newsletterQueuedNote` string).
- **Journey:** Email & communications

## Appendix: needs decision (not acted on)

These are rules that rest only on an "assumed" source, or on an open question in [00-questions.md](00-questions.md). None of them was changed in the code.

### Auth & access

Not acted on: these come from spec questions (spec/auth.md) or assumed-only rules.
- **Q-AUTH-1 (web login error text):** a wrong password on web shows "Nincs bejelentkezve. Kérjük, jelentkezzen be újra." because `errorMessage` resolves `unauthenticated` through the catalogue before the `invalidCredentials` fallback (`frontend/src/app/[locale]/login/page.tsx:55`, `frontend/src/lib/api/errors.ts:86-89`, `frontend/src/messages/hu.json:673`). Recommended: show `t('invalidCredentials')` on a 401 at login. A one-line fix once confirmed.
- **Q-AUTH-2 (lockout on Android):** 429 `too_many_requests` maps to `ApiException.Server` (retryable), and Android checks for a code `account_locked` that the backend never emits (`AutoCrmApi.kt` `errorFor`, `LoginScreen.kt:102`, `ChangePasswordScreen.kt:108`). The user sees the English backend text.
- **Q-AUTH-3 (lockout counter):** after a 15-minute lock expires `failed_logins` is not reset, so every further wrong password re-locks immediately (`backend/src/repo/users.rs:141-154`).
- **Q-AUTH-4 (viewer writes tasks):** `backend/src/api/tasks.rs:1-6` lets any role create, toggle and delete tasks, including other people's. This contradicts "writes need a capability" (`docs/API.md:28`, `backend/src/domain/role.rs:18-19`).
- **Q-AUTH-5:** the rule "new password must differ from current" exists only on Android.
- **Q-AUTH-6:** `/password` has no auth gate and no voluntary-change entry point.
- **Q-AUTH-7:** `wrong_password` and password `validation` messages reach web users in English.
- **Q-AUTH-10:** web login skips the Origin check when no `Origin` header is sent.
- **Q-AUTH-11:** the last-admin guard can race on concurrent demotions (`backend/src/api/users.rs:111-120`, no row lock).
- **Q-AUTH-12:** the `lastAdmin` text mentions only deactivation, not demotion.
- **Q-AUTH-13:** there is no admin per-device revoke, despite the `SessionStore.kt:22` comment.
- **Q-AUTH-15:** a disabled account gets the same 401 as a wrong password.
- **Assumed only:** the web query cache is not cleared on logout, so another user signing in on the same browser within `staleTime` (30 s) could briefly see cached data (`frontend/src/lib/auth/context.tsx:38-45`). `POST /auth/password` does not count wrong current-password attempts toward lockout. `POST /users/{id}/revoke-sessions` returns 200 `{revoked:0}` for a nonexistent user.

### Sales pipeline

Not fixed. Each item is either an open spec question or an assumed-only rule.

- **N1 (Q-SALES-5):** `PATCH /leads/{id}` does not re-check that the contact belongs to the partner, or that the partner exists. Create does (backend/src/service/leads.rs:17-31; api/leads.rs:230-244). This is very likely a bug. It was left alone only because it is an open question in the spec. SALES-L2 closes the web path that produced such pairs.
- **N2 (Q-SALES-1):** Repeat conversion is allowed by the backend (V2.7), but the web hides Convert after the first order (leads/[id]/page.tsx:147-156). docs/API.md:20 and hu.json:678 still name `already_converted`.
- **N3 (Q-SALES-2):** Conversion does not check the lead's stage. A `lost` lead converts to `won` with no reopen note, and a second conversion inserts `won`→`won`.
- **N4 (Q-SALES-3, rest):** Should the conversion currency come from `lead.currency` (the quote) before the partner default? Should the quoted value become a line item?
- **N5 (Q-SALES-4):** Lead documents (the quotation PDF) are not visible from, or moved to, the order after conversion.
- **N6 (Q-SALES-6):** Leads accept an archived partner, while orders refuse one.
- **N7 (Q-SALES-7, Q-SALES-8):** Customer pickers do not filter out suppliers. The web cannot set `role` at all. Android silently classifies an unclassified partner as `customer` on any save (PartnerEditScreen.kt:42,64,97), against "Set deliberately" (0011:33).
- **N8 (Q-SALES-9, SALES-06 assumed):** The Hungarian tax number is checked for 11 digits only: no VAT-code digit (1–5) and no check digit.
- **N9 (Q-SALES-10):** No duplicate detection for partners (for example the same tax number twice).
- **N10 (Q-SALES-11):** `PATCH /documents/{id}` is a full replace (repo/documents.rs:274-282), the same class of bug as SALES-L3 under docs/API.md:27. It is an open question in the spec, so it was left alone.
- **N11 (Q-SALES-12):** `invoice`/`proforma` kinds can be uploaded through `/leads/{id}/uploads` and `/orders/{id}/uploads`.
- **N12 (Q-SALES-13, Q-SALES-16):** There is no web UI to upload lead documents. The web partner detail does not render `PartnerDetail.leads`. docs/API.md sales rows are stale.
- **N13 (Q-SALES-14 = integration-audit A5):** Android's "quote expired" uses the device date, not Europe/Budapest.
- **N14 (Q-SALES-15):** Sending a quotation does not move the lead to `quoted`.
- **N15 (Q-SALES-17):** `POST /leads/{id}/convert` ignores `spec` and the project type's build-spec requirement.

### Order lifecycle

These rules are assumed-only or are open questions from the spec. None was acted on.
- Q-ORD-1: which client produces `completion` photos. Android attaches `production` only (CapturePrefs.kt:90-95) and the web has no upload UI, so neither client alone can satisfy the MEO gate, nor (after ORD-L1) a reopen to `completed`.
- Q-ORD-2: a designer can change stages but cannot record `mileage_in` (EditOrders is admin/office), so a designer can never take an order out of `intake`.
- Q-ORD-3: `/transitions` does not reflect the intake-slip gate (`gates_met` covers image gates only), so both clients offer moves that then fail with 422 `intake_slip_missing`.
- Q-ORD-4: the intake-slip gate also blocks `intake` → `cancelled`, although exits are documented as "without gates" (DECISIONS.md:57).
- Q-ORD-5: `completed` → `cancelled` is classed as a reopen that needs a note. ORD-L1 keeps exit targets ungated; whether the move should be allowed at all is open.
- Q-ORD-6: the web intake slip always sends `valuables_declared` as true/false (deliberate, IntakeSlipSection.tsx:40-41,63-65), so NULL ("not asked") is lost once the slip is saved.
- Q-ORD-7: migration 0009 says "no to-do feature", while 0017 adds tasks.
- Q-ORD-8: any user can delete or toggle anyone's task. The web deletes only open tasks and Android deletes done tasks too. Marking an already-done task done again overwrites `done_at`.
- Q-ORD-9: capture-first photos (picker in the working tree) versus job-first photos (commit 701a0ec).
- Q-ORD-10: there is no stage-based lock on items, spec or slip for terminal orders, and completed orders never leave the board's `completed` set.
- Q-ORD-11: cancelling or completing an order leaves its blockers open and still nudged.
- Q-ORD-12: `intake_slip_missing` (422) and `not_resolved` (409) are missing from the docs/API.md error table. `not_resolved` has no web catalogue entry and falls back to the backend text.
- Q-ORD-13: the web `note_required` text mentions only backward moves, although it also covers reopen. Reopen sends no customer mail.
- ORD-25 (assumed): line items can be edited in any stage. Not acted on.

### Inspection & media

- Q-INSP-1: in-app CameraX camera for inspections vs commit 701a0ec "no in-app camera". Out of scope per coordinator; not touched.
- Q-INSP-2 / MEDIA-12: a 401 mid-upload marks rows `blocked` ("a munkamenet lejárt…"), while UploadWorker.kt:34-36 says rows stay and login re-enqueues. Nothing re-enqueues the worker on login (only app start and capture). Pause-and-resume vs block is a product decision.
- Q-INSP-4: images and signature documents attached to a signed inspection can still be soft-deleted with `DeleteMedia` (backend/src/api/media.rs:217-247). Whether the inspection lock extends to its media is a legal/evidence decision.
- Q-INSP-5: dedupe on `(order_id, sha256)` ignores category, so bytes already on the order as another category cannot become an inspection photo (attach answers "image is not an inspection photo").
- Q-INSP-6: meaning of template `required` vs `optional`. The web editor always sends `required: true` and edits only the `default` set (frontend/src/components/inspections/ZoneTemplates.tsx:43, 63-72), although the migration says both sets are edited from web settings (0026_inspections.sql:110-112).
- Q-INSP-7: who owns verdicts (web vs phone; the phone's sync upserts over web verdicts). Only the locked-record and link-consistency parts were fixed (INSP-L1, INSP-L2).
- Q-INSP-8: notes are accepted on drafts, while the migration describes them as post-lock annotations.
- Q-INSP-9: several draft check-ins per order are allowed (the unique index covers draft check-outs only).
- Q-INSP-10: phone vs server choice of the linked check-out when a newer check-out is signed between walk and sync.
- Q-INSP-11: a `preexisting` verdict with `checkout_damage_id = null` is accepted, and one check-out damage may match several check-in damages.
- Q-INSP-14: the Android queue names every file `.jpg` / `image/jpeg` fallback even for PNG/WEBP/HEIC gallery picks; docs/API.md:107 omits the `inspection` category.
- Q-INSP-15: the phone does not check server length limits (note ≤ 2000, driver ≤ 200, location ≤ 300, fuel ≤ 16), so overflows fail only at sync.
- `add_signature` does not check that the document is an image/PNG of kind `other` (backend/src/api/inspections.rs:586-592). The rule is only implied by comments; not acted on.
- Ticket single-use: tickets are replayable within their 2 h TTL, but complete is idempotent (MEDIA-27). Documented as stateless; no change.

### Invoicing & money

Not acted on: these rules are only "assumed" (law from memory), or they are open spec questions.

- **Q-INV-3 / INV-44: which date sets the MNB rate.** Code: issue date (`backend/src/service/invoicing.rs:395,1237`). Áfa tv. §80 (assumed) points to the tax-point/fulfilment date. Legal semantics were not changed.
- **Q-INV-4: FX staleness.** Invoices accept an MNB rate of any age (`backend/src/repo/fx.rs:79-94`), while reports cap it at 10 days (`backend/migrations/0006_reporting.sql:66`). No documented rule for invoices.
- **Q-INV-5: FX rate not snapshotted.** It is looked up at issue and again at submit.
- **Q-INV-6 / Q-INV-7: VAT in HUF on EUR invoices, and whole-forint rounding of HUF invoices.** The brief's "no fillér" rule contradicts `docs/history/FRONTEND_PLAN.md:182-183` and `docs/history/REMEDIATION.md:16`. Unchanged.
- **Q-INV-8: per-line vs per-rate VAT.** The sidecar's returned `totals` are never compared with the stored totals (`backend/src/service/invoicing.rs:767-811`).
- **Q-INV-9: whether a non-NAV failure (local validation, bad credentials) should spend a number.** This blocks INV-L10.
- **Q-INV-10: dead-lettered submissions leave the row `submitting` forever.** Related to INV-L10. Needs a resolution path (admin retry / manual reject).
- **Q-INV-11 / Q-INV-12: annulment of a storno, and portal approval tracking.**
- **Q-INV-13: customer data not snapshotted at issue.** It is read at submit time.
- **Q-INV-14: `payment_method` is unvalidated and unpersisted.** An invalid value becomes a sidecar `bad_request` and a rejected invoice.
- **Q-INV-15: more than 100 order items.** The sidecar limit is not checked before a number is drawn.
- **Q-INV-16: customer letters print totals in the log format `1270000.00 HUF`.** The same convention is used by quote letters in `backend/src/service/email.rs:179-182`, so it was left for a product decision.
- **Q-INV-17: no ordering rule between payment date, issue date and delivery date** (Áfa tv. §163 is assumed only).
- **Q-INV-1 / Q-INV-2 / Q-INV-18: scope questions.** Corrective (MODIFY) invoices, `docs/history/FRONTEND_PLAN.md` declaring invoicing out of scope, and an Android invoice view.
- **INV-L8 and INV-L9 design choices.** Reconciliation after a timed-out CREATE, and a re-fetch path for a missing PDF.

### Email & communications

Open questions from the spec. None of these were acted on (full text in scratchpad/spec/email.md, Questions):
- Q-MAIL-1: Office (`SendEmail`) may add suppressions, but role.rs:24 files the suppression list under Admin (`ManageConfiguration`).
- Q-MAIL-2: `GET /newsletter/subscriptions` is readable by any logged-in role (Designer, Viewer), although the module doc says `SendEmail`.
- Q-MAIL-4: The newsletter unsubscribe link carries no token, and the email-typed unsubscribe needs no ownership proof. There is no `List-Unsubscribe` header.
- Q-MAIL-5: No consent record or double opt-in is stored for newsletter subscribers (GDPR Art. 7(1), Grt. 6. §), and office adds need no consent.
- Q-MAIL-6: An office add silently resubscribes someone who opted out (`repo/newsletter.rs:264`).
- Q-MAIL-7: Should a newsletter unsubscribe also add a suppression?
- Q-MAIL-8: `DELETE /newsletter/subscriptions/{id}` hard-deletes, contradicting "the address must stay known".
- Q-MAIL-9 / Q-MAIL-10: The email attempt budget is not reset by a manual retry: an email that gave up after 5 attempts gets one more try.
- Q-MAIL-11: Hitting the per-recipient daily limit cancels invoice or pickup letters instead of deferring them.
- Q-MAIL-12: Invoice letters queue regardless of the kill switch and are then cancelled at delivery.
- Q-MAIL-13: The "automatikusan küldte" footer also appears on manual, quotation and newsletter letters.
- Q-MAIL-14: The `lead_acknowledgement` template is unused (E4).
- Q-MAIL-15: The stored SMTP password survives clearing the username.
- Q-MAIL-16: The web UI hides Cancel on failed and needs-review mail, although the backend allows it.
- Q-MAIL-17: The quotation dialog says there is no address while the backend falls back to the partner's email.
- Q-MAIL-18: The Android order compose pre-fills the partner's email instead of the contact's.
- Q-MAIL-19: The newsletter preview has no footer, is addressed to a hard-coded iroda@autotherm.hu, and resolves variables instead of refusing them.
- Q-MAIL-20: Automatic mail cancelled by the rails is not surfaced anywhere.
- Other: a re-entry into `completed` (reopen or backward move) sends a second "ready for pickup" letter (`backend/src/service/stages.rs:159-160`). Whether that is intended depends on stage configuration. This belongs to the stages journey, so it was not touched.
- Other: `add_suppression` re-adding an address without a reason wipes the stored reason (`backend/src/repo/emails.rs:345`). There is no spec source for this.

### Reports, search & time

1. **What "Elkészült" counts in workload.** Workload `completed` includes cancelled (exit) orders: `api/reports.rs:286` says "terminal stage", but throughput excludes exit stages for "completed" (`repo/reports.rs:173`), and both web and Android label it "Elkészült".
2. **Expired quotes in "Lejáró ajánlat".** The dashboard tile includes quotes that have already expired (`DashboardView.tsx:102-104`), but the label means "expiring" (`hu.json:786`).
3. **Phone matching in global search.** Any query containing digits also matches phone numbers (`repo/search.rs:96,124`). Searching plate "ABC-123" fills the partner and lead groups with every phone containing "123". No source says whether that's intended.
4. **Accents in search.** Searching without accents doesn't match accented names ("muller" does not find "Müller"). No source asks for it; ILIKE only ignores case.
5. **Stable paging.** PAGE-04 is assumed only. The orders list has an `o.id` tie-breaker; I didn't audit the other lists.
6. **Migration re-runs.** `docs/DECISIONS.md:124-126` and `docs/migration/README.md:109-110` say a re-run overwrites migrated fields. `migration/load.rs:706` says a re-run must not blank columns staff have filled in (coalesce).
7. **Migrated order numbers.** `DECISIONS.md:31` says imported orders keep their MiniCRM number; `DECISIONS.md:127` says `MC-{id}`. The code switches on `order_number_field` (`load.rs:90-92`).
8. **Tasks.** `load.rs:1092-1093` says "AutoCRM has no task feature", but `0017_tasks.sql` and the dashboard "my tasks" section exist.
9. **Missing CreatedAt.** When CreatedAt is missing, the migration sets the valuation date to the load day (`load.rs:925-931`), which conflicts with "never today" (`DECISIONS.md:27-28`).
10. **Dashboard tile links.** The Megrekedt tile links to `/reports`, which shows no stalled list on web, and the Nyitott akadály tile links to `/orders`. This conflicts with "every number is a link into the list that owns it" (`DashboardView.tsx:4`). Related to C6 and C2.
11. **Capped dashboard counts.** Dashboard counts stop at the list limit of 100 (see F3).
12. **Workload range limit.** It allows a 63-day window (`to − from ≤ 62`), while the error text says "caps at 62 days" (`api/reports.rs:316`).
13. **Midnight in other time zones.** `utc_bounds` panics if `BUSINESS_TIMEZONE` is set to a zone that skips midnight (`api/reports.rs:61-63`). This can't happen with Europe/Budapest.
