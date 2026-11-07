# API reference

Base path `/api`. JSON in, JSON out. The complete, current contract is
`openapi/openapi.json`, generated from the handlers (`cargo run -- openapi`); a running
server shows it at `/api/docs`. This page explains the conventions and indexes every
operation. Request and response shapes are in the contract.

Money is always integer **minor units** (fillér / eurocent) with an explicit currency;
quantities are decimal strings (`"2.5"`). Dates are `YYYY-MM-DD`, instants RFC 3339 UTC;
"today" means the Budapest business day.

## Conventions

**Auth.** Web: `POST /api/auth/login` sets an httpOnly `autocrm_session` cookie.
State-changing cookie requests must send an allowed `Origin` (browsers do this
automatically). Mobile: login with `"client": "mobile"` returns a `token`; send
`Authorization: Bearer <token>`. An account with a temporary password can only reach
`/auth/me`, `/auth/password` and `/auth/logout` until it changes it.

**Errors.** Always `{"error": {"code": "...", "message": "..."}}`. Every code, its status
and its user-facing text: [error-codes.md](error-codes.md). The broad shape:

| Status | Meaning |
|---|---|
| 400 | `validation`: the message names the field |
| 401 | `unauthenticated` |
| 403 | `forbidden` |
| 404 | `not_found` |
| 409 | The request conflicts with current state (`duplicate`, `immutable`, `invoice_in_flight`, …) |
| 422 | A business rule said no (`stage_gate`, `note_required`, `intake_slip_missing`, `currency_locked`, …) |
| 429 | `too_many_requests`: the account is temporarily locked (only answered to the right password) |

**Lists** return `{"items": [...]}` and accept `limit` (1–200, default 50) and `offset`.

**PATCH** bodies: omit a field to keep it, send `null` to clear it.

## Roles

Everyone signed in can read. Writes need a capability (`backend/src/domain/role.rs`):

| Capability | admin | office | designer | viewer |
|---|:-:|:-:|:-:|:-:|
| Users, settings, configuration, system operations, technical annulment | ✓ | | | |
| Edit partners / leads / orders, send email, issue invoices, delete media, view original photos | ✓ | ✓ | | |
| Change order stages, manage blockers, upload media | ✓ | ✓ | ✓ | |

**HR** (`/hr/*`) needs `AccessHr`: admins, plus any user an admin gave `hr_access`.
**Tasks** have no capability check: any signed-in user, viewers included, can create,
complete and delete them. Lead stage changes need EditLeads, not ChangeStages.

## Public endpoints

No login. The website calls the first two from its server, never from the browser.

| Path | Guarded by |
|---|---|
| `POST /leads/website` | `X-Leads-Key` (`LEADS_API_KEY`; off when unset) |
| `POST /newsletter/subscribe` | `X-Newsletter-Key` (`NEWSLETTER_API_KEY`; off when unset) |
| `GET /newsletter/confirm`, `GET /newsletter/unsubscribe` | The token in the link |
| `GET /newsletter/track/open`, `GET /newsletter/track/click` | The token; clicks are HMAC-signed |
| `GET /public/jobs/{slug}`, `POST /public/jobs/{slug}/applications` | The unguessable slug; drafts refuse |
| `GET /health` (outside `/api`) | Nothing; checks database and object storage |

`/api/docs` and `/api/docs/openapi.json` are open in `dev` and `staging` and admin-only in
`production`.

## Uploads

Photos and documents go straight to object storage, never through the API (web and
mobile alike):

1. `POST /orders/{id}/uploads` (or `/leads/{id}/uploads` for a document) with
   `{target: {type: "image", category} | {type: "document", kind}, filename, content_type, byte_size, sha256}`,
   where `sha256` is the hex digest of the file. Image categories: `intake`, `production`,
   `completion`, `marketing`, `inspection`. Document kinds: `design`, `cad`, `certificate`,
   `other`; `invoice` and `proforma` are for the PDFs the server files itself (the upload
   does not refuse them, but nothing should send them).
   - `{"status": "already_uploaded", image_id|document_id}`: nothing to do.
   - `{"status": "upload", ticket, upload: {method, url, headers: [[name, value], ...]}, expires_at}`.
2. Send the file with `upload.method` to `upload.url`, including **every** listed header.
3. `POST /uploads/complete` with `{ticket}` → `201` (new) or `200` (already recorded).

