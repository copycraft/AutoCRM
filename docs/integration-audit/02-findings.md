# 02 — Findings

Every finding involves ≥2 features or layers. Format: title, layers, evidence,
user impact, severity (**critical / high / medium / low**).

Legend: `B:` backend, `F:` frontend, `A:` Android, `O:` openapi.json.

## A. Contract drift (types disagree across layers)

### A1. Money formatting exists in three unlinked implementations — HIGH
- Layers: B + F + A. Evidence: `B:src/domain/money.rs:58-150` (minor-only ops, half-away-from-zero,
  overflow errors) vs `F:src/lib/utils/format.ts:17-76` (`formatMoney`, `parseMajorToMinor`,
  `minorToMajorString`) + lead-local `toMinor/fromMinor` (`F:src/components/forms/LeadForm.tsx:45-53`)
  + item-local `currencySymbol` (`F:src/components/forms/ItemsSection.tsx:26-28`) vs
  `A:util/Format.kt:19-34` (`formatMoney` with HUF/EUR branches).
- Impact: the number the whole project reports on is rendered by three codebases with no shared
  vectors. Past wobble is documented in code (`B:src/domain/money.rs:162-169` "never floats",
  `A:util/Format.kt:11-17` integer-only rewrite). A rounding or HUF-fillér disagreement ships
  contradictory totals on phone vs desktop with no test catching it.

### A2. Scalar widths disagree: Int vs Long vs String numerics — MEDIUM
- Layers: B + A (+F codegen). Evidence: `A:data/api/Dto.kt:159` `Blocker.nudgeCount: Int` and
  `A:data/api/Dto.kt:604` `StalledOrder.daysInStage: Int` vs `StageView.daysInStage: Long`
  (`A:data/api/Dto.kt:126`), money/ids `Long` everywhere else, `EmailMessage.attempts: Int`
  (`A:data/api/Dto.kt:348`); `ItemView.quantity: String` and spec numerics (`targetTempC`,
  `heatOutputKw`) as `String` (`A:data/api/Dto.kt:143,176-193`).
- Impact: same semantics (counts, days) in two widths; quantity/spec strings need per-screen
  parsers (`A:ui/orders/OrderDetailScreen.kt:721`, `A:ui/leads/LeadEditScreen.kt:66`). Overflow
  or parse failure surfaces as phone-only breakage the contract test cannot see (presence-only).

### A3. Contract enforcement is strong on web, weak on Android — HIGH
- Layers: F + A + O. Evidence: every web call validates 2xx against generated zod and fails
  compilation on drift (`F:src/lib/api/endpoints.ts:1-4`, `F:src/lib/api/client.ts:48-60`) while
  Android pins only field *presence* (`A:app/src/test/.../OpenApiContractTest.kt:1-20`). Proven
  escapes: `Completed` oneOf-vs-flatten (`O:#/components/schemas/Completed` vs
  `A:data/api/Dto.kt:332-338`); `Image` vs `ImageView` naming (`O:#/components/schemas/Image`);
  presigned `headers: string[][]` (`O:#/components/schemas/PresignedRequest`) decoded as a Map
  until it broke every upload.
- Impact: wire drift ships silently on mobile and surfaces as "unparseable response" in the
  field instead of a build break.

### A4. Error-code handling diverges by client, no shared catalog — MEDIUM
- Layers: B + F + A. Evidence: backend emits `{error:{code,message}}` with ~40 machine codes
  (`B:src/error.rs:27-160`); web maps via catalogue + `codeKey` + raw passthrough for `validation`
  (`F:src/lib/api/errors.ts:38-41,79-92`); Android maps 400→`Server` instead of `Rule`
  (`A:data/api/AutoCrmApi.kt:133-144`) and rewords everything into HU strings
  (`A:ui/common/Errors.kt:18-32`).
- Impact: the same refusal (e.g. `currency_locked`, `checkout_open`) reads differently per
  platform, and new backend codes render as generic lines on one client but specific ones on
  the other. No single list of codes exists.

