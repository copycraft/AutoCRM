package hu.autotherm.autocrm

import hu.autotherm.autocrm.ui.orders.OrderEditViewModel
import hu.autotherm.autocrm.ui.orders.orderPatchJson
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Logic audit (order lifecycle, ORD-18): PATCH keeps an omitted field and clears one sent
 * as `null`. Emptying a field on the phone's order edit screen must therefore put an
 * explicit `null` on the wire; before the fix the shared serializer dropped it and the
 * server kept the old plate / description / due date.
 */
class OrderPatchBodyTest {

    @Test
    fun emptied_fields_are_sent_as_explicit_nulls_so_the_server_clears_them() {
        val state = OrderEditViewModel.State(
            title = "  Sprinter  ",
            vehicleMake = "Mercedes",
            vehicleModel = "",
            vehiclePlate = "   ",
            vehicleVin = "",
            description = "",
            dueDate = "",
        )
        val wire = Json.parseToJsonElement(orderPatchJson(state).toString()).jsonObject

        assertEquals("Sprinter", wire["title"]!!.jsonPrimitive.content)
        assertEquals("Mercedes", wire["vehicle_make"]!!.jsonPrimitive.content)
        for (key in listOf("vehicle_model", "vehicle_plate", "vehicle_vin", "description", "due_date")) {
            assertTrue("$key must be present to be cleared", wire.containsKey(key))
            assertEquals("$key must be null", JsonNull, wire[key])
        }
    }
}