Thumbnails and display copies are made in the background; until then `thumb_url` is null.
Intake photos are write-once: deleting one answers `409 immutable`.

## Email

Every message is a row before it is sent. Statuses: `queued → sending → sent`, or
`failed`, `cancelled`, `needs_review` (delivery outcome unknown: check the provider before
retrying). Automatic mail passes, in order: the kill switch, the suppression list, a check
for unfilled `{{variables}}`, the send window and the per-recipient daily cap.

Blocker nudges go to `responsible_email`, else the responsible partner's email, once the due
date has passed, every `nudge_interval_days`, switching to the escalated template after
`nudge_escalate_after` nudges.

## Endpoints

Grouped by OpenAPI tag. Generated from `openapi/openapi.json`; notes are filled in where a
parameter or a rule is worth knowing without opening the contract.

### Auth and preferences

| Method | Path | Notes |
|---|---|---|
| POST | `/auth/login` | `{email, password, client?: "web"\|"mobile", device_label?}`; web gets the cookie, mobile a `token` |
| POST | `/auth/logout` | Revokes the current session |
| GET | `/auth/me` |  |
| POST | `/auth/password` | `{current_password, new_password}`; signs out other devices |
| GET | `/auth/sessions` | Own active sessions; `current: true` marks this one |
| DELETE | `/auth/sessions/{id}` |  |
| GET | `/auth/preferences` | Density and page size, per user |
| PUT | `/auth/preferences` |  |
| GET | `/auth/providers` |  |
| GET | `/auth/google/start` |  |
| GET | `/auth/google/callback` |  |
| GET | `/auth/two-factor` |  |
| POST | `/auth/two-factor/setup` |  |
| POST | `/auth/two-factor/enable` |  |
| POST | `/auth/two-factor/disable` |  |
| GET | `/auth/calendar` |  |
| POST | `/auth/calendar` |  |
| DELETE | `/auth/calendar` |  |
| GET | `/calendar/{file}` |  |

### Users (admin)

| Method | Path | Notes |
|---|---|---|
| GET | `/users` |  |
| POST | `/users` | `{email, display_name, role, temporary_password}` |
| PATCH | `/users/{id}` | `{display_name?, role?, is_active?, hr_access?}`; the last active admin cannot be demoted |
| POST | `/users/{id}/password` | `{temporary_password}`; forces a change at next login |
| POST | `/users/{id}/revoke-sessions` |  |
| DELETE | `/users/{id}/two-factor` |  |

### Configuration

| Method | Path | Notes |
|---|---|---|
| GET | `/stage-definitions` | `entity=lead\|order` |
| POST | `/stage-definitions` |  |
| PATCH | `/stage-definitions/{id}` | The key is permanent; label, position, gates, stall days, active |
| GET | `/project-types` |  |
| POST | `/project-types` |  |
| PATCH | `/project-types/{id}` |  |
| GET | `/settings` | Kill switch, send window, caps, nudge cadence, notifications, stalled-alert recipients, SMTP override |
| PUT | `/settings` |  |
| GET | `/config/lookups` | Every client-facing enumeration in one document: damage types, severities, verdicts, fuel levels, task entity types, currencies, invoice… |
| GET | `/stage-photo-categories` | Which category a new photo takes while an order is in each stage |
| PUT | `/stage-photo-categories/{key}` |  |

### Partners and contacts

| Method | Path | Notes |
|---|---|---|
| GET | `/partners` | `q`, `kind=business\|person`, `include_archived` |
| POST | `/partners` | HU tax numbers normalised to `12345678-1-23` |
| GET | `/partners/{id}` | Partner, contacts and orders |
| PATCH | `/partners/{id}` |  |
| POST | `/partners/{id}/archive` |  |
| POST | `/partners/{id}/unarchive` |  |
| GET | `/partners/{id}/contacts` |  |
| POST | `/partners/{id}/contacts` |  |
| PATCH | `/contacts/{id}` |  |
| POST | `/contacts/{id}/archive` |  |
| PUT | `/partners/{id}/invoice-language` |  |
| POST | `/partners/bulk-actions` |  |

### Leads, tags, lost reasons, website enquiries