### A5. Timezone split-brain between layers — HIGH
- Layers: B + F + A. Evidence: backend canonicalizes on Budapest business day
  (`B:src/service/mod.rs:16-18`); Android displays via Budapest (`A:util/Format.kt:19-60`);
  web mixes Budapest (`F:src/components/ui/DayGroups.tsx:8-39`,
  `F:src/app/[locale]/leads/[id]/page.tsx:32-36`) with UTC (`F:src/components/dashboard/DashboardView.tsx:23-31`
  `todayIso/todayPlus` via `toISOString`).
- Impact: "quotes expiring within 7 days" is computed on two calendars on the same screen
  family — dashboard and lead detail can disagree about what is expiring, near midnight, daily.

## B. Duplicated logic (same rule, N implementations)

### B1. Capability matrix copied three times — MEDIUM
- Layers: B + F + A. Evidence: `B:src/domain/role.rs:45-62` vs `F:src/lib/auth/context.tsx:70-109`
  ("UI-only" comment) vs `A:data/auth/SessionStore.kt:54-56`. Both clients add their own
  extras (web `canSendEmail`, Android `canUploadMedia`).
- Impact: a backend role change silently desyncs both UIs (hidden buttons the API would allow,
  or shown buttons it refuses). Only discipline, no test, keeps them aligned.

### B2. Stage knowledge duplicated; web board hardcodes keys — MEDIUM
- Layers: B + F + A. Evidence: gates computed server-side (`B:src/service/stages.rs:75-110`) but
  tone/position/history logic reimplemented (`F:src/lib/utils/stages.ts:9-13`,
  `F:src/components/ui/StageRail.tsx:27-32`); `F:src/components/board/PickupBoard.tsx:19-45`
  hardcodes `WORK_STAGES = ['design','production','meo']` against the documented no-hardcode rule;
  damage-type/severity HU labels duplicated (`F:src/components/inspections/InspectionSection.tsx:25-41`
  vs `A:data/inspection/DraftModels.kt:88-130`).
- Impact: renaming a stage or adding a damage type updates backend + one client and silently
  ages the other.

### B3. Upload PUT duplicated on Android — LOW
- Layers: A + A. Evidence: `A:data/upload/Uploader.kt:109-134` vs
  `A:data/inspection/InspectionSync.kt:281-304` (same header loop, same 403/5xx→Server mapping;
  the copy drops the response detail from the Rule message).
- Impact: a storage-behavior change (e.g. new signed header) must be fixed twice; error detail
  already differs.

### B4. Search-hit mapping copied three times on web — LOW
- Layers: F. Evidence: `F:src/components/search/GlobalSearch.tsx:71-91` vs
  `F:src/components/search/CommandPalette.tsx:111-131` vs `taskHref`
  (`F:src/components/dashboard/DashboardView.tsx:34-38`); plus `statusTone` pasted across
  `F:src/app/[locale]/emails/page.tsx:30-52` and `F:src/app/[locale]/emails/[id]/page.tsx:24-45`.
- Impact: a new searchable entity or status touches 3–4 places; one will be missed.

### B5. Query keys bypass the canonical registry on web — MEDIUM
- Layers: F. Evidence: `PartnerPicker` (`F:src/components/forms/PartnerPicker.tsx:41-46`),
  `ComposeForm` (`F:src/components/email/ComposeForm.tsx:96-210`), `NewsletterList`
  (`F:src/components/email/NewsletterList.tsx:28-31`), `QuotationDialog`
  (`F:src/components/email/QuotationDialog.tsx:60-65`) use ad-hoc keys; creates invalidate raw
  `['leads']/['orders']/['emails']` (`F:src/app/[locale]/leads/new/page.tsx:49`,
  `F:src/app/[locale]/orders/[id]/page.tsx:184`) instead of `qk.*` (`F:src/lib/query/provider.tsx:33-73`).
- Impact: cache invalidation misses → stale lists after mutations; the registry exists precisely
  to prevent this and is silently bypassed where it matters most (compose, pickers).

## C. Missing connections (user journeys break between features)

### C1. Phone photos are invisible on the web — HIGH
- Layers: B + F + A. Evidence: Android produces images via ticket→PUT→complete
  (`A:data/upload/Uploader.kt:46-102`); backend serves `list_images/original/download`
  (`B:src/api/media.rs:141,184,377`); web calls none of the read paths (all `mediaApi` image fns
  unused) and order detail shows only `image_counts` badges
  (`F:src/app/[locale]/orders/[id]/page.tsx:369-381`).
