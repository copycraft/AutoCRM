\# AUTOTHERM CRM — MASTER FRONTEND BUILD PROMPT



You are building the frontend for \*\*Autotherm CRM\*\*.



This is an existing project with an already-built Rust/Axum backend.



You are NOT designing a hypothetical CRM.



You are NOT designing a mockup.



You are NOT building a prototype with fake data.



You are implementing the actual frontend against the existing project.



\---



\# 0. READ THE REPOSITORY BEFORE CODING



The repository root contains:



```text

/

├── DOCS/

├── backend/

└── ...

```



Before writing frontend code, inspect the repository thoroughly.



You MUST read:



```text

DOCS/

backend/

```



and any existing frontend code/configuration.



The documentation describes the intended product and business workflow.



The Rust backend contains the actual implemented API and persistence behavior.



There are also project decision documents describing deliberate architectural decisions that MUST NOT be casually reversed.



Do not begin implementation based solely on this prompt.



First understand the repository.



\---



\# 1. SOURCES OF TRUTH



Use the following hierarchy.



\## Business/product behavior



`DOCS/` is the primary source for:



\* what the CRM does

\* workflows

\* terminology

\* business concepts

\* user expectations

\* required functionality

\* operational behavior



\## API behavior



The existing Rust backend is authoritative for:



\* endpoints

\* HTTP methods

\* request bodies

\* response bodies

\* authentication

\* authorization

\* validation

\* state transitions

\* errors

\* pagination

\* filtering

\* uploads

\* persistence

\* immutable records



The API reference explicitly defines:



```text

Base path: /api

JSON in

JSON out

Money: integer minor units + explicit currency

Quantities: decimal strings

```



Do not invent alternate API behavior.



\## Architectural decisions



`DECISIONS.md` contains decisions already made by the backend implementation.



Do not re-litigate or silently undo those decisions.



For example, the schema exists in the backend migrations, and the project's "if it isn't in the schema, don't build it" rule refers to the actual migrations.



\## This prompt



This prompt controls:



\* frontend architecture

\* UX

\* visual design

\* component strategy

\* client responsibilities

\* implementation constraints



If this prompt conflicts with an actual backend contract, follow the backend.



If this prompt conflicts with documented business behavior, follow `DOCS/`.



\---



\# 2. PRIME DIRECTIVE



The most important requirement:



> \*\*Do not lose functionality from the existing Autotherm/MiniCRM workflow.\*\*



The project-killer is:



> "Something that existed in the old system is missing from the new one."



Therefore, feature completeness matters more than visual polish.



Before declaring the frontend complete, compare the implementation against:



```text

DOCS/

backend/

```



feature by feature.



Do not assume that because a screen looks finished, the CRM is finished.



\---



\# 3. DO NOT INVENT FEATURES



Do not invent:



\* API endpoints

\* database fields

\* business rules

\* stages

\* permissions

\* currencies

\* invoice behavior

\* upload behavior

\* report calculations

\* email behavior

\* migration behavior



If something isn't supported by the backend or documentation, do not fabricate it.



If a documented requirement cannot currently be implemented because the backend does not expose the necessary capability:



1\. identify the gap

2\. verify that an existing endpoint cannot support it

3\. document the gap

4\. do not create a fake frontend implementation



\---



\# 4. DO NOT REMOVE EXISTING FUNCTIONALITY



The backend already exists.



It contains functionality that was deliberately implemented.



Do not hide, delete, replace, or simplify backend functionality merely because this frontend plan didn't mention it.



Inspect the actual API.



If the backend exposes functionality relevant to the CRM, understand why it exists before deciding not to expose it.



\---



\# 5. COMPLETE FILES ONLY



When modifying source files, write the complete file.



Never output truncated code such as:



```text

// rest of file

// existing code

// omitted

...

```



Never leave required functionality as:



```ts

TODO

```



or:



```ts

throw new Error("Not implemented");

```



or:



```ts

return null;

```



just to make compilation pass.



Do not create fake implementations.



\---



\# 6. NO FAKE DATA



Production frontend flows must use the actual API.



Do not populate the application with fake:



\* orders

\* customers

\* invoices

\* reports

\* images

\* financial figures

\* stage histories

\* blockers



for the purpose of making the UI look complete.



If there is no data:



\*\*show the real empty state.\*\*



\---



\# 7. NO SECOND BACKEND



The Rust/Axum backend is the backend.



Next.js is the frontend.



Do NOT create a second backend in Next.js.



Do not create:



\* a second database

\* duplicate business APIs

\* Next.js API routes for backend functionality

\* Server Actions for mutations already handled by Rust

\* duplicate business logic

\* frontend-only persistence pretending to be backend persistence



Architecture:



```text

&#x20;                 ┌─────────────────────┐

&#x20;                 │     Rust / Axum     │

&#x20;                 │      /api/\*         │

&#x20;                 └──────────┬──────────┘

&#x20;                            │

&#x20;               ┌────────────┴────────────┐

&#x20;               │                         │

&#x20;      ┌────────▼────────┐       ┌────────▼────────┐

&#x20;      │      Web        │       │     Android     │

&#x20;      │ Next.js/React   │       │ Kotlin/Compose  │

&#x20;      └─────────────────┘       └─────────────────┘

```



\---



\# 8. API CONTRACT — ABSOLUTE



The API base path is:



```text

/api

```



JSON is used for requests and responses.



Money is represented in integer minor units with an explicit currency.



Quantities are decimal strings such as:



```json

"2.5"

```



Do not convert authoritative money to JavaScript floating point.



Do not invent decimal money representations.



\---



\# 9. API ERRORS



The backend returns:



```json

{

&#x20; "error": {

&#x20;   "code": "...",

&#x20;   "message": "..."

&#x20; }

}

```



Centralize frontend error handling around this contract.



Known status categories include:



```text

400 validation

401 unauthenticated

403 forbidden

404 not\_found

409 duplicate / immutable / already\_converted / already\_resolved /

&#x20;  not\_retryable / ...

422 stage\_gate / note\_required / invalid\_transition /

&#x20;  use\_conversion / lead\_converted / currency\_locked /

&#x20;  password\_change\_required / upload\_missing / upload\_mismatch /

&#x20;  invalid\_ticket / last\_admin / invalid\_reference /

&#x20;  constraint\_violation

429 too\_many\_requests

```



