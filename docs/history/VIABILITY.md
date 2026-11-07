# ERP/CRM Viability Review

Reviewed against the code, not the plans: `backend/migrations/0001`–`0008`, `backend/src/**`,
`frontend/src/**`, `docs/`. Where a plan document is the only evidence, it is named as such.

## Verdict

Autotherm can run production, MEO documentation and supplier chasing on this system. Those
parts are modelled correctly, enforced in the database rather than in handlers, and the stage
/ blocker / image machinery is the best-built thing in the repo. They cannot yet run **sales**
on it: a quotation has nowhere to live, so the six weeks between "we sent them a price" and
"they said yes" are invisible, and the owner's stated reason for the project — reporting on
project value — rests on line items that nothing requires anyone to enter. Two structural
gaps will be expensive to close after 1000 orders are migrated: the vehicle is four text
columns on an order rather than an entity, and no order can reference another order, which
means warranty and rework jobs are unlinkable to the job they repair. The migration is the
most serious risk to the owner's own kill criterion: it loads five MiniCRM concepts and
silently leaves the rest in a `raw_import` column that **no API, screen or query ever reads**
(zero non-migration references in `backend/src`), including vehicle data, addresses, and the
project to-do history. Add to that one live defect on the single most-used screen
(`frontend/src/app/[locale]/orders/[id]/page.tsx:177,253`) which renders source code as body
text. Fix the quotation gap, the vehicle entity and the migration field coverage before
cutover; everything else on this list can wait.

## Scenario results

