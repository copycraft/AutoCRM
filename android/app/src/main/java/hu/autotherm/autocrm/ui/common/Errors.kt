package hu.autotherm.autocrm.ui.common

import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.inspection.ServerErrorTexts
import kotlinx.coroutines.CancellationException

/**
 * Every failure a screen can show, in words.
 *
 * The `Throwable` overload exists because a screen that crashes takes the whole app with
 * it, and on a shop floor that is indistinguishable from "the app is broken". Catching only
 * the exceptions we predicted meant one unforeseen type — a parse failure, a bad cast, a
 * null we did not expect — killed the process on a tab tap.
 *
 * An unrecognised throwable is shown with its class name on purpose. It is not pretty, but
 * "IllegalStateException: …" on screen is something a user can read out down a phone line,
 * and that is worth more than a tidy "Hiba történt" that tells nobody anything.
 */
fun describeError(e: Throwable): String = when (e) {
    is ApiException -> describeApi(e)
    // Not a failure: the screen went away mid-request. Never shown.
    is CancellationException -> throw e
    else -> "${e::class.simpleName}: ${e.message ?: "ismeretlen hiba"}"
}

private fun describeApi(e: ApiException): String = when (e) {
    is ApiException.Network -> "Nincs kapcsolat a szerverrel."
    is ApiException.Unauthenticated -> "A munkamenet lejárt. Jelentkezz be újra."
    is ApiException.Forbidden -> "Ehhez nincs jogosultságod."
    is ApiException.NotFound -> "Nem található."
    // Validation carries the field-level reason in its detail; every other code reads
    // from the shared catalog so the phone and the web say the same thing. Fresh
    // server texts (see ServerErrorTexts) win over the built-in map below.
    is ApiException.Rule ->
        if (e.code == "validation") e.detail ?: ERROR_TEXT.getValue("validation")
        else ServerErrorTexts.texts[e.code] ?: ERROR_TEXT[e.code] ?: e.detail ?: e.code
    is ApiException.Server -> "Szerverhiba (${e.status}): ${e.detail ?: "nincs részlet"}"
}



/**
 * The backend error-code catalog (`backend/src/error.rs::ERROR_CODES`, published in
 * `openapi.json` as `ErrorCode` with `x-error-catalog`), Hungarian text only.
 * `ErrorCatalogTest` fails when a code is missing here or its text drifts.
 */
internal val ERROR_TEXT: Map<String, String> = mapOf(
    "unauthenticated" to "Nincs bejelentkezve. Jelentkezzen be újra.",
    "forbidden" to "Nincs jogosultsága ehhez a művelethez.",
    "not_found" to "A kért adat nem található.",
    "validation" to "Érvényesítési hiba. Ellenőrizze a megadott adatokat.",
    "too_many_requests" to "Túl sok próbálkozás. Próbálja újra később.",
    "duplicate" to "Ilyen adat már létezik.",
    "overlap" to "A munkatársnak ebben az időszakban már van rögzített távolléte.",
    "domain_taken" to "Ez a domain már egy másik címkéhez tartozik.",
    "assistant_unavailable" to "Az asszisztens most nem érhető el (a nyelvi modell nem fut vagy nincs beállítva).",
    "template_in_use" to "Ezt a sablont automatikus levél vagy utánkövetés használja, ezért nem tehető a lomtárba.",
    "status_required" to "Erre az állapotra kerülnek az új vagy a távozó munkatársak, ezért nem archiválható; nevezze át inkább.",
    "invalid_reference" to "Érvénytelen hivatkozás, vagy a hivatkozott adat még használatban van.",
    "constraint_violation" to "Az adat sérti az adatbázis szabályait.",
    "immutable" to "Ez bizonyítási célból védett adat, nem módosítható és nem törölhető.",
    "internal" to "Szerverhiba. Próbálja újra később.",
    "password_change_required" to "Első bejelentkezéskor kötelező jelszót változtatni.",
    "wrong_password" to "Hibás a jelenlegi jelszó.",
    "totp_required" to "Adja meg a hitelesítő alkalmazás hatjegyű kódját is.",
    "totp_invalid" to "Hibás vagy már felhasznált egyszer használatos kód.",
    "last_admin" to "Az utolsó aktív adminisztrátor nem tiltható le.",
    "stage_gate" to "A megrendelés nem léphet tovább, mert egy szükséges feltétel még nem teljesült.",
    "note_required" to "A visszalépéshez indoklás szükséges.",
    "invalid_transition" to "Ez a fázisváltás nem engedélyezett.",
    "intake_slip_missing" to "Az átvétel lezárásához előbb rögzítsd az átvételi lapot (km-óra).",
    "use_conversion" to "A megnyert fázishoz használja az átalakítás funkciót.",
    "lead_converted" to "Ez a lead már megrendeléssé lett alakítva.",
    "stage_required" to "A megnyert lead-fázist az átalakítás használja, nem kapcsolható ki.",
    "currency_locked" to "A pénznem nem módosítható, amíg a megrendelésnek vannak tételei.",
    "already_resolved" to "Ez az akadály már meg van oldva.",
    "not_resolved" to "Ez az akadály nincs megoldva, ezért nem nyitható újra.",
    "upload_missing" to "A feltöltés nem található vagy lejárt. Kezdje újra.",
    "upload_mismatch" to "A feltöltött fájl nem egyezik a bejelentettel.",
    "invalid_ticket" to "Érvénytelen vagy lejárt feltöltési jegy.",
    "already_attached" to "Ez a fotó már csatolva van.",
    "checkout_open" to "Már van nyitott átvételi jegyzőkönyv. Előbb írassa alá vagy dobja el.",
    "checkout_required" to "A kiadáshoz előbb aláírt átvételi jegyzőkönyv kell.",
    "locked" to "Az aláírt jegyzőkönyv már nem módosítható; utólagos megjegyzést lehet hozzáfűzni.",
    "signatures_required" to "A lezáráshoz mindkét aláírás szükséges.",
    "verdicts_pending" to "Előbb minden új sérülésnél dönteni kell.",
    "not_cancellable" to "Ez az e-mail már nem vonható vissza.",
    "not_retryable" to "Ez a művelet nem próbálható újra.",
    "not_failed" to "Csak véglegesen elakadt feladat indítható újra.",
    "invoicing_not_configured" to "A számlázás nincs beállítva ezen a szerveren. Szóljon a rendszergazdának.",
    "invoice_exists" to "Ehhez a megrendeléshez már tartozik élő számla. Előbb sztornózza.",
    "invoice_in_flight" to "A számla bejelentése még folyamatban van a NAV felé. Várja meg a választ.",
    "invoice_data_missing" to "A partnernél hiányzik egy számlázási adat. Pótolja, majd próbálja újra.",
    "fx_rate_missing" to "Nincs árfolyam a teljesítés napjára. Próbálja újra az árfolyam letöltése után.",
    "not_stornoable" to "Csak kiállított számla sztornózható.",
    "not_annullable" to "Csak bejelentett számla érvényteleníthető technikailag.",
    "no_items" to "A számlához legalább egy tétel kell a megrendelésen.",
    "nav_rejected" to "A NAV elutasította a számlát. A részleteket a számla adatlapja mutatja.",
    "nav_unreachable" to "A számlázó szolgáltatás nem érhető el. A megrendelés nem sérült; próbálja újra később.",
    "overpayment" to "A befizetés több, mint a számlán még nyitott összeg.",
    "not_issued" to "Csak kiállított számlának van letölthető bizonylata.",
    "pdf_unavailable" to "A PDF most nem tölthető le. Próbálja újra később.",
)
