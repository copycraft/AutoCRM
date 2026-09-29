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
         * A bare local address (private IP, `localhost`, a one-word or `.local`-style name)
         * gets `http://`: that is a machine on the workshop network with no certificate, and
         * https there fails with a TLS error that reads like the server is down. Any other
         * bare host gets `https://`, because http to a public name sends the password and a
         * year-long token in clear before any redirect could help. An explicit scheme is
         * always kept.
         */
        fun normalize(raw: String): String? {
            val trimmed = raw.trim()
            if (trimmed.isEmpty()) return null
            val withScheme = if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
                trimmed
            } else {
                val host = "http://$trimmed".toHttpUrlOrNull()?.host ?: return null
                if (isLocalHost(host)) "http://$trimmed" else "https://$trimmed"
            }
            val parsed = withScheme.toHttpUrlOrNull() ?: return null
            if (parsed.host.isBlank()) return null
            // Rebuild from the parsed form so a stray path or query cannot ride along and
            // end up prefixed to every endpoint.
            val port = if (parsed.port == defaultPort(parsed.scheme)) "" else ":${parsed.port}"
            return "${parsed.scheme}://${parsed.host}$port"
        }

        private fun defaultPort(scheme: String): Int = if (scheme == "https") 443 else 80

        /**
         * True when this address sends the password over the open internet in clear.
         *
         * The app permits cleartext because a workshop server has no certificate, but that
         * is a trade made for the local network only. Android's network security config
         * cannot express "private ranges only", so the narrowing happens here and the setup
         * screen says so out loud.
         */
        fun isUnencryptedAndRemote(url: String): Boolean {
            val parsed = url.toHttpUrlOrNull() ?: return false
            if (parsed.scheme != "http") return false
            return !isPrivateHost(parsed.host)
        }

        /**
         * A name that only resolves on the local network: a private address, or a one-word
         * or local-suffix hostname (`crmserver`, `crm.local`). Decides the default scheme.
         */
        private fun isLocalHost(host: String): Boolean {
            if (isPrivateHost(host)) return true
            // A public IP literal (v4 or v6): only an explicit http:// opts out of TLS.
            if (host.contains(':')) return false
            if (host.split(".").all { it.toIntOrNull() != null }) return false
            if (!host.contains('.')) return true
            return LOCAL_SUFFIXES.any { host.endsWith(it) }
        }

        private val LOCAL_SUFFIXES = listOf(".local", ".lan", ".home.arpa", ".internal")

        /** RFC 1918, loopback, link-local, and the emulator's view of its host. */
        private fun isPrivateHost(host: String): Boolean {
            if (host == "localhost") return true
            val octets = host.split(".").mapNotNull { it.toIntOrNull() }
            if (octets.size != 4) return false
            val (a, b) = octets[0] to octets[1]
            return when {
                a == 10 -> true
                a == 127 -> true
                a == 192 && b == 168 -> true
                a == 172 && b in 16..31 -> true
                a == 169 && b == 254 -> true
                // Tailscale and other CGNAT-range overlays: private in practice.
                a == 100 && b in 64..127 -> true
                else -> false
            }
        }
    }
}