| # | Scenario | Result | Evidence |
|---|---|---|---|
| 1 | Quotation before order | **Cannot be represented** | No `quotes` table; `leads` (`0002_partners_leads.sql:88`) has no value/amount/currency column; `documents.order_id` is `NOT NULL` (`0004_media.sql:68`); `service/email.rs:389` rejects attachments — "attachments require an order". Only trace is lead stage `quoted`. |
| 2 | Quote revised before acceptance | **Cannot be represented** | Same. Nothing versions a price before `order_items` exists. `audit_log` only starts at order creation (`service/orders.rs` `create_in_tx`). |
| 3 | Lead returns after 8 months | **Works awkwardly** | `repo/leads.rs:169` matches title, contact name/e-mail and partner name, so `/leads?q=Müller` finds it. But `api/partners.rs:204` `PartnerDetail` returns `{partner, contacts, orders}` — **no leads** — and the partner screen (`partners/[id]/page.tsx:151`) renders only orders. Nobody opening the partner sees they were quoted before. |
| 4 | German GmbH and Hungarian sole trader | **Works** | `partners` carries `kind`, `country`, `tax_number`, `eu_tax_number`, `default_currency` (`0002:5-24`); `api/partners.rs:97` applies Hungarian tax-number normalisation only when `country = 'HU'`. |
| 5 | Where the vehicle lives | **Works awkwardly** | Four nullable text columns on `orders`: `vehicle_make`, `vehicle_model`, `vehicle_plate`, `vehicle_vin` (`0003_orders.sql:30-33`). No `vehicles` table. Plate is indexed normalised (`0003:42`) and searched (`repo/orders.rs:266`), which is the one thing that saves scenario 7. VIN is matched only by raw `ILIKE`. Nothing enforces or validates either. |
| 6 | Three identical Sprinters, one enquiry | **Works awkwardly** | Three orders is the only way to keep per-vehicle MEO photos apart (`images.order_id`, `0004:3`). But `orders.lead_id` is `UNIQUE` (`0003:24`) and `service/leads.rs:68` rejects a second conversion with `already_converted`, so only one of the three links back to the enquiry. The other two are orphan orders with no record of where they came from. One order with three vehicles is worse: one plate field, one image pool, one certificate. |
| 7 | 2019 van returns for warranty | **Works for new orders, fails for migrated ones** | `/orders?q=ABC-123` matches the normalised plate (`repo/orders.rs:266`). But `migration/load.rs:380-397` never writes `vehicle_plate`, `vehicle_vin`, `vehicle_make` or `vehicle_model` — for the 1000 migrated orders the plate exists only inside `raw_import`, which nothing queries. A plate search returns the 2019 job only if the plate happens to sit inside the MiniCRM project name (loaded as `title`). Once found, design files and photos are on the order (`GET /orders/{id}/documents`, `/images`) — but there is no UI for either (order tabs are Adatok/Tételek/Fázisok/Blokkolók/Napló, `orders/[id]/page.tsx:118`). |
| 8 | Customer claims you scratched the vehicle | **Works internally, not demonstrable to a third party, and not demonstrable at all today** | Real: `CHECK (category <> 'intake' OR immutable)` (`0004:28`), the `protect_immutable_images` trigger refusing DELETE and identity UPDATE (`0004:35-66`), sha256 stored at upload and verified at finalize (`service/media.rs:233-256`), `captured_at` from EXIF (`media/pipeline.rs:66`), and S3 Object Lock. Weak: the lock is **governance** mode (`media/storage.rs:88`), which a holder of `s3:BypassGovernanceRetention` can lift, and the sha256 is Autotherm's own record with no external timestamp — `docs/DECISIONS.md` admits this ("Not yet: RFC 3161 trusted timestamps (the hash alone isn't third-party proof)"). Practically: `GET /images/{id}/original` needs `ViewOriginalImages` (admin/office, `domain/role.rs:48`) and there is **no gallery screen**, so today a non-technical user cannot show the customer anything. |
| 9 | ATP certificate with an expiry date | **Cannot be represented** | `documents` has `kind ENUM('design','cad','other')` and no validity, issuer, or expiry column (`0004:68-84`), and `order_id` is `NOT NULL` — a certificate belonging to the vehicle for its life is filed under one job. `repo/documents.rs` exposes only `list_for_order`; there is no cross-order document query, so "which certificates expire next quarter" has no answer at any layer. |
| 10 | Cooler model changed mid-build | **Works awkwardly** | The price change is recorded: `api/orders.rs:487` writes an `item_update` audit entry with before/after values and the user id. The new design is a second row in `documents` (content-addressed, so no overwrite). What is missing is the link between them: no change-order entity, no document supersede flag or revision number, no customer-approval record beyond a free-text `note` on a stage move. The shop floor sees "there are two design PDFs, one is newer". |
| 11 | Order cancelled while in design | **Works** | `cancelled` is `is_exit` + `is_terminal` and reachable from any open stage with no gates (`domain/stage.rs:124`). History is append-only (`order_stages`), photos are untouched, blockers stay open (nothing auto-resolves them — see operational findings). The traveller renders it correctly after remediation: `StageRail.tsx:36` derives `done` from `history.has(key)`, so production/MEO/completed show as never-reached grey circles, not ticks. |
| 12 | Paint subcontracted out | **Works awkwardly** | `blockers.responsible_partner_id` points at `partners` (`0003:88`), the same table as customers, and `partners` has no customer/supplier flag. Consequences: the partner picker and `/partners` list mix suppliers into the customer list with no filter (`api/partners.rs:36` offers only `kind` business/person and `include_archived`); `/reports/blocker-load` is the only place suppliers are grouped. Reporting is not corrupted — `volume_by_partner` (`repo/reports.rs:50`) joins through `orders`, so a supplier with no orders never appears. |
| 13 | Two staff edit the same order | **Works** | `orders::lock` takes `SELECT … FOR UPDATE` before every write (`repo/orders.rs:82`), and `orderPatchBody` (`OrderForm.tsx:74-106`) sends only fields the user actually changed. Different fields merge; the same field is last-write-wins with both versions in `audit_log`. No `If-Match`/version column exists, which at 30–40 orders/month and <20 users is the right trade. |
| 14 | Employee leaves, account disabled | **Works awkwardly** | `api/users.rs:130` deactivates and revokes sessions but leaves `leads.assigned_to` / `orders.assigned_to` pointing at them (no `ON DELETE` behaviour needed — the row stays). Their name still shows on the order (`repo/orders.rs:253` LEFT JOINs `users`). There is no bulk reassign and **no UI path to list their work**: `AssigneeField.tsx:82` filters the dropdown to `is_active`, so the departed person cannot be selected as a filter. The backend supports `?assigned_to=<id>`; an admin must hand-edit the URL. |
| 15 | 2019 order with stage names that no longer exist | **Works awkwardly; history is flattened either way** | Truthful storage is possible: `stage_definitions` is configuration with `is_active` (`0002:50-68`), `POST /stage-definitions` exists, and `check_transition` explicitly tolerates a deactivated *current* stage (`domain/stage.rs:108`). `load.rs:225` hard-fails if the mapping names an undefined stage, which forces the decision rather than hiding it. But `load.rs:401` inserts **one** `order_stages` row per order (`WHERE NOT EXISTS`), so every migrated order's entire journey collapses to a single entry noted "MiniCRM import". Stage-duration and throughput reports over historical data are therefore meaningless, and the traveller on a migrated order shows one visited stage. |
| 16 | Open a 2017 order's gallery | **Cannot be evaluated end-to-end; the API path is sound, the pipeline throughput is not** | No gallery screen exists. `GET /orders/{id}/images` (`api/media.rs:113`) returns **every** image in one unpaginated response with two presigned URLs each — for 120 photos that is ~240 local HMAC signings and a ~150–200 KB JSON body, acceptable; the plan's `N9` virtualisation rule (`FRONTEND_PLAN.md §4`) handles the DOM side. The real problem is upstream: `thumb_url`/`display_url` are `null` until the `process_image` job runs (`api/media.rs:129-137`), with no fallback to the original. The worker claims `BATCH = 5` and runs them **sequentially** in one task (`jobs/mod.rs:36,81`). ~100k migrated images at a second or two each is days of single-threaded decode/re-encode, during which migrated galleries are blank for everyone below office role. HEIC originals never get previews at all (`domain/media.rs:49-51`) and stay blank permanently. |
| 17 | MiniCRM data with no destination | **Significant loss; `raw_import` is real but unreachable** | See the parity section. `raw_import JSONB` is populated on `partners`, `contacts`, `leads`, `orders` (`load.rs:277,336,383,446`) — and `grep raw_import backend/src` outside `migration/` returns **nothing**. It is not in any response schema, any screen, or any search predicate. |
| 18 | What the owner can actually learn | **Works, within a narrow band** | See the reporting section. |
| 19 | "Müller GmbH last year, in HUF" | **Works** | FX is per-order, not per-report: `orders.valuation_date` (`0003:22`) plus the `order_values` view (`0006_reporting.sql:36-60`) picking the latest MNB rate within 10 days before that date. MNB ingestion is implemented (`integrations/mnb.rs`, SOAP), scheduled daily after 12:30 Budapest (`jobs/mod.rs:210`), with a `fx-backfill --from 2010-01-01` CLI (`main.rs:87`) for history. No rate in the window → `total_huf_minor` is `NULL`, counted as `missing_fx` in every report row (`repo/reports.rs:33`) and surfaced on `/admin/status` as `orders_missing_fx_rate`. The gap is the question's shape: `volume_by_partner` returns **all** partners for a period and takes no partner filter — the answer is "find Müller in the list". |
| 20 | A number the reports don't cover | **Phone call to the developer** | Five report endpoints exist (`api/reports.rs`). `grep -i csv` across `backend/src` and `frontend/src` returns nothing — the CSV export specified in `FRONTEND_PLAN.md §6.8` is not implemented at either end, and the reports screen is a stub (`reports/page.tsx` → `UnavailableState`). With no export and no ad-hoc query surface, every question outside those five shapes is a developer task. At a company that has been running 30–40 orders a month since 1992, expect this monthly. |