| Method | Path | Notes |
|---|---|---|
| GET | `/lead-tags` |  |
| POST | `/lead-tags` |  |
| PATCH | `/lead-tags/{id}` |  |
| PUT | `/lead-tags/order` |  |
| PUT | `/leads/{id}/tags` |  |
| GET | `/lost-reasons` | Why leads are lost: the choices offered when a lead moves to "Elveszett" |
| POST | `/lost-reasons` |  |
| PUT | `/lost-reasons/{id}` |  |
| GET | `/leads` | `q`, `stage`, `assigned_to`, `tag`, `open`, `sort` |
| POST | `/leads` |  |
| POST | `/leads/website` | Website enquiry: `POST` the form with `X-Leads-Key` |
| GET | `/leads/{id}` |  |
| PATCH | `/leads/{id}` |  |
| POST | `/leads/{id}/stage` | `{stage, note?, lost_reason_id?}`; `won` only via convert |
| GET | `/leads/{id}/transitions` | Every target with `manual`, `requires_note`, `gates_met` |
| POST | `/leads/{id}/convert` | Order body; `partner_id` optional if the lead has one |
| POST | `/leads/{id}/quotation` | Send the quotation letter for a lead: hero band on top, the lead's quotation PDF attached, from the staff member as themselves |
| GET | `/leads/quotes/expiring` | Quotes running out soon on open leads |
| POST | `/leads/bulk-actions` | Assign, tag or move several leads at once |
| GET | `/leads/{id}/conversation` | The whole correspondence with the lead's customer, both ways, oldest first: our letters (manual and automatic, also those about the… |
| GET | `/lead-sources` |  |
| POST | `/lead-sources` |  |
| PATCH | `/lead-sources/{key}` |  |

### Quote follow-ups and reminder steps

| Method | Path | Notes |
|---|---|---|
| GET | `/followup-steps` |  |
| POST | `/followup-steps` |  |
| PATCH | `/followup-steps/{id}` |  |
| GET | `/leads/{id}/followups` |  |
| POST | `/leads/{id}/followups` | One more follow-up for this lead, counted from now |
| POST | `/leads/{id}/followups/cancel` | Stops every follow-up still waiting for this lead |
| POST | `/followups/{id}/cancel` |  |

### Orders, stages, items, build spec

| Method | Path | Notes |
|---|---|---|
| GET | `/orders` | `q` (number, title, partner, VIN, plate in any spelling), `stage`, `partner_id`, `project_type_id`, `assigned_to`, `open`, `sort` |
| POST | `/orders` | `{partner_id, currency, title, …, items?: [{description, quantity, unit_price}]}` |
| GET | `/orders/{id}` | Order, partner, stage with `days_in_stage`, items, value (with HUF), blockers, image counts |
| PATCH | `/orders/{id}` | Currency locked while items exist; intake slip fields live here |
| POST | `/orders/{id}/stage` | `{stage, note?}`; forward may skip but gates apply; backward/reopen need `note` |
| GET | `/orders/{id}/transitions` | Every target with `manual`, `requires_note`, `gates_met` |
| POST | `/orders/bulk-actions` | Assign or move several orders at once |
| PUT | `/orders/{id}/cooling-serial` | The cooling unit's serial number, typically scanned off its plate on the phone |
| GET | `/orders/{id}/stages` |  |
| GET | `/orders/{id}/audit` |  |
| GET | `/orders/{id}/notes` | Imported MiniCRM to-do history |
| GET | `/orders/{id}/spec` |  |
| PUT | `/orders/{id}/spec` | Build spec; the form (heating/cooling) comes from the project type |
| GET | `/orders/{id}/items` |  |
| POST | `/orders/{id}/items` |  |
| PATCH | `/order-items/{id}` |  |
| DELETE | `/order-items/{id}` |  |

### Blockers

| Method | Path | Notes |
|---|---|---|
| GET | `/blockers` | All open; `responsible_partner_id` |
| GET | `/orders/{id}/blockers` |  |
| POST | `/orders/{id}/blockers` | `{what, responsible_partner_id?, responsible_email?, due_date?, notes?, nudge_enabled?=true}` |
| PATCH | `/blockers/{id}` |  |
| POST | `/blockers/{id}/resolve` | `{note?}` |
| POST | `/blockers/{id}/reopen` |  |

### Vehicles

