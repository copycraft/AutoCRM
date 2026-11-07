# 01 — System map

Read-only integration audit, 2026-09-23. Inventory of all three layers: what exists,
where it lives, and how the layers talk. Analysis lives in `02-findings.md`.

Conventions: `B:` = `backend/`, `F:` = `frontend/`, `A:` = `android/`, `O:` = `openapi/openapi.json`
(114 paths, 144 operations, 202 schemas). Line citations were verified by reading the files.

## 1. Feature × layer matrix

`✓` = implemented, `~` = partial/stub, `—` = absent. "Only-X" marks single-layer features.

| Feature | Backend (routes) | Web frontend | Android | Note |
|---|---|---|---|---|
| Auth: login/logout/me | `B:src/api/auth.rs:107,168,182` | `F:src/app/[locale]/login/page.tsx:19`, `F:src/lib/auth/context.tsx:30` | `A:ui/login/LoginScreen.kt:111`, `A:data/api/AutoCrmApi.kt:151` | cookie (web) vs bearer (mobile); lifetimes differ, `B:src/service/auth.rs:28-42` |
| Forced password change | `B:src/api/auth.rs:197` + 422 gate `B:src/api/extract.rs:52` | `F:src/app/[locale]/password/page.tsx:11` | — (refuses, "use web", `A:ui/login/LoginScreen.kt:72`) | asymmetric recovery |
| Sessions list/revoke | `B:src/api/auth.rs:221,242` | — (`F:endpoints.ts:20-22` unused) | — | backend-only |
| Users CRUD | `B:src/api/users.rs:35,53,93,168,206` | names only (`F:endpoints.ts:32-40` unused) | — | backend-only management |
| Dashboard | — (no endpoint) | `F:src/components/dashboard/DashboardView.tsx:54` (6 queries) | — | web-only composition |
| Partners CRUD + archive | `B:src/api/partners.rs:58,181,233,290,393` | lists/detail/new `F:src/app/[locale]/partners/**` | directory/detail/edit `A:ui/directory/DirectoryScreen.kt:175`, `A:ui/partners/PartnerScreens.kt`, `A:ui/partners/PartnerEditScreen.kt` | web splits business/person tabs; Android unifies + inline contacts |
| Contacts CRUD | `B:src/api/partners.rs:426,479,511,543` (no unarchive) | `F:src/components/forms/ContactSection.tsx:127` | `A:ui/partners/PartnerScreens.kt:366` + dialog | same gap everywhere: no unarchive |
| Leads + stages + convert + quotation | `B:src/api/leads.rs:59,165,199,230,316,335,351,398` | lists/detail/new + dialogs `F:src/app/[locale]/leads/**` | lists/detail/edit + stage/convert `A:ui/leads/LeadScreens.kt`, `A:ui/leads/LeadEditScreen.kt` | quotation web-only |
| Orders + stages + spec + items + vehicles-link | `B:src/api/orders.rs:94,350,455,578,750,784,817,891,920,983,1058` | lists/detail/new, 6 tabs `F:src/app/[locale]/orders/**` | lists/detail/edit `A:ui/orders/Order*.kt` | spec read-only on Android; vehicles link read-only both |
| Vehicles CRUD | `B:src/api/vehicles.rs:108` + link routes | — (no `vehiclesApi`, `F:endpoints.ts` gap) | embedded display only | backend-only management |
| Intake slip | fields on order (`B:migrations/0018,0019`) + gate `B:src/service/stages.rs:135` | `F:src/components/orders/IntakeSlipSection.tsx:19` | — (readings live inside inspections) | different shapes per client |
| Invoices/proformas (NAV) | `B:src/api/invoices.rs:66` + worker | `F:src/components/orders/InvoicesSection.tsx:57`, `ProformasSection.tsx:42` | — | web-only |
| Blockers | `B:src/api/blockers.rs:45,66,133,177,250,297` | stub page (`F:src/app/[locale]/blockers/page.tsx:6`), read-only tab | full CRUD on order detail `A:ui/orders/OrderDetailScreen.kt:293` | web page missing |
| Media uploads (ticket→PUT→complete) | `B:src/api/media.rs:51,70,95` + `B:src/service/media.rs:92` | — (all `mediaApi` upload fns unused) | queue + worker `A:data/upload/*.kt` | phone-only producing side |
| Image/document read | `B:src/api/media.rs:141,184,217,253,304,339,377,402` | counts badges + invoice PDFs only | counts only (`imageCounts`) | no gallery UI anywhere |
| Email compose/send/log | `B:src/api/email.rs` (13 ops) + jobs | full compose + preview + log `F:src/app/[locale]/emails/**` | log + minimal compose `A:ui/emails/*.kt` | templates/suppressions unmanageable anywhere |
| Newsletter | `B:src/api/newsletter.rs` (6 ops) | send (office) + list + public unsubscribe | — | web-only |
| Tasks | `B:src/api/tasks.rs:38,80,115,141,158` (no capability gate) | embedded lists + dashboard widget (no page) | my-tasks list + order sections | no dedicated page anywhere |
| Handover inspections | `B:src/api/inspections.rs` (15 ops) | history + verdicts + notes + templates (`F:src/components/inspections/*`) | full walkaround + sync (`A:ui/inspection/*`) | creation phone-only by design |
| Reports | `B:src/api/reports.rs` (7 ops) | workload charts only (+stalled on dashboard) | workload totals + stalled only | 5/7 endpoints unused |
| Global search | `B:src/api/search.rs:48` (8/group) | sidebar + palette `F:src/components/search/*` | — (directory is partners-only) | web-only |
| Pickup board (TV) | — (two `ordersApi.list` calls) | `F:src/components/board/PickupBoard.tsx:21` | — | web-only |
| Stage/project-type CRUD | `B:src/api/configuration.rs:108,170,286,346` | — (unused fns) | — (unused fns) | backend-only |
| Settings/save + email test | `B:src/api/configuration.rs:402,555`, `B:src/api/admin.rs:95,241` | `F:src/app/[locale]/settings/page.tsx:24` | appearance only (`A:ui/settings/AppearanceScreen.kt:46`) | disjoint settings |
| Admin jobs/status | `B:src/api/admin.rs:95,134,154,50,181` | stub page (only `testEmail` used) | — | backend-only ops |
| Preferences (density/page-size) | `B:src/api/auth.rs:268,289` | `F:src/app/[locale]/preferences/page.tsx:16` | — (theme prefs instead) | disjoint customization |
| Migration provenance | `B:src/api/raw_import.rs` (3 ops) | `F:src/components/migration/RawImportPanel.tsx:48` | — | web-only |
| JobSheet print | — (client-side) | `F:src/components/orders/JobSheet.tsx:39` | — | web-only |
| Mobile picker `/mobile/orders` | `B:src/api/mobile.rs:51` | — | dead UI (`A:ui/picker/OrderPickerScreen.kt:142`, no route) | orphaned both sides |

