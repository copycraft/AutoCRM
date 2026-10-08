package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.inspection.DraftPayload
import hu.autotherm.autocrm.ui.inspection.readingsProblem
import org.junit.Test
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull

class ReadingsProblemTest {

    private fun payload(plate: String = "ABC-123", odometer: String = "120000") =
        DraftPayload(vehiclePlate = plate, inspectorName = "Teszt Elek", odometer = odometer)

    @Test
    fun checkoutAsksForPlateBeforeThePhotoAndOdometer() {
        assertEquals("a rendszám kötelező", readingsProblem(payload(plate = ""), "checkout", false))
        assertEquals("készíts fotót a rendszámról", readingsProblem(payload(), "checkout", false))
        assertEquals("az óraállás kötelező az átvételnél", readingsProblem(payload(odometer = ""), "checkout", true))
        assertNull(readingsProblem(payload(), "checkout", true))
    }

    @Test
    fun checkinNeedsOnlyThePlate() {
        assertEquals("a rendszám kötelező", readingsProblem(payload(plate = " "), "checkin", false))
        assertNull(readingsProblem(payload(odometer = ""), "checkin", false))
    }
}