| Method | Path | Notes |
|---|---|---|
| GET | `/vehicles` |  |
| POST | `/vehicles` |  |
| GET | `/vehicles/{id}` |  |
| PATCH | `/vehicles/{id}` |  |
| GET | `/orders/{id}/vehicles` |  |
| POST | `/orders/{id}/vehicles` | One job can cover several identical vans, each with its own MEO photos and certificate |
| DELETE | `/orders/{order_id}/vehicles/{vehicle_id}` |  |
| GET | `/vehicles/decode/{vin}` |  |

### Handover inspections

| Method | Path | Notes |
|---|---|---|
| GET | `/inspections` |  |
| POST | `/inspections` | Phone only in practice; `client_key` makes a retried create return the same row |
| GET | `/inspections/{id}` |  |
| PATCH | `/inspections/{id}` |  |
| DELETE | `/inspections/{id}` |  |
| POST | `/inspections/{id}/photos` |  |
| POST | `/inspections/{id}/damages` |  |
| DELETE | `/inspections/{id}/damages/{damage_id}` |  |
| POST | `/inspections/{id}/signatures` |  |
| POST | `/inspections/{id}/sign` |  |
| POST | `/inspections/{id}/notes` |  |
| GET | `/inspections/{id}/comparison` | Kiadás damages against the átvétel, zone by zone |
| POST | `/inspections/{id}/verdicts` |  |
| GET | `/inspections/templates` | Zone list for a project type and walkaround kind |
| PUT | `/inspections/templates` | Replaces one whole list, in order |
| DELETE | `/inspections/templates` | Removes a project type's own list, so it is served the general one again |
| PUT | `/inspections/{id}/tyres` |  |
| POST | `/inspections/{id}/videos` |  |

### Photos, documents and uploads

| Method | Path | Notes |
|---|---|---|
| POST | `/orders/{id}/uploads` | Step 1 of the upload flow (above) |
| POST | `/leads/{id}/uploads` | Same, for a document filed on a lead |
| GET | `/leads/{id}/documents` | V2.4: documents of a lead — the quotation, before any order exists |
| GET | `/documents` | Across records: `kind`, `expiring_before`, … |
| PATCH | `/documents/{id}` | Validity and vehicle on an existing document (V2.5, V2.1) |
| DELETE | `/documents/{id}` |  |
| POST | `/uploads/complete` | Step 3: `{ticket}` → 201 new, 200 already recorded |
| GET | `/orders/{id}/images` | `category`; presigned `thumb_url`, `display_url` (1 h) |
| GET | `/images/{id}/original` | Original file URL and sha256; needs ViewOriginalImages |
| PATCH | `/images/{id}` |  |
| DELETE | `/images/{id}` | Intake photos → `409 immutable` |
| GET | `/orders/{id}/documents` | The current version of every document of an order (older versions: `GET /documents/{id}/versions`) |
| GET | `/documents/{id}/download` |  |
| GET | `/images/{id}/annotations` |  |
| PUT | `/images/{id}/annotations` | Replaces the shapes drawn over a photo |
| GET | `/images/{id}/timestamp` | The authority's signed answer (RFC 3161 TimeStampResp) for an evidence photo |
| GET | `/orders/{id}/images/zip` | An order's photos as one ZIP, a folder per category, streamed as it is read from storage |
| GET | `/documents/{id}/preview` |  |
| GET | `/documents/{id}/versions` |  |

### Mobile

| Method | Path | Notes |
|---|---|---|
| GET | `/mobile/orders` | Compact order picker, `Cache-Control: private, max-age=60` |

### Email and templates

| Method | Path | Notes |
|---|---|---|
| GET | `/emails` | `order_id` \| `lead_id` \| `partner_id`, `status`, `attention`, `q` |
| POST | `/emails` | `{order_id?\|lead_id?\|partner_id?, to, cc?, template_key?, subject?, body?, body_markdown?, hero?, attachment_document_ids?, embed_document_ids?}` → 202 |
| POST | `/emails/preview` | Same body as send; rendered subject/body, `unresolved`, `recipient_suppressed` |
| GET | `/emails/{id}` |  |
| POST | `/emails/{id}/cancel` | Own queued mail, or any with OperateSystem |
| POST | `/emails/{id}/retry` | Admin; failed or needs-review |
| GET | `/email-templates` |  |
| POST | `/email-templates` |  |
| GET | `/email-templates/variables` | The variable whitelist with descriptions |
| PATCH | `/email-templates/{id}` | Unknown variables rejected; a template in use cannot go to the bin |
| POST | `/email-templates/{id}/copy` | A copy to edit: "<name> (másolat)", same area and folder, a fresh key |
| POST | `/email-templates/preview` | A template as it would read for a real order, lead or partner, while it is being edited (nothing is saved or sent) |
| GET | `/email-suppressions` |  |
| POST | `/email-suppressions` |  |
| DELETE | `/email-suppressions/{email}` | Admin |

