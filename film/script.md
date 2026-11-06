# AutoCRM: showcase film script

**Running time:** 6:00 · **Format:** 1920×1080, 30 fps, H.264 · **Language:** Hungarian (narration and on-screen text)
**Narration:** synthetic Hungarian male voice (OuteTTS, [tts/narrate.py](tts/narrate.py)), calm and warm, about 14 characters a second
**Music:** quiet bed under the narration, rising only at the opening and the end card (track to be supplied)

## The story

The film follows one van, a **Mercedes-Benz Sprinter 316, plate HPH-204**, from the moment its
owner fills in the enquiry form to the moment the finished cooling conversion is handed back.
Billing, email, HR and the overview come in as short side scenes, and the phone app gets about
two of the six minutes.

Autotherm is in **Szeged**. The customer is fictional: **Hópehely Fagylaltgyár Kft.**, an ice
cream maker that needs a refrigerated van, with **Szabó Gergely** as its contact; the office is
**Iroda Ilona**. No real customer appears.

## Look

The details of every move, the virtual camera and the cursor are in [motion.md](motion.md).

- **Web shots** sit in a plain browser frame on a dark background; **phone shots** in a phone
  frame, alone or beside the web when both show the same moment.
- Push-ins (about 115–140 %) onto the one element each line talks about; nothing moves when
  nothing needs to.
- **Section titles** are a small label at the top left (e.g. `03 · MEGRENDELÉS`), **captions**
  sit at the bottom: at most one short line, only for a number or a term worth reading.
- The cursor is visible with a soft ripple on clicks; on the phone, taps show as a circle.
- Cuts land on narration pauses; scenes change with a short push or crossfade.

## At a glance

| # | Time | Scene | Where |
|---|---|---|---|
| 0 | 0:00–0:20 | Nyitás | Photos, title |
| 1 | 0:20–0:50 | Az érdeklődés | Website, web, phone |
| 2 | 0:50–1:15 | Ajánlat | Web |
| 3 | 1:15–1:40 | Megrendelés | Web |
| 4 | 1:40–2:00 | A telefon | Phone |
| 5 | 2:00–2:50 | Átvétel | Phone, web |
| 6 | 2:50–3:10 | Fotózás | Phone, web |
| 7 | 3:10–3:45 | Gyártás | Web, phone |
| 8 | 3:45–4:05 | Udvar és kijelző | Web, phone, TV |
| 9 | 4:05–4:35 | Számlázás | Web, phone |
| 10 | 4:35–4:55 | Levelezés és marketing | Web |
| 11 | 4:55–5:10 | HR | Web, phone |
| 12 | 5:10–5:35 | Áttekintés | Web |
| 13 | 5:35–6:00 | Kiadás és zárás | Phone, web, title |

---

## 0 · Nyitás (0:00–0:20)

| Time | Picture | Source |
|---|---|---|
| 0:00 | Black. Fade into the black refrigerated Sprinter (cooling unit on the roof), front three-quarter, slow push-in | `autotherm-photos/car-2/IMG_20260511_092828.jpg` |
| 0:05 | Crossfade: a hearse conversion's gold-lit interior, slow pan | `autotherm-photos/car-1/IMG_20260818_082645.jpg` |
| 0:09 | Crossfade: the Sprinter's insulated load space with its strip curtain | `autotherm-photos/car-2/IMG_20260511_092940.jpg` |
| 0:13 | The photo darkens and blurs; **AutoCRM** builds in letter by letter, the subtitle fades in below | Title |

**Narráció**
> Minden átalakított autó mögött tucatnyi lépés áll: egy érdeklődés, egy ajánlat, fotók, aláírások, számlák. Az AutoCRM ezt mind egy helyen tartja.

**Felirat (title):** AutoCRM · *Az első érdeklődéstől a kiadásig*

---

## 1 · Az érdeklődés (0:20–0:50)

| Time | Picture | Source |
|---|---|---|
| 0:20 | Section label `01 · ÉRDEKLŐDÉS`. Autotherm website, enquiry form: Szabó Gergely, Hópehely Fagylaltgyár Kft., "Hűtős felépítmény Sprinterre, fagylalt szállításához" is typed in | Website (local), Playwright |
| 0:28 | *Küldés* is clicked, the confirmation appears | Website |
| 0:30 | Cut to the CRM: the bell gets a red **1**, the notification opens, then the new lead with a **Weboldal** source tag | Web |
| 0:38 | Phone beside the web: a system notification arrives, "Új érdeklődés a weboldalról"; a tap opens the lead on the phone | Phone |
| 0:44 | Push-in on *Lead források* (the lead sources report): a bar per channel (web, Facebook, Google) | Web |