## 2. Data model inventory (per entity: DB → Rust → TS → Kotlin)

Money everywhere: integer minor units + explicit currency (`B:src/domain/money.rs:14`). Dates on the wire are strings
(backend serializes dates/timestamps; TS keeps them strings; Kotlin keeps them `String`).

| Entity | DB (migration) | Rust struct | TypeScript | Kotlin | Mismatches |
|---|---|---|---|---|---|
| User/session | `0001` users, sessions | `B:src/repo/users.rs:12`, `B:src/repo/sessions.rs:17` | generated `SessionUser/User` | `A:data/api/Dto.kt:48` `SessionUser` + `A:data/auth/SessionStore.kt:42` Account | role as free string on clients; capability matrix copied 3× (`B:src/domain/role.rs:45`, `F:src/lib/auth/context.tsx:70`, `A:data/auth/SessionStore.kt:54`) |
| Partner/contact | `0002` + `0011` role | `B:src/repo/partners.rs:8`, `B:src/repo/contacts.rs:6` | generated | `A:data/api/Dto.kt:393,408` | `Contact` omits `partner_id/archived_at`; no unarchive op anywhere |
| Lead | `0002` + `0011` quote/fx | `B:src/repo/leads.rs:6` | generated | `A:data/api/Dto.kt:425` | `currency` free string on clients |
| Order/items/spec/vehicles | `0003` + `0011` + `0015` + `0010` | `B:src/repo/orders.rs:9`, `order_items.rs:7`, `order_specs.rs:14`, `vehicles.rs:15` | generated | `A:data/api/Dto.kt:97,140,176,166` | `quantity: String`, spec numerics as `String` on Android; `images.vehicle_id` column exists, unwired in Rust (`B:src/repo/images.rs:14`) |
| Stages | `0002` defs + per-entity history | `B:src/domain/stage.rs:46`, `B:src/repo/stages.rs:8` | generated + `F:src/lib/utils/stages.ts:9` tone map | `A:data/api/Dto.kt:218,228` + local `reason` | stage keys as free strings; web board hardcodes 3 keys (`F:src/components/board/PickupBoard.tsx:19`) against the no-hardcode rule |
| Blockers | `0003` | `B:src/repo/blockers.rs:6` | generated | `A:data/api/Dto.kt:150` (`nudgeCount: Int` vs `Long` everywhere else) | Int/Long width split |
| Images/documents | `0004` + kinds `0013/0014/0020` + `0025` | `B:src/repo/images.rs:14`, `B:src/repo/documents.rs:8` | generated (read-only use) | `A:data/api/Dto.kt:270` (`ImageView`), no document row type | contract `Completed` is `oneOf`, Android flattens to nullable `image`/`document` (`A:data/api/Dto.kt:332`) vs contract `Image` type |
| Email/newsletter | `0005` + `0022` + `0024` | `B:src/repo/emails.rs:11`, `B:src/repo/newsletter.rs:17` | generated | `A:data/api/Dto.kt:348` (`attempts: Int`) | Android drops bodies/preview/templates by design |
| Tasks | `0017` | `B:src/repo/tasks.rs:14` | generated | `A:data/api/Dto.kt:537` (dates `String`, `isDone` derived) | none structural |
| Inspections | `0026` (6 tables) | `B:src/repo/inspections.rs:15` family | generated | `A:data/api/Dto.kt:614` family + local `DraftPayload` (`A:data/inspection/DraftModels.kt:59`, odometer/battery as `String`, `toIntOrNull` at sync `A:data/inspection/InspectionSync.kt:80`) | draft String→Int silent-null; `DamageBody.view` defaults `"top"` client-side |
| Reports | views (`0006`) | rows inline in `B:src/api/reports.rs` | generated (2 used) | `A:data/api/Dto.kt:582` (`daysInStage: Int` vs `StageView.daysInStage: Long`) | Int/Long split again |
| Settings/prefs | single-row `settings`, `user_settings` (`0008`) | `B:src/repo/config.rs:195,312` | generated | theme prefs are Android-local (`A:data/prefs/ThemePrefs.kt:28`) | disjoint customization models |

