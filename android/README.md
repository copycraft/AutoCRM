# AutoCRM Android

The shop-floor client. Kotlin, Jetpack Compose, one Gradle module.

The same work the browser client does, from a phone: orders, stage changes, partners, leads
and the correspondence log. Plus the one thing the browser cannot do — get photos onto a job
**without depending on there being a signal at that moment**.

## What it does

| Screen | What it is for |
|---|---|
| **Kiszolgáló** (first run only) | Which AutoCRM this phone talks to. Stored on the device. |
| **Munkák** | Order list and detail: value, vehicle, build spec, blockers, stage history, MiniCRM notes, stage changes, photos. |
| **Leadek** | Leads with their quotation and stage history. |
| **Ügyfelek** | Partners with the customer/supplier filter, contacts, their orders and leads. |
| **E-mailek** | The correspondence log: what was sent, to whom, whether it arrived. |
| **Sor** | The upload queue: what is waiting, what failed and why, retry or discard. |

## The upload queue

This is the part worth reading the code for. `docs` elsewhere in this repository record that
the phone's offline behaviour was "the single most consequential unknown in the MEO
workflow". The answer is:

1. The shutter writes a JPEG into app-private storage, hashes it, and inserts a row in
   `pending_uploads` **before the shutter is live again**. Nothing waits on a network.
2. `UploadWorker` (WorkManager, network-constrained) drains the queue: ask for a ticket,
   PUT the bytes straight to object storage, hand the ticket back to be confirmed.
3. A network failure backs off — 10 s, 1 m, 5 m, 30 m, 2 h — and stays queued.
   A 422 is marked `blocked` and shown on the queue screen with the server's reason.
4. A file is deleted only after the server has confirmed the row, or when the fitter taps
   *Eldobás*. Nothing else deletes a photo, ever.

Retrying a whole batch is free: the server answers `already_uploaded` for bytes it has
seen, and `(order_id, content_hash)` is unique on both sides.

The behaviours above are covered by `UploadQueueTest` — JVM tests against an in-memory Room
database and a MockWebServer, no device needed.

## Photos

Taken with the **phone's own camera app**, not one built into this one. An in-app
preview-and-shutter on CameraX does not match what the manufacturer's camera produces — its
processing, its stabilisation, its full sensor resolution — and MEO photos are evidence of a
vehicle's condition. Losing quality to save a screen transition is the wrong trade.

Photos are added from inside a job, which is also the right way round: a photo belongs to a
vehicle, and picking the job afterwards is how photos end up on the wrong one. Two doors,
both returning a full-quality original:

- **Kamera** hands the system camera app a file inside the queue's own directory.
- **Galéria** uses the system photo picker — no storage permission at all, and the user
  exposes only the items they choose.

The category is sticky between visits. The one deliberate interruption: choosing **Bevétel**
(intake) raises a banner once an hour, because intake photos can never be deleted or
re-filed — the database refuses both with a trigger — so silence there would be dangerous
rather than efficient.

## Building

Requires JDK 17 and an Android SDK with platform 35 and build-tools 35.0.0.

```bash
cd android
cp local.properties.example local.properties   # then set sdk.dir
./gradlew assembleDebug
```

`local.properties` carries two machine-local settings:

```properties
sdk.dir=/path/to/android-sdk
autocrm.apiUrl=https://crm.autotherm.hu
```

## Which server the app talks to

**Chosen on the phone, not baked into the build.** On first run the app asks for an address
before anything else, checks it against `/health`, and stores it on the device. The login
screen shows the current address with a *Kiszolgáló módosítása* button next to it, because
the moment someone discovers the address is wrong is when a login will not go through — and
a stale address looks exactly like a wrong password otherwise.

The address survives sign-out: it belongs to the phone, not to the person holding it.

`autocrm.apiUrl` (or `AUTOCRM_API_URL`) only sets what that screen **prefills**; debug
builds prefill `http://10.0.2.2:8080`, the emulator's view of the host machine. Typing
`192.168.1.10:8080` is enough — the scheme is added, default ports and any pasted path are
stripped, so two spellings of one server end up as one stored string (`ServerStoreTest`).

## Testing

```bash
./gradlew testDebugUnitTest
```

Three suites:

- `UploadQueueTest` — the queue, end to end, against a mock server.
- `ServerStoreTest` — what a thumb types becomes a usable base URL, or is refused. Every
  request is built by prefixing this string, so a stray path here breaks every endpoint at
  once and reads as "the server is broken".
- `OpenApiContractTest` — reads the committed `openapi/openapi.json` and fails if a field
  the phone parses has been renamed or removed. The DTOs are hand-written and partial (a
  generated client would pull every schema in the document into the APK), so this test is
  what keeps them honest. It has already caught one: `TransitionOption` is
  `{stage_key, manual, gates_met}`, not the `{key, allowed}` first guessed.

## What is deliberately not here

- **Composing email.** The log is readable; writing a body with attachments belongs on a
  desktop.
- **Creating or editing orders, partners and leads.** Not yet built — the phone currently
  reads, photographs and changes stages. This is the next thing to add, not a decision.
- **Line items.** The number the whole project reports on should not be typed on a phone.
- **Reports.** A table that needs a wide screen.
- **Offline caching of orders.** Everything except the queue is read straight from the API.
  Caching would mean deciding what happens when the cached copy and the server disagree,
  which is a synchronisation problem nobody asked for. The picker is cached for 60 seconds
  by the server's own `Cache-Control`, which is enough to survive a walk across the yard.
- **A second language.** Hungarian only, like the web client.
