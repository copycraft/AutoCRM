# 04 — Roadmap

Ordered, independently shippable steps. Foundations first so later steps build on it.
Sizes: S (<1 day), M (1–3 days), L (3+ days). Risks: low/medium/high.

## Step 0. Quick fixes (no foundations needed) — S, low risk
- What, four small real bugs, each shippable alone:
  1. Web UTC `todayIso/todayPlus` → Budapest calendar (`F:src/components/dashboard/DashboardView.tsx:23-31`,
     finding A5). Reuse the existing Budapest helpers; add a midnight-boundary test.
  2. Android forced-password-change screen (finding C3): wire `POST /auth/password` (add the
     client wrapper — the endpoint exists, `B:src/api/auth.rs:197`) into a change screen on the
     login flow, so a flagged fitter recovers on-device instead of "use web".
  3. Android 400→`Rule` mapping (`A:data/api/AutoCrmApi.kt:133-144`, finding A4): match the web
     so validation refusals render as rules, not server faults. Add a unit test.
  4. Int/Long width fixes (finding A2): `Blocker.nudgeCount`, `StalledOrder.daysInStage`,
     `EmailMessage.attempts` (and audit `Vehicle.year`, `ZoneTemplate.position` while there)
     → `Long`, matching every other count on the wire.
- Resolves: A5 (symptom), C3, A4 (symptom), A2.
- Notes: deliberately *not* the systemic fixes — those stay in Steps 3–4 (catalog, fixtures)
  so the quick patches don't pretend to be the cure.

## Step 1. Contract hygiene foundation — S, low risk
- What: complete the 7 missing OpenAPI tags (`B:src/api/openapi.rs:23-27`); fix the
  `Image` vs `ImageView` naming drift; document the `0023` migration gap; regenerate clients.
- Resolves: E4 (partly: tags, naming, migration gap).
- Notes: pure additive/rename work; CI (`committed_openapi_document_is_current`,
  `gen:api` diff check) verifies.

## Step 2. Kill or wire the dead: picker decision — S, low risk
- What: decide `GET /mobile/orders` + `OrderPickerScreen` + sticky `currentOrder`: either wire
  the picker into a capture-first photography entry (drawer item) or delete the UI, the
  `PickerOrder` DTO + test pins, and (separately, backend minor) the endpoint.
- Resolves: E1, E2.
- Notes: forces the "who photographs from where" decision that the inspection feature
  left open; do not leave a third state.

## Step 3. Shared error-code catalog — S, low risk
- What: publish the backend code list (generated from `B:src/error.rs` into the OpenAPI doc
  or `docs/error-codes.md`); enumerate handled codes per client instead of ad-hoc
  (`stage_gate` pattern).
- Resolves: A4 (systemic), F2 (foundation).
- Notes: the Step-0 400-mapping fix already aligns the plumbing; this step aligns the words.

## Step 4. Shared domain fixtures (money/dates/phones/plates) — M, low risk
- What: add `fixtures/domain-vectors.json` (input→expected for money formatting/parsing,
  Budapest day boundaries, HU phone normalization, plate normalization); consume it in Rust
  unit tests, vitest, and JVM tests.
- Resolves: A1, A5 (recurrence-proof), F1.
- Notes: the highest-value insurance in the plan; files only, no UI. (The Step-0
  `todayIso` patch stops the bleeding; this stops the next one.)

## Step 5. Dead-code sweep + board stage keys — S, low risk
- What: delete unused composables/keys/imports (E4), dead `comparison` field + re-check
  `inspectionComparison()` usage (E3), duplicated `putBytes` merge (B3), `statusTone` +
  search-mapping dedup (B4), canonicalize query keys with a lint rule (B5),
  `order_specs::delete` (wire or delete), `lead_acknowledgement` (wire or delete),
  `images.vehicle_id` (wire or drop column). Plus: replace the hardcoded
  `WORK_STAGES = ['design','production','meo']` (`F:src/components/board/PickupBoard.tsx:19`)
  with the `is_terminal`/`is_exit` flags the stage endpoint already serves (B2, key half).
- Resolves: E3, E4, B3, B4, B5, B2 (keys), C5 (column decision).
- Notes: mechanical; keep each deletion its own commit for easy revert.

