# API reference

Base path `/api`. JSON in, JSON out. Money is always integer **minor units** (fillér /
eurocent) with an explicit currency; quantities are decimal strings (`"2.5"`).

## Conventions

**Auth.** Web: `POST /api/auth/login` sets an httpOnly `autocrm_session` cookie. State-changing
cookie requests must send an allowed `Origin` (browsers do this automatically). Mobile: login
with `"client": "mobile"` returns a `token`; send `Authorization: Bearer <token>`.

**Errors.** Always `{"error": {"code": "...", "message": "..."}}`.

| Status | Codes |
|---|---|
| 400 | `validation` |
| 401 | `unauthenticated` |
| 403 | `forbidden` |
| 404 | `not_found` |
| 409 | `duplicate`, `immutable`, `already_converted`, `already_resolved`, `not_retryable`, … |
| 422 | `stage_gate`, `note_required`, `invalid_transition`, `use_conversion`, `lead_converted`, `currency_locked`, `password_change_required`, `upload_missing`, `upload_mismatch`, `invalid_ticket`, `last_admin`, `invalid_reference`, `constraint_violation` |
| 429 | `too_many_requests` (account temporarily locked; only answered to the right password — a wrong one gets `401` like an unknown address) |

**Lists** return `{"items": [...]}` and accept `limit` (1–200, default 50) and `offset`.

**PATCH** bodies: omit a field to keep it, send `null` to clear it.

**Roles.** Everyone signed in can read. Writes need a capability:

| Capability | admin | office | designer | viewer |
|---|:-:|:-:|:-:|:-:|
| Users, settings, configuration, system operations | ✓ | | | |
| Edit partners / leads / orders, send email, delete media, view original photos | ✓ | ✓ | | |
| Change stages, manage blockers, upload media | ✓ | ✓ | ✓ | |

## Auth & users

| Method | Path | Notes |
|---|---|---|
| POST | `/auth/login` | `{email, password, client?: "web"\|"mobile", device_label?}` |
| POST | `/auth/logout` | revokes the current session |
| GET | `/auth/me` | |
| POST | `/auth/password` | `{current_password, new_password}` — signs out other devices |
| GET | `/auth/sessions` | own active sessions, `current: true` marks this one |
| DELETE | `/auth/sessions/{id}` | |
| GET / POST | `/users` | admin; create: `{email, display_name, role, temporary_password}` |
| PATCH | `/users/{id}` | `{display_name?, role?, is_active?}` |
| POST | `/users/{id}/password` | `{temporary_password}` — forces change at next login |
| POST | `/users/{id}/revoke-sessions` | |

## Partners & contacts

| Method | Path | Notes |
|---|---|---|
| GET | `/partners` | `q`, `kind=business\|person`, `include_archived` |
| POST | `/partners` | `{kind, name, tax_number?, eu_tax_number?, country?="HU", default_currency?="HUF", email?, phone?, website?, postal_code?, city?, address_line?, notes?}` — HU tax numbers normalised to `12345678-1-23` |
| GET | `/partners/{id}` | partner + contacts + orders |
| PATCH | `/partners/{id}` | |
| POST | `/partners/{id}/archive`, `/partners/{id}/unarchive` | |
| GET / POST | `/partners/{id}/contacts` | `{name, email?, phone?, position?, notes?}` |
| PATCH | `/contacts/{id}` | |
| POST | `/contacts/{id}/archive` | |

## Leads

| Method | Path | Notes |
|---|---|---|
| GET | `/leads` | `q`, `stage`, `assigned_to`, `open` |
| POST | `/leads` | `{title, partner_id?, contact_id?, contact_name?, contact_email?, contact_phone?, source?, description?, assigned_to?}` |
| GET | `/leads/{id}` | lead + current stage + history + converted order ref |
| PATCH | `/leads/{id}` | |
| POST | `/leads/{id}/stage` | `{stage, note?}` — `won` only via convert |
| POST | `/leads/{id}/convert` | order body (below); `partner_id` optional if the lead has one; title defaults to the lead's |

