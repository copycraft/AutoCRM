# Error codes

Every non-2xx response is `{"error": {"code", "message"}}`. `code` is the machine
interface; `message` is English detail for logs and for `validation`, where it names the
field. Users see the Hungarian text below, never `message` (except for `validation`).

## Where the list lives

- **Source of truth:** `ERROR_CODES` in `backend/src/error.rs`.
  `backend/tests/error_codes.rs` fails if a `rule("…")`/`conflict("…")` code is missing
  from it or an entry is sent by nothing.
- **Published:** `openapi/openapi.json` → `components.schemas.ErrorCode` (enum) with the
  full table under `x-error-catalog` (status, retryable, hu, en).
- **Web:** `frontend/src/messages/hu.json` → `errors.<camelCaseCode>`;
  `frontend/src/test/error-codes.test.ts` fails when a code has no text.
- **Android:** `ERROR_TEXT` in `ui/common/Errors.kt`, regenerated with
  `node android/scripts/gen-error-text.js .`; `ErrorCatalogTest` fails on a missing or
  drifted entry.

## Adding a code

1. Add the entry to `ERROR_CODES` (the backend test tells you if you forget).
2. `cargo run --bin autocrm -- openapi --out ../openapi/openapi.json`, then `npm run gen:api`.
3. Add `errors.<camelCaseCode>` to `hu.json`; run the Android generator.
4. Add the row below.

Unknown codes (an older client against a newer server) render the backend `message` rather
than nothing — `ErrorDetail.code` stays a plain string on the wire for that reason.

Status mapping on Android: 400/409/422 → `ApiException.Rule` (rendered from this table),
401/403/404 → their own types, anything else (429, 5xx) → `Server`, which the upload queue retries.

## The catalog

| Code | HTTP | Retryable | Text (hu) |
|---|---|---|---|
| `unauthenticated` | 401 | no | Nincs bejelentkezve. Jelentkezzen be újra. |
| `forbidden` | 403 | no | Nincs jogosultsága ehhez a művelethez. |
| `not_found` | 404 | no | A kért adat nem található. |
| `validation` | 400 | no | Érvényesítési hiba. Ellenőrizze a megadott adatokat. |
| `too_many_requests` | 429 | yes | Túl sok próbálkozás. Próbálja újra később. |
| `duplicate` | 409 | no | Ilyen adat már létezik. |
| `invalid_reference` | 422 | no | Érvénytelen hivatkozás, vagy a hivatkozott adat még használatban van. |
| `constraint_violation` | 422 | no | Az adat sérti az adatbázis szabályait. |
| `immutable` | 409 | no | Ez bizonyítási célból védett adat, nem módosítható és nem törölhető. |
| `internal` | 500 | yes | Szerverhiba. Próbálja újra később. |
| `password_change_required` | 422 | no | Első bejelentkezéskor kötelező jelszót változtatni. |
| `wrong_password` | 422 | no | Hibás a jelenlegi jelszó. |
| `last_admin` | 422 | no | Az utolsó aktív adminisztrátor nem tiltható le. |
| `stage_gate` | 422 | no | A megrendelés nem léphet tovább, mert egy szükséges feltétel még nem teljesült. |
| `note_required` | 422 | no | A visszalépéshez indoklás szükséges. |
| `invalid_transition` | 422 | no | Ez a fázisváltás nem engedélyezett. |
| `intake_slip_missing` | 422 | no | Az átvétel lezárásához előbb rögzítsd az átvételi lapot (km-óra). |
| `use_conversion` | 422 | no | A megnyert fázishoz használja az átalakítás funkciót. |
| `lead_converted` | 422 | no | Ez a lead már megrendeléssé lett alakítva. |
| `stage_required` | 422 | no | A megnyert lead-fázist az átalakítás használja, nem kapcsolható ki. |
| `currency_locked` | 422 | no | A pénznem nem módosítható, amíg a megrendelésnek vannak tételei. |
| `already_resolved` | 409 | no | Ez az akadály már meg van oldva. |
| `not_resolved` | 409 | no | Ez az akadály nincs megoldva, ezért nem nyitható újra. |
| `upload_missing` | 422 | no | A feltöltés nem található vagy lejárt. Kezdje újra. |
| `upload_mismatch` | 422 | no | A feltöltött fájl nem egyezik a bejelentettel. |
| `invalid_ticket` | 422 | no | Érvénytelen vagy lejárt feltöltési jegy. |
| `already_attached` | 422 | no | Ez a fotó már csatolva van. |
| `checkout_open` | 422 | no | Már van nyitott kiadási jegyzőkönyv. Előbb írassa alá vagy dobja el. |
| `checkout_required` | 422 | no | A visszavételhez előbb aláírt kiadási jegyzőkönyv kell. |
| `locked` | 422 | no | Az aláírt jegyzőkönyv már nem módosítható; utólagos megjegyzést lehet hozzáfűzni. |
| `signatures_required` | 422 | no | A lezáráshoz mindkét aláírás szükséges. |
| `verdicts_pending` | 422 | no | Előbb minden új sérülésnél dönteni kell. |
| `not_cancellable` | 409 | no | Ez az e-mail már nem vonható vissza. |
| `not_retryable` | 409 | no | Ez a művelet nem próbálható újra. |
| `not_failed` | 409 | no | Csak véglegesen elakadt feladat indítható újra. |
| `invoicing_not_configured` | 422 | no | A számlázás nincs beállítva ezen a szerveren. Szóljon a rendszergazdának. |
| `invoice_exists` | 409 | no | Ehhez a megrendeléshez már tartozik élő számla. Előbb sztornózza. |
| `invoice_in_flight` | 409 | yes | A számla bejelentése még folyamatban van a NAV felé. Várja meg a választ. |
| `invoice_data_missing` | 422 | no | A partnernél hiányzik egy számlázási adat. Pótolja, majd próbálja újra. |
| `fx_rate_missing` | 422 | yes | Nincs árfolyam a teljesítés napjára. Próbálja újra az árfolyam letöltése után. |
| `not_stornoable` | 422 | no | Csak kiállított számla sztornózható. |
| `not_annullable` | 422 | no | Csak bejelentett számla érvényteleníthető technikailag. |
| `no_items` | 422 | no | A számlához legalább egy tétel kell a megrendelésen. |
| `nav_rejected` | 422 | no | A NAV elutasította a számlát. A részleteket a számla adatlapja mutatja. |
| `nav_unreachable` | 422 | yes | A számlázó szolgáltatás nem érhető el. A megrendelés nem sérült; próbálja újra később. |
| `not_issued` | 422 | no | Csak kiállított számlának van letölthető bizonylata. |
| `pdf_unavailable` | 422 | yes | A PDF most nem tölthető le. Próbálja újra később. |