## Gaps — EXPENSIVE LATER

### 1. The vehicle is not an entity

**Missing.** A `vehicles` table keyed by VIN/plate, with orders referencing it.

**Broke scenarios** 5, 6, 7, 9.

**Schema change.** New `vehicles (id, vin, plate, make, model, year, partner_id, notes)`;
`orders.vehicle_id BIGINT REFERENCES vehicles(id)`; backfill from the four text columns, then
deduplicate. For a multi-vehicle order it becomes `order_vehicles` (many-to-many) and `images`
/ `documents` gain a nullable `vehicle_id` so MEO photos and certificates attach to the right
van.

**Cost if deferred past migration.** The four text columns are today `NULL` on all ~1000
migrated rows (`load.rs` never writes them), so the backfill is not "split a column" — it is
"re-parse MiniCRM `raw_import` per account-specific custom field, then reconcile against free
text typed by staff in the meantime". Every month of live use adds unvalidated plate strings
to deduplicate. The `images` re-attachment is the painful half: it is a manual,
photo-by-photo decision on orders that already have 80–120 photos, and intake images are
immutable — the trigger at `0004:35` blocks `UPDATE` on `order_id` but not on a new
`vehicle_id` column, so it is doable without downtime, but only with a hand-written
reclassification pass. Doing it now costs one migration and a mapping-file field.

