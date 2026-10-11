package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.IncomingInvoice
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.ui.incoming.bucketLabel
import hu.autotherm.autocrm.ui.incoming.majorToMinor
import hu.autotherm.autocrm.ui.incoming.minorToMajor
import hu.autotherm.autocrm.ui.incoming.paymentLabel
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** What the incoming-invoices screens rely on: wire shapes, gates, amount math. */
class IncomingInvoiceTest {

    private val json = Json { ignoreUnknownKeys = true }

    private fun row(extra: String = "") = json.decodeFromString(
        IncomingInvoice.serializer(),
        """{"id":3,"kind":"invoice","supplier_name":"Hűtőgép Kft","currency":"HUF",
            "payment_method":"TRANSFER","paid_amount":0,"booking_only":false,"bucket":"open_invoice",
            "created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-01T00:00:00Z"$extra}""",
    )

    @Test
    fun `a fresh upload parses with amounts still null`() {
        val invoice = row()
        assertEquals(3L, invoice.id)
        assertEquals("Hűtőgép Kft", invoice.supplierName)
        assertNull(invoice.grossAmount)
        assertNull(invoice.fileName)
        assertEquals("open_invoice", invoice.bucket)
    }

    @Test
    fun `a filled row parses amounts and file metadata`() {
        val invoice = row(
            "," + "\"supplier_tax_number\":\"12345678-1-23\"," +
                "\"invoice_number\":\"SZ-42\"," +
                "\"issue_date\":\"2026-09-30\",\"gross_amount\":125000," +
                "\"file_name\":\"szamla.pdf\",\"file_type\":\"application/pdf\"," +
                "\"file_size\":48210,\"paid_amount\":125000,\"paid_on\":\"2026-10-02\"",
        )
        assertEquals(125000L, invoice.grossAmount)
        assertEquals("szamla.pdf", invoice.fileName)
        assertEquals(48210L, invoice.fileSize)
    }

    @Test
    fun `unknown buckets and methods read raw, never blank`() {
        assertEquals("valami_uj", bucketLabel("valami_uj"))
        assertEquals("BARTER", paymentLabel("BARTER"))
        assertEquals("Nyitott", bucketLabel("open_invoice"))
        assertEquals("Átutalás", paymentLabel("TRANSFER"))
    }

    @Test
    fun `typed amounts become minor units per currency`() {
        assertEquals(12500L, majorToMinor("12 500", "HUF"))
        assertEquals(12500L, majorToMinor("12500,4", "HUF"))
        assertEquals(1250L, majorToMinor("12,5", "EUR"))
        assertNull(majorToMinor("", "HUF"))
        assertEquals("12500", minorToMajor(12500L, "HUF"))
        assertEquals("12,50", minorToMajor(1250L, "EUR"))
        var failed = false
        try {
            majorToMinor("nem szám", "HUF")
        } catch (e: IllegalArgumentException) {
            failed = true
        }
        assertTrue(failed)
    }

    private fun account(role: String, capabilities: Set<String> = emptySet()) = SessionStore.Account(
        userId = 1, email = "a@b.hu", displayName = "A", role = role,
        mustChangePassword = false, capabilities = capabilities,
    )

    @Test
    fun `the incoming screen is for the invoicing right only`() {
        assertTrue(account("admin").canIssueInvoices)
        assertTrue(account("office").canIssueInvoices)
        assertFalse(account("designer").canIssueInvoices)
        assertFalse(account("viewer").canIssueInvoices)
        // A per-user grant opens it regardless of role; an empty set falls back to role.
        assertTrue(account("viewer", setOf("issue_invoices")).canIssueInvoices)
        assertFalse(account("office", setOf("comment")).canIssueInvoices)
    }
}
