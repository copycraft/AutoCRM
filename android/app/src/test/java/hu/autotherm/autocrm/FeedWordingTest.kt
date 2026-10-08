package hu.autotherm.autocrm

import hu.autotherm.autocrm.ui.common.describeNetwork
import hu.autotherm.autocrm.ui.common.describeServer
import hu.autotherm.autocrm.util.relativeTime
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

/** Feed times read as "how long ago"; failures say what happened and what to do. */
class FeedWordingTest {
    // 2026-10-08 14:00 in Budapest (UTC+2 in October).
    private val now = Instant.parse("2026-10-08T12:00:00Z")

    @Test
    fun recentMomentsInWords() {
        assertEquals("most", relativeTime("2026-10-08T11:59:40Z", now))
        assertEquals("12 perce", relativeTime("2026-10-08T11:48:00Z", now))
        assertEquals("3 órája", relativeTime("2026-10-08T09:00:00Z", now))
        assertEquals("tegnap 16:30", relativeTime("2026-10-07T14:30:00Z", now))
        assertEquals("2026. 10. 01. 10:00", relativeTime("2026-10-01T08:00:00Z", now))
    }

    @Test
    fun networkFailuresNameTheirCause() {
        assertTrue(describeNetwork(null, phoneOnline = false).startsWith("Nincs hálózat a telefonon"))
        assertTrue(describeNetwork(java.net.SocketTimeoutException(), true).contains("nem válaszolt időben"))
        assertTrue(describeNetwork(java.net.ConnectException(), true).contains("újraindul"))
        assertTrue(describeNetwork(java.net.UnknownHostException(), true).contains("címe nem található"))
    }

    @Test
    fun serverFailuresKeepTheStatusForSupport() {
        assertTrue(describeServer(503, null).contains("most nem érhető el"))
        assertTrue(describeServer(503, null).endsWith("(503)"))
        assertTrue(describeServer(500, "boom").endsWith("(500)"))
        assertTrue(describeServer(200, "unparseable response: x").contains("váratlan választ"))
    }
}
