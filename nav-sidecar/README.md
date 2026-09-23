# nav-sidecar

A small HTTP service that wraps [`open-nav`](https://github.com/eshton/open-nav)
(`@open-nav/core`, `@open-nav/client`, `@open-nav/invoicing`) so the Rust backend
can report invoices to NAV's **Online Számla** system by sending flat JSON.

Everything NAV-shaped stays in here: the XML, the SHA3 request signature, the
AES-decrypted exchange token, base64 payloads, the `RECEIVED → PROCESSING →
DONE/ABORTED` polling loop, and the `InvoiceData` schema itself. The caller sends
a supplier, a customer, some lines and a currency, and gets back a transaction id
and a verdict.

Node 22, Express 5, ESM, no database.

## Two groups of routes, and only one of them talks to NAV

| Route                              | NAV call?                                     |
| ---------------------------------- | --------------------------------------------- |
| `POST /invoices`                   | **yes** — `manageInvoice` (CREATE), then polls |
| `POST /invoices/:number/storno`    | **yes** — `manageInvoice` (STORNO), then polls |
| `POST /invoices/:number/annul`     | **yes** — `manageAnnulment`, then polls        |
| `GET /invoices/:number/chain`      | **yes** — `queryInvoiceChainDigest`            |
| `GET /invoices/:number/pdf`        | **yes** — `queryInvoiceData`, then renders     |
| `POST /proformas`                  | **no**                                         |
| `GET /proformas/:id/pdf`           | **no**                                         |
| `GET /health`                      | **no**                                         |

A **proforma (díjbekérő) is not an invoice.** It is a request for payment, it
carries no VAT deduction right, and Online Számla has nothing to say about it.
Nothing under `/proformas` opens a NAV transaction, exchanges a token, or uses a
credential — it builds the same document data and renders it. See
[Proformas](#proformas-díjbekérő--never-reported) below.

## Running it

```bash
docker compose --profile nav up -d nav-sidecar    # from the repository root
```

That starts it in **mock mode**: an in-process stand-in for NAV, so the whole
thing works end to end with no technical user. It listens on `http://localhost:8081`.

Without Docker:

```bash
cd nav-sidecar && npm ci && npm run build && MOCK_MODE=true node dist/index.js
```

```bash
cd nav-sidecar && npm test
```

## Configuration

Environment variables only. **No endpoint accepts a credential as a parameter** —
a caller that could name its own technical user would turn this into an open
relay for reporting invoices under someone else's tax number. Request bodies are
strict: an unknown field is a `400`, not a silent omission.

### Required (unless `MOCK_MODE=true`)

| Variable                   | Meaning                                                                     |
| -------------------------- | --------------------------------------------------------------------------- |
| `NAV_LOGIN`                | Technical user login (Online Számla portal → Technikai felhasználó)          |
| `NAV_PASSWORD`             | Technical user password, in clear; it is hashed before transmission          |
| `NAV_SIGN_KEY`             | Signature key (aláírókulcs)                                                  |
| `NAV_EXCHANGE_KEY`         | Exchange key (cserekulcs), used to decrypt the exchange token                |
| `NAV_TAX_NUMBER`           | The taxpayer being reported for — **8 digits**, not the 11-digit form        |
| `NAV_SOFTWARE_ID`          | Exactly 18 characters of `[0-9A-Z-]`. No registry: prefix it with your tax number |
| `NAV_SOFTWARE_NAME`        | Billing software name, for NAV's statistics and support                      |
| `NAV_SOFTWARE_DEV_CONTACT` | Developer's electronic contact                                               |

`NAV_ENVIRONMENT` is `test` (the default) or `production`. The test and production
systems issue **separate, non-interchangeable** technical users.

### Optional

| Variable                        | Default                | Meaning                                                 |
| ------------------------------- | ---------------------- | ------------------------------------------------------- |
| `MOCK_MODE`                     | `false`                | Start a fake NAV in-process; no credentials needed       |
| `PORT` / `HOST`                 | `8080` / `0.0.0.0`     | Where to listen                                          |
| `NAV_SOFTWARE_DEV_NAME`         | `NAV_SOFTWARE_NAME`    | Developer's name                                         |
| `NAV_SOFTWARE_VERSION`          | `1.0`                  | Release version (the version does **not** go in the id)  |
| `NAV_SOFTWARE_OPERATION`        | `LOCAL_SOFTWARE`       | Or `ONLINE_SERVICE`                                      |
| `NAV_SOFTWARE_DEV_COUNTRY_CODE` | —                      | ISO-3166 alpha-2                                         |
| `NAV_SOFTWARE_DEV_TAX_NUMBER`   | —                      | Developer's tax number                                   |
| `NAV_BASE_URL`                  | from `NAV_ENVIRONMENT` | Overrides the endpoint; for a mock running elsewhere     |
| `NAV_REQUEST_ID_PREFIX`         | —                      | Prefix on generated `requestId`s, for tracing            |
| `NAV_TIMEOUT_MS`                | `30000`                | Per HTTP attempt towards NAV                             |
| `NAV_POLL_TIMEOUT_MS`           | `60000`                | How long to wait for a transaction's verdict             |
| `NAV_POLL_INITIAL_DELAY_MS`     | `1000`                 | First delay before polling; it then backs off to 8s      |
| `PDF_LANGUAGE`                  | `hu`                   | `hu`, `en` or `de`                                       |
| `PROFORMA_TTL_SECONDS`          | `3600`                 | How long a rendered proforma stays fetchable by id       |
| `PROFORMA_CACHE_MAX`            | `200`                  | Most proformas held in memory at once                    |
| `BODY_LIMIT`                    | `1mb`                  | Largest request body accepted                            |

See [`.env.example`](.env.example).

## The invoice payload

One shape serves every endpoint that takes an invoice — `POST /invoices`, the
`original` of a storno, and `POST /proformas`. It is flat on purpose: NAV's own
`InvoiceData` is built from it inside the sidecar.

```json
{
  "invoiceNumber": "AT-2026-0001",
  "issueDate": "2026-09-21",
  "deliveryDate": "2026-09-21",
  "paymentDate": "2026-10-05",
  "currency": "HUF",
  "exchangeRate": 1,
  "appearance": "PAPER",
  "paymentMethod": "TRANSFER",
  "priceMode": "net",
  "orderNumbers": ["MEGR-2026-77"],
  "deliveryPeriod": { "start": "2026-09-01", "end": "2026-09-30" },
  "supplier": {
    "name": "Autotherm Kft",
    "taxNumber": "12345678",
    "bankAccount": "12345678-12345678-12345678",
    "address": {
      "countryCode": "HU",
      "postalCode": "1117",
      "city": "Budapest",
      "streetName": "Kossuth",
      "publicPlaceCategory": "utca",
      "number": "12"
    }
  },
  "customer": {
    "name": "Beszerző Kft",
    "taxNumber": "99887764",
    "vatStatus": "DOMESTIC",
    "address": {
      "postalCode": "6000",
      "city": "Kecskemét",
      "streetName": "Petőfi",
      "publicPlaceCategory": "tér",
      "number": "3"
    }
  },
  "lines": [
    {
      "description": "Hűtőgépes felépítmény beépítése",
      "quantity": 1,
      "unit": "PIECE",
      "unitPrice": "1200000",
      "vatPercentage": "0.27",
      "nature": "SERVICE"
    }
  ]
}
```

Only these are required: `invoiceNumber`, `issueDate`, `supplier` (name, taxNumber,
address), `customer` (name, address) and at least one line.

Notes that save a rejection:

- **`vatPercentage` is a fraction.** `0.27`, never `27`. A value above `1` is a
  `400`; the sidecar will not guess at a tax rate.
- **Money and quantities take a number or a decimal string.** Use strings
  (`"1200000.50"`) where the exact figure matters — JSON numbers are doubles.
- **`supplier.taxNumber` must be the taxpayer `NAV_TAX_NUMBER` authenticates as.**
  NAV rejects a report filed for someone else, and so does the sidecar, before
  sending anything.
- **`customer.taxNumber`** is omitted for a private individual; `vatStatus` then
  defaults to `PRIVATE_PERSON`.
- `priceMode: "gross"` takes each `unitPrice` as VAT-inclusive and derives the
  net from the line's rate. NAV is net-based, so the reported gross total may
  differ from `quantity × grossUnitPrice` by a rounding unit.
- `currency` defaults to `HUF`; anything else needs `exchangeRate` to HUF.
- `unit` is one of NAV's: `PIECE`, `KILOGRAM`, `TON`, `KWH`, `DAY`, `HOUR`,
  `MINUTE`, `MONTH`, `LITER`, `KILOMETER`, `CUBIC_METER`, `METER`,
  `LINEAR_METER`, `CARTON`, `PACK`, `OWN`.
- An invoice number appearing in a path must be percent-encoded if it contains
  `/` or other URL-significant characters.

## Endpoints that report to NAV

### `POST /invoices`

Builds the invoice, validates it, submits it as a `CREATE` operation, and polls
until NAV has a verdict. Body: the invoice payload above.

`201 Created`:

```json
{
  "invoiceNumber": "AT-2026-0001",
  "operation": "CREATE",
  "transactionId": "4Q7ZXY8K2M1N",
  "status": "DONE",
  "accepted": true,
  "messages": [],
  "totals": { "currency": "HUF", "net": "1200000.00", "vat": "324000.00", "gross": "1524000.00" }
}
```

`status` is NAV's own: `DONE` means stored. `messages` carries NAV's `WARN`
findings on an accepted invoice — those do not block the report, but they
usually mean something is wrong with the data. `transactionId` is worth storing:
it is the handle for any later support question.

### `POST /invoices/:invoiceNumber/storno`

Full cancellation of an issued invoice. The path names the **original**; the body
names the storno document.

```json
{
  "stornoInvoiceNumber": "AT-2026-0001-S",
  "issueDate": "2026-09-22",
  "modificationIndex": 1,
  "chainLineBase": 3,
  "original": { "...": "the invoice payload, optional" }
}
```

Only `stornoInvoiceNumber` is required. **Omit `original`** and the sidecar reads
the invoice back from NAV (`queryInvoiceData`) and reverses that — the sidecar
keeps no invoice store, and NAV holds the authoritative copy. Pass `original`
when the invoice was never reported through this sidecar, or when the caller
holds the authoritative copy; its `invoiceNumber` must match the path.

`modificationIndex` and `chainLineBase` matter only when the invoice already has
a modification chain: a storno cancels the invoice's *current* state, and reusing
a chain position NAV has already seen is `INVOICE_LINE_ALREADY_EXISTS`.

Response: the submission shape above, with `"operation": "STORNO"`, the storno's
own number as `invoiceNumber`, `originalInvoiceNumber`, and negated `totals`.

### `POST /invoices/:invoiceNumber/annul`

Technical annulment (`manageAnnulment`) — for a data report that should never
have been filed, as opposed to an invoice that should be cancelled.

```json
{ "code": "ERRATIC_DATA", "reason": "A vevő adószáma hibásan került be" }
```

`code` is one of `ERRATIC_DATA`, `ERRATIC_INVOICE_NUMBER`,
`ERRATIC_INVOICE_ISSUE_DATE`, `ERRATIC_ELECTRONIC_HASH_VALUE`. Response: the
submission shape, with `"operation": "ANNUL"` and no `totals`.

An annulment is submitted, then **approved or rejected by a person** in the
Online Számla portal. A `201` here means NAV accepted the request, not that the
report is gone.

### `GET /invoices/:invoiceNumber/chain`

The modification/storno chain, every page walked.

Query: `direction` (`OUTBOUND`, the default, or `INBOUND`), `taxNumber` (the
counterparty's — the search criterion for an `INBOUND` query, not expected on an
outbound one).

```json
{
  "invoiceNumber": "AT-2026-0001",
  "direction": "OUTBOUND",
  "elements": [
    {
      "invoiceNumber": "AT-2026-0001",
      "operation": "CREATE",
      "supplierTaxNumber": "12345678",
      "customerTaxNumber": "99887764",
      "insDate": "2026-09-21T09:14:02Z",
      "originalRequestVersion": "3.0"
    },
    {
      "invoiceNumber": "AT-2026-0001-S",
      "operation": "STORNO",
      "supplierTaxNumber": "12345678",
      "insDate": "2026-09-22T11:02:44Z",
      "originalRequestVersion": "3.0",
      "modifies": {
        "originalInvoiceNumber": "AT-2026-0001",
        "modificationIndex": 1,
        "modifyWithoutMaster": false
      }
    }
  ]
}
```

### `GET /invoices/:invoiceNumber/pdf`

The invoice as a PDF, rendered from **what NAV holds** — so the document shows
the data that was actually reported, not a local copy that may have drifted.

Query: `direction` (default `OUTBOUND`), `language` (`hu`, `en`, `de`; defaults
to `PDF_LANGUAGE`), `supplierTaxNumber` (needed to disambiguate an `INBOUND`
query).

`200 OK`, `Content-Type: application/pdf`. Rendering needs nothing installed: an
embedded Roboto subset spells `ő` and `ű` correctly, which the PDF core fonts
cannot.

## Proformas (díjbekérő) — never reported

**No NAV call happens on either of these routes.** No token exchange, no
transaction, no credential; they work with `MOCK_MODE` off and no technical user
configured at all. A proforma is a request for payment: it looks like an invoice,
grants no VAT deduction right, and Online Számla neither wants nor accepts it.
The data usually becomes a real invoice later — that is what `POST /invoices` is
for, and only that call reports anything.

### `POST /proformas`

Body: the invoice payload, plus three optional fields — `id` (what
`GET /proformas/:id/pdf` will answer to; defaults to `invoiceNumber`), `note`
(printed under the totals, e.g. payment instructions) and `language`.

`201 Created`:

```json
{
  "id": "proforma-1",
  "documentType": "proforma",
  "invoiceNumber": "DB-2026-0001",
  "reportedToNav": false,
  "totals": { "currency": "HUF", "net": "1200000.00", "vat": "324000.00", "gross": "1524000.00" },
  "pdfBase64": "JVBERi0xLjMK...",
  "expiresAt": "2026-09-21T13:22:58.848Z",
  "messages": []
}
```

The PDF comes back in the response, so one call is enough. `messages` carries the
local validator's findings **for information only** — nothing is submitted, so
nothing here blocks the document; it is a cheap early warning about data that is
headed for a real invoice.

### `GET /proformas/:id/pdf`

`200 OK`, `Content-Type: application/pdf`, served from an in-memory hold left by
the render.

**The sidecar persists nothing.** It has no database, and the mapping from an id
to a payload is the caller's to keep. The hold is a convenience with a TTL
(`PROFORMA_TTL_SECONDS`) and a size cap (`PROFORMA_CACHE_MAX`); after that — or
after a restart — the answer is a `404` saying so, and the caller re-`POST`s the
payload it stored. Losing the hold costs a re-render and nothing else.

## `GET /health`

Liveness only: no credentials are used and NAV is not contacted, so it answers
while the tax authority is down and can be polled as often as a container runtime
likes.

```json
{
  "status": "ok",
  "mockMode": false,
  "environment": "production",
  "softwareId": "12345678-AUTOCRM1",
  "uptimeSeconds": 1841
}
```

## Errors

Every failure answers with the same envelope, and **NAV's own fault codes are
passed through rather than folded into a generic 500** — the backend needs the
real code to log and show.

```json
{
  "error": {
    "kind": "nav_rejected",
    "message": "NAV rejected the invoice: INVOICE_NUMBER_ALREADY_EXISTS: Invoice number already exists",
    "transactionId": "4Q7ZXY8K2M1N",
    "messages": [
      {
        "source": "business",
        "level": "ERROR",
        "code": "INVOICE_NUMBER_ALREADY_EXISTS",
        "message": "Invoice number already exists",
        "path": "/InvoiceData/invoiceNumber"
      }
    ]
  }
}
```

`messages` is always present, possibly empty. `navErrorCode` and `navFuncCode`
appear when NAV answered with a fault of its own; `transactionId` appears when
the failure happened after a batch was accepted.

| `kind`            | HTTP | Means                                                                   |
| ----------------- | ---- | ----------------------------------------------------------------------- |
| `bad_request`     | 400  | The body or a query parameter does not match the documented shape        |
| `validation`      | 422  | NAV would reject this; caught locally, nothing was sent                  |
| `nav_rejected`    | 422  | NAV took the batch and then rejected the invoice (`ABORTED`)             |
| `not_found`       | 404  | No such invoice at NAV, no such held proforma, or no such route          |
| `nav_error`       | 502  | NAV answered with a fault — bad credentials, bad signature, bad request  |
| `nav_unreachable` | 504  | NAV could not be reached, or the transaction never settled in time       |
| `config`          | 500  | The sidecar is misconfigured                                             |
| `internal`        | 500  | A bug here                                                               |

`message` is for humans; branch on `kind` and on each message's `code`.

The distinction between `validation` and `nav_rejected` is worth keeping: the
first means the document never left the building, the second means NAV has a
transaction recorded for it.

## Mock mode

`MOCK_MODE=true` starts [`@open-nav/mock-server`](https://www.npmjs.com/package/@open-nav/mock-server)
in-process and points the client at it. It is not a stub: it speaks the real XML
on the real paths, recomputes the request signature the way NAV does, rejects a
replayed `requestId`, spends an exchange token exactly once, and decides an
invoice's fate with the same validator — so a broken invoice comes back
`ABORTED` with the fault code NAV would have reported.

Two things to know when testing against it:

- **`queryInvoiceChainDigest` is not implemented by the mock.** `GET /chain`
  therefore answers `502` with `navErrorCode: INVALID_REQUEST` — which is itself
  a fair test of the error path, but not of the chain.
- **`nav_rejected` is unreachable in mock mode.** The mock and the sidecar share
  a validator, so anything the mock would abort is already refused locally as
  `validation`. Against the real service the two differ, because NAV knows things
  no local check can (an invoice number already used, a taxpayer struck off).

Credentials in mock mode are stand-ins and need not be set. Its acceptance is not
evidence that NAV will accept the same document: it catches what is decidable
locally, early and cheaply.

## Layout

```
src/
  index.ts        start-up, graceful shutdown; exports startSidecar() for tests
  app.ts          the Express app: /health, and the two route groups
  config.ts       the environment, and nothing but the environment
  nav.ts          the NAV client, and the mock when MOCK_MODE is on
  schemas.ts      the HTTP boundary — strict, flat, generated against
  invoices.ts     build → validate → submit → poll, and the NAV queries
  proformas.ts    build → render. No client, no credentials, no transaction
  errors.ts       one envelope; NAV's codes kept verbatim
  routes/
    invoices.ts   everything here talks to NAV
    proformas.ts  nothing here does
test/
  e2e.test.js     every endpoint, end to end, against the mock
  errors.test.js  the error translation, including paths the mock cannot reach
```
