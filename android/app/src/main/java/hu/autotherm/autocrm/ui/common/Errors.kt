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

/**
 * Why the server could not be reached, in terms of what to do next: wait for signal,
 * wait for a restart, or check the address. "No connection" for all of them sent people
 * hunting for Wi-Fi while the server was restarting.
 */
internal fun describeNetwork(cause: Throwable?, phoneOnline: Boolean): String = when {
    !phoneOnline -> "Nincs hálózat a telefonon. Amint lesz, próbáld újra."
    cause is java.net.SocketTimeoutException ->
        "A szerver nem válaszolt időben (gyenge jel vagy lassú kapcsolat). Próbáld újra."
    cause is java.net.UnknownHostException ->
        "A szerver címe nem található. Van internet? Ha igen, ellenőrizd a szerver címét a beállításokban."
    cause is java.net.ConnectException ->
        "A szerver nem fogad kapcsolatot — lehet, hogy épp újraindul. Próbáld újra egy perc múlva."
    cause is javax.net.ssl.SSLException ->
        "Nem jött létre biztonságos kapcsolat a szerverrel (tanúsítvány vagy hálózati szűrő)."
    else -> "Nincs kapcsolat a szerverrel. Próbáld újra."
}

/** A 5xx in words; the status stays at the end for whoever takes the support call. */
internal fun describeServer(status: Int, detail: String?): String = when {
    detail?.startsWith("unparseable response") == true ->
        "A szerver váratlan választ adott. Frissítsd az alkalmazást; ha marad, szólj az irodának. ($status)"
    status == 502 || status == 503 || status == 504 ->
        "A szerver most nem érhető el (frissítés vagy újraindulás). Próbáld újra egy perc múlva. ($status)"
    else -> "Hiba történt a szerveren. Próbáld újra; ha ismétlődik, szólj az irodának. ($status)"
}

private fun describeApi(e: ApiException): String = when (e) {
    is ApiException.Network ->
        describeNetwork(e.cause, hu.autotherm.autocrm.data.net.NetworkState.online.value)
    is ApiException.Unauthenticated -> "A munkamenet lejárt. Jelentkezz be újra."
    is ApiException.Forbidden -> "Ehhez nincs jogosultságod."
    is ApiException.NotFound -> "Nem található."
    // Validation carries the field-level reason in its detail; every other code reads
    // from the shared catalog so the phone and the web say the same thing. Fresh
    // server texts (see ServerErrorTexts) win over the built-in map below.
    is ApiException.Rule ->
        if (e.code == "validation") e.detail?.let(::huValidation) ?: ERROR_TEXT.getValue("validation")
        else ServerErrorTexts.texts[e.code] ?: ERROR_TEXT[e.code] ?: e.detail ?: e.code
    is ApiException.Server -> describeServer(e.status, e.detail)
}





/**
 * The backend error-code catalog (`backend/src/error.rs::ERROR_CODES`, published in
 * `openapi.json` as `ErrorCode` with `x-error-catalog`), Hungarian text only.
 * `ErrorCatalogTest` fails when a code is missing here or its text drifts.
 */