### 2. No relationship between orders

**Missing.** `orders.parent_order_id` (or a typed `order_links` table) for warranty, rework
and repeat jobs.

**Broke scenarios** 6, 7, 10.

**Schema change.** `ALTER TABLE orders ADD COLUMN related_order_id BIGINT REFERENCES
orders(id), ADD COLUMN relation TEXT` — or `order_links (from_order_id, to_order_id, kind)` if
one job can relate to several.

**Cost if deferred past migration.** The column is additive and cheap; the *data* is not. The
links that matter most are between a 2024 warranty job and its 2019 original, and nobody will
reconstruct 1000 orders' worth of those retroactively. Every month without it is a month of
permanently unlinked repeat work. Add the column before go-live even if no screen uses it yet.

### 3. Quotations have no home

**Missing.** Either a `quotes` table, or — cheaper and probably sufficient here — a value,
currency and validity on `leads` plus letting `documents` and e-mail attachments hang off a
lead.

**Broke scenarios** 1, 2, 3, and the pipeline half of 18.

**Schema change.** Minimum: `leads` gains `quoted_value_minor BIGINT`, `currency CHAR(3)`,
`quote_valid_until DATE`. Proper: `documents.order_id` becomes nullable with a
`CHECK (num_nonnulls(order_id, lead_id) = 1)`, and `service/email.rs:389` learns to attach a
lead's documents. A revision history needs `quote_revisions` or, at minimum, an
`audit_log` entry on lead value changes (the `audit_log` table already supports
`entity = 'lead'`).

**Cost if deferred past migration.** The nullable-`order_id` change on `documents` is the
expensive part: it is a `NOT NULL` drop plus a new check constraint on a table that by then
holds a document row per migrated file, and the `documents_order_hash_key` partial unique
index (`0004:85`) has to be rebuilt to cope with `NULL` order ids. Doable without downtime
(`NOT VALID` then `VALIDATE`), but it touches the one table that will have the most rows. More
importantly: while it is absent, every quotation continues to live in Outlook, and the habit
of keeping sales outside the system is exactly the adoption failure that kills these projects.

### 4. Documents have no validity period and no cross-order view

**Missing.** `documents.valid_from`, `valid_until`, `issuer`, a `certificate` value on
`document_kind`, and a `GET /documents` query across orders.

**Broke scenario** 9.

**Schema change.** `ALTER TYPE document_kind ADD VALUE 'certificate'` (cannot run inside a
transaction with other DDL in some Postgres versions — needs its own migration), plus three
nullable columns and one index on `valid_until WHERE deleted_at IS NULL`.

**Cost if deferred past migration.** The columns are additive and cheap. The dates are not:
every migrated ATP certificate arrives as a PDF with `kind = 'other'` (`load.rs:485` hardcodes
`DocumentKind::Other` for every non-image file), so the expiry has to be read out of each PDF
by hand later. If certificates matter — and for refrigerated bodies exported to Austria and
Bavaria they do — capture the date at load time or accept that the historical set is
permanently undated.

## Gaps — CHEAP LATER