## Orders

| Method | Path | Notes |
|---|---|---|
| GET | `/orders` | `q` (number, title, partner, VIN, plate ignoring spaces/dashes), `stage`, `partner_id`, `project_type_id`, `assigned_to`, `open` |
| POST | `/orders` | `{partner_id, currency: "HUF"\|"EUR", title, contact_id?, project_type_id?, valuation_date?, vehicle_make?, vehicle_model?, vehicle_plate?, vehicle_vin?, description?, due_date?, assigned_to?, items?: [{description, quantity, unit_price}]}` |
| GET | `/orders/{id}` | order, partner, stage (`days_in_stage`), items with `line_total_minor`, `value` (total + HUF normalisation), blockers, `image_counts` |
| PATCH | `/orders/{id}` | currency locked while items exist |
| POST | `/orders/{id}/stage` | `{stage, note?}` — forward may skip but gates apply; backward/reopen need `note` |
| GET | `/orders/{id}/stages` | history with `entered_at`, `left_at`, who, note |
| GET | `/orders/{id}/audit` | |
| GET / POST | `/orders/{id}/items` | `{description, quantity, unit_price, position?}` |
| PATCH / DELETE | `/order-items/{id}` | |

## Blockers

| Method | Path | Notes |
|---|---|---|
| GET | `/blockers` | all open; `responsible_partner_id` |
| GET / POST | `/orders/{id}/blockers` | `{what, responsible_partner_id?, responsible_email?, due_date?, notes?, nudge_enabled?=true}` |
| PATCH | `/blockers/{id}` | |
| POST | `/blockers/{id}/resolve` | `{note?}` |
| POST | `/blockers/{id}/reopen` | |

Nudges go to `responsible_email`, else the responsible partner's email, once the due date has
passed, every `nudge_interval_days`, switching to the escalated template after
`nudge_escalate_after` nudges.

## Images & documents

Upload flow (web and mobile):

1. `POST /orders/{id}/uploads` with
   `{target: {type: "image", category: "intake"|"production"|"completion"|"marketing"} | {type: "document", kind: "design"|"cad"|"other"}, filename, content_type, byte_size, sha256}`
   (`sha256` = hex of the file).
   - `{"status": "already_uploaded", image_id|document_id}` — nothing to do.
   - `{"status": "upload", ticket, upload: {method, url, headers: [[name, value], ...]}, expires_at}`.
2. Send the file with `upload.method` to `upload.url`, including **every** listed header.
3. `POST /uploads/complete` with `{ticket}` → `201` (new) or `200` (already recorded).

Thumbnails and display copies are generated in the background; until then `thumb_url` is null.

| Method | Path | Notes |
|---|---|---|
| GET | `/orders/{id}/images` | `category`; includes presigned `thumb_url`, `display_url` (1 h) |
| GET | `/images/{id}/original` | original file URL + sha256; needs ViewOriginalImages |
| DELETE | `/images/{id}` | intake photos → `409 immutable` |
| GET | `/orders/{id}/documents` | |
| GET | `/documents/{id}/download` | presigned URL |
| DELETE | `/documents/{id}` | |
| GET | `/mobile/orders` | `q`, `all` — compact order picker, `Cache-Control: private, max-age=60` |

## Email

