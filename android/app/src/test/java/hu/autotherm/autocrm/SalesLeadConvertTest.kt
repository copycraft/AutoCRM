package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.OrderRef
import hu.autotherm.autocrm.ui.leads.canChangeLeadStage
import hu.autotherm.autocrm.ui.leads.convertBody
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Sales journey (SALES-L): converting a lead on the phone must follow the same rules as
 * the web. The order currency starts from the partner's default (REMEDIATION.md MAJOR-03),
 * and a converted lead cannot change stage (DECISIONS.md, `lead_converted`).
 */
class SalesLeadConvertTest {

    @Test
    fun conversionCurrencyFollowsTheEurPartnerDefault() {
        assertEquals("EUR", convertBody("Hűtős Sprinter", "EUR").currency)
    }

    @Test
    fun conversionCurrencyFallsBackToHufWithoutAPartnerDefault() {
        assertEquals("HUF", convertBody("Hűtős Sprinter", null).currency)
        assertEquals("HUF", convertBody("Hűtős Sprinter", "HUF").currency)
    }

    @Test
    fun conversionSendsTheTitle() {
        assertEquals("Hűtős Sprinter", convertBody("Hűtős Sprinter", "EUR").title)
    }

    @Test
    fun convertedLeadOffersNoStageChange() {
        assertFalse(canChangeLeadStage(listOf(OrderRef(1, "2026-0001"))))
        assertTrue(canChangeLeadStage(emptyList()))
    }
}