- **Supplier/customer flag on `partners`** — one nullable `role` column or a boolean pair, plus a filter on `api/partners.rs:36`. Nothing depends on the distinction today.
- **Leads section on the partner detail screen** — `PartnerDetail` (`api/partners.rs:204`) needs one more query; the lead search already supports the filter server-side.
- **Reassign-on-leave** — a `PATCH /users/{id}` side effect or a bulk reassign endpoint, plus letting `AssigneeField` show inactive users when they are the current filter value.
- **CSV export on the five report endpoints** — an `Accept: text/csv` branch; no schema change.
- **Partner filter on `/reports/volume`** — one bind parameter in `repo/reports.rs:44`.
- **Stalled-*lead* report** — `stage_definitions.stall_after_days` is populated for lead stages (`0002:71-75`) but `repo/reports.rs:196` only queries orders, so that configuration is currently inert.
- **Blockers left open on a cancelled order** — nothing closes them; they keep generating nudges to suppliers about a job that was cancelled, unless `automatic_email_enabled` is off. One condition in `blockers::nudge_candidates`.
- **HEIC previews** — already recorded as known in `docs/DECISIONS.md`.
- **`raw_import` viewer** — a read-only "MiniCRM eredeti adatok" panel on migrated records. Additive, and the single cheapest insurance against the owner's kill criterion.

## Correctly scoped out

These exclusions are right and should stop being revisited:

- **Invoicing, VAT, receivables.** `docs/DECISIONS.md` records that `vat_rate` was deliberately *not* added to `order_items` despite the plan listing it, because half a VAT implementation is worse than none. `order_items` carries description/quantity/unit_price/currency and nothing else — enough to value a job, not enough to pretend it is a bill.
- **Cost tracking and margin.** Leaving it out keeps `order_items` a single-sided price list with no ambiguity about whose number is in a row. The later addition is genuinely additive (a `cost_minor` column or a parallel table), so the deferral costs nothing.
- **Inventory, purchase orders, timesheets.** The `blockers` table is the correct minimal substitute for the only part of procurement this business actually needs to track: "we are waiting on a thing from someone, chase them" (`0003:86-104`). It has a responsible party, a due date, a nudge counter and a resolution — and no stock levels. This is the single best scoping decision in the system.
- **Customer portal.** With 60 leads a month and exports to two countries, the correspondence path is e-mail, and `email_messages` already keeps a permanent, per-order, never-deleted log (`0005_email.sql:57`). A portal would add an authentication surface for no operational gain.
- **No materialized views, no report builder, no job-queue service.** `docs/DECISIONS.md` justifies plain views and a Postgres-backed queue by data volume. At 30–40 orders a month that is correct and it is why the whole system is one binary plus Postgres plus object storage.
- **No self-registration, four fixed roles.** `domain/role.rs` uses an exhaustive match instead of a policy DSL, so adding a capability forces a decision for every role. For <20 users this is the right amount of machinery.

## MiniCRM parity assessment

### Clear destination

| MiniCRM | AutoCRM | Where |
|---|---|---|
| Contact (Type=business) | `partners` (kind=business) | `load.rs:243-283` |
| Contact (person, no BusinessId) | `partners` (kind=person) | same |
| Contact (person at a business) | `contacts` | `load.rs:286-330` |
| Project in an `order` category | `orders` + one `order_stages` row | `load.rs:380-412` |
| Project in a `lead` category | `leads` + one `lead_stages` row | `load.rs:437-470` |
| Project status | mapped stage key via `mapping.json` | `load.rs:366` |
| Project value / currency fields | one `order_items` row "MiniCRM érték" | `load.rs:414-435` |
| Any file URL found anywhere in the JSON | `images` or `documents`, content-addressed | `load.rs:472-500`, `manifest.rs:96` |
| Source id | `minicrm_id` (unique, kept forever) | all four tables |

The file discovery is the strongest part: `manifest.rs` scans **structurally** for URL-shaped
strings at any JSON path rather than guessing field names, and reports counts per field group
for comparison with the MiniCRM UI. That is the right way to avoid missing an attachment
field nobody remembered.

### Dropped

Deliberate and recorded:

- Deleted projects (`load.rs:357`) and categories mapped to `skip`.

Not recorded anywhere, and this is the finding:

