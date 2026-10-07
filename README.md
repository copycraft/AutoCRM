# AutoCRM

The CRM and project lifecycle system for Autotherm's vehicle conversions (refrigerated,
heated and funeral bodies), replacing MiniCRM:

```
Lead → Order → Design → Production → MEO documentation → Done
```

Around that pipeline: invoicing reported to NAV, email with automatic reminders,
newsletters, reports, HR and recruitment, a phone app for the shop floor, and a small local
assistant.

One Rust binary (API and background worker), one Postgres, one S3-compatible object store,
a Next.js web app, an Android app, and an optional Node sidecar for NAV invoice reporting.
Built for correctness, longevity and low operational burden, not scale (60 leads/month,
<20 users). Hungarian only.

- Architecture decisions and why: [docs/DECISIONS.md](docs/DECISIONS.md)
- API: [docs/API.md](docs/API.md), and `/api/docs` on a running server
- Error codes: [docs/error-codes.md](docs/error-codes.md)
- Website integration (leads, newsletter signup): [docs/websiteleadsinstructions.md](docs/websiteleadsinstructions.md)
- Sending mail through Google Workspace: [docs/email-google-workspace.md](docs/email-google-workspace.md)
- MiniCRM migration: [docs/migration/README.md](docs/migration/README.md)
- Audits: [docs/logic-audit/](docs/logic-audit/), [docs/integration-audit/](docs/integration-audit/);
  older snapshots in [docs/history/](docs/history/README.md)

## Scope

If it isn't in `backend/migrations/`, it isn't part of the system. Anything that widens the
scope gets a line here and an entry in `docs/DECISIONS.md` in the same change.

| Area | What is there |
|---|---|
| **Sales** | Partners (businesses and private customers, customer/supplier role) and contacts. Leads with stages, quotations (value, validity, PDF letter), quote follow-up letters, expiry alerts, lost reasons, per-market lead tags, website enquiries with source tracking, conversion to orders |
| **Projects** | Orders with line items, build specification (heating or cooling), configurable stages with photo gates, blockers with automatic nudges, tasks, vehicles (several per order), intake slip, handover inspections (átvétel/kiadás walkarounds with damages, photos and signatures), photos and documents with write-once intake evidence, a pickup board for the workshop TV |
| **Billing** | Invoices, stornos and technical annulments reported to NAV Online Számla through [`nav-sidecar`](nav-sidecar/README.md); proformas (díjbekérő, not reported); invoice lists (to issue, issued, paid, archived, stornoed); automatic payment reminders; incoming supplier invoices with the file attached |
| **Email and marketing** | Templates (area, folder, bin), manual and automatic mail, the correspondence log, customer replies read from the sales mailbox, suppression list. Newsletter subscribers with double opt-in, sectioned tags, signup forms per list, and tracked sends (one letter per reader, opens, clicks, scheduled) |
| **HR** | Staff directory with photos, personal/financial/family data driving employee statuses, documents with expiry alerts, leave and absence with a team calendar, job postings with a public application form |
| **Across everything** | Global search, per-record history timeline, notifications (web bell, phone), reports (EUR normalised to HUF at MNB rates), Excel/CSV export, saved views, bulk actions, a read-only assistant on a local language model |

Out of scope: inventory, cost tracking, purchase orders, time tracking and a customer
portal. Inbound mail means customer replies to one mailbox matched to CRM records; it is not
a general inbox.

## Optional parts

Each of these is off until its variable is set (`backend/.env.example`), and the system
works without it.