**Narráció**
> Kedd reggel, Szeged. Egy fagylaltgyár hűtős furgont szeretne, és ajánlatot kér a weboldalon. Az érdeklődés egyenesen a rendszerbe érkezik: felkerül a leadek közé, az iroda levelet kap róla, és a telefonon is megjelenik. Azt is látjuk, honnan jött: a weboldalról, egy Facebook-hirdetésből vagy a Google-ből.

**Felirat:** Weboldal · Facebook · Instagram · Google Ads

---

## 2 · Ajánlat (0:50–1:15)

| Time | Picture | Source |
|---|---|---|
| 0:50 | `02 · AJÁNLAT`. Lead page; the stage chip moves *Új → Megkeresve*, a note is added | Web |
| 0:55 | Stage *Árajánlat*: value **6 850 000 Ft**, valid for 30 days | Web |
| 1:00 | The quotation letter is generated; the PDF opens and slides in over the page | Web |
| 1:05 | Push-in on the dashboard's *Lejáró árajánlatok (7 nap)* list | Web |
| 1:09 | Lead list: one lead set to *Elveszett* with a lost reason picked from the list | Web |

**Narráció**
> Felvesszük vele a kapcsolatot, és árajánlatot adunk: összeg, érvényesség, és kész a PDF ajánlatlevél. Ha az ügyfél nem jelentkezik, a rendszer szól, mielőtt az ajánlat lejár. Ha egy munka elmegy, rögzítjük, miért, így később is látszik, hol veszítünk.

**Felirat:** Ajánlatlevél PDF-ben · Emlékeztető lejárat előtt

---

## 3 · Megrendelés (1:15–1:40)

| Time | Picture | Source |
|---|---|---|
| 1:15 | `03 · MEGRENDELÉS`. Lead set to *Megnyert*; the conversion dialog opens | Web |
| 1:20 | The new order with the customer and contact carried over; project type *Hűtőfelépítmény* | Web |
| 1:27 | Push-in on the build spec: the cooling section of the order (unit, temperature range, insulation) | Web |
| 1:33 | The stage rail across the top: **Átvétel → Tervezés → Gyártás → MEO → Kész**, each stage lighting up in turn | Web, animated highlight |

**Narráció**
> Az ügyfél igent mond. A leadből egy kattintással megrendelés lesz, az ügyféllel és a kapcsolattartóval együtt. A hűtős munkához hűtős gyártási adatlap tartozik, és innentől a munka végigmegy a fázisokon, az átvételtől a kiadásig.

**Felirat:** Átvétel · Tervezés · Gyártás · MEO · Kész

---

## 4 · A telefon (1:40–2:00)

| Time | Picture | Source |
|---|---|---|
| 1:40 | `04 · A TELEFON`. Phone, centred. The AutoCRM icon is tapped; the fingerprint prompt, then the app opens on *Munkák* | Phone |
| 1:46 | The drawer slides open: *Munkák, Fotózás, Leadek, Névjegyzék, E-mailek, Bejövő számlák, Feladatok, Jelentések, HR, Sor* | Phone |
| 1:52 | Airplane mode on: the job list stays, with the banner saying how old the copy is | Phone |
| 1:57 | Quick switch light → dark → pure black theme | Phone |

**Narráció**
> A munka nagy része nem az irodában zajlik, hanem az udvaron és a műhelyben. Ezért az AutoCRM a telefonon is ott van: ujjlenyomattal nyílik, és térerő nélkül is mutatja, amit utoljára látott.

**Felirat:** Ujjlenyomat · Offline is olvasható · Világos és sötét mód

---

## 5 · Átvétel (2:00–2:50)

| Time | Picture | Source |
|---|---|---|
| 2:00 | `05 · ÁTVÉTEL`. Order HPH-204 on the phone → *Átvétel-átadás* → *Átvétel indítása*; the plate is checked | Phone |
| 2:07 | The photo checklist for a cooling van; the in-app camera shoots front, sides, back, load space and the odometer; each item gets a tick | Phone (real phone for the camera) |
| 2:20 | The car outline: the left rear door is tapped, type *Karc*, severity *Enyhe*; the description is dictated | Phone |
| 2:30 | Readings: km, fuel 1/2, battery, warning lights | Phone |
| 2:36 | Two signatures drawn with a finger: *Iroda Ilona*, then the driver | Phone |
| 2:43 | *Átvétel lezárva*. Cut to the web: the same inspection with its photos, the marked damage and both signatures | Web |

**Narráció**
> Megérkezik a Sprinter, és az átvételt a telefon vezeti végig. A szükséges fotók listáját a rendszer adja, autótípusonként, így egy szög sem marad ki. A képek a beépített kamerával, ott helyben készülnek. A sérülést az autó rajzán jelöljük, a leírást elég bemondani. Kilométer, üzemanyag, figyelmeztető lámpák, végül két aláírás a kijelzőn. Térerő nélkül sem vész el semmi: a telefon megőrzi, és amint lehet, feltölti.