- **Every order field except title, partner, contact, currency, valuation date and value.** `load.rs:380` inserts exactly those columns. `vehicle_make`, `vehicle_model`, `vehicle_plate`, `vehicle_vin`, `description`, `due_date`, `assigned_to` and `project_type_id` are never written. `project_type_id` is the dimension the owner wants to report on, and it arrives empty for all historical orders — `volume_by_project_type` will bucket 1000 orders as `(nincs megadva)`.
- **Every partner field except name, email, phone, website.** `load.rs:266` — `tax_number`, `eu_tax_number`, `country`, `default_currency`, `postal_code`, `city`, `address_line`, `notes` are all skipped, even though the columns exist and `extract.rs:178` *downloads* `Api/R3/AddressList/{cid}` into `raw/addresses/`. Those files are written to disk and then never opened by any code.
- **Project to-do / activity history.** `extract.rs:141` fetches `Api/R3/ToDoList/{id}` behind an opt-in `--todos` flag, into `raw/todos/`. Nothing in `load.rs` or `reconcile.rs` mentions them. For a company whose entire project management currently runs in MiniCRM, this is likely the largest single body of institutional record, and it has no destination table at all.
- **E-mail correspondence.** Not extracted. `extract.rs` calls `Category`, `Schema/Project`, `Project`, `ToDoList`, `Contact` and `AddressList` — no e-mail or message endpoint. `email_messages` therefore starts empty; the order correspondence tab will show nothing for any historical job.
- **Stage history.** Collapsed to one row per record (scenario 15).
- **The customer-facing order number.** `load.rs:384` writes `MC-{MiniCRM Id}`. `docs/DECISIONS.md` states both "Imported MiniCRM orders keep whatever number they had" and "Migrated orders are numbered `MC-{MiniCRM id}`"; the schema comment at `0003:19` agrees with the first, the code with the second. If Autotherm's order numbers are the MiniCRM project ids, this is fine. **If they keep their own numbering in a MiniCRM custom field, every historical order becomes unfindable by the number printed on the paperwork** — confirm which before cutover.

All of the above land in `raw_import`, which is where the design intends them to be — and
`raw_import` is read by nothing. "Never silently dropped" is true of the bytes and false of
the user experience. A staff member who opens a migrated 2019 order sees a title, a partner, a
date, a money figure and a photo count, and has no way to discover that the van's plate,
the customer's address and forty to-do entries are sitting in a JSONB column.

### Can anyone tell the difference?

Partly, and better than most migrations manage. `reconcile.rs` produces a signed-off Markdown
report that goes well past row counts: source-vs-destination counts for partners, contacts,
orders and leads; **an explicit list of MiniCRM project ids missing from AutoCRM**
(`reconcile.rs:139`); file coverage (manifest references, downloaded, never downloaded,
currently failing, unique contents after dedup); **per-order file-count mismatches against the
manifest** (`reconcile.rs:213`); order value by year and currency for comparison with
MiniCRM's own totals; orders with no stage history; and a sign-off block requiring a named
person to spot-check 20 orders across years. `backend/tests/migration.rs:96` exercises the
whole load → re-load → reconcile path on a fixture and asserts the mismatch markers appear.

What it does **not** compare is field-level content. There is no check that any order's
vehicle, due date, assignee or project type survived, because none of them are loaded — so
reconciliation reports ✅ on a migration that dropped them. A person spot-checking 20 orders
against MiniCRM will notice within the first one; the report will not.

### What I could not determine

The repo contains no MiniCRM schema dump, no real `raw/schema/project-*.json`, and
`mapping.example.json` is a plausible illustration with invented category and status ids. So I
cannot say which custom fields this account actually has, whether the vehicle plate exists as
a structured field or only inside project names, whether MiniCRM holds the order numbers
Autotherm prints, or how large the to-do history is. **Nobody can currently prove parity from
this repository** — it depends entirely on running `extract` + `manifest` against the live
account, which `docs/migration/README.md` correctly makes the week-1 task.

## Operational findings

