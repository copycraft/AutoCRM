package hu.autotherm.autocrm.ui.common

import hu.autotherm.autocrm.data.api.ApiException
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
    is ApiException.Rule -> e.detail ?: e.code
    is ApiException.Server -> "Szerverhiba (${e.status}): ${e.detail ?: "nincs részlet"}"
}
