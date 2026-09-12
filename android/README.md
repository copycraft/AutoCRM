# AutoCRM Android

The shop-floor client. Kotlin, Jetpack Compose, one Gradle module.

It exists for one thing the web client cannot do: put a camera in the fitter's hand and get
the photo onto the order **without depending on there being a signal at that moment**. The
rest of the CRM is here too — orders, stage changes, partners, leads — but the capture tab
is the first tab because it is the reason the app is worth installing.

## What it does

| Screen | What it is for |
|---|---|
| **Fotó** (opens here) | Camera, sticky order, sticky category, one-tap shutter. |
| **Munkák** | Order list and detail: value, vehicle, build spec, blockers, stage history, MiniCRM notes, stage changes. |
| **Ügyfelek** | Partners with the customer/supplier filter, contacts, their orders and leads; leads with their quotation. |
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

## Taps to first photo

The viability review counted at least five and named the two fixes; both are implemented.
The order is sticky (`CapturePrefs.currentOrder`) and the category is sticky
(`CapturePrefs.category`), so a returning fitter is one tap — the shutter — from a photo.

The one deliberate interruption: while the sticky category is **Bevétel** (intake), a banner
appears once an hour. Intake photos can never be deleted or re-filed — the database refuses
it with a trigger — so silence there would be dangerous rather than efficient.

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

The debug build ignores `autocrm.apiUrl` and points at `http://10.0.2.2:8080`, which is the
host machine as seen from the emulator — run the backend locally and the app talks to it.
Release builds use `autocrm.apiUrl`, or `AUTOCRM_API_URL` from the environment.

## Testing

```bash
./gradlew testDebugUnitTest
```

Two suites:

- `UploadQueueTest` — the queue, end to end, against a mock server.
- `OpenApiContractTest` — reads the committed `openapi/openapi.json` and fails if a field
  the phone parses has been renamed or removed. The DTOs are hand-written and partial (a
  generated client would pull every schema in the document into the APK), so this test is
  what keeps them honest. It has already caught one: `TransitionOption` is
  `{stage_key, manual, gates_met}`, not the `{key, allowed}` first guessed.

## What is deliberately not here

- **Creating or editing orders, partners and leads.** The phone reads and photographs, and
  changes stages. A 14-field order form on a gloved thumb is how bad data gets in.
- **Line items.** The number the whole project reports on should not be typed on a phone.
- **Reports.** A table that needs a wide screen.
- **Offline caching of orders.** Everything except the queue is read straight from the API.
  Caching would mean deciding what happens when the cached copy and the server disagree,
  which is a synchronisation problem nobody asked for. The picker is cached for 60 seconds
  by the server's own `Cache-Control`, which is enough to survive a walk across the yard.
- **A second language.** Hungarian only, like the web client.