| Set | Turns on |
|---|---|
| `NAV_SIDECAR_URL` (+ `NAV_SIDECAR_TOKEN`, `NAV_SUPPLIER_*`) | Invoicing. Without it every invoicing endpoint refuses with a rule error |
| `LEADS_API_KEY` | `POST /api/leads/website`, the website enquiry form |
| `NEWSLETTER_API_KEY` | `POST /api/newsletter/subscribe`, the website newsletter signup |
| `IMAP_HOST` (+ `IMAP_USER`, `IMAP_PASSWORD`) | Reading customer replies from the sales mailbox |
| `AI_URL` | The assistant (the button is hidden without it) |
| `TSA_URL` | RFC 3161 timestamps on intake and inspection photos |
| `GOOGLE_CLIENT_ID` + `GOOGLE_CLIENT_SECRET` | "Sign in with Google" for existing accounts of the Workspace domain |
| `META_APP_SECRET` + `META_VERIFY_TOKEN` + `META_PAGE_TOKEN` | Facebook/Instagram lead ads filed as leads (`/api/webhooks/meta`) |
| `META_PIXEL_ID` + `META_CAPI_TOKEN` | Won leads that came from Meta reported to its Conversions API |
| `GOOGLE_ADS_EXPORT_KEY` | The Google Ads offline-conversion CSV of won leads with a `gclid` |
| `VIN_DECODER_ONLINE=true` | Make and model from NHTSA when a VIN is read (the offline reading always works) |
| `NAV_SUPPLIER_BANK_ACCOUNT_EUR` | A separate account on EUR invoices and proformas |
| `EMAIL_MODE=smtp` | Real delivery. Otherwise mail is dry-run: rows are written and bodies logged |

Automatic mail (nudges, follow-ups, reminders, confirmations, alerts) also needs the kill
switch turned on in Beállítások. It is off by default, and mail queued while it is off is
cancelled, not held.

## Local development (Windows, native Rust)

Prerequisites: Rust (stable, MSVC), Node 20+, Docker Desktop (for Postgres and MinIO only),
`sqlx-cli`.

```bash
docker compose up -d postgres minio minio-init
```

```bash
cd backend && cp .env.example .env
```

```bash
cd backend && cargo run -- migrate
```

```bash
cd backend && AUTOCRM_ADMIN_PASSWORD='choose-a-long-password' cargo run -- create-admin --email you@autotherm.hu --name "Your Name"
```

```bash
cd backend && cargo run
```

```bash
cd frontend && npm ci && npm run dev
```

The API listens on http://127.0.0.1:8080 (`/health`, `/api/...`) with the background
worker in the same process. The web app is on http://localhost:3000 and proxies `/api` to
the API. The Android app has its own instructions in [android/README.md](android/README.md).

| Service | Address | Notes |
|---|---|---|
| Postgres | `localhost:5432/autocrm` | `autocrm` / `autocrm` (dev only) |
| MinIO console | http://localhost:9001 | `autocrm` / `autocrm-dev-secret` (dev only) |
| Mailpit | http://localhost:8025 | `docker compose --profile mail up -d mailpit`, then `EMAIL_MODE=smtp` |
| NAV sidecar (mock) | http://localhost:8081 | `docker compose --profile nav up -d nav-sidecar`; no NAV credentials needed |
| Assistant model | http://localhost:8082 | `docker compose --profile ai up -d ai`; downloads ~400 MB on first start |

Outside production, SMTP may only reach a local sink (localhost, Mailpit) unless
`EMAIL_REDIRECT_TO` sends everything to one internal address; the config refuses anything
else at startup.

**Changing the API.** The backend owns the contract. After changing a handler or a response
type, regenerate the document and the web client:

```bash
cd backend && cargo run -- openapi --out ../openapi/openapi.json
```

```bash
cd frontend && npm run gen:api
```

Then restart `npm run dev` with `frontend/.next` deleted: the running dev server keeps the
old generated client. The Android DTOs are hand-written; `OpenApiContractTest` tells you if
one of them no longer matches.

**Exchange rates.** The worker fetches MNB rates daily after 12:30 Budapest time. Before
migrating historical orders, backfill:

```bash
cd backend && cargo run -- fx-backfill --from 2010-01-01 --to 2026-09-11
```

**Email check.** `cargo run -- email-test --to you@autotherm.hu` sends one message straight
through the configured transport and prints what the server said.

## Tests

```bash
cd backend && cargo test
```

```bash
cd frontend && npm test
```

```bash
cd nav-sidecar && npm test
```

```bash
cd android && ./gradlew testDebugUnitTest
```

Backend unit tests cover the domain rules (money, stages, templates, send policy, nudges,
invoices), auth primitives, upload tickets, MNB parsing and the image pipeline. Integration
tests (`backend/tests/`) run against a real Postgres through `#[sqlx::test]` and need
`DATABASE_URL`. `tests/openapi.rs` fails when `openapi/openapi.json` is stale, and
`tests/error_codes.rs` when the error catalog and the code disagree.

