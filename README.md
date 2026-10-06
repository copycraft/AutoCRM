# AutoCRM

Project lifecycle system for Autotherm's refrigerated vehicle conversions:

```
Lead → Order → Design → Production → MEO documentation → Done
```

One Rust binary, one Postgres, one S3-compatible object store, plus an optional Node
sidecar for NAV invoice reporting. Built for correctness, longevity and low operational
burden, not scale (60 leads/month, <20 users).

- Architecture decisions and deviations from the original plan: [docs/DECISIONS.md](docs/DECISIONS.md)
- API reference: [docs/API.md](docs/API.md)
- Past audits and plans (point-in-time snapshots, not current): [docs/history/](docs/history/README.md)

## Scope

In scope, and defined in `backend/migrations/`:

- Partners and contacts (customers and suppliers), leads with quotations, orders with line
  items, build specification, stages, blockers and follow-up tasks
- Vehicles, intake slip and handover inspections (check-out/check-in damage records)
- Photos and documents, with write-once intake evidence
- Email: templates, manual and automatic mail, the correspondence log, a newsletter list
- Reports, with EUR normalised to HUF at MNB rates
- **Invoicing**: invoices, stornos and technical annulments reported to NAV Online Számla
  through [`nav-sidecar`](nav-sidecar/README.md), plus proformas (díjbekérő), which are not
  reported. Off unless `NAV_SIDECAR_URL` is set.

Out of scope: inventory, cost tracking, purchase orders, time tracking, inbound mail
(the email screens show what AutoCRM sent, not a mailbox), customer portal.

If it isn't in `backend/migrations/`, it isn't part of the system. Anything that widens the
scope gets a line here and an entry in `docs/DECISIONS.md` in the same change.

## Local development (Windows, native Rust)

Prerequisites: Rust (stable, MSVC), Docker Desktop (for Postgres + MinIO only), `sqlx-cli`.

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

The API listens on http://127.0.0.1:8080 (`/health`, `/api/...`). The background worker
runs in the same process.

| Service | URL | Credentials (dev only) |
|---|---|---|
| Postgres | `localhost:5432/autocrm` | `autocrm` / `autocrm` |
| MinIO console | http://localhost:9001 | `autocrm` / `autocrm-dev-secret` |
| Mailpit (optional, `docker compose --profile mail up -d mailpit`) | http://localhost:8025 | — |

Email is **dry-run** outside production: rows are written and bodies logged, nothing is
sent. To see real messages locally, run Mailpit and set `EMAIL_MODE=smtp`; the config
refuses any non-local SMTP host outside production.

### Exchange rates

The worker fetches MNB rates daily after 12:30 Budapest time. Before migrating historical
orders, backfill:

```bash
cd backend && cargo run -- fx-backfill --from 2010-01-01 --to 2026-09-11
```

## Tests

```bash
cd backend && cargo test
```

Unit tests cover the domain rules (money, stages, templates, send policy, nudges), auth
primitives, upload tickets, MNB parsing and the image pipeline. Integration tests
(`backend/tests/`) run against a real Postgres via `#[sqlx::test]` and need `DATABASE_URL`.

SQL is checked at compile time against the database in `DATABASE_URL`. For builds without
a database (CI), refresh the offline metadata after changing queries:

```bash
cd backend && cargo sqlx prepare -- --all-targets
```

## Layout

```
backend/
  migrations/     forward-only SQL migrations (the schema is the source of truth)
  src/
    domain/       pure types and business rules — no async, no IO
    repo/         SQL, one module per aggregate
    service/      use cases: auth, orders, stages, leads, media, email, automation
    api/          thin Axum handlers
    jobs/         Postgres-backed worker and scheduler
    integrations/ SMTP mailer, MNB exchange rates, NAV sidecar client
    media/        object storage, upload tickets, image pipeline
frontend/         Next.js office client (generated API client from openapi/)
android/          Kotlin shop-floor client with an offline photo queue (see android/README.md)
nav-sidecar/      Node service that speaks NAV Online Számla (see nav-sidecar/README.md)
openapi/          openapi.json, emitted by the backend and checked in CI
docs/             decisions, API, error codes, migration, audits; docs/history/ holds snapshots
docker-compose.yml  dev Postgres, MinIO (object lock enabled), Mailpit, nav-sidecar (mock)
```

## Production outline

- Build: `cargo build --release` → `backend/target/release/autocrm`
- Behind Caddy (TLS), serving the Next.js app and `/api` from one origin so cookies stay first-party
- systemd unit with `Restart=on-failure`; `APP_ENV=production`, `COOKIE_SECURE=true`, `LOG_FORMAT=json`, `LOG_DIR=/var/log/autocrm`
- Postgres: nightly `pg_dump` + WAL archiving off-site; object storage with versioning and replication
- Test a restore before go-live, and yearly after
- Sending domain: SPF, DKIM, DMARC (`p=none` first) before automatic email is switched on
- Invoicing: run the `nav-sidecar` image next to the API with the NAV technical user's
  credentials and a `SIDECAR_TOKEN` (`openssl rand -hex 32`), publishing its port on
  loopback only; set `NAV_SIDECAR_URL`, `NAV_SIDECAR_TOKEN` (the same value) and the
  `NAV_SUPPLIER_*` fields (`backend/.env.example`). Report to NAV's test environment before
  switching to production.