1. **Forgotten password.** No self-service reset and no reset e-mail: `grep -rn "forgot\|reset"` finds only `POST /users/{id}/password` (`api/users.rs:168`), admin-only, which sets a temporary password and forces a change at next login. For 20 staff with an admin on site, fine. The uncovered case is **the admin forgetting their own password** — there is no second path, and recovery means a developer with `psql` running `create-admin` or an `UPDATE`. `api/users.rs:119` prevents deactivating the last admin but nothing prevents there being only one.
2. **Adding an employee.** Self-service for an admin: `POST /users` with e-mail, name, role and a temporary password (`api/users.rs:48`). No database insert needed. But the admin screen is a stub (`admin/page.tsx` → `UnavailableState`), so today it is a `curl` call.
3. **Server dies.** The documented path is four bullet points at the end of `README.md`: nightly `pg_dump` + WAL archiving off-site, object storage with versioning and replication, "test a restore before go-live, and yearly after". The repo contains **no** systemd unit, no Caddyfile, no backup script, no restore runbook and no `docker-compose.prod.yml` — `docker-compose.yml` is explicitly dev-only. Realistic data loss window is therefore whatever the person setting up the host chooses, and no restore has been tested because there is nothing to run. `/health` checks Postgres and object storage (`api/mod.rs:82`), which is a good start and is the only monitoring in the repo.
4. **Object storage full or unreachable mid-upload.** The design is sound: the client PUTs directly to storage with a presigned, checksum-bound request, then calls `/uploads/complete`, which HEADs the object and refuses on size or hash mismatch (`service/media.rs:233-256`). A failed PUT leaves no database row, and the ticket stays valid for two hours (`service/media.rs:26`), so a retry works. What happens on a phone I **cannot evaluate — the Android app is not in this repository**; whether it queues failed photos, retries on reconnect, or shows a red X and loses them is the single most consequential unknown in the MEO workflow.
5. **Order deleted by accident.** Cannot happen: there is no `DELETE /orders` route anywhere in `api/orders.rs`. Cancellation is a stage move and reversible with a note. Images and documents are soft-deleted with `deleted_by` (`repo/images.rs:181`), and intake photos refuse deletion at the trigger. This is well done.
6. **Nudge e-mails stopped three weeks ago.** Visible only to someone who looks: `GET /admin/status` (`api/admin.rs:92`) reports `failed_jobs`, `pending_jobs`, `emails_needing_attention`, `orders_missing_fx_rate` and `latest_eur_rate_day` — the right five numbers. But the admin screen is a stub, there is no alerting, and `/health` does not check whether the worker loop is alive. If the worker thread dies while the HTTP server lives, `/health` stays green, nudges stop, and `pending_jobs` climbs where nobody sees it. `settings.stalled_alert_recipients` exists for stalled *orders*, not for a stalled *worker*.
7. **Developer unavailable for two weeks.** Unfixable without them: any report question outside the five endpoints (no CSV, no query surface); anything needing `raw_import`; a locked-out sole admin; a stuck `process_image` backlog; a schema change. Fixable by staff: users, roles, passwords, stage definitions, project types, e-mail templates, suppression list, SMTP settings and the automatic-mail kill switch are all API-configurable (`api/configuration.rs`, `api/users.rs`) — though most have no screen yet, so "fixable" currently means "fixable with `curl`".

## Adoption risks

**The order detail screen is visibly broken right now.**
`frontend/src/app/[locale]/orders/[id]/page.tsx:177` and `:253` contain unbraced JSX
ternaries inside `<Tabs.Content>` — `editing ? (` and `canEdit ? (` are children of the
element, not expressions. Both branches render simultaneously, with the literal text
`editing ? (`, `) : (` and `)` printed on the page. This is in the compiled output
(`grep 'editing ? (' frontend/.next/server/app/[locale]/orders/[id]/page.js` matches), so it
ships. `REMEDIATION.md` records the Radix Tabs migration (MAJOR-11) as green on `tsc`, `lint`
and `build` — all three pass, because this is valid JSX. It means the most-used screen in the
system shows the edit form and the read-only view stacked on top of each other with source
code between them, and there are no frontend tests to catch it.

**Data entry versus MiniCRM.** Creating a lead is one required field (`LeadForm.tsx:126`,
title) and eight optional ones; creating an order is three required (title, partner, currency)
and ten optional (`OrderForm.tsx:183-275`). That is not more than MiniCRM asks for, and the
partner picker is a proper combobox. Order creation is heavier only in one respect: `currency`
is mandatory and locks once line items exist (`api/orders.rs:337`), so getting it wrong means
deleting every item to fix it.

