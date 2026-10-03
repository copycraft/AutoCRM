package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.ContactHit
import hu.autotherm.autocrm.data.api.EmailHit
import hu.autotherm.autocrm.data.api.EmployeeHit
import hu.autotherm.autocrm.data.api.LeadHit
import hu.autotherm.autocrm.data.api.OrderHit
import hu.autotherm.autocrm.data.api.PartnerHit
import hu.autotherm.autocrm.data.api.SearchResults
import hu.autotherm.autocrm.ui.search.searchHits
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** What a tap on a search result opens. */
class SearchHitsTest {

    @Test
    fun `every kind of hit opens the screen that shows it`() {
        val hits = searchHits(
            SearchResults(
                orders = listOf(OrderHit(1, "2026-0001", "Sprinter", "ABC-123", "Gyártás")),
                partners = listOf(PartnerHit(2, "Kovács Kft.", "business", "Győr")),
                leads = listOf(LeadHit(3, "Weboldal: Hűtőkamra", "Kiss Péter", "Új")),
                contacts = listOf(ContactHit(9, 4, "Szabó Ilona", "Telefon Kft.", null, "+36 20 111 2222")),
                emails = listOf(EmailHit(5, "Festék", "a@b.hu", "sent")),
                employees = listOf(EmployeeHit(6, "Nagy Anna", null, "+36 30 1", archived = true)),
            ),
        )
        assertEquals(
            listOf("order/1", "partner/2", "lead/3", "partner/4", "email/5", "hr"),
            hits.map { it.route },
        )
        // A contact opens its company; the row says who they are.
        assertEquals("Szabó Ilona", hits[3].title)
        assertEquals("Telefon Kft. · +36 20 111 2222", hits[3].sub)
        assertEquals("#2026-0001 · ABC-123", hits[0].title)
        assertEquals("+36 30 1 · Kilépett", hits[5].sub)
    }

    @Test
    fun `nothing found is an empty list and missing parts leave no stray separators`() {
        assertTrue(searchHits(SearchResults()).isEmpty())
        val hit = searchHits(SearchResults(partners = listOf(PartnerHit(1, "X", "person", null)))).single()
        assertEquals("Magán", hit.sub)
    }
}