**Felirat:** Fotólista autótípusonként · Sérülés a rajzon · Aláírás a kijelzőn

---

## 6 · Fotózás (2:50–3:10)

| Time | Picture | Source |
|---|---|---|
| 2:50 | `06 · FOTÓZÁS`. *Fotózás*: "HPH" typed, the van picked by plate | Phone |
| 2:55 | The phone's own camera app; three photos in a row, category *Gyártás* | Phone (real phone) |
| 3:00 | *Sor*: "3 vár feltöltésre" with no signal; wifi comes back, the three tick off | Phone |
| 3:05 | Web: the order's photos, filed by category | Web |

**Narráció**
> A szerelők a napi fotókat is a telefonnal küldik, rendszám alapján, teljes minőségben. Minden kép előbb a telefonra mentődik, és csak utána indul el, így egy gyenge wifi miatt sem tűnik el egy sem.

**Felirat:** Egy fotó sem vész el

---

## 7 · Gyártás (3:10–3:45)

| Time | Picture | Source |
|---|---|---|
| 3:10 | `07 · GYÁRTÁS`. Order page: stage *Tervezés → Gyártás*; a drawing is dropped into *Dokumentumok* | Web |
| 3:17 | The blocker on the order: *"Hűtőagregát a beszállítónál"*, with its due date (added on the phone; the web shows it read-only) | Web |
| 3:23 | Phone: *Hűtőagregát sorozatszáma* → the camera reads the barcode on the unit's plate, the number lands on the spec | Phone (real phone) |
| 3:30 | Phone: a short voice note is recorded on the job; on the web it plays from the documents | Phone, web |
| 3:36 | Web: the stage list shows *Kész* greyed out, "feltétel nem teljesül", until there is a completion photo | Web |

**Narráció**
> Gyártás közben minden a megrendelésen van: tervek, teendők, megjegyzések. Ha valami akadályozza a munkát, határidővel rögzítjük, és a rendszer szól, ha lejárt. Az agregát sorozatszámát a telefon olvassa le a vonalkódról, és hangjegyzetet is lehet hagyni. Késznek pedig csak az a munka jelölhető, amelyikről van kész fotó.

**Felirat:** Akadály határidővel · Vonalkód · Hangjegyzet

---

## 8 · Udvar és kijelző (3:45–4:05)

| Time | Picture | Source |
|---|---|---|
| 3:45 | `08 · UDVAR`. The yard map in 2D: buildings, workshop bays, parking spots, a card for each van | Web |
| 3:50 | The HPH-204 card is dragged to *4. állás*; switch to 3D, the camera orbits once | Web |
| 3:56 | Phone: the same move on the yard screen, "HPH-204 → 4. állás" | Phone |
| 4:00 | The workshop TV (*Kijelző*), full screen in a TV frame: *Készülőben* and *Átvehető* | Web (board page) |

**Narráció**
> Az udvar térképén látszik, melyik autó hol áll, 2D-ben és 3D-ben is. Áthelyezni annyi, mint a kártyát a helyére húzni, a gépen vagy a telefonon. A műhely tévéjén pedig mindig ott van, mi készül, és mi vihető már.

**Felirat:** 2D és 3D udvartérkép · Műhelykijelző

---

## 9 · Számlázás (4:05–4:35)

| Time | Picture | Source |
|---|---|---|
| 4:05 | `09 · SZÁMLÁZÁS`. From the order: *Számla*, payment by transfer; the preview, then issued | Web (NAV mock) |
| 4:12 | The NAV status chip turns to accepted; push-in | Web |
| 4:16 | *Számlázó* tabs flick by: to issue, issued, paid, archived, stornoed; then a proforma and the partner statement | Web |
| 4:22 | An Austrian partner: the EU VAT number check turns green, and only then is the 0 % invoice allowed | Web |
| 4:27 | Phone: *Bejövő számlák → Szkennelés*, two pages photographed, bound into one PDF, uploaded | Phone (real phone) |

**Narráció**
> A számla a megrendelésből készül, és egyenesen a NAV Online Számlába megy. Díjbekérő, sztornó, fizetési emlékeztető és folyószámla, mind egy helyen. Uniós partnernek nullaszázalékos számla csak érvényes adószám-ellenőrzés után mehet ki. A beszállítói számlát pedig elég lefényképezni, és egy PDF-ként érkezik meg.

**Felirat:** NAV Online Számla · Uniós adószám-ellenőrzés (VIES)

---

## 10 · Levelezés és marketing (4:35–4:55)

