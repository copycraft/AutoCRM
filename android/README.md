# AutoCRM Android

The shop-floor client. Kotlin, Jetpack Compose, one Gradle module.

Most of the day-to-day work the browser client does, from a phone: jobs and their stages,
leads, partners, tasks, the correspondence log, HR for those who have it. Plus the two
things the browser cannot do: get photos onto a job **without depending on there being a
signal at that moment**, and walk around a vehicle at handover recording its condition.

## What it does

The drawer, in order. Entries a user may not use are not shown (the server refuses them
anyway).

| Screen | What it is for |
|---|---|
| **Kiszolgáló** (first run only) | Which AutoCRM this phone talks to. Stored on the device. |
| **Keresés** | Search across orders, partners, leads, contacts, emails and (HR access) staff. |
| **Munkák** | Order list and detail: value, line items, vehicle, build spec, intake slip, blockers (add, resolve, reopen), tasks, stage history and changes, MiniCRM notes, photos, handover inspections. New orders and editing. |
| **Fotózás** | Capture first: pick the van by plate, then shoot. For a fitter photographing six vans in a row. |
| **Leadek** | Leads with their quotation, source and stage history; stage changes, conversion to an order, new leads and editing. |
| **Névjegyzék** | Partners with the customer/supplier filter, contacts (add, edit), their orders and leads. New partners and editing. |
| **E-mailek** | The correspondence log, and a short manual letter about a record. |
| **Feladatok** | My tasks, open first. |
| **Jelentések** | Workshop load over the last 30 days and the stalled-jobs list. |
| **Értesítések** | The notification feed (new website leads and the like). |
| **HR** (HR access) | Staff directory with photos: add, edit, archive. Távollétek: leave and absence for the team. |
| **Felhasználók** (admin) | Every user with role, active and HR access. Creating accounts and resetting passwords stay on the web. |
| **Sor** | The upload queue: what is waiting, what failed and why, retry or discard. |
| **Beállítások** | Appearance: light, dark, or pure black. |

A user who must change their password does that first, on the phone.

## The upload queue

This is the part worth reading the code for. Photos are the one thing the phone produces
that exists nowhere else, so losing one is not an option:

1. The shutter writes a JPEG into app-private storage, hashes it, and inserts a row in
   `pending_uploads` **before the shutter is live again**. Nothing waits on a network.
2. `UploadWorker` (WorkManager, network-constrained) drains the queue: ask for a ticket,
   PUT the bytes straight to object storage, hand the ticket back to be confirmed.
3. A network failure backs off (10 s, 1 m, 5 m, 30 m, 2 h) and stays queued.
   A 422 is marked `blocked` and shown on the queue screen with the server's reason.
4. A file is deleted only after the server has confirmed the row, or when the fitter taps
   *Eldobás*. Nothing else deletes a photo, ever.

Retrying a whole batch is free: the server answers `already_uploaded` for bytes it has
seen, and `(order_id, content_hash)` is unique on both sides.

`UploadQueueTest` covers this end to end: JVM tests against an in-memory Room database and
a MockWebServer, no device needed.

## Photos

Job photos are taken with the **phone's own camera app**, not one built into this one. An
in-app preview-and-shutter does not match what the manufacturer's camera produces (its
processing, its stabilisation, its full sensor resolution), and MEO photos are evidence of
a vehicle's condition. Losing quality to save a screen transition is the wrong trade.

Photos are added from inside a job (or after picking it on Fotózás), which is the right
way round: a photo belongs to a vehicle, and picking the job afterwards is how photos end
up on the wrong one. Two doors, both returning a full-quality original:

- **Kamera** hands the system camera app a file inside the queue's own directory.
- **Galéria** uses the system photo picker: no storage permission at all, and the user
  exposes only the items they choose.

The category is sticky between visits. The one deliberate interruption: choosing **Bevétel**
(intake) raises a banner once an hour, because intake photos can never be deleted or
re-filed (the database refuses both with a trigger), so silence there would be dangerous
rather than efficient.

## Handover inspections

The átvétel (vehicle arriving, API `checkout`) and kiadás (vehicle leaving, API `checkin`)
walkarounds are created only on the phone; the web shows them and records verdicts.

