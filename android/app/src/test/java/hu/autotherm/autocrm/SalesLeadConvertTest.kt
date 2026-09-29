package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.OrderRef
import hu.autotherm.autocrm.ui.leads.canChangeLeadStage
import hu.autotherm.autocrm.ui.leads.convertBody
import hu.autotherm.autocrm.ui.leads.isExpired
import java.time.LocalDate
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Sales journey (SALES-L): converting a lead on the phone must follow the same rules as
 * the web. The order currency starts from the partner's default (docs/history/REMEDIATION.md MAJOR-03),
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

    @Test
    fun quoteExpiryIsDecidedOnTheBudapestCalendarDay() {
        // Like the web (SALES-49) and the reports window (TIME-L1): "today" is the
        // Europe/Budapest date, not the device zone.
        val budapest = ZoneId.of("Europe/Budapest")
        val today = LocalDate.now(budapest)
        assertTrue(isExpired(today.minusDays(1).toString(), today))
        assertFalse(isExpired(today.toString(), today))
        assertFalse(isExpired(today.plusDays(1).toString(), today))
        assertFalse(isExpired("not-a-date", today))
    }
}