- Impact: the office cannot see the evidence the shop floor just uploaded without leaving the
  product. The upload pipeline's entire read side is dead on desktop.

### C2. Blockers: full loop on phone, stub on web — HIGH
- Layers: B + F + A. Evidence: `blockersApi.create/patch/resolve/reopen/forOrder`
  (`F:src/lib/api/endpoints.ts:142-153`) never called; `/blockers` renders `UnavailableState`
  (`F:src/app/[locale]/blockers/page.tsx:6-15`); order tab is read-only and links into the stub
  (`F:src/app/[locale]/orders/[id]/page.tsx:546-556`); Android implements the whole loop
  (`A:ui/orders/OrderDetailScreen.kt:293-325`).
- Impact: office users cannot work blockers at all; the phone owns a core workflow alone.

### C3. forced-password change bricks mobile with no path forward — HIGH
- Layers: B + F + A. Evidence: backend 422-blocks everything except me/password/logout
  (`B:src/api/extract.rs:52-64`); web has the change screen (`F:src/app/[locale]/password/page.tsx:11`);
  Android refuses login with "use web" and offers no screen (`A:ui/login/LoginScreen.kt:72-85`).
- Impact: a fitter flagged `must_change_password` (e.g. after admin reset,
  `B:src/api/users.rs:168-194`) cannot self-recover on the device they work with.

### C4. Tasks have no home on either client — MEDIUM
- Layers: B + F + A. Evidence: backend full CRUD (`B:src/api/tasks.rs:38-158`); web embeds lists +
  dashboard widget, no page (`F:src/components/tasks/TaskList.tsx:28`); Android has my-tasks list
  but creates only from order detail (`A:ui/tasks/TasksScreen.kt:60-124`); no notifications,
  no assignment picker for non-admins ("backend gap", `F:src/components/forms/AssigneeField.tsx:43-62`).
- Impact: follow-ups fall through the cracks; the feature exists but has no front door.

### C5. Vehicles: API with no UI and a half-wired column — MEDIUM
- Layers: B + F (+A display). Evidence: 7 vehicle ops (`O:vehicles` tag) with zero web UI
  (no `vehiclesApi` in `F:src/lib/api/endpoints.ts`); `images.vehicle_id` column exists
  (`migrations/0010`) but is absent from the Rust struct (`B:src/repo/images.rs:14-40`) so only
  documents carry vehicle attribution; both clients show vehicle as order text fields.
- Impact: multi-vehicle orders cannot attribute photos per vehicle on any client despite the
  column existing.

### C6. Reports are 2/7 on both clients — MEDIUM
- Layers: B + F + A. Evidence: `volume/stage-durations/throughput/blocker-load/fx-rates`
  (`O:reports` tag) called by nobody; web renders workload charts + stalled widget
  (`F:src/components/reports/WorkloadCharts.tsx:65`); Android renders totals + stalled
  (`A:ui/reports/ReportsScreen.kt:91`).
- Impact: the Monday-meeting numbers exist server-side but neither client shows them; two
  half-reporting surfaces instead of one coherent story.

### C7. Email management lopsided — MEDIUM
- Layers: B + F + A. Evidence: templates/suppressions manageable only via API
  (`B:src/api/email.rs:209,327`); web composes fully but can't manage them
  (`F:endpoints.ts:199-210` unused); Android composes minimally (no templates/attachments,
  `A:ui/emails/EmailComposeScreen.kt:37-41`) and can't download attachments
  (`A:ui/emails/EmailScreens.kt:289-296`).
- Impact: office owns templates via API calls; phone can't finish the correspondence loop.

### C8. Admin surface missing on both clients — MEDIUM
- Layers: B + F + A. Evidence: `admin/status/jobs/retry/run/fx-fetch` (`B:src/api/admin.rs:50-241`);
  web `/admin` is a stub (`F:src/app/[locale]/admin/page.tsx:6-15`, only `testEmail` used);
  Android has nothing; users/stages/project-types/templates management likewise backend-only
  (unused `createStage/patchStage/...` on both clients).