| Time | Picture | Source |
|---|---|---|
| 4:35 | `10 · LEVELEZÉS`. The order's correspondence: the quote letter that went out, and the customer's reply under it | Web |
| 4:40 | *Sablonok*: a letter written from a template | Web |
| 4:44 | *Hírlevél*: subscribers with tags, a send with opens and clicks | Web |
| 4:50 | *Értékesítés és marketing*: enquiry → quote → order, per website | Web |

**Narráció**
> Minden levél a rekord mellett marad, az ügyfél válaszai is. Sablonból írunk, az emlékeztetők maguktól mennek. A hírlevélnél látszik, ki nyitotta meg, és a jelentés megmutatja, melyik csatorna hoz valódi megrendelést.

**Felirat:** Ügyfélválaszok a rekordnál · Megnyitás és kattintás

---

## 11 · HR (4:55–5:10)

| Time | Picture | Source |
|---|---|---|
| 4:55 | `11 · HR`. Staff directory with photos; a profile with a document expiring in 30 days, flagged | Web |
| 5:00 | *Távollétek*: the team calendar, with leave and sick days | Web |
| 5:04 | The public job page with its application form; then the same leave list on the phone | Web, phone |

**Narráció**
> A csapat is itt van: munkatársak fotóval, figyelmeztetés a lejáró papírokra, szabadságok közös naptárban, és álláshirdetések saját jelentkezési oldallal.

**Felirat:** Lejáró dokumentumok · Szabadságnaptár · Álláshirdetés

---

## 12 · Áttekintés (5:10–5:35)

| Time | Picture | Source |
|---|---|---|
| 5:10 | `12 · ÁTTEKINTÉS`. The dashboard: *Teendőim, Átvehető, Munkában lévő autók, Figyelmet igényel*; push across the tiles | Web |
| 5:16 | "/" opens search; "HPH" finds the order, the partner and the letters at once | Web |
| 5:20 | The order's *Előzmények*, scrolling | Web |
| 5:24 | *Jelentések*: a chart in forints, then the Excel export | Web |
| 5:28 | *Asszisztens*: "Mely munkák állnak egy hete ugyanabban a fázisban?" and the answer appears | Web |

**Narráció**
> Reggel az irányítópult mutatja, mi a teendő, és mi akadt el. A kereső rendszámra és névre is talál, minden rekordnak megvan a teljes előzménye, a jelentések forintban számolnak, MNB-árfolyamon. És kérdezni is lehet: az asszisztens a cég saját gépén fut.

**Felirat:** Keresés · Előzmények · Jelentések · Helyi asszisztens

---

## 13 · Kiadás és zárás (5:35–6:00)

| Time | Picture | Source |
|---|---|---|
| 5:35 | `13 · KIADÁS`. Phone: *Kiadás indítása* on HPH-204; the scratch from the átvétel comes up as "már átvételkor is megvolt", confirmed with a tap | Phone |
| 5:42 | The customer signs; the web stage moves to **Kész**, the van appears under *Átvehető* on the workshop TV | Phone, web |
| 5:48 | Back to the real Sprinter, from the side; the picture darkens | `autotherm-photos/car-2/IMG_20260511_092854.jpg` |
| 5:51 | End card: **AutoCRM**, *Egy rendszer. Az első érdeklődéstől a kiadásig.* Music up, fade to black at 6:00 | Title |

**Narráció**
> Kiadáskor a telefon minden sérülést összevet az átvételkor rögzítettel, így azonnal kiderül, mi volt már ott. Az ügyfél aláír, a munka kész. AutoCRM: egy rendszer, az első érdeklődéstől a kiadásig.

**Felirat (end card):** AutoCRM · *Egy rendszer. Az első érdeklődéstől a kiadásig.*

---

## Before filming

- **Fresh data.** Reset and seed the local stack; the current database holds test leftovers.
  On top of the demo seed, a film-only seed adds: the hero lead from the website form, invoices
  through the NAV mock, a proforma and a payment, the correspondence with a customer reply, a
  newsletter send with opens and clicks, a yard layout, staff photos and an expiring document,
  and real van photos from `autotherm-photos` on the orders.
- **Services:** the NAV mock (`--profile nav`) and the assistant model (`--profile ai`) running;
  automatic mail switched on in Beállítások, sending to Mailpit only.
- **Font:** IBM Plex Sans (the app's own) installed for the titles and captions.
- **Phone:** a real phone over USB for the camera shots (scenes 5, 6, 7, 9); the emulator is
  enough for everything else. Clean status bar for the recording (full battery, fixed time).
- **Timing:** the narration is about 3 400 characters, so about 4:00 of speech in a 6:00
  film (each scene is 50–75 % narrated). The gaps are where the picture carries the scene. Timecodes get locked once the voice
  is generated, with each shot cut to its line.