### Newsletter

| Method | Path | Notes |
|---|---|---|
| GET | `/newsletter/subscriptions` | The list as the office sees it, unsubscribed included |
| POST | `/newsletter/subscriptions` | `{email, name?}` hand-add, confirmed on insert |
| DELETE | `/newsletter/subscriptions/{id}` |  |
| POST | `/newsletter/send` | Legacy single blast with everyone in BCC; no screen uses it |
| POST | `/newsletter/subscribe` | Public, `X-Newsletter-Key`; always 202; mails a confirmation link |
| GET | `/newsletter/unsubscribe` | Public; `?token=` or `?email=`; always 200, never reveals membership |
| GET | `/newsletter/confirm` | Public; `?token=` from the confirmation letter, valid 7 days |
| GET | `/newsletter/sends` |  |
| POST | `/newsletter/sends` | Tracked send: one letter per reader, to all or to chosen tags, now or later |
| GET | `/newsletter/sends/{id}/stats` | Opens, clicks and unsubscribes |
| POST | `/newsletter/sends/{id}/cancel` |  |
| GET | `/newsletter/track/open` | Public; the open pixel |
| GET | `/newsletter/track/click` | Public; HMAC-signed redirect, never an open redirect |
| GET | `/newsletter/tags` |  |
| POST | `/newsletter/tags` |  |
| PATCH | `/newsletter/tags/{id}` |  |
| PUT | `/newsletter/tags/order` |  |
| GET | `/newsletter/subscribers` | The subscriber list, a page at a time, newest first, each row with its tag ids |
| GET | `/newsletter/subscribers/counts` |  |
| PUT | `/newsletter/subscriptions/{id}/tags` |  |
| POST | `/newsletter/subscriptions/tags` | Tag or untag many subscribers at once (the ticked rows of the list) |
| PUT | `/newsletter/subscriptions/{id}/language` |  |
| POST | `/newsletter/subscriptions/bulk-actions` |  |
| POST | `/newsletter/import` | Pasted addresses join confirmed; opted-out ones are skipped |
| GET | `/newsletter/audience` | How many readers a tag selection reaches |

### Notifications

| Method | Path | Notes |
|---|---|---|
| GET | `/notifications` | Own feed and unread count |
| POST | `/notifications/read` |  |
| POST | `/notifications/read-all` |  |

### Invoices and proformas

| Method | Path | Notes |
|---|---|---|
| GET | `/invoices` | Every invoice and storno with order and partner, newest first |
| GET | `/invoices/buckets` | Counts per list: to issue, issued, paid, archived, stornoed, storno |
| POST | `/invoices/{id}/paid` | `{paid}`: mark a transfer invoice paid, or not |
| POST | `/invoices/{id}/reminders` | `{off}`: stop or resume payment reminders for one invoice |
| GET | `/invoices/overdue` | Unpaid invoices past their deadline, oldest first |
| GET | `/proformas` |  |
| GET | `/orders/{id}/invoices` |  |
| POST | `/orders/{id}/invoices` | Issue; the row is `submitting` until the NAV job decides |
| GET | `/invoices/{id}` |  |
| POST | `/invoices/{id}/storno` | A numbered storno with negative amounts |
| POST | `/invoices/{id}/annul` | Admin; technical annulment, approved later in the NAV portal |
| GET | `/invoices/{id}/chain` | The invoice's modification chain, straight from NAV |
| POST | `/invoices/{id}/pdf` | Fetch the PDF again from the sidecar |
| GET | `/orders/{id}/proformas` |  |
| POST | `/orders/{id}/proformas` | Díjbekérő, rendered inline, never reported |
| GET | `/invoices/{id}/payments` |  |
| POST | `/invoices/{id}/payments` |  |
| DELETE | `/invoices/{id}/payments/{payment_id}` |  |
| GET | `/partners/{id}/statement` | A partner's statement of account (folyószámla-kivonat, egyenlegközlő): invoices and stornos issued in the period, older invoices still… |

### Incoming invoices