These are business-relevant errors, not generic HTTP failures.



The UI must handle them intentionally.



For example:



```text

409 immutable

```



should explain that an immutable record cannot be deleted.



```text

422 stage\_gate

```



should explain that the order cannot move because a required stage condition has not been satisfied.



```text

422 currency\_locked

```



should explain why currency cannot be changed.



Do not just display:



```text

Error 422

```



\---



\# 10. LIST API CONVENTION



Lists return:



```json

{

&#x20; "items": \[]

}

```



and accept:



```text

limit: 1–200

offset

```



with default limit:



```text

50

```



Use the actual backend pagination.



Do not fetch an unlimited dataset just because it is easier.



Do not implement frontend pagination over an already-unbounded API request if the backend supports pagination.



\---



\# 11. PATCH SEMANTICS



PATCH bodies have explicit semantics:



```text

omit field → keep existing value

null       → clear value

```



Do not accidentally send every optional field as `null`.



Do not accidentally clear fields because a frontend form omitted them.



Preserve the backend's PATCH semantics.



\---



\# 12. AUTHENTICATION — WEB



Web authentication uses:



```text

POST /api/auth/login

```



with:



```json

{

&#x20; "email": "...",

&#x20; "password": "...",

&#x20; "client": "web"

}

```



The backend sets:



```text

autocrm\_session

```



as an httpOnly cookie.



Cookie-authenticated state-changing requests must send an allowed `Origin`.



Normal browser requests should handle this correctly.



Never expose the session token to JavaScript if the backend deliberately uses an httpOnly cookie.



\---



\# 13. AUTHENTICATION — ANDROID



Mobile login uses:



```json

{

&#x20; "email": "...",

&#x20; "password": "...",

&#x20; "client": "mobile"

}

```



The backend returns a bearer token.



Android sends:



```text

Authorization: Bearer <token>

```



Do not make Android pretend to use the web cookie flow.



\---



\# 14. AUTH SECURITY



The backend decisions specify:



\* session tokens are 256-bit random

\* only SHA-256 is stored

\* web session cookie is `SameSite=Lax`

\* web sessions have 7-day idle / 30-day absolute limits

\* mobile tokens have 60-day idle / 365-day absolute limits

\* 10 failed logins lock an account for 15 minutes

\* unknown emails consume equivalent Argon2 work

\* admin-created accounts must change password

\* the last active admin cannot be demoted/deactivated



The frontend must correctly represent these states.



It must not try to bypass them.



\---



\# 15. ROLES AND CAPABILITIES



The API defines:



| Capability                                                           | Admin | Office | Designer | Viewer |

| -------------------------------------------------------------------- | ----: | -----: | -------: | -----: |

| Users/settings/configuration/system operations                       |     ✓ |        |          |        |

| Edit partners/leads/orders, send email, delete media, view originals |     ✓ |      ✓ |          |        |

| Change stages/manage blockers/upload media                           |     ✓ |      ✓ |        ✓ |        |



Everyone signed in can read.



Writes require capabilities.



The backend is the security boundary.



The frontend should nevertheless hide or disable unavailable actions so the interface is understandable.



Never treat:



```ts

if (user.role === "admin")

```



as security.



The API must still reject unauthorized operations.



\---



\# 16. AUTH \& USER SCREENS



Implement the backend-supported functionality:



```text

POST /auth/login

POST /auth/logout

GET  /auth/me

POST /auth/password



GET    /auth/sessions

DELETE /auth/sessions/{id}



GET    /users

POST   /users

PATCH  /users/{id}

POST   /users/{id}/password

POST   /users/{id}/revoke-sessions

```



Admin UI must respect the capability model.



Support active-session management where appropriate.



Support forced password changes.



Do not allow the UI to make the last active admin invalid.



\---



\# 17. PARTNERS AND CONTACTS



Support the backend's actual partner/contact model.



Partners:



```text

GET  /partners

POST /partners

GET  /partners/{id}

PATCH /partners/{id}



POST /partners/{id}/archive

POST /partners/{id}/unarchive

```



Search:



```text

q

kind=business|person

include\_archived

```



Partner creation supports fields such as:



```text

kind

name

tax\_number

eu\_tax\_number

country

default\_currency

email

phone

website

postal\_code

city

address\_line

notes

```



Hungarian tax numbers are normalized by the backend.



Do not implement your own conflicting normalization.



Contacts:



```text

GET/POST /partners/{id}/contacts

PATCH /contacts/{id}

POST /contacts/{id}/archive

```



\---



\# 18. LEADS



Backend:



```text

GET  /leads

POST /leads

GET  /leads/{id}

PATCH /leads/{id}



POST /leads/{id}/stage

POST /leads/{id}/convert

```



Lead search/filtering includes:



```text

q

stage

assigned\_to

open

```



Lead creation supports:



```text

title

partner\_id

contact\_id

contact\_name

contact\_email

contact\_phone

source

description

assigned\_to

```



Lead stage changes support:



```json

{

&#x20; "stage": "...",

&#x20; "note": "..."

}

```



Important backend rule:



> `won` is only reachable through conversion.



A converted lead cannot change stage.



Do not implement a frontend "mark as won" button that bypasses conversion.



\---



\# 19. ORDERS



Backend supports:



```text

GET  /orders

POST /orders

GET  /orders/{id}

PATCH /orders/{id}



POST /orders/{id}/stage

GET  /orders/{id}/stages

GET  /orders/{id}/audit



GET/POST /orders/{id}/items

PATCH/DELETE /order-items/{id}

```



Order search supports:



```text

number

title

partner

VIN

plate

```



Plate searching ignores spaces and dashes.



Filters include:



```text

stage

partner\_id

project\_type\_id

assigned\_to

open

```



Order creation includes:



```text

partner\_id

currency

title

contact\_id

project\_type\_id

valuation\_date

vehicle\_make

vehicle\_model

vehicle\_plate

vehicle\_vin

description

due\_date

assigned\_to

items

```



Currency is:



```text

HUF

EUR

```



\---



\# 20. ORDER NUMBERING



Order numbers are:



```text

YYYY-NNNN

```



and are allocated transactionally by the backend.



Migrated MiniCRM orders are different:



```text

MC-{MiniCRM id}

```



Do not generate order numbers in the frontend.



