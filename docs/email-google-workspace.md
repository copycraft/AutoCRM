# Sending email through Google Workspace

AutoCRM sends all mail — automatic nudges and alerts, and staff-written messages — through
Google Workspace's **SMTP relay**. Mail leaves from Autotherm's own domain, which already has
sending history with Gmail and Outlook, so there is no new provider, no new reputation to
build and no extra bill.

What you get:

- **Automatic mail** comes from a real mailbox (e.g. `beszerzes@autotherm.hu`), so supplier
  replies to a nudge reach a person. It carries `Auto-Submitted` headers, so vacation
  responders don't answer it.
- **Staff mail** is sent *as the staff member* (`kovacs@autotherm.hu`) when their address is in
  `EMAIL_SENDER_DOMAINS`, so replies land in their own Gmail inbox.
- Limitation: mail sent through the relay does **not** appear in the sender's Gmail *Sent*
  folder. The full message is in AutoCRM's email log on the order. If that becomes a problem,
  the next step is sending manual mail through the Gmail API instead.

Menu names below match the Google Admin console at the time of writing; Google moves things
occasionally.

## 1. Admin console: enable the relay

Needs a Workspace super admin.

1. **Admin console → Apps → Google Workspace → Gmail → Routing**.
2. Find **SMTP relay service** → **Configure** (or *Add another rule*). Name it `AutoCRM`.
3. **Allowed senders:** *Only addresses in my domains*.
   (*Only registered Apps users* is stricter: then `EMAIL_FROM_AUTOMATIC` must be a real user or
   alias, not a group.)
4. **Authentication:** tick *Only accept mail from the specified IP addresses* and add the
   server's **public** IP address(es). Leave *Require SMTP Authentication* off — then
   `SMTP_USERNAME`/`SMTP_PASSWORD` stay empty.
5. Tick **Require TLS encryption**.
6. **Save.** Changes can take up to an hour or so to apply.

**The IPv6 trap.** Many VPS hosts have IPv6. If the server connects to Google over IPv6 but
only its IPv4 address is allowlisted, Google answers `550 5.7.0 Mail relay denied`. Either
allowlist both addresses, or set `SMTP_FORCE_IPV4=true` (recommended). Find the server's
addresses with `curl -4 ifconfig.me` and `curl -6 ifconfig.me`.

**Dynamic home IP?** The IP allowlist needs a static address. Otherwise enable
*Require SMTP Authentication* on the relay and give AutoCRM a dedicated Workspace account's
credentials (`SMTP_USERNAME`, `SMTP_PASSWORD`).

## 2. Mailboxes

- `EMAIL_FROM_AUTOMATIC` — the sender for nudges and alerts. Use a mailbox or Google Group that
  someone reads. If it's a group, allow external members to post to it, or supplier replies bounce.
- `EMAIL_REPLY_TO_DEFAULT` — where replies to automatic mail go (can be the same address).
- Staff accounts in AutoCRM should use their Workspace addresses so they send as themselves.

## 3. DNS: check before switching on automatic email

Look these up with `nslookup -type=txt autotherm.hu` (and the names below), or an online DNS
checker.

| Record | Name | Should contain |
|---|---|---|
| SPF | `autotherm.hu` TXT | `v=spf1 ... include:_spf.google.com ... ~all` (exactly **one** SPF record) |
| DKIM | `google._domainkey.autotherm.hu` TXT | `v=DKIM1; k=rsa; p=...` |
| DMARC | `_dmarc.autotherm.hu` TXT | `v=DMARC1; p=none; rua=mailto:dmarc@autotherm.hu` to start |

**DKIM is the one most often missing.** Admin console → Apps → Google Workspace → Gmail →
**Authenticate email** → generate the key, publish the TXT record, wait for DNS, then press
**Start authentication**. Without DKIM, Gmail and Outlook increasingly send mail to spam or
reject it.

Start DMARC at `p=none`, watch the reports for a few weeks, then tighten to `quarantine`.

## 4. Configure AutoCRM

In the production `.env`:

```
EMAIL_MODE=smtp
SMTP_HOST=smtp-relay.gmail.com
SMTP_PORT=587
SMTP_SECURITY=starttls
SMTP_HELO_NAME=autotherm.hu
SMTP_FORCE_IPV4=true
EMAIL_FROM_AUTOMATIC=beszerzes@autotherm.hu
EMAIL_FROM_NAME=Autotherm
EMAIL_REPLY_TO_DEFAULT=iroda@autotherm.hu
EMAIL_MESSAGE_ID_DOMAIN=autotherm.hu
EMAIL_SENDER_DOMAINS=autotherm.hu
```

`SMTP_HELO_NAME` matters: Google throttles or refuses relays that announce themselves as
`localhost` or a bare machine name.

## 5. Test, in this order

1. **Connection + one message**, straight through the transport, from the server:

   ```bash
   ./autocrm email-test --to you@autotherm.hu
   ```

   It prints the connection details, what Google answered, and a hint for common errors.

2. **Headers.** In Gmail open the test message → *⋮ → Show original*. SPF, DKIM and DMARC
   should all say **PASS**. Also send one to a non-Google address (e.g. an Outlook.com account)
   and check it isn't in spam.

3. **Staging / parallel run.** Set `EMAIL_REDIRECT_TO=iroda@autotherm.hu`. Every message —
   nudges, stage notifications, staff mail — is delivered to that one inbox with the original
   recipients in the subject (`[TESZT – eredeti címzett: …]`). This is the only way real SMTP
   is allowed outside `APP_ENV=production`, and it's also useful in production for the first
   days after cutover.

4. **Go live.** Remove `EMAIL_REDIRECT_TO`, restart, then switch on automatic email in
   Settings (the kill switch is off by default). `GET /api/admin/status` shows the current
   mode, SMTP host and whether a redirect is active.

## Troubleshooting

Failed messages keep the server's answer plus a hint in the email log (`error` field).

| Error | Meaning / fix |
|---|---|
| `550 5.7.0 Mail relay denied` | Server IP not allowlisted (check IPv6 → `SMTP_FORCE_IPV4=true`), or the From address is outside your domains |
| `550 5.7.1 ... not authorized` | *Only registered Apps users* is set and the From address is a group or unknown |
| `421 4.7.0 Try again later` | Temporary: rate limiting or a bad EHLO name. Retried automatically; set `SMTP_HELO_NAME` |
| `535 5.7.8 Username and Password not accepted` | Wrong credentials, or authentication isn't what the relay rule expects |
| Connection refused / timed out | Wrong host/port, or the hosting provider blocks outbound SMTP ports (some do by default; ask them to open 587) |
| Delivered but in spam | SPF/DKIM/DMARC not all passing; see section 3 |

Relay sending limits are per user per day and far above AutoCRM's volume (tens of messages a day).
