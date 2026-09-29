package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.prefs.ServerStore
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * What someone types on a phone keyboard, and what it has to become.
 *
 * This matters more than it looks: every request in the app is built by prefixing this
 * string, so a trailing slash or a stray path here corrupts every endpoint at once, and the
 * symptom reads as "the server is broken" rather than "the address has a typo".
 */
class ServerStoreTest {

    private fun norm(raw: String) = ServerStore.normalize(raw)

    @Test
    fun `a bare host and port gets http, because that is what a local machine is`() {
        assertEquals("http://192.168.1.10:8080", norm("192.168.1.10:8080"))
        assertEquals("http://10.0.2.2:8080", norm("10.0.2.2:8080"))
    }

    @Test
    fun `a bare local name gets http too`() {
        assertEquals("http://crmserver:8080", norm("crmserver:8080"))
        assertEquals("http://localhost:8080", norm("localhost:8080"))
        assertEquals("http://crm.local:8080", norm("crm.local:8080"))
        assertEquals("http://nas.home.arpa", norm("nas.home.arpa"))
    }

    @Test
    fun `a bare public host gets https, so the password never crosses the internet in clear`() {
        assertEquals("https://crm.autotherm.hu", norm("crm.autotherm.hu"))
        assertEquals("https://crm.autotherm.hu:8443", norm("crm.autotherm.hu:8443"))
        assertEquals("https://93.184.216.34:8080", norm("93.184.216.34:8080"))
        assertFalse(ServerStore.isUnencryptedAndRemote(norm("crm.autotherm.hu")!!))
    }

    @Test
    fun `an explicit scheme is kept`() {
        assertEquals("https://crm.autotherm.hu", norm("https://crm.autotherm.hu"))
        assertEquals("http://192.168.1.10:8080", norm("http://192.168.1.10:8080"))
    }

    @Test
    fun `default ports are dropped so two spellings of one server are one string`() {
        assertEquals("https://crm.autotherm.hu", norm("https://crm.autotherm.hu:443"))
        assertEquals("http://crm.autotherm.hu", norm("http://crm.autotherm.hu:80"))
    }

    @Test
    fun `surrounding whitespace and trailing slashes are forgiven`() {
        assertEquals("http://192.168.1.10:8080", norm("  192.168.1.10:8080/  "))
        assertEquals("http://192.168.1.10:8080", norm("http://192.168.1.10:8080///"))
    }

    @Test
    fun `a pasted path or query is stripped rather than prefixed onto every endpoint`() {
        // Someone pastes the browser URL they had open. Keeping the path would turn every
        // call into /hu/orders/api/orders and nothing would work.
        assertEquals("http://192.168.1.10:3000", norm("http://192.168.1.10:3000/hu/orders"))
        assertEquals("https://crm.autotherm.hu", norm("https://crm.autotherm.hu/hu/login?x=1"))
    }

    @Test
    fun `cleartext is only flagged when it leaves the local network`() {
        // The app permits cleartext because a workshop server has no certificate. The cost
        // is only real off the local network, so that is the only case that gets a warning.
        val private = listOf(
            "http://192.168.50.224:8080",
            "http://10.5.0.2:8080",
            "http://172.16.4.4:8080",
            "http://127.0.0.1:8080",
            "http://localhost:8080",
            "http://100.95.18.113:8080", // Tailscale
        )
        for (url in private) {
            assertFalse(url, ServerStore.isUnencryptedAndRemote(url))
        }

        assertTrue(ServerStore.isUnencryptedAndRemote("http://crm.autotherm.hu"))
        assertTrue(ServerStore.isUnencryptedAndRemote("http://93.184.216.34:8080"))
        // https anywhere is fine, local or not.
        assertFalse(ServerStore.isUnencryptedAndRemote("https://crm.autotherm.hu"))
    }

    @Test
    fun `what cannot be used is refused rather than half-accepted`() {
        assertNull(norm(""))
        assertNull(norm("   "))
        assertNull(norm("http://"))
        assertNull(norm("not a url at all"))
    }
}