Do not assume every order follows `YYYY-NNNN`.



Do not sort/display them as though all numbers have identical semantics.



\---



\# 21. ORDER CURRENCY



Order currency is shared by its line items.



The database enforces this.



An order's currency cannot change while it has items.



The API also reports:



```text

currency\_locked

```



when appropriate.



The UI should disable or explain the currency control when the order is locked.



Do not attempt to circumvent the restriction by modifying line items first in the frontend unless that is an explicitly supported workflow.



\---



\# 22. MONEY



Money is integer minor units.



Create a single reusable:



```tsx

<Money />

```



component.



It must be presentation-only.



It must not perform authoritative financial calculations.



Line totals round:



```text

half away from zero

```



and the Rust and PostgreSQL implementations cross-check each other.



The frontend should display backend totals rather than independently reproducing accounting calculations.



\---



\# 23. VALUATION DATE



Orders have:



```text

valuation\_date

```



It defaults to the order's creation day.



It is editable.



It is NOT:



```text

today when the report runs

```



Reports use the valuation date for currency normalization.



The frontend must not substitute the current date.



\---



\# 24. ORDER STAGES



This is extremely important.



Do NOT hardcode a simplistic workflow such as:



```text

Lead

→ Design

→ Production

→ MEO

→ Invoice

→ Paid

```



unless those are actually returned by the backend.



Stages are configurable.



Stage definitions are fetched from:



```text

GET /stage-definitions?entity=lead

GET /stage-definitions?entity=order

```



The seeded stages are placeholders.



The real Autotherm stage names/project types still need to be confirmed by a human.

Therefore:



\*\*Do not hardcode the final Hungarian stage names into components.\*\*



Render backend stage labels.



\---



\# 25. STAGE RULES



The backend has deliberate stage semantics:



\### Forward moves



May skip stages.



However:



> Every image gate between the current stage and target stage still applies.



Skipping MEO does NOT bypass its image requirement.



\### Backward moves



Allowed with a required note.



\### Exit stages



`lost` and `cancelled` are reachable from any open stage without gates.



\### Terminal stages



Leaving a terminal stage is a reopen and requires a note.



\### MEO gate



The seeded/configurable MEO rule is:



```text

min\_images = 1

category = completion

```



but this is configurable per stage.



\### Lead won



A lead reaches `won` only through conversion.



The frontend must treat the backend as authoritative.



\---



\# 26. STAGE RAIL / TRAVELLER



Create a custom:



```tsx

<StageRail />

```



This is the signature UI component.



It must communicate:



```text

current stage

stage history

days in stage

blockers

required gates

ability to advance

```



The order detail API already returns stage information including:



```text

days\_in\_stage

```



and blockers/image counts.



Use backend data.



Do not invent stage durations.



Do not calculate a competing version of stage history in the frontend.



\---



\# 27. STAGE HISTORY



Backend:



```text

GET /orders/{id}/stages

```



provides:



```text

entered\_at

left\_at

who

note

```



The UI should make historical stage movement inspectable.



The backend intentionally uses `clock\_timestamp()` rather than transaction-start `now()` to preserve correct ordering under concurrency.



The frontend must not reorder historical records based on assumptions.



\---



\# 28. BLOCKERS



Backend:



```text

GET /blockers

GET/POST /orders/{id}/blockers

PATCH /blockers/{id}

POST /blockers/{id}/resolve

POST /blockers/{id}/reopen

```



Blockers can contain:



```text

what

responsible\_partner\_id

responsible\_email

due\_date

notes

nudge\_enabled

```



The UI should clearly show:



```text

what is blocking

who is responsible

due date

overdue status

resolved/open state

```



Blocker resolution and reopening must use the real backend operations.



Do not simulate resolution by only changing local React state.



\---



\# 29. BLOCKER NUDGES



The backend handles automatic nudges.



They are sent to:



```text

responsible\_email

```



or the responsible partner's email.



After the due date:



```text

nudge\_interval\_days

```



controls recurrence.



After:



```text

nudge\_escalate\_after

```



nudges, the escalated template is used.



The frontend should expose relevant status/configuration but must not recreate the nudge scheduler.



\---



\# 30. IMAGES AND DOCUMENTS



Upload API:



```text

POST /orders/{id}/uploads

```



The request includes:



```text

target

filename

content\_type

byte\_size

sha256

```



Image categories:



```text

intake

production

completion

marketing

```



Document kinds:



```text

design

cad

other

```



Upload response can be:



```text

already\_uploaded

```



or:



```text

upload

```



with:



```text

ticket

upload.method

upload.url

upload.headers

expires\_at

```



Then:



```text

POST /uploads/complete

```



with:



```json

{

&#x20; "ticket": "..."

}

```



\---



\# 31. UPLOADS — DO NOT BREAK THE SECURITY MODEL



The backend deliberately uses content-bound uploads.



The upload ticket is bound to:



```text

user

order

key

hash

size

```



and expires after two hours.



The storage key is content-addressed:



```text

orders/{id}/{category}/{sha256}.ext

```



The presigned PUT signs:



```text

x-amz-checksum-sha256

```



and finalization verifies size/checksum.



Do not modify this workflow.



Do not upload to some invented endpoint.



Do not send files without the backend-provided headers.



Do not invent your own ticket format.



\---



\# 32. IDEMPOTENT UPLOADS



The backend guarantees idempotent image batches through:



```text

(order\_id, content\_hash)

```



uniqueness.



A retried upload should resolve to the existing image rather than creating a duplicate.



The frontend should treat this as normal behavior.



Do not display "duplicate upload failure" when the API intentionally reports:



```text

already\_uploaded

```



\---



\# 33. IMAGE IMMUTABILITY



This is critical.



Original intake images are immutable.



Backend protection exists at multiple levels:



```text

database trigger

S3 Object Lock

processing-job hash verification

```



Deleting an intake image returns:



```text

409 immutable

```



The frontend must never imply that an intake image can be deleted.



If the API rejects deletion because of immutability, explain that it is intentional.



\---



\# 34. IMAGE ORIGINALS VS DERIVED COPIES



Originals are stored:



```text

byte-for-byte

EXIF included

```



Derived display copies:



```text

≤2560px

JPEG

```



Thumbnails:



```text

≤480px

JPEG

```