## 3. Communication map

- REST: 144 ops under `/api` (`O:openapi.json#/paths`); web rewrites `/api/:path*` → backend (`F:next.config.js:44`); Android prefixes stored base URL + `/api` per call (`A:data/api/AutoCrmApi.kt:68`).
- Auth: httpOnly cookie (web, `credentials:include`, `F:src/lib/api/client.ts:36`) vs bearer header (mobile, `A:data/api/AutoCrmApi.kt:99`); lifetimes web 7d/30d, mobile 60d/365d (`B:src/service/auth.rs:28`).
- Errors: always `{error:{code,message}}` (`B:src/error.rs:14`); web parses via zod (`F:src/lib/api/errors.ts:43`); Android via `ApiErrorBody` (`A:data/api/AutoCrmApi.kt:133`).
- Validation at boundary: web validates every 2xx against generated zod (`F:src/lib/api/client.ts:48`); Android presence-tests contract (`A:app/src/test/.../OpenApiContractTest.kt:1`); backend compile-checks SQL (`B:src/repo/mod.rs:1`).
- Uploads: ticket → direct-to-S3 PUT → complete (`B:src/service/media.rs:92`); phone queue drains via WorkManager (`A:data/upload/UploadWorker.kt:33`); web never uploads.
- Jobs: PG queue consumed by backend worker (`B:src/repo/jobs.rs:66`); clients only poll status (invoice `submitting` 3s poll `F:src/components/orders/InvoicesSection.tsx:77`; board 30s `F:src/components/board/PickupBoard.tsx:25`).
- Offline/sync: Android Room (`pending_uploads`, `inspection_drafts`) + two workers (`A:data/upload/UploadWorker.kt:33`, `A:data/inspection/InspectionSyncWorker.kt:48`); web has no offline path (banner only, `F:src/components/layout/OfflineBanner.tsx:8`).
- Push/notifications: none on any layer.
- Dead surface (backend serves, nobody calls): `GET /mobile/orders` (picker UI dead), users/stages/project-types/templates management, sessions UI, `GET /orders/{id}/spec`, lead-documents media ops, newsletter subscribe, 5 report ops, blockers/admin web pages (stubs).

