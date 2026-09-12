package hu.autotherm.autocrm.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

private val Context.serverDataStore by preferencesDataStore(name = "server")

/**
 * Which AutoCRM the phone talks to.
 *
 * A build-time constant was wrong for this: the same APK has to reach a laptop on the
 * workshop Wi-Fi during a parallel run, the office server afterwards, and a different
 * address again if the van is at a customer's site. Baking it in means a rebuild for each,
 * which is not a thing a fitter can do.
 *
 * Device-wide and survives sign-out on purpose — the address belongs to the phone, not to
 * the person holding it. Signing out and back in must not mean retyping an IP.
 */
class ServerStore(private val context: Context) {

    private object Keys {
        val BASE_URL = stringPreferencesKey("base_url")
    }

    /** Null until someone has chosen one; that is what puts the setup screen first. */
    val baseUrl: Flow<String?> = context.serverDataStore.data.map { it[Keys.BASE_URL] }

    suspend fun current(): String? = context.serverDataStore.data.first()[Keys.BASE_URL]

    /**
     * Reads the stored address, throwing if there is none. Every API call goes through this,
     * so an unconfigured app fails loudly at the call site rather than quietly building
     * requests against an empty host.
     */
    suspend fun require(): String =
        current() ?: error("no server address configured")

    suspend fun save(raw: String) {
        val normalized = normalize(raw) ?: error("not a usable address: $raw")
        context.serverDataStore.edit { it[Keys.BASE_URL] = normalized }
    }

    suspend fun clear() {
        context.serverDataStore.edit { it.remove(Keys.BASE_URL) }
    }

    companion object {
        /**
         * Turns what someone types on a phone keyboard into a base URL, or null.
         *
         * Accepts `192.168.50.10:8080`, `http://192.168.50.10:8080/`, `crm.autotherm.hu`.
         * A bare host gets `http://` rather than `https://`: the address typed by hand is
         * almost always a machine on the local network with no certificate, and defaulting
         * to https there produces a TLS error that reads like the server is down.
         * Anyone reaching production types the `https://` themselves, and the field's
         * placeholder shows it.
         */
        fun normalize(raw: String): String? {
            val trimmed = raw.trim()
            if (trimmed.isEmpty()) return null
            val withScheme = if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
                trimmed
            } else {
                "http://$trimmed"
            }
            val parsed = withScheme.toHttpUrlOrNull() ?: return null
            if (parsed.host.isBlank()) return null
            // Rebuild from the parsed form so a stray path or query cannot ride along and
            // end up prefixed to every endpoint.
            val port = if (parsed.port == defaultPort(parsed.scheme)) "" else ":${parsed.port}"
            return "${parsed.scheme}://${parsed.host}$port"
        }

        private fun defaultPort(scheme: String): Int = if (scheme == "https") 443 else 80
    }
}