| Method | Path | Notes |
|---|---|---|
| GET | `/incoming-invoices` |  |
| GET | `/incoming-invoices/buckets` |  |
| GET | `/incoming-invoices/suppliers` | Suppliers seen before, for filling in the next invoice from the same one |
| GET | `/incoming-invoices/{id}` |  |
| PATCH | `/incoming-invoices/{id}` |  |
| DELETE | `/incoming-invoices/{id}` |  |
| GET | `/incoming-invoices/{id}/file` |  |
| POST | `/incoming-invoices/bulk-actions` |  |
| POST | `/incoming-invoices/upload` | The file first; the figures are filled in after |

### Reports

| Method | Path | Notes |
|---|---|---|
| GET | `/reports/volume` | `group=month\|partner\|project_type`, `include_cancelled`; HUF, EUR, normalised HUF, `missing_fx` |
| GET | `/reports/stage-durations` | `project_type_id`; avg/median/p90 days over finished visits |
| GET | `/reports/throughput` | Completed per month, median/avg lead time |
| GET | `/reports/workload` | Daily workshop load: placed, completed and in-workshop cars per business-tz day |
| GET | `/reports/stalled` | Open orders past their stage's `stall_after_days` |
| GET | `/reports/blocker-load` | Per responsible party: open, overdue, nudges, waiting days |
| GET | `/reports/fx-rates` | Stored MNB rates |
| GET | `/reports/lead-sources` | Leads and wins per channel, campaign, landing page |
| GET | `/reports/website-conversion` | Website leads to orders, with lost reasons |
| GET | `/reports/sales-funnel` | The period's new leads, and how far they got |
| GET | `/reports/salespeople` | Each salesperson's leads from the period: quoted, won, lost, value, response time |
| GET | `/reports/first-response` | How long new leads waited for a first human answer (a hand-written email or a stage moved by a person), by source |
| GET | `/reports/revenue-by-country` | Order value per customer country and year (the export markets next to Hungary) |
| GET | `/reports/cumulative-flow` | How many orders sat in each stage at the end of each week: widening bands are bottlenecks |
| GET | `/reports/newsletter-trends` | Subscribers joining and leaving by month, and each tracked send's reach |
| GET | `/reports/weekly/recipients` |  |
| PUT | `/reports/weekly/recipients` |  |

### Search

| Method | Path | Notes |
|---|---|---|
| GET | `/search` | Every word must match; accents, punctuation, phone and plate spellings ignored |

### Tasks

| Method | Path | Notes |
|---|---|---|
| GET | `/tasks` | The caller's open tasks: assigned to them or created by them, most urgent first |
| POST | `/tasks` | Open to every signed-in user (no capability check) |
| GET | `/tasks/for/{entity}/{id}` |  |
| POST | `/tasks/{id}/done` |  |
| DELETE | `/tasks/{id}` |  |

### History

| Method | Path | Notes |
|---|---|---|
| GET | `/timeline/{entity}/{id}` | lead, order, partner, employee or incoming invoice |

### HR (AccessHr)

