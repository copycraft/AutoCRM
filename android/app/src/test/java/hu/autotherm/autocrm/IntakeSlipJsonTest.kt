package hu.autotherm.autocrm

import hu.autotherm.autocrm.ui.orders.intakeSlipJson
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ORD-L6: the phone records the intake slip through PATCH, so emptied fields must
 * reach the wire as explicit nulls (the server keeps omitted fields, ORD-L4).
 */
class IntakeSlipJsonTest {

    @Test
    fun `a full slip sends every field`() {
        val body = intakeSlipJson(
            mileageKm = 123456,
            fuelLevel = "1/2",
            keyCount = 2,
            condition = "Karcolás balra",
            hasValuables = true,
            valuables = "Laptop",
        )

        assertEquals(123456, body["mileage_in"]!!.jsonPrimitive.int)
        assertEquals("1/2", body["fuel_level"]!!.jsonPrimitive.content)
        assertEquals(2, body["key_count"]!!.jsonPrimitive.int)
        assertEquals("Karcolás balra", body["intake_condition"]!!.jsonPrimitive.content)
        assertEquals(true, body["valuables_declared"]!!.jsonPrimitive.boolean)
        assertEquals("Laptop", body["valuables"]!!.jsonPrimitive.content)
    }

    @Test
    fun `emptied optionals are explicit nulls so the server clears them`() {
        val body = intakeSlipJson(
            mileageKm = 100,
            fuelLevel = null,
            keyCount = null,
            condition = "  ",
            hasValuables = false,
            valuables = "Laptop",
        )

        assertEquals(100, body["mileage_in"]!!.jsonPrimitive.int)
        assertEquals(JsonNull, body["fuel_level"])
        assertEquals(JsonNull, body["key_count"])
        assertEquals(JsonNull, body["intake_condition"])
        // Saving the slip is the answer: a definite false, never "not asked".
        assertEquals(false, body["valuables_declared"]!!.jsonPrimitive.boolean)
        // The valuables text only travels with a checked box.
        assertEquals(JsonNull, body["valuables"])
    }

    @Test
    fun `the body survives a JSON round trip with its nulls intact`() {
        val body = intakeSlipJson(
            mileageKm = 5,
            fuelLevel = null,
            keyCount = null,
            condition = null,
            hasValuables = false,
            valuables = null,
        )
        val wire = body.toString()

        assertTrue("cleared fields must reach the wire, got: $wire", wire.contains("\"fuel_level\":null"))
        assertTrue(wire.contains("\"valuables_declared\":false"))
    }
}