## 4. Cross-cutting concerns per layer

| Concern | Backend | Web | Android |
|---|---|---|---|
| State | stateless handlers + PG | React Query (`F:src/lib/query/provider.tsx:10`, stale 30s, `keepPreviousData`) + URL state (`F:src/hooks/useUrlState.ts:14`) + localStorage drafts | StateFlow/ViewModel + Room + DataStore; no HTTP cache (except `mobile/orders` 60s server header) |
| Pagination | `limit=clamp(50,1,200)` (`B:src/api/mod.rs:137`) | offset UI, no totals ("backend gap", `F:src/components/ui/Pagination.tsx:20`) | load-more only on orders (`A:ui/orders/OrderListScreen.kt:100`); fixed limits elsewhere |
| Time | Budapest business day (`B:src/service/mod.rs:16`); UTC instants | mixed: Budapest (`F:src/components/ui/DayGroups.tsx:8`) vs UTC bug (`F:src/components/dashboard/DashboardView.tsx:23`) | Budapest display (`A:util/Format.kt:19`); capture `Instant.now` |
| Money | `Money` minor-only ops (`B:src/domain/money.rs:58`) | `formatMoney` + 3 sibling converters (`F:src/lib/utils/format.ts:17`, `F:LeadForm.tsx:45`, `F:ItemsSection.tsx:26`) | `formatMoney` (`A:util/Format.kt:19`); quote/item inputs drop fillér |
| Validation | `required/optional/patch` helpers (`B:src/api/mod.rs:146`); regexes per entity | zod per form + server passthrough (`F:src/components/forms/LeadForm.tsx:133`) | minimal (blank checks; server owns rules) |
| i18n | HU-only strings in code | `hu.json` 30 namespaces (`F:src/messages/hu.json`) | hardcoded HU literals everywhere |
| Permissions | capability matrix (`B:src/domain/role.rs:45`) | mirrored UI gates (`F:src/lib/auth/context.tsx:70`) | mirrored (`A:data/auth/SessionStore.kt:54`) |
| Theming | — | light-only Tailwind tokens | light/dark/AMOLED (`A:ui/theme/Theme.kt:26`, `A:ui/settings/AppearanceScreen.kt:46`) |
| Logging/diag | tracing + `audit_log` (`B:src/repo/audit.rs:8`) | `console.error` on contract break (`F:src/lib/api/errors.ts:24`) | StrictMode log in debug, no crash reporting |
| Config | `config.rs` from env (60+ keys) | 3 env vars (`F:next.config.js:21`) | server URL on device + build prefill (`A:app/build.gradle.kts:68`) |