SQL written with the `query!` macros is checked at compile time; offline builds (CI,
`build.sh`) check it against the committed `backend/.sqlx` metadata, so refresh it after
changing such a query:

```bash
cd backend && cargo sqlx prepare -- --all-targets
```

Newer modules (HR, recruitment, lead and newsletter tags, follow-ups, reminders, the
mailbox reader, notifications, incoming invoices) use runtime `sqlx::query(...)` instead.
Those queries are only exercised by the integration tests, so a change to them needs a test
run against a database, not just a build.

The web tests are Vitest with Testing Library; `npm run e2e` is a Playwright smoke run
(log in, open an order, every tab) against a running API with an admin from `E2E_EMAIL` /
`E2E_PASSWORD`. CI (`.github/workflows/ci.yml`) runs all of the above, plus `cargo fmt`,
`clippy -D warnings`, the web lint and build, a check that the generated web client is
current, and a debug APK build.

## Layout

```
backend/
  migrations/     forward-only SQL migrations (the schema is the source of truth)
  src/
    domain/       pure types and business rules: no async, no IO
    repo/         SQL, one module per aggregate
    service/      use cases: auth, orders, stages, leads, media, email, invoicing,
                  follow-ups, reminders, mailbox, newsletter, notifications, assistant
    api/          thin Axum handlers, each with its OpenAPI annotation
    jobs/         Postgres-backed worker and scheduler
    integrations/ SMTP mailer, IMAP reader, MNB exchange rates, NAV sidecar client
    media/        object storage, upload tickets, image pipeline
    migration/    the MiniCRM migration (`autocrm-migrate`)
  tests/          integration tests against a real Postgres
frontend/         Next.js office app; API types and zod schemas generated from openapi/
android/          Kotlin shop-floor app (see android/README.md)
nav-sidecar/      Node service that speaks NAV Online Számla (see nav-sidecar/README.md)
openapi/          openapi.json, emitted by the backend and checked in CI
docs/             decisions, API, error codes, website and email guides, migration, audits
build.sh          builds every part into ./build for deployment (on Windows: build.ps1, via Git Bash)
docker-compose.yml  dev Postgres, MinIO (object lock on), Mailpit, NAV sidecar (mock), model server
```

## Background work

One worker loop runs the Postgres job queue (claimed with `FOR UPDATE SKIP LOCKED`, leased,
retried with backoff, dead-lettered after the last attempt). A scheduler enqueues the
periodic jobs with dedupe keys, so each period's job exists once even with several
instances:

| When (Budapest time) | Job |
|---|---|
| Hourly | Blocker nudges; quote follow-up letters that are due |
| Every 10 minutes | Read the sales mailbox (only with `IMAP_HOST`) |
| Daily from 07:00 | Stalled-order alerts; quote and HR document expiry alerts |
| Daily from 09:00 | Payment reminders for overdue invoices |
| Daily from 12:30 | MNB exchange rates for the last ten days |

On demand: sending each email, processing each uploaded photo, reporting each invoice to
NAV, technical annulments, and newsletter sends at their scheduled time. Admins see failed
and pending jobs through `/api/admin/status` and `/api/admin/jobs`.

## Building and deploying

```bash
./build.sh
```

Builds the release backend (`autocrm`, `autocrm-migrate`), the standalone Next.js server,
the NAV sidecar and a signed release APK into `./build`, with a `DEPLOY.md` that says how to
run each part. `--skip backend|frontend|sidecar|android` leaves a part out;
`LINUX_BINARY=1` also builds a Linux backend in Docker. The first Android build generates
the release keystore in `android/`: back it up, because the phones only accept updates
signed with the same key.

Production outline:

- Behind Caddy (TLS), serving the web app and `/api` from one origin so cookies stay
  first-party. Run the web server with `HOSTNAME=::`; binding it to 127.0.0.1 only breaks
  its own `/api` proxy.
- systemd unit with `Restart=on-failure`; `APP_ENV=production`, `COOKIE_SECURE=true`,
  `LOG_FORMAT=json`, `LOG_DIR=/var/log/autocrm`. `PUBLIC_BASE_URL` must be https: it is in
  every confirmation, unsubscribe and tracking link.