| Method | Path | Notes |
|---|---|---|
| GET | `/hr/employees` |  |
| POST | `/hr/employees` |  |
| GET | `/hr/employees/{id}` |  |
| PATCH | `/hr/employees/{id}` |  |
| POST | `/hr/employees/{id}/archive` |  |
| POST | `/hr/employees/{id}/unarchive` |  |
| PUT | `/hr/employees/{id}/photo` | The profile picture: the raw image bytes (JPEG, PNG or WebP, up to 8 MB) as the body |
| DELETE | `/hr/employees/{id}/photo` |  |
| GET | `/hr/employees/{id}/details` |  |
| PUT | `/hr/employees/{id}/details` |  |
| GET | `/hr/employees/{id}/documents` |  |
| POST | `/hr/employees/{id}/documents` |  |
| DELETE | `/hr/employees/{id}/documents/{doc_id}` |  |
| GET | `/hr/documents/expiring` | Documents that ran out or run out within 30 days, across all employees |
| GET | `/hr/employees/{id}/documents/{doc_id}/file-url` |  |
| POST | `/hr/employees/{id}/documents/{doc_id}/file` | The scan of a document: the raw file as the body (PDF, JPEG, PNG or WebP, at most 25 MB), its name in `?filename=` |
| PUT | `/hr/employees/{id}/user` |  |
| GET | `/hr/checklist-items` |  |
| POST | `/hr/checklist-items` |  |
| PUT | `/hr/checklist-items/{id}` |  |
| DELETE | `/hr/checklist-items/{id}` |  |
| POST | `/hr/employees/{id}/checklists` |  |
| GET | `/hr/statuses` |  |
| POST | `/hr/statuses` |  |
| PATCH | `/hr/statuses/{id}` |  |
| PUT | `/hr/statuses/order` |  |
| PUT | `/hr/employees/{id}/status` | Moves the employee to a status |
| GET | `/hr/absences` | Absences touching a date range: the team calendar |
| POST | `/hr/employees/{id}/absences` |  |
| DELETE | `/hr/absences/{id}` |  |
| GET | `/hr/leave-summary` | Allowance, used and remaining per employee for a year |
| GET | `/hr/absences/{id}/file-url` |  |
| POST | `/hr/absences/{id}/file` | The paper behind an absence — a doctor's certificate (orvosi igazolás) for sick leave: the raw file as the body (PDF, JPEG, PNG or WebP,… |
| GET | `/hr/jobs` |  |
| POST | `/hr/jobs` | Creates a draft and returns it with its link |
| GET | `/hr/jobs/{id}` |  |
| PATCH | `/hr/jobs/{id}` |  |
| DELETE | `/hr/jobs/{id}` | A listing nobody applied to can go |
| POST | `/hr/jobs/{id}/publish` | Opens the public form |
| POST | `/hr/jobs/{id}/close` | Stops accepting applications; the link then says the position is closed |
| GET | `/hr/jobs/{id}/applications` |  |
| PATCH | `/hr/applications/{id}` |  |
| DELETE | `/hr/applications/{id}` | Deletes the profile and its resume file: the applicant is not getting the job |

### Recruitment

| Method | Path | Notes |
|---|---|---|
| GET | `/public/jobs/{slug}` | Public; the posting behind the link |
| POST | `/public/jobs/{slug}/applications` | Public; multipart with the resume (PDF/DOC/DOCX/ODT/RTF, ≤ 10 MB) |

### Assistant

| Method | Path | Notes |
|---|---|---|
| GET | `/assistant` | Whether the assistant is configured |
| POST | `/assistant/chat` | Read-only answers and list filters |

### Admin

| Method | Path | Notes |
|---|---|---|
| GET | `/admin/status` | Email mode, kill switch, failed/pending jobs, emails needing attention, FX coverage |
| GET | `/admin/jobs` | `state=failed\|pending` |
| POST | `/admin/jobs/{id}/retry` |  |
| POST | `/admin/fx/fetch` | `{from, to}` |
| POST | `/admin/run/{kind}` | `nudge_blockers` \| `stalled_orders` |
| POST | `/admin/email/test` | One test letter through the current transport |

### MiniCRM provenance

| Method | Path | Notes |
|---|---|---|
| GET | `/partners/{id}/raw-import` |  |
| GET | `/leads/{id}/raw-import` |  |
| GET | `/orders/{id}/raw-import` |  |

### Comments

| Method | Path | Notes |
|---|---|---|
| GET | `/comments` |  |
| POST | `/comments` |  |
| PATCH | `/comments/{id}` |  |
| DELETE | `/comments/{id}` |  |
| GET | `/comments/mentionable` | Everyone who can be mentioned: the active users, by name |

### Yard

| Method | Path | Notes |
|---|---|---|
| GET | `/yard/locations` |  |
| POST | `/yard/locations` |  |
| PATCH | `/yard/locations/{id}` |  |
| GET | `/yard/board` |  |
| POST | `/yard/moves` |  |
| GET | `/vehicles/{id}/moves` |  |

### Incidents

| Method | Path | Notes |
|---|---|---|
| GET | `/incidents` |  |
| POST | `/incidents` |  |
| GET | `/incidents/{id}` |  |
| PATCH | `/incidents/{id}` |  |
| POST | `/incidents/{id}/rework` | Opens the rework job for an incident that has none yet |

### Ad platforms (public)

| Method | Path | Notes |
|---|---|---|
| GET | `/webhooks/meta` | Meta's subscription check: echoes the challenge when the verify token matches \`META_VERIFY_TOKEN\` |
| POST | `/webhooks/meta` | A lead form was filled in |
| GET | `/ads/google/conversions.csv` | Won leads that came from a Google Ads click, in Google's offline-conversion CSV format |