Derived copies strip EXIF/GPS.



UI and emails use derived copies.



Originals require:



```text

ViewOriginalImages

```



capability.



Therefore:



\* normal gallery → thumbnails/display copies

\* original download/view → capability-controlled endpoint



Do not expose originals to everyone.



\---



\# 35. IMAGE UI



Backend:



```text

GET /orders/{id}/images

GET /images/{id}/original

DELETE /images/{id}

```



Image listing can be filtered by category.



Display URLs are presigned and expire.



Therefore:



\*\*do not permanently cache presigned URLs as though they never expire.\*\*



Refresh them when required.



\---



\# 36. IMAGE GALLERY



Build:



```tsx

<ImageGrid />

<ImageLightbox />

<Uploader />

```



The gallery must support large image sets.



Expected scale:



```text

80–120+ images per order

```



Use virtualization.



Requirements:



\* virtualized grid

\* thumbnails

\* lazy loading

\* category grouping

\* selection

\* bulk selection

\* lightbox

\* keyboard navigation

\* escape

\* arrows

\* upload progress

\* retry

\* errors

\* empty states



Never render 120 full-resolution images simultaneously.



\---



\# 37. DOCUMENTS



Backend:



```text

GET /orders/{id}/documents

GET /documents/{id}/download

DELETE /documents/{id}

```



Documents include types such as:



```text

design

cad

other

```



Use backend-provided download URLs.



Do not expose storage credentials.



\---



\# 38. EMAIL SYSTEM



Backend supports:



```text

GET /emails

GET /emails/{id}

POST /emails/preview

POST /emails

POST /emails/{id}/cancel

POST /emails/{id}/retry

```



Email templates:



```text

GET/POST /email-templates

GET /email-templates/variables

PATCH /email-templates/{id}

```



Suppressions:



```text

GET/POST /email-suppressions

DELETE /email-suppressions/{email}

```



\---



\# 39. EMAIL STATUS



Email states:



```text

queued

sending

sent

failed

cancelled

needs\_review

```



`needs\_review` is special.



It means the delivery outcome is unknown.



The backend deliberately prevents blindly retrying messages that might already have been sent.



The UI must clearly distinguish:



```text

failed

```



from:



```text

needs\_review

```



Do not automatically provide a "retry" action for every non-sent message.



Follow backend permissions and state rules.



\---



\# 40. EMAIL SECURITY / BUSINESS RULES



Backend decisions include:



\* SMTP via `lettre`

\* provider remains configurable

\* `sending` is committed before SMTP starts

\* timeouts can become `needs\_review`

\* automatic email has idempotency keys

\* blocker nudges include both blocker and order

\* kill switch defaults off

\* when kill switch is off, automatically discovered mail is cancelled

\* non-production can only reach local SMTP sink

\* unresolved variables are rejected for manual mail

\* automatic unresolved-variable mail becomes visible `failed`

\* customer stage-change mail happens on forward moves only, if enabled

\* template bodies are plain text and HTML is derived/escaped



Do not recreate these mechanisms in the frontend.



Expose the states and controls appropriately.



\---



\# 41. EMAIL PREVIEW



Use:



```text

POST /emails/preview

```



before sending where appropriate.



The response may contain:



```text

rendered subject

rendered body

unresolved

recipient\_suppressed

```



The UI should make unresolved variables and suppression visible before sending.



Do not try to reproduce backend template interpolation in JavaScript.



\---



\# 42. CONFIGURATION



Backend supports:



```text

GET /stage-definitions

POST /stage-definitions

PATCH /stage-definitions/{id}



GET/POST /project-types

PATCH /project-types/{id}



GET/PUT /settings

```



Settings include things such as:



```text

kill switch

rate limit

send window

nudge cadence

notifications

stalled-alert recipients

```



Stage keys are permanent.



Labels/positions/configuration can change.



Therefore:



\*\*never use stage label text as a stable database identifier.\*\*



Use:



```text

stage.key

```



where the backend exposes it.



\---



\# 43. PROJECT TYPES



Project types are configurable.



Do not hardcode:



```text

Sprinter

Ducato

...

```



as project types unless the backend says so.



Project types have:



```text

key

label\_hu

position

is\_active

```



The UI should use backend configuration.



\---



\# 44. REPORTING



Backend reports:



```text

/reports/volume

/reports/stage-durations

/reports/throughput

/reports/stalled

/reports/blocker-load

/reports/fx-rates

```



All accept:



```text

from

to

```



with default last 12 months.



Do not invent the five-report model from the original design if the actual backend has a different report surface.



\*\*Use the actual backend report API.\*\*



\---



\# 45. REPORT DETAILS



\### Volume



Supports grouping by:



```text

month

partner

project\_type

```



and can include cancelled orders.



Returns:



```text

orders

HUF

EUR

normalised HUF

missing\_fx

totals

```



\### Stage durations



Provides:



```text

visits

currently in stage

average

median

p90

```



Finished stage visits are used for duration statistics.



Open visits are counted separately.



\### Throughput



Provides:



```text

completed per month

median lead time

average lead time

```



\### Stalled



Shows open orders past their stage's configured `stall\_after\_days`.



\### Blocker load



Provides:



```text

responsible party

blockers

open

overdue

nudges

waiting days

share of waiting

```



\### FX rates



Returns stored MNB rates.



\---



\# 46. FX / REPORTING RULES



The backend uses MNB rates.



Important behavior:



\* rates are stored

\* valuation date determines normalization

\* weekends/holidays use the latest rate within 10 days before valuation date

\* beyond 10 days, normalized value is null

\* such values count as `missing\_fx`



Do not calculate FX rates in the frontend.



Do not fetch exchange rates directly from MNB in the browser.



Use the backend report values.



\---



\# 47. ADMIN



Backend supports:



```text

GET /admin/status

GET /admin/jobs

POST /admin/jobs/{id}/retry

POST /admin/fx/fetch

POST /admin/run/{kind}

```



Supported run kinds include:



```text

nudge\_blockers

stalled\_orders

```



Admin status includes:



```text

email mode

kill switch

failed/pending jobs

emails needing attention

FX coverage

```



These are operational controls.



Do not expose them to ordinary users.



\---



\# 48. HEALTH



Backend exposes:



```text

GET /health

```



without authentication.



It checks:



```text

database

object storage

```



