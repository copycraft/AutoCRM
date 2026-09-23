# Follow-up

Issues noticed during remediation that are outside the finding being fixed. Not fixed here.

- **Canonical frontend plan missing.** `autotherm-crm-frontend-plan.md` was not in the workspace;
  `FRONTEND_PLAN.md` was assembled from the audit spec and the remediation rule corrections
  (see its provenance note). Replace it with the canonical document and re-check the section
  numbers cited in source comments (`§5`, `§12`, `§13`, `§14`).
- **Backend test failing before remediation started — FIXED 2026-09-12 (unit level).**
  `backend/tests/smtp.rs::unreachable_server_fails_the_connection_test_with_a_hint` failed
  because `integrations/email.rs` picked the operator hint by substring-matching the transport
  error text, and Windows renders OS errors in the display language. Fixed by also matching
  locale-independent OS error codes (`os error 10060/10061/110/111/…`) and Winsock names,
  with Hungarian regression asserts in the existing `google_relay_errors_get_hints` unit test
  (`cargo test --lib`: 100 passed). The integration target itself was not re-run (dev server
  holds the binary lock); re-run after a restart. See also the 2026-09-12 section below.
- **Residual `as` casts on API-adjacent data (post-R1).** The 7 non-null assertions are gone,
  but ~28 `as` casts remain (e.g. `(order.currency === 'EUR' ? 'EUR' : 'HUF') as Currency`,
  `JSON.parse(text) as T`-style narrowing in client code, `as const` literals). None were
  introduced to silence the R1.6 flag errors (tsc is clean without them), but M2's "no `as`"
  bar is not fully met. Left for a dedicated typing pass, not any R2–R9 finding.
- **Next.js major migration (14.2.35 → 16.x).** 14.2.35 is the latest 14.x; stable is 16.3.5.
  Deliberately not done during remediation. Known blockers, each verified against the migration
  guides, not guessed: (1) `middleware.ts` must become `proxy.ts` (16 renames/deprecates
  middleware — our next-intl locale middleware included); (2) route `params`/`searchParams`
  become async — every `[id]` page, `new` page and layout touching them needs `await`
  (~10 files); (3) `next lint` is removed — the `lint` script and the CI frontend job need
  an ESLint 9 flat-config replacement; (4) React 18 → 19 for the whole tree (RHF, Radix,
  TanStack compat to re-verify with zero frontend tests as a net); (5) next-intl 3.26.5
  against Next 16 needs a compat check, possibly a major of its own. Do this as one
  dedicated task on a green tree with a full regression pass, not layered over other work.

## Bug-hunt 2026-09-12 — deferred (found, deliberately not fixed)

Backend (`backend/src`):
- **PATCH /documents/{id} wipes ungiven fields** (`api/media.rs:339`, `repo/documents.rs:274`).
  `DocumentPatch` uses plain `Option`, so absent ≡ explicit null and `set_validity` overwrites
  all four columns; sending `{}` nulls them. Proper fix is `Option<Option<T>>` (or a patch
  wrapper) per field — COALESCE alone would make explicit clearing impossible. Data-loss
  class; needs a careful change with tests, not a drive-by.
- **Blocker update/resolve/reopen skip the order lock** (`api/blockers.rs:177-309` vs the
  lock discipline in `repo/orders.rs:92-93`). Only create locks. Narrow race, real.
- **Stage history INNER JOINs stage_definitions** (`repo/stages.rs:29-43`). A deleted (not
  deactivated) definition silently drops history rows and fabricates `left_at` across the
  gap. LEFT JOIN + fallback label, as order detail already does (`api/orders.rs:430`).
- **Explicit `"quantity": null` on PATCH /order-items 400s** (`api/orders.rs:842-843`),
  contradicting the nullable schema. Semantically harmless; fix in `decimal_str_or_number`
  would touch all numeric fields — verify each call site first.
- **Nudge re-check ignores due-date edits under lock** (`service/automation.rs:63-67`):
  one stale email worst case.
- **Terminal → exit moves classified Reopen, no customer notification**
  (`domain/stage.rs:118-127`, `service/stages.rs:143`). Cancelling out of `completed` needs
  only a note and notifies nobody. Possibly intended — needs a product decision, not code.
- **COOKIE_SECURE defaults true** (`config.rs:252`): web login never sticks over plain-http
  localhost dev. Safe for prod; consider `false` when the base URL is http/local.
- **No pagination metadata** (`api/mod.rs:111-125`): lists are bare arrays, no total/has_more.
  Fine today; matters for Android offline sync design.
- Informational only: report `+=` on i64 (`api/reports.rs:119-127`), `next_position` i32
  (`repo/order_items.rs:43-51`, plus unvalidated client `position`), both need ~1e18/2e8
  to matter.

Web (`frontend/src`):
- **Inbox search is client-side over one page** (`app/[locale]/emails/page.tsx`). Pager now
  counts filtered rows, but searching beyond the loaded page needs a backend `q` param.
- **Pagination Next on exact-multiple totals** (`components/ui/Pagination.tsx:21`): one wasted
  click lands on an empty page. No total exists to do better without an API change.
- **Unknown enum/category keys render raw** (`orders/[id]/page.tsx:322-326`, relation/defrost
  labels). Unreachable today (contract matches), but a new backend key leaks untranslated.

App (`android/app/src/main`):
- **UploadWorker double backoff** (`UploadWorker.kt:81-92`): row-level `backoffFor` (up to 2h)
  compounds with WorkManager retry; hammering retry-all defeats backoff. Battery/churn tuning.
- **`formatMoney(Long.MIN_VALUE)`** (`util/Format.kt`): negation overflows. Unreachable
  (≈9e18 Ft); guarded currency case, left the extreme.
- **Bundled fonts are untracked** (`app/src/main/res/font/`): builds locally, breaks on a
  fresh clone/CI until committed. Needs a commit, which was not requested.
- **SMTP integration test not re-run**: `tests/smtp.rs::unreachable...` root-caused and fixed
  at the unit level (locale-independent os-error/Winsock matching + Hungarian regression
  asserts in `email.rs`), but the integration target could not relink while the dev server
  (`autocrm.exe`, :8080) holds the binary lock. Re-run after a server restart.