- Impact: every operational action (retry failed job, fetch FX, manage users/stages) requires
  raw API access; the "office" has no office tools.

### C9. Global search is web-only — MEDIUM
- Layers: B + F + A. Evidence: `GET /search` (`B:src/api/search.rs:48`) consumed by sidebar +
  palette (`F:src/components/search/GlobalSearch.tsx:19`, `F:src/components/search/CommandPalette.tsx:46`);
  Android's Névjegyzék searches partners only (`A:ui/directory/DirectoryScreen.kt:175`).
- Impact: phone users can't find orders/leads by plate/title from anywhere; each list has its
  own search instead of one front door.

### C10. Dashboard exists only as web client-side composition — LOW
- Layers: B + F + A. Evidence: no backend endpoint; web fires 6 list queries
  (`F:src/components/dashboard/DashboardView.tsx:64-96`); Android starts at orders, no overview.
- Impact: two "home" philosophies; any dashboard number change needs a web-only edit and can
  never be shared.

### C11. Audit trail readable for orders only — LOW
- Layers: B + F. Evidence: backend records entity `lead/partner/...` (`B:src/repo/audit.rs:8-27`)
  but only `GET /orders/{id}/audit` is served (`B:src/api/orders.rs:875-884`); web renders it
  (`F:src/app/[locale]/orders/[id]/page.tsx:621`); lead/partner changes are unauditable in UI.
- Impact: accountability story covers one entity.

### C12. Session management UI missing everywhere — LOW
- Layers: B + F + A. Evidence: list/revoke-session endpoints (`B:src/api/auth.rs:221,242`;
  `F:endpoints.ts:20-22` unused); neither client shows active sessions; admin revoke exists
  server-side only (`B:src/api/users.rs:206-214`).
- Impact: a lost phone is revocable only via API, and users can't see their own sessions.

## D. Platform asymmetry without justification

### D1. Creation asymmetry: phone writes what web can't read and vice versa — MEDIUM
- Evidence: phone creates blockers/tasks/inspection photos the web can't display or manage
  (C1, C2); web creates quotations, proformas views, and emails with embeds the phone can't
  (`F:src/components/email/ComposeForm.tsx:47`, `A:ui/emails/EmailComposeScreen.kt:37-41`).