internal val ERROR_TEXT: Map<String, String> = mapOf(
    "unauthenticated" to "Nincs bejelentkezve. Jelentkezzen be ├║jra.",
    "forbidden" to "Nincs jogosults├íga ehhez a m┼▒velethez.",
    "not_found" to "A k├ęrt adat nem tal├ílhat├│.",
    "validation" to "├ërv├ęnyes├şt├ęsi hiba. Ellen┼Ĺrizze a megadott adatokat.",
    "too_many_requests" to "T├║l sok pr├│b├ílkoz├ís. Pr├│b├ílja ├║jra k├ęs┼Ĺbb.",
    "duplicate" to "Ilyen adat m├ír l├ętezik.",
    "overlap" to "A munkat├írsnak ebben az id┼Ĺszakban m├ír van r├Âgz├ştett t├ívoll├ęte.",
    "domain_taken" to "Ez a domain m├ír egy m├ísik c├şmk├ęhez tartozik.",
    "assistant_unavailable" to "Az asszisztens most nem ├ęrhet┼Ĺ el (a nyelvi modell nem fut vagy nincs be├íll├ştva).",
    "template_in_use" to "Ezt a sablont automatikus lev├ęl vagy ut├ínk├Âvet├ęs haszn├ílja, ez├ęrt nem tehet┼Ĺ a lomt├írba.",
    "status_required" to "Erre az ├íllapotra ker├╝lnek az ├║j vagy a t├ívoz├│ munkat├írsak, ez├ęrt nem archiv├ílhat├│; nevezze ├ít ink├íbb.",
    "invalid_reference" to "├ërv├ęnytelen hivatkoz├ís, vagy a hivatkozott adat m├ęg haszn├ílatban van.",
    "constraint_violation" to "Az adat s├ęrti az adatb├ízis szab├ílyait.",
    "immutable" to "Ez bizony├şt├ísi c├ęlb├│l v├ędett adat, nem m├│dos├şthat├│ ├ęs nem t├Âr├Âlhet┼Ĺ.",
    "internal" to "Szerverhiba. Pr├│b├ílja ├║jra k├ęs┼Ĺbb.",
    "password_change_required" to "Els┼Ĺ bejelentkez├ęskor k├Âtelez┼Ĺ jelsz├│t v├íltoztatni.",
    "wrong_password" to "Hib├ís a jelenlegi jelsz├│.",
    "totp_required" to "Adja meg a hiteles├şt┼Ĺ alkalmaz├ís hatjegy┼▒ k├│dj├ít is.",
    "totp_invalid" to "Hib├ís vagy m├ír felhaszn├ílt egyszer haszn├ílatos k├│d.",
    "last_admin" to "Az utols├│ akt├şv adminisztr├ítor nem tilthat├│ le.",
    "stage_gate" to "A megrendel├ęs nem l├ęphet tov├íbb, mert egy sz├╝ks├ęges felt├ętel m├ęg nem teljes├╝lt.",
    "note_required" to "A visszal├ęp├ęshez indokl├ís sz├╝ks├ęges.",
    "invalid_transition" to "Ez a f├ízisv├ílt├ís nem enged├ęlyezett.",
    "intake_slip_missing" to "Az ├ítv├ętel lez├ír├ís├íhoz el┼Ĺbb r├Âgz├ştsd az ├ítv├ęteli lapot (km-├│ra).",
    "use_conversion" to "A megnyert f├ízishoz haszn├ílja az ├ítalak├şt├ís funkci├│t.",
    "lead_converted" to "Ez a lead m├ír megrendel├ęss├ę lett alak├ştva.",
    "stage_required" to "A megnyert lead-f├ízist az ├ítalak├şt├ís haszn├ílja, nem kapcsolhat├│ ki.",
    "currency_locked" to "A p├ęnznem nem m├│dos├şthat├│, am├şg a megrendel├ęsnek vannak t├ętelei.",
    "already_resolved" to "Ez az akad├íly m├ír meg van oldva.",
    "not_resolved" to "Ez az akad├íly nincs megoldva, ez├ęrt nem nyithat├│ ├║jra.",
    "upload_missing" to "A felt├Âlt├ęs nem tal├ílhat├│ vagy lej├írt. Kezdje ├║jra.",
    "upload_mismatch" to "A felt├Âlt├Âtt f├íjl nem egyezik a bejelentettel.",
    "invalid_ticket" to "├ërv├ęnytelen vagy lej├írt felt├Âlt├ęsi jegy.",
    "already_attached" to "Ez a fot├│ m├ír csatolva van.",
    "checkout_open" to "M├ír van nyitott ├ítv├ęteli jegyz┼Ĺk├Ânyv. El┼Ĺbb ├şrassa al├í vagy dobja el.",
    "checkout_required" to "A kiad├íshoz el┼Ĺbb al├í├şrt ├ítv├ęteli jegyz┼Ĺk├Ânyv kell.",
    "locked" to "Az al├í├şrt jegyz┼Ĺk├Ânyv m├ír nem m├│dos├şthat├│; ut├│lagos megjegyz├ęst lehet hozz├íf┼▒zni.",
    "signatures_required" to "A lez├ír├íshoz mindk├ęt al├í├şr├ís sz├╝ks├ęges.",
    "verdicts_pending" to "El┼Ĺbb minden ├║j s├ęr├╝l├ęsn├ęl d├Ânteni kell.",
    "not_cancellable" to "Ez az e-mail m├ír nem vonhat├│ vissza.",
    "not_retryable" to "Ez a m┼▒velet nem pr├│b├ílhat├│ ├║jra.",
    "not_failed" to "Csak v├ęglegesen elakadt feladat ind├şthat├│ ├║jra.",
    "invoicing_not_configured" to "A sz├íml├íz├ís nincs be├íll├ştva ezen a szerveren. Sz├│ljon a rendszergazd├ínak.",
    "invoice_exists" to "Ehhez a megrendel├ęshez m├ír tartozik ├ęl┼Ĺ sz├ímla. El┼Ĺbb sztorn├│zza.",
    "invoice_in_flight" to "A sz├ímla bejelent├ęse m├ęg folyamatban van a NAV fel├ę. V├írja meg a v├ílaszt.",
    "invoice_data_missing" to "A partnern├ęl hi├ínyzik egy sz├íml├íz├ísi adat. P├│tolja, majd pr├│b├ílja ├║jra.",
    "fx_rate_missing" to "Nincs ├írfolyam a teljes├şt├ęs napj├íra. Pr├│b├ílja ├║jra az ├írfolyam let├Âlt├ęse ut├ín.",
    "not_stornoable" to "Csak ki├íll├ştott sz├ímla sztorn├│zhat├│.",
    "not_annullable" to "Csak bejelentett sz├ímla ├ęrv├ęnytelen├şthet┼Ĺ technikailag.",
    "no_items" to "A sz├íml├íhoz legal├íbb egy t├ętel kell a megrendel├ęsen.",
    "nav_rejected" to "A NAV elutas├ştotta a sz├íml├ít. A r├ęszleteket a sz├ímla adatlapja mutatja.",
    "nav_unreachable" to "A sz├íml├íz├│ szolg├íltat├ís nem ├ęrhet┼Ĺ el. A megrendel├ęs nem s├ęr├╝lt; pr├│b├ílja ├║jra k├ęs┼Ĺbb.",
    "overpayment" to "A befizet├ęs t├Âbb, mint a sz├íml├ín m├ęg nyitott ├Âsszeg.",
    "not_issued" to "Csak ki├íll├ştott sz├íml├ínak van let├Âlthet┼Ĺ bizonylata.",
    "pdf_unavailable" to "A PDF most nem t├Âlthet┼Ĺ le. Pr├│b├ílja ├║jra k├ęs┼Ĺbb.",
)
