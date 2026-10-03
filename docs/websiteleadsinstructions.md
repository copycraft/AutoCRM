# Website leads API: integration guide

How the autotherm.hu website sends enquiries (contact form, quote request) to the AutoCRM
as leads. One endpoint, one secret key, one JSON body.

## Summary

| | |
|---|---|
| **Endpoint** | `POST https://<crm-host>/api/leads/website` |
| **Auth** | Header `X-Leads-Key: <secret>` (the CRM's `LEADS_API_KEY`) |
| **Body** | JSON, `Content-Type: application/json` |
| **Success** | `202 Accepted`, empty body |
| **Where it lands** | A new lead in the CRM's first stage, unassigned, source `website` |

`<crm-host>` is the address the CRM is deployed at (for example `https://crm.autotherm.hu`).
You will be given the real host and the key by the CRM owner.

## The key must stay on your server

The key authorises anyone holding it to create leads. **Never put it in browser JavaScript,
a mobile app, or a public repository.** The visitor's browser submits the form to the
website's own backend (a Next.js route handler, a PHP script, a serverless function,
whatever the site runs on), and that backend calls the CRM with the key. Store the key in an
environment variable or secret store.

```
visitor's browser ──form──▶ website backend ──POST + X-Leads-Key──▶ CRM
```

## Request

```
POST /api/leads/website
X-Leads-Key: <secret>
Content-Type: application/json
```

```json
{
  "name": "Kiss Péter",
  "email": "peter@example.hu",
  "phone": "+36 30 123 4567",
  "message": "Hűtős kisteherautó átalakítás érdekel, árajánlatot kérek.",
  "subject": "Hűtőkamra",
  "vehicle": "Mercedes Sprinter 316",
  "page": "/szolgaltatasok/hutokamra",
  "company": ""
}
```

| Field | Required | Max length | Notes |
|---|---|---|---|
| `name` | **yes** | 200 | The person asking. Blank (or whitespace only) is rejected. |
| `email` | one of `email` / `phone` | 300 | Must be a valid address if sent. Stored lower-cased. |
| `phone` | one of `email` / `phone` | 50 | Any format; stored as typed. |
| `message` | no | 5000 | The visitor's text. |
| `subject` | no | 300 | What they ask about (service, product). Becomes the lead title: `Weboldal: <subject>`; if absent the title is `Weboldal: <name>`. |
| `vehicle` | no | 300 | Free text. Added to the description as `Jármű: …`. |
| `page` | no | 300 | The page the form was on. Added to the description as `Oldal: …`. |
| `company` | no | | **Honeypot, see below.** Send empty or omit. |

At least one of `email` or `phone` is required, so the office has a way to reply.
Empty strings count as "not sent". Unknown extra fields are ignored. All text is stored
as plain text, never interpreted as HTML.

### Honeypot (anti-spam)

Add an extra input named `company` to the HTML form, hidden with CSS
(`position:absolute; left:-9999px`, `tabindex="-1"`, `autocomplete="off"`). People never see
or fill it; simple bots fill every field. If `company` arrives non-empty the CRM answers
`202` as usual but **stores nothing**, so the bot learns nothing. Forward the field's value
as-is.

## Responses

| Status | Meaning | What to do |
|---|---|---|
| `202` | Accepted (or silently dropped as spam). | Show the visitor a thank-you message. |
| `400` | Invalid form. | Show a form error. Body names the problem (below). |
| `403` | Key missing or wrong, **or the endpoint is switched off** on the CRM. | Configuration problem, not the visitor's fault. Log and alert; do not retry. |
| `5xx`, timeout | CRM unavailable. | Retry (see below) and keep the enquiry so it is not lost. |

Error bodies share one shape:

```json
{ "error": { "code": "validation", "message": "email or phone is required" } }
```

Validation messages you can get (English, for your logs; show visitors your own text):

- `name is required`
- `email is not a valid address`
- `email or phone is required`
- `<field> is too long (at most N characters)`
- a JSON parse message if the body is not valid JSON