## Step 6. Backend gaps + server-provided UI facts — M, medium risk
- What: pagination totals (or documented cursor), contacts unarchive, audit reads for
  lead/partner, server-side email attention flag. Plus two evaluated B1/B2 outcomes:
  - **B1 verdict: yes, serve capabilities.** Add optional `capabilities: string[]` to the
    `SessionUser` shape in `GET /auth/me`, computed server-side from `Role::can`
    (`B:src/domain/role.rs:45`). Optional (not required) so old clients keep working during
    rollout; clients prefer it when present and fall back to their mirrors otherwise, then
    delete the mirrors (`F:src/lib/auth/context.tsx:70`, `A:data/auth/SessionStore.kt:54`).
    No new endpoint, no extra round trip — evaluated over a dedicated capabilities endpoint
    and rejected (same data, more calls).
  - **B2 verdict: half served, half glossary.** Stage tone needs no backend change — the
    flags are already in `StageDefinition` (Step 5 fixes the board to use them). Damage-type
    and severity labels are *not* served anywhere, so add `GET /inspections/meta`
    (`{damage_types[], severities[], purposes[]}` with HU labels); both clients consume it
    instead of their local maps (`F:InspectionSection.tsx:25-41`,
    `A:data/inspection/DraftModels.kt:88-130`). Offline walkaround keeps a bundled copy.
- Resolves: F3, C11, C6 (partly), B1, B2 (labels), unblocks C4 attention list.
- Notes: touches hot list queries — index the count queries; keep the 200-cap.

## Step 7. Feature parity round (biggest user-visible win) — L, medium risk
- What: web order image/document gallery reusing `list_images`/`downloadDocument` (resolves C1);
  full web blockers page reusing `blockersApi.*` replacing the stub (resolves C2); link both
  from order detail and the dashboard attention widget. Android global search screen reusing
  `GET /search` (resolves C9 — orders/leads/partners in one place, directory stays the
  people view). Reports coherence (C6): wire `volume/stage-durations/throughput/blocker-load/
  fx-rates` into the web reports page and Android totals, or explicitly drop individual
  endpoints from the served surface with a deprecation note.
- Resolves: C1, C2, C9, C6.
- Notes: read-mostly UI over existing endpoints; needs the `images`/`documents` query keys
  (currently unused) canonicalized under `qk`.

## Step 8. Android codegen for DTOs — L, medium-high risk
- What: generate Kotlin DTOs from `openapi.json` replacing hand-written `Dto.kt`, per tag,
  old and new models coexisting (see migration below); promote contract tests from
  presence to type-level (unknown-field strictness opt-in, starting with inspections).
- Resolves: A3 (fully).
- Notes: do AFTER steps 0–4 so the generator input is clean.

### Why L, not M: honest findings from checking the generator (openapi-generator 7.x, `kotlin` generator docs + issues #18167, #19928, #20501)
- `generateOneOfAnyOfWrappers` (the flag that turns `oneOf` like our `Completed` into a typed
  wrapper) is only supported for the **`jvm-retrofit2`** library — not for okhttp, ktor, or
  multiplatform. The app uses hand-rolled OkHttp, so generated *models* must be paired with the
  existing hand-rolled `AutoCrmApi`, or the HTTP layer gets rewritten too (rejected: out of scope).
- Without the wrappers flag, `oneOf` degrades to weak typing (nullable-everything/`Any`) and
  there are open upstream bugs around Kotlin polymorphism/discriminators and duplicate
  `@Serializable` annotations. Our `Completed` (image|document arms) is exactly the construct
  that breaks. It stays hand-written until generated output proves itself against both arms.
- Our spec is OAS 3.1 (`type: [integer, null]`, `4XX/5XX` wildcards): supported via
  `NORMALIZE_31SPEC`, but 3.1 normalization has its own fix history — verify, don't assume.
- Dates must stay `String` (`dateLibrary=string`) to match every current DTO and the
  `DateField`/`formatDate` pipeline; enum-as-`String` likewise (no Kotlin enums for
  `stage_key`/`kind`/`status`, or every client `when` needs an `else` audit).

### Incremental migration (per tag, old + new coexist)
1. **Spike (S, part of this step):** pin a generator version, generate into a scratch module
   (`serializationLibrary=kotlinx_serialization`, `dateLibrary=string`, `collectionType=list`,
   `explicitApi=true`), and decode-check every `oneOf`/`anyOf` schema in our spec
   (`Completed`, plus any arms under `Items_*`) against recorded payloads for *all* arms.
   Go/no-go on the spike before touching app code.
2. **Tag by tag, models only:** generate once for the whole spec into `data/api/gen/`, but
   switch imports one tag at a time (`tasks`+`reports` first — small, stable, well-tested;
   `inspections` last — largest surface). Hand-written and generated models coexist;
   `AutoCrmApi` signatures change per swapped tag. Each tag swap ships independently with
   its tests green.
