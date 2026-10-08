package hu.autotherm.autocrm

import androidx.compose.ui.text.AnnotatedString
import hu.autotherm.autocrm.data.prefs.RecentPartners
import hu.autotherm.autocrm.util.GroupedNumberTransformation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Amounts read grouped while typed; the cursor still lands where the finger put it. */
class GroupedNumberTest {

    private fun shown(raw: String) = GroupedNumberTransformation.filter(AnnotatedString(raw)).text.text

    @Test
    fun groupsTheWholePartOnly() {
        assertEquals("1 250 000,5", shown("1250000,5"))
        assertEquals("-12 500", shown("-12500"))
        assertEquals("999", shown("999"))
        assertEquals("1 000.25", shown("1000.25"))
        assertEquals("", shown(""))
        assertEquals("abc", shown("abc"))
    }

    @Test
    fun cursorMapsAcrossTheGaps() {
        val t = GroupedNumberTransformation.filter(AnnotatedString("1250000"))
        // "1 250 000": original 1 sits before the first gap, 4 before the second.
        assertEquals(0, t.offsetMapping.originalToTransformed(0))
        assertEquals(1, t.offsetMapping.originalToTransformed(1))
        assertEquals(3, t.offsetMapping.originalToTransformed(2))
        assertEquals(9, t.offsetMapping.originalToTransformed(7))
        for (o in 0..7) {
            assertEquals(o, t.offsetMapping.transformedToOriginal(t.offsetMapping.originalToTransformed(o)))
        }
        // A tap on the gap itself resolves to the digit boundary next to it.
        assertEquals(1, t.offsetMapping.transformedToOriginal(2))
    }

    @Test
    fun huTaxNumberDrawsItsDashesAndMapsTheCursor() {
        val t = hu.autotherm.autocrm.util.HuTaxNumberTransformation.filter(AnnotatedString("12345678123"))
        assertEquals("12345678-1-23", t.text.text)
        for (o in 0..11) {
            assertEquals(o, t.offsetMapping.transformedToOriginal(t.offsetMapping.originalToTransformed(o)))
        }
        // Part-typed numbers grow their dashes as they go.
        assertEquals("12345678-1", hu.autotherm.autocrm.util.HuTaxNumberTransformation.filter(AnnotatedString("123456781")).text.text)
        assertEquals("1234", hu.autotherm.autocrm.util.HuTaxNumberTransformation.filter(AnnotatedString("1234")).text.text)
    }

    @Test
    fun recentPartnersSurviveTheirOwnStorage() {
        val e = RecentPartners.Entry(42, "Kovács\tés Társa Kft.", "EUR", "AT")
        val back = RecentPartners.decode(RecentPartners.encode(e))!!
        assertEquals(42L, back.id)
        assertEquals("EUR", back.defaultCurrency)
        assertEquals("AT", back.country)
        assertEquals("Kovács\tés Társa Kft.", back.name)
        assertNull(RecentPartners.decode("garbage"))
        assertEquals(null, RecentPartners.decode("7\t\t\tNév")!!.defaultCurrency)
    }
}