- **The photo list comes from the server**, per vehicle kind (the order's project type) and
  walkaround, as edited in the web settings. The phone downloads the lists when the
  Átvétel-átadás screen opens and keeps the last one of each for yards with no signal. With
  nothing downloaded it refuses to start rather than guess. A draft freezes its list at the
  start, so editing a list never changes a walkaround in progress.
- **Inspection photos use an in-app camera** (CameraX): live view only, no gallery, so every
  photo in an inspection was taken there and then. This is the exception to the rule above,
  on purpose: here proving when a photo was taken matters more than the camera app's
  processing.
- Damages are marked on a car outline and classified by type and severity; readings
  (odometer, fuel, battery, warning lights) and finger-drawn signatures of the inspector and
  the customer complete it.
- **Offline first.** The whole draft lives in Room (`inspection_drafts`) as one JSON
  payload while the photo bytes ride the normal upload queue. `InspectionSyncWorker` then
  creates the inspection, attaches damages, photos, signatures and verdicts, and signs it.
  Every step is idempotent (the create carries the draft's UUID as `client_key`), so a dead
  battery mid-sync resumes cleanly. A draft is never dropped silently: a failed sync stays
  visible with its error.
- **At kiadás**, every damage is compared with the átvétel by zone: same zone and type
  suggests "pre-existing", and the inspector confirms, re-links or dismisses each one.

## Offline reads

Every read the phone makes is kept as the last good copy (`data/cache/ResponseCache.kt`) and
shown when the server cannot be reached, with a banner saying how old it is. The cache is a
separate, disposable database: the photo queue and the inspection drafts hold work that
exists nowhere else, while this only holds copies of what the server already has. It is per
user, capped, and cleared on sign-out.

Writes are never faked: editing needs a connection; photos and inspections queue. Not
cached on purpose: the staff directory and user list, notifications, search, and anything
with an expiring link (photo and document URLs).

The server's lookup lists (damage types, categories, error texts…) are cached too
(`LookupsCache`), so the app starts and speaks the server's words with no signal.

## Notifications

`NotificationPollWorker` asks the server what is new about every 15 minutes, the shortest
interval Android allows for periodic work, and shows a system notification for anything
the phone has not shown yet; a tap opens the record. This is polling, not push: a new lead
can take up to a quarter of an hour to reach the phone. Instant delivery needs Firebase Cloud
Messaging and a Firebase project; the feed and the cursor are what it would plug into.

## Which server the app talks to

**Chosen on the phone, not baked into the build.** On first run the app asks for an address
before anything else, checks it against `/health`, and stores it on the device. The login
screen shows the current address with a *Kiszolgáló módosítása* button next to it, because
the moment someone discovers the address is wrong is when a login will not go through, and
a stale address looks exactly like a wrong password otherwise.

The address survives sign-out: it belongs to the phone, not to the person holding it.

`autocrm.apiUrl` (or `AUTOCRM_API_URL`) only sets what that screen **prefills** in release
builds; debug builds prefill `http://10.0.2.2:8080`, the emulator's view of the host machine.
Typing `192.168.1.10:8080` is enough: the scheme is added, default ports and any pasted path
are stripped, so two spellings of one server end up as one stored string
(`ServerStoreTest`). Cleartext http is permitted, and the setup screen says when it costs
something.

## Building

Requires JDK 17 and an Android SDK with platform 35 and build-tools 35.0.0. If `java` is not
on the PATH, point `JAVA_HOME` at a JDK (the root `build.sh` falls back to
`~/android-tools/jdk`).

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

**Release builds** come from the root `build.sh`, which writes a signed APK to
`build/android/autocrm.apk`. The first run generates `android/autocrm-release.keystore` and
`android/keystore.properties` (both git-ignored). **Back them up**: Android only installs an
update signed with the same key, so losing the keystore means uninstalling the app on every
phone. Without `keystore.properties` a release build is unsigned.

## Testing

```bash
./gradlew testDebugUnitTest
```

JVM tests only, no device. The ones worth knowing:

- `UploadQueueTest`: the photo queue, end to end, against a mock server.
- `OpenApiContractTest`: reads the committed `openapi/openapi.json` and fails if a field the
  phone parses has been renamed or removed. The DTOs are hand-written and partial (a
  generated client would pull every schema in the document into the APK), so this test is
  what keeps them honest.
- `ErrorCatalogTest`: the phone's error texts match the server's catalog. After a new error
  code, regenerate them from the repository root with
  `node android/scripts/gen-error-text.js .`
- `ServerStoreTest`: what a thumb types becomes a usable base URL, or is refused.
- `ResponseCacheTest`, `InspectionLogicTest`, `ZoneListsTest`, `NotificationPlannerTest`,
  `SessionExpiryTest` and the DTO tests cover offline reads, walkarounds, notifications,
  sign-out and the request bodies the server is picky about (an emptied field is an
  explicit `null`).

`scripts/smoke.sh` drives the debug APK on a running emulator against a local backend and
screenshots each screen: the build passing proves the code compiles, not that a fitter can
see anything.

## What is deliberately not here

- **Billing.** Invoices, proformas and incoming invoices are desk work.
- **Rich email.** Templates, quotation letters, attachments and newsletters belong on a
  desktop; the phone writes a short letter about a record.
- **Configuration.** Settings, templates, zone lists, stages and user accounts are edited
  on the web.
- **Most of HR.** Personal data, papers, statuses and recruitment stay on the web; the
  phone has the directory and absences.
- **Full reports.** A table that needs a wide screen; the phone has the totals.
- **Push.** See Notifications above.
- **A second language.** Hungarian only, like the web client.