- Postgres: nightly `pg_dump` and WAL archiving off-site; object storage with versioning and
  replication. Test a restore before go-live, and yearly after.
- Sending domain: SPF, DKIM, DMARC (`p=none` first) before automatic email is switched on.
- Invoicing: run the `nav-sidecar` next to the API with the NAV technical user's
  credentials and a `SIDECAR_TOKEN` (`openssl rand -hex 32`), its port published on loopback
  only; set `NAV_SIDECAR_URL`, `NAV_SIDECAR_TOKEN` (the same value) and the `NAV_SUPPLIER_*`
  fields. Report to NAV's test environment before switching to production.
- Website forms: set `LEADS_API_KEY` and `NEWSLETTER_API_KEY`, and switch automatic email on
  first, or confirmation letters are cancelled and nobody can confirm. The website's server
  (never the browser, or the key leaks) posts to the API; see
  [docs/websiteleadsinstructions.md](docs/websiteleadsinstructions.md).
- Public pages that must be reachable without a login: `/hu/adatkezeles`,
  `/hu/newsletter/confirm`, `/hu/newsletter/unsubscribe`, `/hu/jobs/{slug}`, and the API's
  `/api/newsletter/track/*` and `/api/public/jobs/*`.

## Feature notes

**Stages.** Stages and project types are configuration (`stage_definitions`,
`project_types`), and history is kept per record; the current stage is the latest row.
Forward moves may skip stages, but every photo gate on the way applies (MEO needs a
completion photo). Backward moves and reopening need a note; lost and cancelled are always
reachable. Leaving intake needs the mileage on the intake slip. A lead is won only by
converting it into an order. The stage names and project types seeded in `0002`/`0003` are
placeholders, and there is no screen for changing them yet: the API is
`/api/stage-definitions` and `/api/project-types`.

**Website enquiries.** The autotherm.hu form's server posts JSON to `/api/leads/website`
with the header `X-Leads-Key`: `name` plus `email` and/or `phone` are required; `message`,
`subject`, `vehicle`, `page` and the source fields are optional; `company` is a hidden
honeypot. The answer is 202. The lead appears unassigned in the first stage with source
`website`. Every admin and office user gets a notification, and an email goes to
`LEADS_NOTIFY_TO` (default vastag.peter@autotherm.hu; set it empty to switch it off).

**Source tracking.** The form can send `utm_source`, `utm_medium`, `utm_campaign`,
`referrer` and `landing_page`. The server sorts them into a channel (paid, organic, social,
email, referral, direct). The lead shows where it came from, and Reports shows leads and
wins per channel, campaign and landing page, and the website conversion with lost reasons.

**Lead tags.** Per-market lists (Magyar, Román, Német, Olasz…), like the per-language sales
lists in MiniCRM, edited at Leadek → Címkék kezelése. A tag can claim website domains: a
lead whose site, page, landing page, referrer, UTM source or typed source falls under one
(subdomains included) gets the tag on arrival, and the lead shows which domain did it. A
domain belongs to at most one live tag.

**Quotations and follow-ups.** A quotation letter goes to the lead's contact with the lead's
PDF documents attached. Sending it schedules the follow-up letters (1 week, 2 weeks,
1 month by default; the sequence is edited on Sablonok → Értékesítés), sent from whoever
sent the quotation so the answer reaches them. A lead that is won, lost or converted gets no
more, and a customer reply read from the mailbox stops them. Once a quote has a week or
less left, its salesperson (or, when the lead is unassigned, every admin and office user)
gets one notification per validity date.

**Customer replies.** With `IMAP_HOST` set, the sales mailbox is read every ten minutes. A
message is matched by its headers when it answers one of our letters, else by the sender's
address, and stored on that lead's, order's or partner's history. Mail that matches nothing
is not stored. The mailbox is only read: nothing is moved, flagged or deleted.

**Invoices.** Issuing writes the invoice as `submitting` and queues a job that reports it
through the sidecar; the screen polls until NAV answers. Corrections are stornos (their own
numbered documents) or technical annulments, never edits. Számlázó lists every invoice,
storno and proforma. A cash invoice is paid on issue; a transfer is marked paid by the
office. Overdue transfer invoices get payment reminders (3 days and 14 days after the
deadline by default, edited on Sablonok → Számlázó), which the office can switch off per
invoice. Bejövő számlák holds supplier invoices: drop in the PDF or photo, fill in the
figures, and the list it lands on follows from them.

