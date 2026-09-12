package hu.autotherm.autocrm

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * The DTOs in `data/api/Dto.kt` are hand-written and partial, which keeps the APK small at
 * the cost of the compiler no longer proving they match the server. This test buys that
 * proof back: it reads the committed `openapi/openapi.json` — the same document the web
 * client generates from — and fails if a field the phone reads has been renamed or removed.
 *
 * It is deliberately a *presence* check, not a full schema comparison. A field the phone
 * ignores may come and go freely; a field it reads may not vanish without this going red.
 */
class OpenApiContractTest {

    private val document: JsonObject by lazy {
        // app/ → android/ → repository root.
        val file = File("../../openapi/openapi.json")
        assertTrue(
            "openapi/openapi.json not found at ${file.absolutePath}. " +
                "Regenerate it with: cargo run --bin autocrm -- openapi --out ../openapi/openapi.json",
            file.exists(),
        )
        Json.parseToJsonElement(file.readText()).jsonObject
    }

    private fun schema(name: String): JsonObject {
        val schemas = document["components"]!!.jsonObject["schemas"]!!.jsonObject
        val entry = schemas[name]
        assertTrue("the API no longer publishes a schema called '$name'", entry != null)
        return entry!!.jsonObject
    }

    /** For a oneOf schema: the arm containing [marker] must carry every one of [fields]. */
    private fun assertHasOneOfArmFields(schemaName: String, marker: String, vararg fields: String) {
        val arms = schema(schemaName)["oneOf"]?.jsonArray
        assertTrue("$schemaName is no longer a oneOf", arms != null)
        val arm = arms!!.map { it.jsonObject }
            .firstOrNull { it["properties"]?.jsonObject?.containsKey(marker) == true }
        assertTrue("$schemaName has no arm carrying '$marker'", arm != null)
        val properties = arm!!["properties"]!!.jsonObject
        for (field in fields) {
            assertTrue(
                "$schemaName['$marker'].$field is gone from the API contract, but the " +
                    "Android client still reads it (see data/api/Dto.kt)",
                properties.containsKey(field),
            )
        }
    }

    private fun assertHasFields(schemaName: String, vararg fields: String) {
        val properties = schema(schemaName)["properties"]?.jsonObject
        assertTrue("$schemaName has no properties block", properties != null)
        for (field in fields) {
            assertTrue(
                "$schemaName.$field is gone from the API contract, but the Android client " +
                    "still reads it (see data/api/Dto.kt)",
                properties!!.containsKey(field),
            )
        }
    }

    @Test
    fun `the picker the capture screen opens on still returns what it reads`() {
        assertHasFields(
            "PickerOrder",
            "id", "number", "title", "partner_name", "vehicle", "vehicle_plate",
            "stage_key", "stage_label",
        )
    }

    @Test
    fun `the mobile login still returns a bearer token`() {
        assertHasFields("LoginResponse", "user", "token", "expires_at")
        assertHasFields("SessionUser", "id", "email", "display_name", "role", "must_change_password")
    }

    @Test
    fun `the three step upload contract is unchanged`() {
        // If any of this moves, photos stop reaching the server and the queue fills up
        // silently — the single worst failure this client can have.
        assertHasFields("UploadRequest", "target", "filename", "content_type", "byte_size", "sha256")
        assertHasFields("PresignedRequest", "method", "url", "headers")
        assertHasFields("CompleteBody", "ticket")
        // Completed is a oneOf discriminated by `type`; the phone only reads the image arm.
        assertHasOneOfArmFields("Completed", "image", "type", "image", "created")
    }

    @Test
    fun `the order detail still carries the fields the phone shows`() {
        assertHasFields(
            "OrderDetail",
            "order", "partner", "stage", "items", "value", "blockers", "vehicles", "spec",
            "image_counts",
        )
        assertHasFields("Order", "id", "number", "title", "vehicle_plate", "vehicle_vin", "due_date")
        assertHasFields("OrderValue", "currency", "total_minor", "total_huf_minor")
        assertHasFields("StageView", "key", "label_hu", "days_in_stage", "is_terminal")
    }

    @Test
    fun `the stage transition list still explains why a move is refused`() {
        // `reason` is what turns "the button does nothing" into "there is no completion
        // photo yet", which is a problem the fitter holding the phone can actually fix.
        assertHasFields(
            "TransitionOption",
            "stage_key", "label_hu", "manual", "requires_note", "gates_met",
        )
    }

    @Test
    fun `the build spec and the quotation are still where the phone looks`() {
        assertHasFields("OrderSpec", "form", "target_temp_c", "cooling_unit_make", "heater_make")
        assertHasFields("Lead", "quoted_value_minor", "currency", "quote_valid_until")
    }

    @Test
    fun `imported MiniCRM history is still readable`() {
        assertHasFields("OrderNote", "id", "author_name", "body", "occurred_at")
    }

    @Test
    fun `the endpoints the phone calls still exist`() {
        val paths = document["paths"]!!.jsonObject
        val required = listOf(
            "/auth/login", "/auth/me", "/auth/logout",
            "/mobile/orders",
            "/orders", "/orders/{id}", "/orders/{id}/stages", "/orders/{id}/transitions",
            "/orders/{id}/stage", "/orders/{id}/notes", "/orders/{id}/images",
            "/orders/{id}/uploads", "/uploads/complete",
            "/partners", "/partners/{id}", "/leads", "/leads/{id}",
        )
        for (path in required) {
            assertTrue("the API no longer serves $path", paths.containsKey(path))
        }
    }
}
