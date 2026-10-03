# Decisions

Where the build deviates from or resolves open points in the original architecture plan.
Each entry says what was decided and why, so the next person (or agent) doesn't re-litigate it.

## Schema

**Every table is defined in the migrations.** The plan named `orders`, `leads`, `contacts`,
`documents`, `users`, `sessions` and `audit_log` without schemas. They now exist in
`backend/migrations/`. The scope rule "if it isn't in the schema, don't build it" refers to
the migrations, not to §2 of the plan. The full table list:

| Area | Tables |
|---|---|
| Foundation | `users`, `sessions`, `audit_log`, `settings` (single row), `jobs`, `user_settings` |
| Partners & leads | `partners`, `contacts`, `stage_definitions`, `leads`, `lead_stages` |
| Orders | `project_types`, `orders`, `order_items`, `order_stages`, `blockers`, `order_notes`, `order_specs`, `tasks` |
| Vehicles & inspections | `vehicles`, `order_vehicles`, `inspections`, `inspection_damages`, `inspection_photos`, `inspection_verdicts`, `inspection_signatures`, `inspection_notes`, `inspection_zone_templates` |
| Media | `images`, `documents` |
| Email | `email_templates`, `email_messages`, `email_suppressions`, `newsletter_subscriptions` |
| Invoicing | `invoices`, `invoice_lines`, `proformas` |
| Reporting | `fx_rates` + views `order_current_stage`, `lead_current_stage`, `order_stage_intervals`, `order_values` |

**No VAT on line items.** The plan both added `vat_rate` and said not to. It is not there.
VAT lives on `invoice_lines` instead (see Invoicing below): a rate belongs to a bill, not
to a job.

**Scope grew past the original plan, on purpose.** The plan and `docs/history/FRONTEND_PLAN.md`
listed invoicing as out of scope. It was built (migrations `0020`–`0022`, `0028`–`0030`),
along with tasks, the intake slip, handover inspections and a newsletter list. The README's
Scope section is the current list. Inventory, cost tracking, purchase orders, time
tracking, inbound mail and a customer portal remain out.

**Orders carry `number`, `project_type_id` and `valuation_date`.** Reports group by project
type (a configurable table, like stages) and normalise currency at the valuation date's
MNB rate. The valuation date defaults to the order's creation day and is editable; it is
never "today at report time".

**Order numbers are `YYYY-NNNN`**, allocated under a transaction-scoped advisory lock.
Imported MiniCRM orders keep whatever number they had.

**Line items share the order's currency, enforced by the database** (composite foreign key
`(order_id, currency)`). An order's currency cannot change while it has items.

**Line totals round half away from zero** to whole minor units — the same in `Money`
(Rust) and `round()` (Postgres). The order detail endpoint cross-checks both and logs an
error if they ever disagree.

**Stage history references stage keys by foreign key.** `order_stages`/`lead_stages` store
`stage_key` with a composite FK to `stage_definitions(entity, key)`, so a typo or a lead
stage on an order is impossible. Leads got their own history table.

**`clock_timestamp()` for stage entries**, not `now()`: `now()` is the transaction start and
can precede a concurrently committed change, scrambling history order.

**Migration numbers are never backfilled.** There is no `0023_*` migration (the sequence
jumps `0022` → `0024`). sqlx runs pending migrations in version order, so a file named
`0023_*` added later would run *between* them on fresh databases but never on existing
ones — a guaranteed schema divergence. The next migration is always max+1.

## Stage rules

- Forward moves may skip stages, but every image gate between current and target applies
  (skipping MEO does not bypass the MEO photo requirement).
- Backward moves are allowed with a note — design rework happens.
- Exit stages (`lost`, `cancelled`) are reachable from any open stage without gates.
  `cancelled` was added to the seeded order stages.
- Leaving a terminal stage is a reopen and needs a note.
- The MEO gate is `min_images = 1` of category `completion`, configurable per stage.
- A lead reaches `won` only through conversion; a converted lead can't change stage.

## Images

- **Originals are stored byte-for-byte, EXIF included.** Display copies (≤2560px) and
  thumbnails (≤480px) are re-encoded JPEGs, which strips EXIF/GPS by construction. The UI
  and emails use derived copies; originals need the `ViewOriginalImages` capability.