This can be useful for an internal admin/system-status interface, but do not expose sensitive backend diagnostics to normal users.



\---



\# 49. REPORTING ARCHITECTURE



Backend reporting intentionally uses:



```text

plain views

```



rather than materialized views.



Do not build frontend assumptions around refresh jobs or stale materialized data.



\---



\# 50. MINICRM MIGRATION



There is an existing migration system:



```text

autocrm-migrate

```



with:



```text

docs/migration/README.md

```



The frontend is not responsible for performing the migration.



However, the frontend MUST understand that migrated data has special semantics.



Important:



\* migration extraction does not transform raw JSON

\* file discovery is structural

\* migration originals are content-addressed

\* originals are retained for every category

\* migrated intake images receive object-lock retention

\* load is idempotent

\* migrated records preserve `raw\_import`

\* migrated orders use `MC-{MiniCRM id}` numbers

\* mapping is JSON-driven

\* reconciliation produces a Markdown sign-off report



Do not assume migrated records behave identically to newly created records.



\---



\# 51. WEB DESIGN DIRECTION



The user requirement:



> Not pixel-identical to MiniCRM, but structurally familiar.



Therefore:



\## Familiar structure, better materials.



The application should feel recognizable to an existing user while clearly being a better product.



The visual language should come from:



\* refrigerated vehicles

\* insulated panels

\* galvanised steel

\* brushed aluminium

\* workshop signage

\* ATP certification

\* technical documentation



Avoid:



\* generic SaaS dashboards

\* excessive rounded cards

\* glassmorphism

\* decorative gradients

\* giant hero sections

\* marketing-page aesthetics

\* excessive shadows

\* playful illustrations

\* AI-generated-dashboard aesthetics



This is a work tool.



\---



\# 52. COLOURS



Use:



```css

\--panel: #F7F8F7;

\--surface: #FFFFFF;



\--steel-900: #1B2327;

\--steel-500: #6B767C;

\--steel-200: #D5DBDC;



\--signal: #E8590C;

\--cold: #0F5C7A;

\--done: #2F7A3E;

```



Semantic meaning:



```text

signal → blocked / overdue / attention

cold   → MEO / certification / temperature state

done   → completed / paid

```



Saturated colors encode state.



Do not use them decoratively.



\---



\# 53. TYPOGRAPHY



Use:



```text

IBM Plex Sans

IBM Plex Mono

```



Hungarian must work correctly.



Test:



```text

Őrült űrhajós

```



and:



```text

Őrült űrhajós — 4 850 000 Ft

```



Typography:



```text

UI/body        Plex Sans

headings       Plex Sans

money          Plex Mono

dates          Plex Mono

IDs            Plex Mono

technical nums Plex Mono

plates         Plex Mono

```



Base size:



```text

16px

```



Scale:



```text

12.8px metadata

16px    body/workhorse

20px    section

25px    record title

31px    page title/key metric

```



\---



\# 54. HUNGARIAN-FIRST



Hungarian is the primary language.



Do not treat Hungarian as a translation added later.



Every user-visible string must go through i18n.



Design against Hungarian string lengths.



English infrastructure may exist, but Hungarian is the first-class UX.



Do not use:



```text

Order

Customer

Invoice

```



where the product vocabulary is:



```text

Megrendelés

Ügyfél/Partner

Számla

```



Use the terminology established in `DOCS/`.



\---



\# 55. NUMBER FORMATTING



Examples:



```text

HUF 4 850 000 Ft

EUR 12 400,00 €

Date 2026. 09. 10.

```



Use locale-aware formatting.



Do not manually format money/date values in every component.



Centralize formatting.



\---



\# 56. WEB STACK



Use:



```text

Next.js

App Router

TypeScript strict

React

Tailwind

TanStack Query

TanStack Table

react-hook-form

Zod where appropriate

next-intl

date-fns

```



Charts:



```text

Recharts

```



or lightweight SVG for simple visuals.



Use Motion only for functional transitions.



\---



\# 57. SHADCN



shadcn/ui may be used as accessible primitives.



But:



> \*\*Do not ship default shadcn visual styling.\*\*



Restyle it to Autotherm's design system.



The final application must not look like a generated shadcn dashboard.



\---



\# 58. SERVER/CLIENT COMPONENTS



Use Server Components where useful for:



\* page shells

\* static structure

\* initial non-interactive content



Use Client Components for:



\* tables

\* filters

\* forms

\* galleries

\* dialogs

\* interactive order details

\* mutations



Do not turn the entire application into:



```text

"use client"

```



without a reason.



\---



\# 59. NO SERVER ACTIONS FOR BUSINESS MUTATIONS



Do not use Next.js Server Actions to duplicate Rust API mutations.



Mutations go to the Rust API.



Next.js should not become a second application backend.



\---



\# 60. TANSTACK QUERY



Use TanStack Query for server state.



Server state includes:



```text

orders

leads

partners

contacts

images

documents

emails

reports

settings

users

stages

project types

```



After mutations, invalidate or update affected queries.



Do not use arbitrary:



```ts

setTimeout(() => refetch(), 1000)

```



as a replacement for correct cache invalidation.



\---



\# 61. UI STATE



Keep UI state separate from server state.



UI state:



```text

active tab

sidebar state

dialogs

selected rows

density

temporary filters

```



Do not create one giant global state store for the entire application.



\---



\# 62. CUSTOM COMPONENTS



Build:



```text

<AppShell />

<Sidebar />

<PageHeader />

<DataTable />

<Money />

<StageRail />

<StatusBadge />

<EmptyState />

<ErrorState />

<LoadingState />

<ConfirmDialog />

<ImageGrid />

<ImageLightbox />

<Uploader />

<FilterBar />

```



as appropriate.



Do not over-abstract.



Do not duplicate the same complex UI across ten screens.



\---



\# 63. TABLES



Tables are a primary interface.



Use TanStack Table.



Support where appropriate:



\* sorting

\* filtering

\* pagination

\* selection

\* density

\* column visibility

\* keyboard interaction



Do not automatically convert every table into cards on mobile.



\---



\# 64. ORDER DETAIL



The order detail is the most important screen.



Conceptually:



