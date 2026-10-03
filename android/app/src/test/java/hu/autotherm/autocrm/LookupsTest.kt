package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.ImageCategoryEntry
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.api.LookupItem
import hu.autotherm.autocrm.data.inspection.ServerErrorTexts
import hu.autotherm.autocrm.data.inspection.attachableCategories
import hu.autotherm.autocrm.data.inspection.categoryLabel
import hu.autotherm.autocrm.data.inspection.damageTypeLabel
import hu.autotherm.autocrm.data.inspection.lookupLabel
import hu.autotherm.autocrm.data.inspection.severityLabel
import hu.autotherm.autocrm.data.inspection.verdictLabel
import hu.autotherm.autocrm.data.inspection.walkaroundKindLabel
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.prefs.LookupsCache
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The server's enumerations (`GET /config/lookups`): selects, chips and labels
 * render from the cached document, and an unknown key reads as itself rather
 * than blank. Nothing about which values exist is kept in the app.
 */
class LookupsTest {

    private fun lookups() = Lookups(
        damageTypes = listOf(LookupItem("scratch", "Karcolás")),
        severities = listOf(LookupItem("minor", "Enyhe")),
        verdicts = listOf(LookupItem("preexisting", "Már megvolt")),
        walkaroundKinds = listOf(LookupItem("checkout", "Átvétel")),
        fuelLevels = listOf(LookupItem("1/2", "1/2")),
        currencies = listOf(LookupItem("HUF", "Forint"), LookupItem("EUR", "Euró")),
        imageCategories = listOf(
            ImageCategoryEntry("intake", "Bevétel", immutable = true, attachable = false),
            ImageCategoryEntry("production", "Gyártás", immutable = false, attachable = true),
            ImageCategoryEntry("inspection", "Átvétel", immutable = false, attachable = false),
        ),
        errorTexts = listOf(
            hu.autotherm.autocrm.data.api.ErrorTextEntry("locked", "Zárolva"),
        ),
    )

    @Test
    fun `labels come from the document, unknown keys read as themselves`() {
        val l = lookups()
        assertEquals("Karcolás", damageTypeLabel("scratch", l))
        assertEquals("Enyhe", severityLabel("minor", l))
        assertEquals("Már megvolt", verdictLabel("preexisting", l))
        assertEquals("Bevétel", categoryLabel("intake", l))
        assertEquals("1/2", lookupLabel(l.fuelLevels, "1/2"))
        assertEquals("Forint", lookupLabel(l.currencies, "HUF"))
        assertEquals("Átvétel", walkaroundKindLabel("checkout", l))
        // A value the office removed, or a newer server than this build.
        assertEquals("Some new type", damageTypeLabel("some_new_type", l))
    }

    @Test
    fun `offline with nothing cached labels fall back without crashing`() {
        assertEquals("Scratch", damageTypeLabel("scratch", null))
        assertEquals("Minor", severityLabel("minor", null))
        assertEquals("Intake", categoryLabel("intake", null))
        // Only production is hand-attachable; intake stays evidence.
        assertEquals(listOf("production"), CapturePrefs.attachable(null))
        assertTrue(CapturePrefs.isImmutable("intake", null))
        assertFalse(CapturePrefs.isImmutable("production", null))
    }

    @Test
    fun `attachability and immutability come from the document`() {
        val l = lookups()
        assertEquals(listOf("production"), attachableCategories(l).map { it.key })
        assertEquals(listOf("production"), CapturePrefs.attachable(l))
        assertTrue(CapturePrefs.isImmutable("intake", l))
        assertFalse(CapturePrefs.isImmutable("inspection", l))
    }

    @Test
    fun `a cached document reads back as it was written`() {
        val l = lookups()
        assertEquals(l, LookupsCache.decode(LookupsCache.encode(l)))
    }    @Test
    fun `a damaged cache entry is a miss, not a crash`() {
        assertNull(LookupsCache.decode("not json"))
        assertNull(LookupsCache.decode("""{"a":1}"""))
    }

    @Test
    fun `fresh server error texts layer over the built-in map`() {
        try {
            ServerErrorTexts.update(lookups())
            assertEquals("Zárolva", ServerErrorTexts.texts["locked"])
        } finally {
            ServerErrorTexts.texts = emptyMap()
        }
    }
}
