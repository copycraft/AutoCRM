package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.prefs.ServerStore
import org.junit.Assert.assertEquals
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
    fun `what cannot be used is refused rather than half-accepted`() {
        assertNull(norm(""))
        assertNull(norm("   "))
        assertNull(norm("http://"))
        assertNull(norm("not a url at all"))
    }
}