```text

┌──────────┬─────────────────────────────────────┬──────────────┐

│          │ #782 · Kovács Kft. · Sprinter 316   │  TRAVELLER   │

│   nav    │ ─────────────────────────────────── │              │

│          │ \[Adatok]\[Tervek]\[Képek]\[Költség]    │  ● Stage     │

│          │ \[Blokkolók]\[Számlák]               │  ● Stage     │

│          │                                     │  ◐ Current   │

│          │         tab content                 │  ○ Stage     │

└──────────┴─────────────────────────────────────┴──────────────┘

```



Traveller remains visible.



Approximate width:



```text

280px

```



sticky on desktop.



\---



\# 65. ORDER TABS



Expose all relevant backend functionality.



Potential sections:



```text

Adatok

Tervek

Képek

Költségek

Blokkolók

Számlák

```



But verify the exact business requirements against `DOCS/` and the backend.



Do not omit functionality just because it wasn't listed in this prompt.



\---



\# 66. LEAD SCREEN



Provide:



\* list

\* search

\* filtering

\* age

\* assignee

\* stage

\* detail

\* create

\* edit

\* conversion



Do not implement a separate frontend state machine.



Use backend stage definitions.



\---



\# 67. ORDER SCREEN



Provide:



\* search

\* filters

\* sorting

\* pagination

\* stage

\* partner

\* project type

\* assignee

\* open/closed state

\* vehicle information

\* financial value

\* blockers

\* useful dates



Search by plate must behave consistently with the backend's normalization rules.



\---



\# 68. RESPONSIVENESS



Desktop-primary.



Responsive enough for smaller screens.



Do not sacrifice data density unnecessarily.



For narrow screens:



\* hide low-priority columns

\* allow horizontal scrolling when appropriate

\* preserve important actions

\* use detail screens

\* keep text readable



Do not make the desktop application unusable just to achieve a fashionable mobile layout.



\---



\# 69. ACCESSIBILITY



Required:



\* keyboard navigation

\* semantic controls

\* visible focus

\* accessible dialogs

\* proper labels

\* table accessibility

\* useful error messages

\* contrast

\* reduced motion



Colour must never be the only carrier of information.



\---



\# 70. MOTION



Almost none.



No:



\* page-load animations

\* decorative transitions

\* parallax

\* scroll reveals

\* bouncing UI



Allowed:



\* upload progress

\* meaningful stage transition

\* panel/dialog interaction

\* functional feedback



Respect:



```text

prefers-reduced-motion

```



\---



\# 71. LOADING STATES



Every async experience needs a real loading state.



Examples:



```text

table skeleton

detail skeleton

gallery skeleton

report skeleton

button submitting state

upload progress

```



Do not use blank screens.



\---



\# 72. EMPTY STATES



Differentiate:



```text

no data

no matching data

no permission

request failed

server unavailable

```



Do not make every empty state look identical.



\---



\# 73. FORMS



Forms must:



\* validate obvious mistakes

\* preserve input after recoverable failure

\* show field-level errors

\* prevent double submission

\* show saving state

\* handle backend validation

\* confirm successful mutation



Do not clear the entire form after a failed request.



\---



\# 74. DESTRUCTIVE ACTIONS



Use confirmation for meaningful destructive operations:



\* deleting documents

\* deleting non-immutable images

\* irreversible actions

\* administrative changes

\* other backend-defined destructive actions



Do not require confirmation for every harmless action.



\---



\# 75. ANDROID — PURPOSE



Android is NOT a mobile version of the CRM.



Its purpose is:



> \*\*Get photos from a phone onto the correct order reliably.\*\*



V1:



```text

Find order

↓

Choose category

↓

Capture many photos

↓

Review

↓

Queue

↓

Upload

```



Out of scope unless backend/docs require it:



```text

full CRM

lead management

invoicing

reporting

cost entry

administration

contact sync

calling

```



\---



\# 76. ANDROID STACK



Use:



```text

Kotlin

Jetpack Compose

Material 3, restyled

MVVM

Repository

Ktor or Retrofit

kotlinx.serialization

Room

WorkManager

CameraX

Coil

Hilt

DataStore

```



Use the actual generated API contract.



\---



\# 77. ANDROID ORDER PICKER



Backend provides:



```text

GET /mobile/orders

```



with:



```text

q

all

```



and:



```text

Cache-Control: private, max-age=60

```



Use this endpoint for the compact mobile order picker instead of loading the entire normal order model unnecessarily.



Search should support the useful identifiers documented by the backend.



\---



\# 78. ANDROID OFFLINE SAFETY



The failure case to design against:



```text

100 photos captured

↓

signal disappears

↓

app closes

↓

photos disappear

```



This must never happen because of app architecture.



Flow:



```text

capture

↓

persist locally

↓

queue

↓

upload

↓

server confirmation

↓

safe cleanup

```



Never delete the only local copy before successful server confirmation.



\---



\# 79. ANDROID UPLOAD QUEUE



Use Room to persist upload state.



Possible states:



```text

PENDING

UPLOADING

RETRYING

FAILED

COMPLETED

```



WorkManager should handle durable background work.



Test:



\* app close

\* process death where applicable

\* signal loss

\* Wi-Fi changes

\* retries

\* partial completion



\---



\# 80. ANDROID PHOTO QUALITY



For intake images:



```text

original quality

```



must be preserved.



The backend explicitly stores originals byte-for-byte, including EXIF.



Derived copies are created by the backend.



Do not destroy evidentiary quality on the phone before upload.



\---



\# 81. ANDROID NETWORK POLICY



If a Wi-Fi-only preference is implemented:



\* it must actually affect WorkManager constraints

\* it must persist

\* it must survive app restarts

\* it must not merely change a visual toggle



Default conservatively for large photo uploads.



\---



\# 82. OFFLINE SCOPE



Do not build full bidirectional offline synchronization.



The Android app mainly needs:



```text

cached order picker

durable upload queue

```



New photos do not conflict with existing photos because uploads are content-addressed/idempotent.



Do not invent conflict resolution for nonexistent conflicts.



\---



\# 83. API CLIENT GENERATION



If the backend exposes/generates OpenAPI, use it.



Preferred pipeline:



```text

Rust types/handlers

&#x20;       ↓

OpenAPI

&#x20;       ↓

TypeScript client

&#x20;       ↓

Web

```



and:



```text

OpenAPI

&#x20;       ↓

Kotlin client

&#x20;       ↓

Android

```



