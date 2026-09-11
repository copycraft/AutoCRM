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
| Foundation | `users`, `sessions`, `audit_log`, `settings` (single row), `jobs` |
| Partners & leads | `partners`, `contacts`, `stage_definitions`, `leads`, `lead_stages` |
| Orders | `project_types`, `orders`, `order_items`, `order_stages`, `blockers` |
| Media | `images`, `documents` |
| Email | `email_templates`, `email_messages`, `email_suppressions` |
| Reporting | `fx_rates` + views `order_current_stage`, `lead_current_stage`, `order_stage_intervals`, `order_values` |

**No VAT on line items.** The plan both added `vat_rate` and said not to. It is not there.
Invoicing will add it with the rest of the VAT handling.

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
- Template bodies are plain text; the HTML part is derived and fully escaped.

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

## Auth

- Session tokens: 256-bit random, only their sha256 is stored. Web: httpOnly `SameSite=Lax`
  cookie (7-day idle / 30-day absolute). Mobile: bearer token (60-day idle / 365-day absolute).
- CSRF: cookie-authenticated state-changing requests must carry an allowed `Origin`.
- 10 failed logins lock the account for 15 minutes; unknown emails cost the same Argon2 time.
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