**Where bad data gets in silently.** Three places:

- **An order with no line items is valid and reports as zero.** Nothing requires a value: `order_values` coalesces to 0 (`0006:44`) and `volume_by_month` counts the order while adding nothing to the total. Since project value is the owner's stated reason for the project, and since entering it is optional and lives behind a tab, the reports will quietly undercount from month one. This is the highest-probability failure in the whole review.
- **`valuation_date` defaults to today** (`0003:22`). Entering last month's job today values it at today's FX and files it in this month's volume. The field is editable and on the form, but the default is invisible to someone typing fast.
- **Vehicle fields are free text with no validation.** `ABC-123`, `ABC123` and `abc 123` all store; the plate search normalises (`domain/order.rs:26`) so lookup survives, but the same van will read three ways across three orders, and there is no duplicate detection.

**MEO photo upload: taps from launch to attached.** Not countable from this repository — the
Android app is not here. What the API forces is a lower bound of: authenticate (cached
session, 0), pick the order from `GET /mobile/orders` (`api/mobile.rs:66`, cached 60s,
open-orders-only, plate-searchable — a good picker), choose one of four image categories
(`UploadTarget::Image { category }` is mandatory, `media/upload_token.rs`), then camera →
confirm. That is at least **five** taps for the first photo, and the category choice is the
one that cannot be defaulted away, because `intake` is irreversible: `CHECK (category <>
'intake' OR immutable)` means a photo filed as intake by mistake can never be deleted or
moved. To get under four taps the app needs the order pre-selected (deep link or
"currently working on" state) and the category sticky per session, with a visible,
confirm-once banner when the sticky category is `intake`. Batch upload after that is cheap:
`(order_id, content_hash)` is unique, so retrying a whole batch is idempotent
(`0004:31`).

**What people will work around.** Three predictions, in order of confidence:

1. **Quotations stay in Outlook and Word**, because there is nowhere to put them. The lead's `quoted` stage becomes a checkbox somebody ticks late or never, and the real sales pipeline — who we quoted, for how much, and when it expires — stays in one salesperson's mailbox. This is the workaround that becomes the real process.
2. **Order values get entered at the end, in round numbers, or not at all**, because line items are optional, sit behind a tab, and nothing in the workflow blocks on them. The MEO photo gate (`min_images = 1` of `completion`) is the only gate in the system; there is no equivalent gate on value, so the one number the owner wants is the one nothing protects.
3. **Photos keep being taken on personal phones and shared in a chat group**, then uploaded in a batch later — or not. This one is winnable: the MEO gate blocks the move to `completed` without a completion photo, which is real leverage. It is also the reason the Android app's offline and retry behaviour (item 4 above) decides whether the whole evidence chain works.

## What I could not evaluate

- **The Android app.** Not in this repository. Every claim about taps, offline behaviour, upload retry and what a user sees when storage is unreachable is bounded by the API contract only.
- **The gallery, e-mail compose, correspondence, reports, settings and admin screens.** Stubs (`UnavailableState`) or absent. I assessed the specified design and the backing endpoints; I could not assess whether the specification survives contact with 120 photos, and `FRONTEND_PLAN.md` itself carries a provenance warning that it is a reconstruction, not the canonical plan.
- **MiniCRM's actual contents.** No schema dump, no real extract, no live credentials (see the parity section). The dropped-field list is derived from what `load.rs` writes, which is certain; what is *lost* depends on what the account holds, which is not.
- **Real behaviour at scale.** No migrated dataset exists here. Query plans, the `process_image` backlog, the 2 TB transfer and gallery response sizes are reasoned from the SQL, the worker loop and the response shapes, not measured.
- **Production deployment.** No systemd unit, no reverse-proxy config, no backup or restore scripts in the repo. Recovery-time and data-loss claims are about what is documented, not what is configured.
- **The canonical plan documents.** `autotherm-crm-backend-plan.md` and `autotherm-crm-frontend-plan.md` are not in this workspace; `FOLLOWUP.md` records the frontend one as missing. Section references in source comments (`§5`, `§12`, `§13`, `§14`) could not be checked against their source.