Do not manually maintain large duplicated API type definitions when generation is possible.



\---



\# 84. TYPESCRIPT



Use strict TypeScript.



Absolute rule:



```text

NO `any`

```



No:



```ts

as any

```



No:



```ts

Record<string, any>

```



No hiding type errors.



Use proper API-generated types.



\---



\# 85. SECURITY



Never put secrets into the frontend bundle.



Never trust:



\* hidden fields

\* disabled buttons

\* client-side roles

\* client-side prices

\* client-side permissions



Backend remains authoritative.



Do not log:



\* passwords

\* access tokens

\* secrets

\* credentials



\---



\# 86. PERFORMANCE



Important areas:



\### Images



Virtualize.



Use thumbnails.



Lazy-load full resolution.



\### Tables



Use backend pagination.



Do not render thousands of rows unnecessarily.



\### Reports



Do not calculate massive report datasets in React when backend reports already exist.



\### React



Avoid unnecessary global state and giant re-render boundaries.



\---



\# 87. NO FRONTEND ACCOUNTING ENGINE



The frontend may display:



```text

line\_total\_minor

order value

normalised HUF

```



but should not become an accounting engine.



Backend is authoritative for:



\* totals

\* FX normalization

\* stage gates

\* permissions

\* invoice/business rules

\* state transitions



\---



\# 88. NO FRONTEND STAGE ENGINE



The frontend may present stage controls.



It must not decide:



```text

"this transition is allowed"

```



based on a duplicated hardcoded state machine.



It should ask the backend.



If the backend responds:



```text

stage\_gate

note\_required

invalid\_transition

```



display the correct explanation.



\---



\# 89. ERROR UX



Map backend errors to human-readable Hungarian messages.



Examples:



```text

stage\_gate

→ "A megrendelés nem léphet tovább, mert egy szükséges feltétel még nem teljesült."



note\_required

→ "A visszalépéshez indoklás szükséges."



currency\_locked

→ "A pénznem nem módosítható, amíg a megrendelésnek vannak tételei."



immutable

→ "Ez a kép bizonyítási célból védett, ezért nem törölhető."



already\_converted

→ "Ez a lead már megrendeléssé lett alakítva."

```



Do not expose raw backend internals.



However, preserve useful backend messages where appropriate.



\---



\# 90. QUERY INVALIDATION



Examples:



```text

create lead

→ invalidate lead list



convert lead

→ invalidate lead

→ invalidate order list



change order stage

→ invalidate order

→ invalidate stage history

→ invalidate relevant order list



create blocker

→ invalidate order

→ invalidate blocker list



resolve blocker

→ invalidate order

→ invalidate blocker list



upload image

→ invalidate image list

→ update upload state



create/edit partner

→ invalidate partner lists/detail

```



Use actual query keys.



Do not solve stale state with arbitrary timers.



\---



\# 91. VISUAL DENSITY



The CRM should be comfortable for six hours of use.



Prefer:



```text

clear hierarchy

dense information

strong alignment

quiet surfaces

meaningful state

```



over:



```text

huge cards

giant whitespace

decorative UI

```



\---



\# 92. THE TRAVELLER IS THE SIGNATURE FEATURE



If there is one thing that should make the user say:



> "This is better than MiniCRM."



it should be the traveller.



It must make these obvious:



```text

Where is the vehicle?

How long has it been there?

What's blocking it?

Can it move?

What happened before?

```



Do not waste the majority of the design effort on decorative UI.



\---



\# 93. IMPLEMENTATION ORDER



\## PHASE 0 — UNDERSTANDING



Before coding:



```text

\[ ] inspect DOCS

\[ ] inspect backend

\[ ] inspect migrations

\[ ] inspect routes

\[ ] inspect auth

\[ ] inspect existing frontend

\[ ] map API

\[ ] map entities

\[ ] map workflows

\[ ] map permissions

\[ ] identify gaps

```



Produce an internal feature map.



\---



\## PHASE 1 — FOUNDATION



Implement:



```text

Next.js

TypeScript strict

Tailwind

design tokens

fonts

i18n

API client

auth

route protection

app shell

navigation

error handling

query infrastructure

```



\---



\## PHASE 2 — PARTNERS + LEADS



Implement:



```text

partner list

partner detail

partner create/edit

contacts

lead list

lead detail

lead create/edit

lead stages

lead conversion

```



\---



\## PHASE 3 — ORDERS



Implement:



```text

order list

search

filters

pagination

order creation

order detail

order editing

line items

stage rail

stage history

audit

```



\---



\## PHASE 4 — BLOCKERS



Implement:



```text

blocker list

order blockers

create

edit

resolve

reopen

due dates

responsibility

nudge state

```



\---



\## PHASE 5 — MEDIA



Implement:



```text

upload initiation

direct storage upload

complete upload

idempotency

gallery

categories

thumbnails

lightbox

original access

immutable intake handling

documents

downloads

```



\---



\## PHASE 6 — EMAIL



Implement:



```text

email history

email detail

preview

send

cancel

retry where permitted

templates

variables

suppressions

```



\---



\## PHASE 7 — REPORTING



Implement all actual backend reports:



```text

volume

stage durations

throughput

stalled

blocker load

FX rates

```



\---



\## PHASE 8 — ADMIN / SETTINGS



Implement:



```text

users

roles

sessions

password management

stage definitions

project types

settings

admin status

jobs

FX fetch

operational jobs

```



where the authenticated user's capability permits.



\---



\## PHASE 9 — HARDENING



Test:



```text

loading

empty

errors

permissions

401

403

404

409

422

429

network failures

expired sessions

stale data

duplicate uploads

immutable media

stage gates

```



\---



\# 94. TEST CRITICAL WORKFLOWS



\## Authentication



```text

login

logout

expired session

wrong password

forced password change

session revocation

```



\## Lead



```text

create

edit

stage

convert

attempt invalid won transition

attempt duplicate conversion

```



\## Order



```text

create

edit

items

currency locking

stage forward

stage skip

stage gate

stage backward

reopen

cancel

```



\## Blocker



```text

create

edit

resolve

reopen

overdue

```



\## Media



```text

upload

already uploaded

failed upload

retry

complete

original access

immutable delete

```



\## Email



```text

preview

send

unresolved variable

suppressed recipient

cancel

failed

needs\_review

retry permission

```