3. **Keep hand-written:** `Completed` (until the spike proves both arms), `DraftPayload`
   (local-only, not a wire shape), and the PATCH-omit semantics — pin
   `explicitNulls=false`-equivalent behavior in generator config and prove it with the
   existing `UploadResponseTest`/`UploadQueueTest` suite plus a new PATCH-omit test.
4. **CI:** pin the generator version in CI, regenerate + `git diff --exit-code` exactly like
   the web `gen:api` drift check; forbid new hand-written wire DTOs after cutover.

## Step 9. Admin console (web) — M, medium risk
- What: one surface for users, stages, project types, zone templates (move existing editor),
  email templates/suppressions, jobs/FX/status, sessions (resolves C12 — lost-phone revoke
  and own-session list finally have UI). Reuses existing endpoints + step-6 totals.
- Resolves: C8, C7 (partly: templates/suppressions manageable), C4 (assignee picker unblocked
  by users UI), C12.
- Notes: admin-only routes already gated server-side; web needs a role gate on the page
  (settings page currently has none).

## Step 10. Glossary + strings + tokens + docs refresh — S, low risk
- What: `docs/glossary.md` + Android `strings.xml` migration for product terms (damage types,
  verdicts, statuses — now referencing the Step-6 `meta` endpoint as canonical, glossary as
  the human-readable mirror); `docs/design-tokens.md` + explicit dark-mode-on-web decision;
  refresh `AUDIT.md`/`VIABILITY.md`/`README.md`/READMEs that contradict the tree;
  document "web is online-only" (D3) and the deliberate asymmetries (03, "stays split").
- Resolves: D4, D5, B2 (human mirror), D3 (documented), contracts-agent §8.7/8.9, docs-agent findings.
- Notes: do last so it documents what was actually built, not what was planned.

## Explicitly out of this roadmap (with reasons)
- Push notifications: no infra, no requirement stated.
- Web offline mutations: rejected asymmetry — office is online, queue semantics belong to the
  shop floor (D3); documented in Step 10 instead of built.
- Second locale: both layers Hungarian by design.
- Shared Rust core via FFI: no evidence any client logic needs native sharing once fixtures +
  codegen exist.
- Vehicles management UI (rest of C5): the column decision ships in Step 5, but no demand
  evidences a full vehicle registry — order text fields + document attribution suffice until a
  multi-vehicle workflow asks for more.
- Full email compose on Android (rest of C7): minimal compose stays deliberate per the
  Android README (bodies belong on desktop); templates/suppressions become manageable via
  Step 9 instead.
- Dashboard on Android (C10): stays a web composition; phone keeps lists-first navigation.
- Merged cross-device preferences (D2): theme stays device-local, density/page-size stay
  account-level — different axes, no user value in merging.

## Traceability: every finding → step(s)

| Finding | Step(s) | Finding | Step(s) |
|---|---|---|---|
| A1 money ×3 | Step 4 | C1 phone photos invisible on web | Step 7 |
| A2 Int/Long widths | Step 0 | C2 blockers stub on web | Step 7 |
| A3 contract enforcement gap | Steps 1, 8 | C3 password bricks mobile | Step 0 |
| A4 error-code divergence | Steps 0, 3 | C4 tasks homeless | Steps 6, 9 |
| A5 timezone split-brain | Steps 0, 4 | C5 vehicles half-wired | Step 5 + out (mgmt UI) |
| B1 capability matrix ×3 | Step 6 | C6 reports 2/7 | Step 7 |
| B2 stage keys + damage labels | Steps 5, 6 | C7 email lopsided | Step 9 + out (full Android compose) |
| B3 duplicated PUT | Step 5 | C8 admin surface missing | Step 9 |
| B4 search/status copies | Step 5 | C9 search web-only | Step 7 |
| B5 query-key bypass | Step 5 | C10 dashboard web-only | out (deliberate) |
| C11 order-only audit | Step 6 | C12 sessions UI missing | Step 9 |
| D1 creation asymmetry | Steps 7, 10 | D2 disjoint prefs | out (different axes) |
| D3 offline one-sided | Step 10 (document) | D4 catalogue vs literals | Step 10 |
| D5 tokens mirrored | Step 10 | E1 dead picker | Step 2 |
| E2 write-only sticky order | Step 2 | E3 dead comparison dup | Step 5 |
| E4 unused odds and ends | Steps 1, 5 | F1 no shared vectors | Step 4 |
| F2 validation/message split | Step 3 | F3 pagination totals | Step 6 |
