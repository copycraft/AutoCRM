package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.patchOf
import hu.autotherm.autocrm.util.parseQuantity
import hu.autotherm.autocrm.util.parseSignedMajorToMinor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** What a fitter types reaches the server in the shape the server accepts. */
class FormInputTest {

    @Test
    fun quantitiesTakeCommaOrDotAndAtMostThreeDecimals() {
        assertEquals("2.5", parseQuantity("2,5"))
        assertEquals("2.5", parseQuantity("2.50"))
        assertEquals("1", parseQuantity("1"))
        assertEquals("0.125", parseQuantity("0,125"))
        assertEquals("1500", parseQuantity("1 500"))
        // The server refuses these (quantity must be positive with at most 3 decimals).
        assertNull(parseQuantity("0"))
        assertNull(parseQuantity("-1"))
        assertNull(parseQuantity("0,0001"))
        assertNull(parseQuantity("két"))
    }

    @Test
    fun pricesKeepCentsAndDiscountSign() {
        assertEquals(1_250_050L, parseSignedMajorToMinor("12 500,50"))
        assertEquals(-500_000L, parseSignedMajorToMinor("-5000"))
        assertEquals(-1_050L, parseSignedMajorToMinor("−10,5"))
        assertNull(parseSignedMajorToMinor("-"))
        assertNull(parseSignedMajorToMinor(""))
    }

    @Test
    fun hungarianPhonesReadInGroups() {
        val f: (String?) -> String? = { hu.autotherm.autocrm.util.formatPhone(it) }
        assertEquals("+36 30 123 4567", f("+36301234567"))
        assertEquals("+36 30 123 4567", f("+36 30 123-4567"))
        assertEquals("06 30 123 4567", f("06301234567"))
        assertEquals("+36 1 234 5678", f("+3612345678"))
        assertEquals("+36 1 234 5678", f("003612345678"))
        // Not recognisably Hungarian, or more than a number: shown as written.
        assertEquals("+49 151 2345678", f("+49 151 2345678"))
        assertEquals("30/123-4567 mellék 12", f("30/123-4567 mellék 12"))
        assertNull(f(null))
    }

    @Test
    fun patchWritesNullsOutLoud() {
        // Omitted would mean "leave it"; the form emptied it, so it must say null.
        val body = patchOf("phone" to null, "name" to "Kovács Kft.", "partner_id" to 7L)
        assertEquals("""{"phone":null,"name":"Kovács Kft.","partner_id":7}""", body.toString())
    }
}