\## Reports



```text

date filters

missing FX

empty report

large report

```



\---



\# 95. VISUAL QA



For every important screen, verify:



```text

\[ ] Hungarian text fits

\[ ] no clipping

\[ ] numbers align

\[ ] money is correctly formatted

\[ ] dates are correctly formatted

\[ ] status is obvious

\[ ] permissions are respected

\[ ] empty state is useful

\[ ] error state is useful

\[ ] loading state is useful

\[ ] keyboard navigation works

\[ ] responsive layout works

```



\---



\# 96. DO NOT CALL THE PROJECT COMPLETE BECAUSE IT COMPILES



Compilation is not completion.



This:



```text

npm run build

```



passing does NOT mean the CRM is finished.



A complete feature means:



```text

UI

\+

real API integration

\+

loading

\+

empty

\+

error

\+

permissions

\+

mutation

\+

cache invalidation

\+

correct business behavior

```



where applicable.



\---



\# 97. DO NOT OVERENGINEER



Do not introduce unnecessary:



```text

microservices

GraphQL

event buses

custom state frameworks

massive abstractions

duplicate domain layers

WebSockets

```



unless the repository or requirements actually require them.



Prefer boring, obvious code.



\---



\# 98. DO NOT UNDERENGINEER



Do not solve serious problems with:



```text

setTimeout refetches

fake local persistence

huge useEffect chains

global mutable state

floating-point financial arithmetic

hardcoded stage machines

hardcoded permissions

fake upload progress

```



Build the real thing.



\---



\# 99. IMPORTANT BACKEND-SPECIFIC WARNINGS



These are easy mistakes an AI coding agent might make.



DO NOT:



```text

hardcode final stage names

```



because real stage names are still configurable.



DO NOT:



```text

calculate FX rates in frontend

```



because backend owns MNB rates.



DO NOT:



```text

delete intake images

```



because they are immutable.



DO NOT:



```text

expose original image URLs to viewers

```



because originals require a capability.



DO NOT:



```text

change order currency with line items

```



because it is locked.



DO NOT:



```text

mark a lead won manually

```



because conversion owns that transition.



DO NOT:



```text

bypass image gates when skipping stages

```



because gates apply across skipped stages.



DO NOT:



```text

retry every needs\_review email

```



because delivery may already have happened.



DO NOT:



```text

invent invoice/VAT behavior

```



if the backend does not currently expose it.



The backend decisions explicitly state:



> VAT is not currently on line items; invoicing will add VAT handling later.



Therefore, do not add a frontend `vat\_rate` field to order line items just because an older plan mentioned it.



\---



\# 100. KNOWN HUMAN OPEN QUESTIONS



Do not pretend these are solved.



The current decision document identifies:



1\. transactional email provider

2\. MiniCRM file-download access

3\. GDPR/retention stance for immutable intake images and email logs

4\. historical invoice handling

5\. invoice numbering continuity

6\. actual stage names and project types



The frontend should be architected so these decisions can change without a rewrite.



Especially:



```text

stage definitions

project types

email provider

invoice behavior

retention behavior

```



must not be baked into dozens of components.



\---



\# 101. FINAL FEATURE AUDIT



Before declaring the project finished, compare the actual implementation against the repository.



Create a mental checklist:



```text

AUTH

\[ ] login

\[ ] logout

\[ ] session

\[ ] password

\[ ] forced password change

\[ ] session management

\[ ] permissions



USERS

\[ ] list

\[ ] create

\[ ] edit

\[ ] deactivate

\[ ] password reset

\[ ] session revocation



PARTNERS

\[ ] list

\[ ] search

\[ ] create

\[ ] edit

\[ ] archive

\[ ] contacts



LEADS

\[ ] list

\[ ] search

\[ ] filters

\[ ] detail

\[ ] create

\[ ] edit

\[ ] stage

\[ ] conversion



ORDERS

\[ ] list

\[ ] search

\[ ] filters

\[ ] pagination

\[ ] create

\[ ] edit

\[ ] items

\[ ] money

\[ ] vehicle information

\[ ] stage

\[ ] stage history

\[ ] audit



BLOCKERS

\[ ] list

\[ ] create

\[ ] edit

\[ ] resolve

\[ ] reopen

\[ ] due date

\[ ] responsibility



MEDIA

\[ ] upload

\[ ] retry

\[ ] idempotency

\[ ] gallery

\[ ] categories

\[ ] thumbnails

\[ ] lightbox

\[ ] originals

\[ ] immutable intake

\[ ] documents

\[ ] downloads



EMAIL

\[ ] correspondence

\[ ] detail

\[ ] preview

\[ ] send

\[ ] cancel

\[ ] retry

\[ ] templates

\[ ] variables

\[ ] suppressions

\[ ] needs\_review



REPORTS

\[ ] volume

\[ ] stage duration

\[ ] throughput

\[ ] stalled

\[ ] blocker load

\[ ] FX



CONFIG

\[ ] stages

\[ ] project types

\[ ] settings



ADMIN

\[ ] status

\[ ] jobs

\[ ] retries

\[ ] FX fetch

\[ ] maintenance jobs



ANDROID

\[ ] login

\[ ] order picker

\[ ] local cache

\[ ] camera

\[ ] categories

\[ ] review

\[ ] queue

\[ ] durable upload

\[ ] retries

\[ ] network loss

\[ ] app close recovery

```



If any item is unsupported by the backend, mark it as a backend gap instead of fabricating it.



\---



\# 102. FINAL RULE



Above everything else:



> \*\*Build Autotherm CRM, not a generic CRM template.\*\*



The finished application should feel like:



> \*\*Autotherm's own internal software.\*\*



Not:



> "a shadcn dashboard with an Autotherm logo."



It should be:



\* familiar to existing MiniCRM users

\* significantly easier to scan

\* dense but calm

\* Hungarian-first

\* financially safe

\* permission-aware

\* resilient to network failure

\* correct against the Rust backend

\* complete against the documented workflow

\* particularly excellent at the digital job traveller

\* reliable with large numbers of vehicle photos



And most importantly:



> \*\*There must not be a hidden workflow gap where something the old system could do has disappeared simply because nobody remembered to build it.\*\*



Before implementing anything, inspect the repository and understand the existing backend.