**Newsletters.** Website signups are double opt-in: the form stores a pending row and mails
a confirmation link (valid 7 days); only the click adds the address. A signup form can name
lists, applied on confirmation. Office hand-adds and pasted imports are confirmed on insert.
Addresses that opted out are skipped by imports. A newsletter goes to everyone or to chosen
tags, now or at a set time, as one letter per reader with its own unsubscribe link, an open
pixel and signed click tracking; Marketing → Kiküldések shows opens, clicks and
unsubscribes. The older single-letter blast (`POST /api/newsletter/send`, everyone in BCC)
is still in the API but no screen uses it.

**HR.** `/hr` (web) and HR (phone) are open to admins and to users an admin has given
**HR access** on the Users page. The check is in the API (`Capability::AccessHr` on every
`/api/hr/*` route), not just the menu. The directory holds name, photo, company and personal
phone, email, and a status from HR's own lists; the personal, financial and family data
drive the "…adatokra vár" statuses. Employee documents (medical, contract, licence,
training) can carry an expiry date; HR is notified 30 days before and on the day. Photos and
files are stored in the object store under `hr/`.

**Leave.** HR records leave, sick days, unpaid and other absence. The team calendar shows
the month; each employee has an annual allowance (default 20 days) and a balance. Weekends
and Hungarian public holidays do not use leave; the yearly government bridge-day swaps are
not in any formula, so HR adjusts those by hand. Absences cannot overlap.

**Recruitment.** A job posting has an unguessable public link (`/hu/jobs/{slug}`) with a
generic application form: name, contact details, age, city, message and a resume (PDF, DOC,
DOCX, ODT or RTF, up to 10 MB). Drafts refuse applications and closed postings say so. HR
gets a notification for each application.

**Notifications.** Per user: a bell with an unread count in the web menu (refreshed every
30 s) and a system notification on the phone. New website leads, expiring quotes, job
applications and expiring employee documents raise them. The phone polls about every 15
minutes (Android's floor for background work), so a lead can take that long to reach it;
instant push would need Firebase Cloud Messaging. The feed is kept 60 days.

**Search.** The header box (`/` to focus, or the command palette) and the phone's Keresés
screen search orders, partners, leads, contacts, emails and, for users with HR access, the
staff directory. Every word must match, in any order; accents and punctuation are ignored
(`kovacs gyor` finds Kovács, Győr), and phone numbers and plates match in any spelling.
Exact matches rank first, then prefixes.

**History.** Leads, orders, partners, employees and incoming invoices each have one
timeline: field changes, stages, files, tasks, emails sent and received, and imported
MiniCRM notes.

**Assistant.** With `AI_URL` set, a panel answers questions about the data and builds list
filters that can be opened or saved as views. It runs on a local model (Qwen2.5-0.5B on
llama.cpp by default), so CRM data never leaves the building, and it can only read. Plain
questions are routed straight to the right query without the model.

**Offline phone.** Every read the phone makes is kept as the last good copy and shown when
the server cannot be reached, with a banner saying how old it is. Writes are never faked:
editing needs a connection, while photo uploads and handover inspections queue. Not kept on
the phone: the staff directory and user list, notifications, search, and anything with an
expiring link. The copy is cleared on sign-out.

**API docs.** `/api/docs` is a browsable page for the whole API (Swagger UI, loaded from a
CDN, so the browser needs internet); `/api/docs/openapi.json` is the live contract it
reads. In `dev` and `staging` both are open and "Try it out" works with your session
cookie. In `production` they need an admin session and the page is read-only. The
checked-in copy is `openapi/openapi.json`.

**Data protection.** `/hu/adatkezeles` is a public page explaining, in plain Hungarian, who
keeps a person's data, what is stored and why, their rights and how to unsubscribe. Link to
it from emails, website forms and the full notice. Its company details come from the
`PRIVACY_*` variables on the web server (`frontend/.env.example`); a line that is not set is
not shown. A draft of the full *Adatkezelési tájékoztató*, built from what the system
actually stores, is in `docs/adatkezelesi-tajekoztato-vazlat.md`: it needs a lawyer's
review before publication.