- Impact: each device sees a different product; cross-device handoff ("I filed it on the
  phone, check it on desktop") routinely disappoints.

### D2. Customization models are disjoint — LOW
- Evidence: web density/page-size via `PUT /auth/preferences`
  (`F:src/app/[locale]/preferences/page.tsx:16`); Android theme/AMOLED on-device only
  (`A:ui/settings/AppearanceScreen.kt:46`); no shared "my settings" concept.
- Impact: minor, but two preference silos where one account has two faces.

### D3. Offline exists on one platform only — LOW (document; do not "fix" blindly)
- Evidence: Room + two workers on Android (`A:data/db/AutoCrmDatabase.kt:17`,
  `A:data/upload/UploadWorker.kt:33`, `A:data/inspection/InspectionSyncWorker.kt:48`); web has
  a banner (`F:src/components/layout/OfflineBanner.tsx:8`) and no queued mutations.
- Impact: acceptable asymmetry (shop floor vs office), but it is undocumented and the web
  silently drops user intent on disconnect (forms just fail).

### D4. i18n: catalogue vs literals — MEDIUM
- Evidence: web `hu.json` 30 namespaces (`F:src/messages/hu.json`) with hard-coded leaks
  (`Üzleti/Magán` in both search components, `DAMAGE_HU`, `FUEL_LEVELS`, `Morzsamenü`);
  Android 100% hardcoded HU literals; damage-type labels duplicated across layers (B2).
- Impact: no shared glossary; the same term drifts per screen (already true for Ügyfél/Magán
  vs business/person kind labels). A second locale is currently impossible, which is fine —
  inconsistency within the first locale is the real cost.

### D5. Theming tokens are manually mirrored — LOW
- Evidence: `A:ui/theme/Theme.kt:26-69` comments "same palette as web"; web light-only Tailwind
  vs Android light/dark/AMOLED. No shared token file.
- Impact: every palette tweak needs two edits; dark mode exists where the web will never
  follow (deliberate) but nothing records that decision except code comments.

## E. Dead and orphaned pieces

### E1. Order picker: dead UI + live API — MEDIUM
- Evidence: `A:ui/picker/OrderPickerScreen.kt:142` + `OrderPickerViewModel`
  (`:58-140`) + `PickerOrder` (`A:data/api/Dto.kt:67`) + `api.pickerOrders()`
  (`A:data/api/AutoCrmApi.kt:179`) have zero callers (no route in `A:MainActivity.kt:267-572`);
  backend maintains `GET /mobile/orders` (`B:src/api/mobile.rs:51`) incl. 60s cache header.
- Impact: maintained endpoint + tested UI that no user can reach; the capture flow it served
  no longer exists. Wire it (capture-first photography) or delete both sides.

### E2. Sticky capture order is write-only — LOW
- Evidence: `A:data/prefs/CapturePrefs.kt:38-55` written by `A:AutoCrmApp.kt:45-47`
  (`selectOrderForCapture`, zero callers) and the dead picker
  (`A:ui/picker/OrderPickerScreen.kt:123-135`); only `category` is ever read
  (`A:ui/photos/OrderPhotoSection.kt:85-96`).
- Impact: dead prefs surface; collapses with E1.

### E3. Walkaround comparison field + endpoint unused — LOW
- Evidence: `WalkaroundViewModel.State.comparison` never assigned
  (`A:ui/inspection/WalkaroundViewModel.kt:90` vs `:531` branch always taking the
  no-comparison path); `inspectionComparison()` (`A:data/api/AutoCrmApi.kt:507`) unused —
  check-in review re-fetches `inspection(checkoutId)` (`A:ui/inspection/WalkaroundViewModel.kt:444`).
- Impact: dead DTO + endpoint wrapper + a state branch that can never be true; the real flow
  duplicates what the endpoint was built for.

### E4. Unused shared UI has no owner — LOW
- Evidence: `LoadingState`, `RefreshingBar`, `KeyValueRow` (`A:ui/common/Components.kt:221,312,478`)
  zero call sites; `qk.orderBlockers/images/invoice/adminJobs` (`F:src/lib/query/provider.tsx:49-56`)
  unused; `lead_acknowledgement` template seeded but unreferenced (`migrations/0005`);
  `order_specs::delete` (`B:src/repo/order_specs.rs:delete`) uncalled; `images.vehicle_id`
  unwired (C5); `0023` migration gap (undocumented).
- Impact: individually trivial, collectively a map full of false doors for the next reader.

## F. Missing shared core (client logic that belongs further down)

### F1. No shared test vectors for domain formatting — MEDIUM
- Evidence: money/date/phone/plate rules each have per-layer tests (or none) but no common
  fixture. The rules live in `B:src/domain/money.rs`, `B:src/domain/partner.rs:18-54`,
  `F:src/lib/utils/format.ts:17-83`, `F:src/test/*`, `A:util/Format.kt:19-60`.
- Impact: A1/A5 can only be caught by humans comparing screens. A shared fixture file (input→
  expected) consumed by Rust/TS/Kotlin tests would make drift a build break.

### F2. Validation split with no contract for messages — MEDIUM
- Evidence: backend owns rules (`B:src/api/orders.rs:261-295`, stages, email rails); clients
  pre-validate for UX (zod schemas per form, Android blank-checks) and render backend
  sentences (`F:src/components/forms/OrderForm.tsx:97-99`, `A:ui/common/Errors.kt:18-32`).
  Machine codes exist but no client enumerates the ones it handles beyond ad-hoc
  (`stage_gate` on Android `A:ui/orders/OrderDetailScreen.kt:191-196`).
- Impact: new backend code renders as generic lines until each client learns it (A4).

### F3. Pagination totals belong to the backend — MEDIUM
- Evidence: both clients hack around missing totals (`F:src/components/ui/Pagination.tsx:20-21`
  "backend gap", `A:ui/orders/OrderListScreen.kt:100` offset-append). Backend clamps
  (`B:src/api/mod.rs:137-143`) but never counts.
- Impact: "next disabled when loaded < limit" misfires on exact-multiple pages on both
  clients simultaneously.