- Newsletter signup from the website: set `NEWSLETTER_API_KEY`, and switch automatic email on
  first, or the confirmation letters are cancelled and nobody can confirm.
- Website enquiries: set `LEADS_API_KEY` (each new one is also mailed to `LEADS_NOTIFY_TO`, default vastag.peter@autotherm.hu; needs automatic email on) (`openssl rand -hex 32`). The autotherm.hu form's
  server (not the browser, or the key leaks) POSTs JSON to `/api/leads/website` with the
  header `X-Leads-Key`: `name` plus `email` and/or `phone` are required; `message`,
  `subject`, `vehicle`, `page` are optional; `company` is a hidden honeypot field. Answer is
  202; the lead appears unassigned in the first stage with source `website`.

## HR module and user access

`/hr` (web) is the staff directory: name, photo, company and personal phone, email. It is open
to admins and to any user an admin has given **HR access** on the Users page (`/admin`, admins
only), which lists every registered user with role, active and HR-access controls. The check
is enforced by the API (`Capability::AccessHr`, every `/api/hr/*` route), not just hidden in
the menu. Photos are stored in the object store under `hr/employees/`. Migration `0034_hr.sql`
adds `users.hr_access` and the `employees` table.

## Leave and notifications

**Leave (HR module, `/hr` -> Szabadság; phone: HR -> Távollétek).** HR records leave, sick days,
unpaid and other absence for employees. The team calendar shows the month; each employee has an
annual allowance (default 20 days, editable on the employee) and a balance. Weekends and
Hungarian public holidays do not use leave; the yearly government bridge-day swaps are not in
any formula, so HR adjusts those by hand. An employee cannot have overlapping absences.

**Notifications.** A new website lead notifies every admin and office user: a bell entry with an
unread count in the web menu (refreshed every 30 s), and a system notification on the phone.
The phone **polls about every 15 minutes** (Android's minimum for background work), so a lead
can take up to 15 minutes to reach it; instant push would need Firebase Cloud Messaging and a
Firebase project. The feed (`GET /api/notifications`) is per user, kept 60 days.

## Source tracking, search and offline

**Source tracking.** The website form can send `utm_source`, `utm_medium`, `utm_campaign`,
`referrer` and `landing_page` (see `docs/websiteleadsinstructions.md`, with a copy-paste
snippet). The server sorts them into a channel (paid, organic, social, email, referral,
direct). The lead shows where it came from, and Reports shows leads and wins per channel,
campaign and landing page. Only leads the website filed have a source.

**Lead tags.** Per-market lists (Magyar, Román, Német, Olasz…), like the per-language
sales lists in MiniCRM, edited at Leadek → Címkék kezelése. A tag can claim website
domains: a lead whose `site`, page, landing page, referrer, UTM source or typed source
falls under one (subdomains included) gets the tag on arrival, and the lead shows which
domain did it. A domain belongs to at most one live tag.

**Search.** The header box (`/` to focus, or the command palette) and the phone's Keresés
screen search orders, partners, leads, contacts, emails and, for users with HR access, the
staff directory. Every word must match, in any order; accents and punctuation are ignored
(`kovacs gyor` finds Kovács, Győr), and phone numbers and plates match in any spelling.
Better matches (exact, then prefix) rank first.

**Offline phone.** Every read the phone makes is kept as the last good copy and shown when the
server cannot be reached, with a banner saying how old it is. Writes are never faked: editing
needs a connection (photo uploads and handover inspections still queue as before). Not kept on
the phone on purpose: the staff directory and user list, notifications, search, and anything
with an expiring link (photos, documents). The copy is cleared on sign-out.

## API docs

`/api/docs` is a browsable page for the whole API (Swagger UI, loaded from a CDN, so the
browser needs internet); `/api/docs/openapi.json` is the live contract it reads. In `dev` and
`staging` both are open and "Try it out" works with your session cookie. In `production` they
need an admin session and the page is read-only. The same pages are reachable through the web
app at `/api/docs`. The checked-in copy is `openapi/openapi.json`.

## Data protection

`/hu/adatkezeles` is a public page (no login) explaining, in plain Hungarian, who keeps a
person's data, what is stored and why, their rights and how to unsubscribe. Link to it from
emails, website forms and the full notice. Its company details (name, address, registration
number, contact, link to the full notice) come from the `PRIVACY_*` variables on the web
server (`frontend/.env.example`); a line that is not set is simply not shown. The
`newsletter` confirm and unsubscribe pages link to it. A draft of the full *Adatkezelési
tájékoztató*, built from what the system actually stores, is in
`docs/adatkezelesi-tajekoztato-vazlat.md`: it needs a lawyer's review before publication.