Do not echo the CRM's `message` to visitors.

## Reliability

- The call is **not idempotent**: the CRM does not de-duplicate. Two identical `POST`s make
  two leads. Disable the submit button after the click.
- Use a timeout of about 10 seconds. On a network error or `5xx`, retry a few times with a
  short back-off. Do not retry `400` or `403`.
- If the CRM stays unreachable, store the enquiry (queue, file, or e-mail to the office)
  rather than showing success and losing it.
- There is **no rate limiting** on this endpoint. Put CAPTCHA (for example Cloudflare
  Turnstile) or rate limiting on your own form endpoint.

## Examples

### curl

```bash
curl -i https://crm.autotherm.hu/api/leads/website \
  -H "X-Leads-Key: $LEADS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name":"Teszt Elek","email":"teszt@example.hu","message":"Teszt érdeklődés"}'
# HTTP/1.1 202 Accepted
```

### Node / Next.js route handler (server side)

```ts
// app/api/contact/route.ts
export async function POST(req: Request) {
  const form = await req.json();

  const res = await fetch(`${process.env.CRM_URL}/api/leads/website`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Leads-Key": process.env.CRM_LEADS_KEY!,
    },
    body: JSON.stringify({
      name: form.name,
      email: form.email,
      phone: form.phone,
      message: form.message,
      subject: form.subject,
      vehicle: form.vehicle,
      page: form.page,
      company: form.company, // the hidden honeypot field
    }),
    signal: AbortSignal.timeout(10_000),
  });

  if (res.status === 202) return Response.json({ ok: true });
  if (res.status === 400) return Response.json({ ok: false }, { status: 400 });
  console.error("CRM lead API failed", res.status); // 403 = key/config problem
  return Response.json({ ok: false }, { status: 502 });
}
```

### PHP

```php
$ch = curl_init(getenv('CRM_URL') . '/api/leads/website');
curl_setopt_array($ch, [
  CURLOPT_POST => true,
  CURLOPT_HTTPHEADER => [
    'Content-Type: application/json',
    'X-Leads-Key: ' . getenv('CRM_LEADS_KEY'),
  ],
  CURLOPT_POSTFIELDS => json_encode([
    'name' => $_POST['name'] ?? '',
    'email' => $_POST['email'] ?? '',
    'phone' => $_POST['phone'] ?? '',
    'message' => $_POST['message'] ?? '',
    'company' => $_POST['company'] ?? '',
  ]),
  CURLOPT_RETURNTRANSFER => true,
  CURLOPT_TIMEOUT => 10,
]);
curl_exec($ch);
$status = curl_getinfo($ch, CURLINFO_HTTP_CODE); // 202 on success
```

## Testing

1. Send the curl example above with the real key. Expect `202`.
2. The lead appears in the CRM under *Leadek* with title `Weboldal: …` and source `website`.
3. Send it with a wrong key. Expect `403`. Send it without `email` and `phone`. Expect `400`.
4. Send a body with `"company": "x"`. Expect `202`, and **no** lead appears.

## For the CRM owner

- The key is the CRM's `LEADS_API_KEY` environment variable. Generate one with
  `openssl rand -hex 32`. Unset it and the endpoint answers `403` to everyone.
- To rotate the key: set a new `LEADS_API_KEY`, restart the CRM, update the website's secret.
- The machine-readable contract is `openapi/openapi.json` (operation `POST /leads/website`).
- **Office notification:** every accepted website lead also queues an email to the office
  mailbox (default `vastag.peter@autotherm.hu`; change it with `LEADS_NOTIFY_TO`, or set it
  empty to switch the alert off). It is automatic mail, so it only goes out when
  *Automatic email* is switched on in the CRM settings and the mail transport is configured
  (`EMAIL_MODE=smtp`). The send window and the per-recipient daily cap do not apply to it.
  A failed alert never loses the lead; check the *E-mailek* list if one does not arrive.
