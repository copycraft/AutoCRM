package hu.autotherm.autocrm

import hu.autotherm.autocrm.util.isCompleteVin
import hu.autotherm.autocrm.util.spokenToVin
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** A VIN read out loud reaches the field as the 17 characters it stands for. */
class SpokenVinTest {

    @Test
    fun hungarianLetterAndDigitNames() {
        assertEquals(
            "WDB9066351S123456",
            spokenToVin("dupla vé dé bé kilenc nulla hat hat három öt egy es egy kettő három négy öt hat"),
        )
    }

    @Test
    fun charactersSaidInOneBreathAndMixedTokens() {
        assertEquals("WDB906635K1234567", spokenToVin("WDB 906 635 K 1234567"))
        assertEquals("WDB906635K1234567", spokenToVin("w-d-b 906635k 12345 67"))
    }

    @Test
    fun lettersNeverInAVinBecomeTheirLookalikeDigits() {
        assertEquals("WDB9O6", "WDB9O6") // sanity
        assertEquals("WDB906", spokenToVin("WDB9O6"))
        assertEquals("1", spokenToVin("i"))
    }

    @Test
    fun completeness() {
        assertTrue(isCompleteVin("WDB906635K1234567"))
        assertFalse(isCompleteVin("WDB906635K123456"))
        assertFalse(isCompleteVin("WDB906635O1234567"))
    }
}