| Method | Path | Notes |
|---|---|---|
| GET | `/emails` | `order_id` | `lead_id` | `partner_id` (includes its orders and leads), `status`, `attention`, `q` (subject/recipient, server-side) |
| GET | `/emails/{id}` | full rendered message |
| POST | `/emails/preview` | same body as send; returns rendered subject/body, `unresolved`, `recipient_suppressed` |
| POST | `/emails` | `{order_id?\|lead_id?\|partner_id?, to, cc?, template_key?, subject?, body?, body_markdown?, hero?, attachment_document_ids?, embed_document_ids?}` → `202`; subject/body override the template; unresolved variables are rejected. `body_markdown` renders Markdown to the HTML part (refused together with a template) |
| POST | `/emails/{id}/cancel` | own queued mail, or any with system operations |
| POST | `/emails/{id}/retry` | admin; failed or needs-review |
| GET / POST | `/email-templates` | create: `{key, name, subject, body}` |
| GET | `/email-templates/variables` | the whitelist with descriptions |
| PATCH | `/email-templates/{id}` | `{name?, subject?, body?}` — unknown variables rejected |
| GET / POST | `/email-suppressions` | `{email, reason?}` |
| DELETE | `/email-suppressions/{email}` | admin |
| POST | `/leads/{id}/quotation` | `{subject?, hero?, body?, body_markdown?, attachment_document_ids?}` → `202`; hero-banded quotation letter to the lead's contact (partner fallback), lead documents attachable; empty fields fall back to lead-built defaults |
| GET | `/newsletter/subscriptions` | the list as the office sees it, unsubscribed included |
| POST | `/newsletter/subscriptions` | `{email, name?}` hand-add (office) |
| DELETE | `/newsletter/subscriptions/{id}` | remove (office) |
| POST | `/newsletter/send` | `{subject, body, body_markdown?, hero?, attachment_document_ids?, embed_document_ids?}` → one row, everyone in BCC; `{{variables}}` refused (a blast has no recipient to resolve against). `![alt](doc:ID)` embeds the document as an inline `cid:` image; `hero` adds the red band |
| POST | `/newsletter/subscribe` | website signup: `{email, name?}` + `X-Newsletter-Key`; always `202`. Double opt-in: mails a confirmation link unless already subscribed; an earlier unsubscribe stands until the link is clicked |
| GET | `/newsletter/confirm` | `?token=` from the confirmation letter; `{confirmed}`. Links expire after 7 days |
| GET | `/newsletter/unsubscribe` | `?token=` one-click or `?email=`; always 200, never reveals membership |

Statuses: `queued → sending → sent`, or `failed`, `cancelled`, `needs_review` (delivery
outcome unknown — check the provider before retrying).

## Configuration

| Method | Path | Notes |
|---|---|---|
| GET | `/stage-definitions` | `entity=lead\|order` |
| POST | `/stage-definitions` | `{entity, key, label_hu, position, min_images?, required_image_category?, is_terminal?, is_exit?, stall_after_days?}` |
| PATCH | `/stage-definitions/{id}` | `{label_hu?, position?, min_images?, required_image_category?, stall_after_days?, is_active?}` — key is permanent |
| GET / POST | `/project-types` | `{key, label_hu, position}` |
| PATCH | `/project-types/{id}` | `{label_hu?, position?, is_active?}` |
| GET / PUT | `/settings` | kill switch, rate limit, send window, nudge cadence, notifications, stalled-alert recipients |

## Reports

All accept `from`/`to` (dates, default last 12 months).

| Path | Returns |
|---|---|
| `/reports/volume` | `group=month\|partner\|project_type`, `include_cancelled` — orders, HUF, EUR, normalised HUF, `missing_fx` + totals |
| `/reports/stage-durations` | `project_type_id` — visits, currently in stage, avg/median/p90 days (finished visits) |
| `/reports/throughput` | completed per month, median/avg lead time |
| `/reports/stalled` | open orders past their stage's `stall_after_days` |
| `/reports/blocker-load` | per responsible party: blockers, open, overdue, nudges, waiting days, share of waiting |
| `/reports/fx-rates` | `base` (EUR) — stored MNB rates |

## Admin

| Method | Path | Notes |
|---|---|---|
| GET | `/admin/status` | email mode, kill switch, failed/pending jobs, emails needing attention, FX coverage |
| GET | `/admin/jobs` | `state=failed\|pending` |
| POST | `/admin/jobs/{id}/retry` | |
| POST | `/admin/fx/fetch` | `{from, to}` |
| POST | `/admin/run/{kind}` | `nudge_blockers` \| `stalled_orders` |

`GET /health` (no auth) checks database and object storage.