- **Direct-to-storage uploads bound to content.** The presigned PUT signs
  `x-amz-checksum-sha256`, so storage rejects any body other than the declared file. Keys
  are content-addressed (`orders/{id}/{category}/{sha256}.ext`). Finalize verifies size and
  checksum (or re-hashes if the store reports none) before inserting the row.
- **Upload tickets are stateless HMAC tokens** instead of a pending-uploads table. They are
  bound to user, order, key, hash and size, and expire after 2 hours.
- **Immutability is enforced three ways:** a DB trigger refuses delete/identity changes on
  immutable rows; S3 Object Lock (governance mode, `S3_INTAKE_LOCK_YEARS`) on intake
  objects; and the processing job re-verifies the stored hash.
- **Idempotent batches:** `(order_id, content_hash)` is unique, so a retried mobile upload
  returns the existing image instead of a duplicate.
- Not yet: RFC 3161 trusted timestamps (the hash alone isn't third-party proof), HEIC
  previews (originals are stored, previews report an error), multipart uploads > 5 GB.

## Email

- **SMTP via `lettre`, provider still open.** Works with Postmark/Resend/Mailgun SMTP or
  Autotherm's existing mail host.
- **`sending` is committed before the SMTP conversation starts.** A row found in `sending`
  by a later attempt goes to `needs_review`, never auto-resent. Timeouts mid-send also go
  to `needs_review`. That is the "same nudge fifteen times" guard.
- **Idempotency keys on automatic mail** (`nudge:{blocker}:{n}`, `stage:{stage_row}`,
  `stalled:{order}:{stage entry}:{recipient}:{ISO week}`) make double-queueing impossible.
- **Blocker nudges carry both `blocker_id` and `order_id`**, so they appear in the order's
  correspondence history.
- **Kill switch defaults to off.** Automatic mail found by the sender while it is off is
  cancelled, not held — switching it back on doesn't release a backlog of stale nudges.
- **Non-production can only reach a local SMTP sink** (localhost/mailpit), enforced in config.
- Manual mail with unresolved `{{variables}}` is rejected; automatic mail with them is marked
  `failed` (visible) instead of being sent.
- Customer stage-change notifications go out on forward moves only, when enabled in settings.
- Template bodies are plain text; the HTML part is derived and fully escaped. A body the
  office writes in Markdown keeps the author's own markup, but every `{{variable}}` value is
  backslash-escaped before parsing, so a record's data is always text, never a tag or link.
- **Newsletter signups are double opt-in.** The website form stores a pending row and
  mails a confirmation link (valid 7 days, at most one letter per address per day); only
  the click makes the address part of the audience, and only the click lifts an earlier
  opt-out. The public endpoint answers the same `202` whatever the address's history, and
  unsubscribing by typed address always answers "done", so neither reveals who is on the
  list. Office hand-adds are confirmed on insert: the office holds that consent. Rows from
  before migration `0031` were kept active. The confirmation letter is automatic mail, so it
  is cancelled while the kill switch is off: switch automatic mail on before putting the
  signup form live.

## Reporting

- Plain views, no materialized views, no refresh job — the data volume doesn't need them.
- Stage duration statistics use finished stage visits only; open visits are counted
  separately.
- MNB rates: SOAP over **plain http** (the https endpoint returns 404). Weekends/holidays
  use the latest rate within 10 days before the valuation date; beyond that, the
  normalised value is null and counted as `missing_fx`.

## MiniCRM migration

Procedure: [docs/migration/README.md](migration/README.md). Tool: `autocrm-migrate`.

- **Extract never transforms.** Raw JSON lands on disk once; everything after it reruns offline.
- **The manifest is structural.** It finds file URLs anywhere in the JSON instead of
  guessing MiniCRM field names, and prints counts per field for comparison with the UI.
- **Downloads are content-addressed** under `migration/originals/{sha256}.{ext}`, not
  `orders/{id}/...`: order ids don't exist until `load`, and the download has to start
  weeks before it. Rows point at whatever key the file has; `storage_key` is per row.
- **Originals are kept for every category**, not only intake/MEO. No re-encoding pass:
  storage is cheaper than deciding per photo which ones are evidence.
- **Migrated intake photos get object-lock retention at load time**, once their category is
  known from the mapping.
- **Load is idempotent** (upsert by `minicrm_id`) and keeps the full source record in
  `raw_import`. Re-running overwrites migrated fields, so stop re-running once staff start
  editing migrated records.
- **Migrated orders are numbered `MC-{MiniCRM id}`**, outside the `YYYY-NNNN` sequence.
- **Mapping lives in a JSON file**, not code: categories → order/lead/skip, statuses →
  stage keys, file fields → photo categories, optional value/currency fields.
- **Reconciliation is a Markdown report with a sign-off section.** It checks counts, missing
  orders, file coverage, per-order file counts and value by year.

## Invoicing

The full rule set, with sources, is `INV-*` in
[logic-audit/01-behavior-spec.md](logic-audit/01-behavior-spec.md). The decisions behind it:

- **NAV lives in a sidecar.** `nav-sidecar/` (Node, wrapping `open-nav`) owns the XML, the
  request signature, the token exchange and the polling. The backend sends flat JSON. It
  is optional: without `NAV_SIDECAR_URL` every invoicing endpoint refuses with a rule error.
- **The sidecar trusts one caller.** It holds the NAV technical user's credentials, so every
  route but `/health` requires `Authorization: Bearer` with `SIDECAR_TOKEN` (the backend's
  `NAV_SIDECAR_TOKEN`, ≥ 32 characters, required outside mock mode), and it listens on
  loopback by default. A token mismatch is retried, never recorded as a rejection: it is a
  deployment fault, and nothing reached NAV.
- **Reporting is asynchronous.** Issuing writes an invoice in `submitting` and queues one
  job per invoice (`nav_submit:{id}`). A NAV outage delays an invoice; it never loses one.
- **An invoice is a snapshot.** `invoice_lines` copies the order's figures with the VAT
  rate that applied, so editing the order later doesn't change the bill.
- **Numbers are `{prefix}{year}-{seq:04}`**, drawn under an advisory lock and never
  reused, not even after a rejection. Cash and transfer invoices share one series; each
  invoice stores its `payment_method`, which is `TRANSFER` or `CASH` and nothing else.
- **Corrections are stornos or technical annulments**, never edits. A storno is its own
  numbered document with negative amounts.
- **A retry re-files; the read-back decides.** The sidecar keeps nothing, so a submission
  retried after a timeout is filed again. When NAV answers `INVOICE_NUMBER_ALREADY_EXISTS`,
  the job reads back what NAV holds under the number and adopts it if the date and totals
  match ours: for invoices and stornos alike (an adopted storno marks its original
  `stornoed`, as a fresh one does). An annulment has no number to read back, so NAV's
  refusal of a retried annulment is recorded as "may already have been filed; check the
  portal", not as a plain refusal.
- **Proformas (díjbekérő) are not invoices.** They have their own number series and lock,
  are reported nowhere, and never consume an invoice number.
- **Számlázó is the billing desk.** A sidebar page with every invoice and storno plus
  every díjbekérő across orders (`GET /invoices`, `GET /proformas`, newest first, with
  order number and partner on each row), and a create panel on the side: pick an order
  and the per-order invoice/proforma sections open there. There is no "invoice from a
  proforma" operation: the invoice is built from the order's line items, the same items
  the proforma rendered, so invoicing from a proforma is picking its order.

## Client enumerations

- **The server owns every list the clients render.** `GET /config/lookups` publishes
  damage types, severities, verdicts, walkaround headings, fuel marks, heating fuels,
  defrost modes, order relations, task entity types, currencies, payment methods,
  annulment codes, image categories (with the immutable/attachable rules), email
  composer starters and the error-text catalog in one document (`backend/src/domain/
  lookups.rs`). Validation accepts exactly the published keys, so the two cannot drift.
  Changing, renaming or adding a value is a server deploy, never an app update. The
  phone caches the document for offline starts; unknown keys read as themselves,
  spaced out, never blank.
- **Cold start keeps static fallbacks on purpose.** A login failure happens before the
  first fetch, so the built-in error-text map (phone) and catalogue (web) stay; fresh
  server texts layer over them once a fetch has happened. Device preferences (theme),
  the humanized-key fallback itself and tone mappings stay local: they are presentation,
  not data.

## Handover inspections

The phone-only átvétel / kiadás walkarounds (`backend/migrations/0026`). Rules with sources:
`INSP-*` in [logic-audit/01-behavior-spec.md](logic-audit/01-behavior-spec.md).

- **The photo list depends on the vehicle kind and on the walkaround.** A bare chassis cab
  arriving for a box needs different photos from the finished vehicle leaving, and a
  chassis with a box different ones from a converted van. A list is addressed by (project
  type, walkaround kind); a project type without its own list is served the general one.
  The project type is the vehicle kind: it is already on every order and the office can
  rename and add to it without a deploy.
- **Lists are whole, never merged.** The old `default` + `cooling` lookup hid what a
  vehicle actually walked behind a union of two tables. What the office sees in settings
  is exactly what the phone walks. Migration `0032` turned the old behaviour into explicit
  lists, so nothing changed for the project types that already had the extras.
- **`checkout` is the first walkaround (átvétel, intake) and `checkin` the second (kiadás,
  outgo).** The names came from a rental-car model. They are API values stored on every
  inspection, so only the labels changed.
- **Every intake zone key exists at outgo.** A damage is compared between the walkarounds by
  zone key; an intake zone with no outgo twin would never be checked for new damage. A test
  holds this for the alváz-with-box lists.
- **Alváz with a box ("Hűtőfelépítmény") is seeded from the office's own example**: 38 photos
  of a finished vehicle. Outgo asks for all of them, in the order they were taken, plus an
  optional odometer shot. Intake asks for the ones that exist on the bare cab. Names and
  left/right were read off the photos and are editable in settings. A converted van has no
  list of its own yet: it keeps the general zones plus the cargo extras.
- **Zones have a short title**, sent with every inspection (`zone_titles`), so neither client
  keeps a table of zone names, and a list the office builds needs no app update.
- **The phone has no photo list of its own.** Which photos are needed, in which order, under
  which name, and whether each may be skipped is decided on the server and changed in
  settings (or by asking for it), with no app update. The phone downloads the lists when the
  Átvétel-átadás screen opens and keeps the last one per vehicle kind and walkaround, for
  walkarounds started in a yard with no signal. With nothing downloaded it refuses to start
  and says so, rather than guess from a list built into the app. A draft freezes its list at
  the start, so editing a list never changes a walkaround in progress. What still needs an
  app update is new behaviour: a new kind of photo step, say, not a new photo in a list.
- **A zone is required or optional, never both.** The two columns held one fact and could
  disagree (the roof was both); the phone reads one. They are now forced to be opposites.

## Auth

- Session tokens: 256-bit random, only their sha256 is stored. Web: httpOnly `SameSite=Lax`
  cookie (7-day idle / 30-day absolute). Mobile: bearer token (60-day idle / 365-day absolute).
- CSRF: cookie-authenticated state-changing requests must carry an allowed `Origin`.
- 10 failed logins lock the account for 15 minutes; unknown emails cost the same Argon2 time.
  The password is checked before the lock: a locked account with a wrong password answers
  401 like an unknown address, so locking cannot reveal which addresses have accounts; only
  the right password learns about the lock (429). Anyone can still lock a known account by
  guessing: per-IP throttling belongs in the reverse proxy (Caddy `rate_limit`), not here.
- Admin-created accounts must change their password before doing anything else.
- The last active admin can't be demoted or deactivated.

## Open questions (need a human)

1. **Transactional email provider** — or relay through Autotherm's existing mail host.
   SPF/DKIM/DMARC setup should start early; DNS and DMARC monitoring take calendar time.
2. **MiniCRM file access** — confirm the REST API can actually download attached images
   before relying on it for the 2 TB transfer.
3. **Retention/GDPR stance** for write-once intake photos and the never-deleted email log.
4. **Historical invoices** — read-only export vs. keeping MiniCRM accessible for a while.
5. **Invoice numbering continuity** when billing leaves MiniCRM (accountant question).
6. **Real stage names and project types** — the seeded ones are placeholders.
