package hu.autotherm.autocrm.data.api

import java.io.IOException

/**
 * Everything that can go wrong between a tap and an answer, as a closed set.
 *
 * The distinction that matters is [Network] versus the rest: a network failure means "try
 * again later, nothing is lost", and the upload queue treats it as retryable. A 422 means
 * "this will fail identically forever", and retrying it is how a queue turns into an
 * infinite loop that flattens a battery.
 */
sealed class ApiException(message: String, cause: Throwable? = null) : Exception(message, cause) {

    /** No route to the server: aeroplane mode, a dead Wi-Fi AP, a workshop with thick walls. */
    class Network(cause: IOException) : ApiException("network unavailable", cause)

    /** 401. The session is gone; the only cure is signing in again. */
    class Unauthenticated : ApiException("session expired")

    /** 403. The account is real but lacks the capability. Retrying changes nothing. */
    class Forbidden : ApiException("not permitted")

    class NotFound(what: String) : ApiException("$what not found")

    /**
     * 422 with the API's own error code (`validation`, `stage_gate`, `currency_locked`,
     * `immutable`, …). The code is kept, not just the message, so a screen can react to
     * `stage_gate` differently from a typo in a field.
     */
    class Rule(val code: String, val detail: String?) : ApiException(detail ?: code)

    /** 5xx, or a body that does not parse. Retryable: the server may be mid-deploy. */
    class Server(val status: Int, val detail: String?) : ApiException(detail ?: "server error $status")

    /** True when trying the same request later could plausibly succeed. */
    val isRetryable: Boolean
        get() = this is Network || this is Server
}
