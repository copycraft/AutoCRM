package hu.autotherm.autocrm

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
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

    /** For an allOf schema: one arm must be a $ref to [refName]. */
    private fun assertHasAllOfRef(schemaName: String, refName: String) {
        val allOf = schema(schemaName)["allOf"]?.jsonArray
        assertTrue("$schemaName is no longer an allOf composition", allOf != null)
        val refs = allOf!!.map { it.jsonObject["\$ref"]?.jsonPrimitive?.content }.filterNotNull()
        assertTrue(
            "$schemaName no longer composes $refName (refs: $refs)",
            refs.any { it.endsWith("/$refName") },
        )
    }

    /** Fields carried by the inline (non-$ref) arms of an allOf schema. */
    private fun assertHasAllOfFields(schemaName: String, vararg fields: String) {
        val allOf = schema(schemaName)["allOf"]?.jsonArray
        assertTrue("$schemaName is no longer an allOf composition", allOf != null)
        val inline = allOf!!
            .map { it.jsonObject }
            .flatMap { arm -> arm["properties"]?.jsonObject?.keys ?: emptySet() }
            .toSet()
        for (field in fields) {
            assertTrue(
                "$schemaName no longer carries $field in its inline arms, but the " +
                    "Android client still reads it (see data/api/Dto.kt)",
                field in inline,
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
        assertHasFields("SessionUser", "id", "email", "display_name", "role", "must_change_password", "hr_access")
        assertHasFields("Employee", "id", "full_name", "email", "company_phone", "personal_phone", "photo_url", "archived_at")
        assertHasFields("User", "id", "email", "display_name", "role", "is_active", "hr_access")
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
    fun `image views still compose the row shape with presigned urls`() {
        // ImageView is Image + urls (allOf). The phone reads a subset of the row
        // plus both urls; if the composition breaks up or a side moves, gallery
        // and thumbnail code reads the wrong shape.
        assertHasAllOfRef("ImageView", "Image")
        assertHasAllOfFields("ImageView", "thumb_url", "display_url")
        assertHasFields(
            "Image", "id", "category", "captured_at", "uploaded_at", "immutable",
        )
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
    fun `the correspondence log still carries what the phone shows`() {
        assertHasFields(
            "EmailSummary",
            "id", "order_id", "to_address", "subject", "status", "queued_at", "sent_at",
            "is_automatic", "sent_by_name", "error",
        )
        assertHasFields(
            "EmailMessage",
            "id", "to_address", "cc", "from_address", "subject", "body_text", "attachments",
            "status", "attempts", "queued_at", "sent_at",
        )
        assertHasFields("AttachmentRef", "document_id", "filename", "byte_size")
    }

    @Test
    fun `imported MiniCRM history is still readable`() {
        assertHasFields("OrderNote", "id", "author_name", "body", "occurred_at")
    }

    @Test
    fun `the error envelope still carries a machine readable code`() {
        // The phone routes 409/422/400 by code (stage_gate gets its own explainer); if the
        // envelope flattens, every refusal degrades to a generic validation line.
        assertHasFields("ErrorBody", "error")
        assertHasFields("ErrorDetail", "code", "message")
    }

    @Test
    fun `the write paths the phone now uses still exist`() {
        val paths = document["paths"]!!.jsonObject
        val required = listOf(
            "/partners/{id}/contacts", "/contacts/{id}",
            "/leads/{id}/stage", "/leads/{id}/transitions", "/leads/{id}/convert",
            "/orders/{id}/items", "/order-items/{id}",
            "/orders/{id}/blockers", "/blockers/{id}/resolve", "/blockers/{id}/reopen",
            "/tasks", "/tasks/for/{entity}/{id}", "/tasks/{id}/done", "/tasks/{id}",
            "/reports/workload", "/reports/stalled",
        )
        for (path in required) {
            assertTrue("the API no longer serves $path", paths.containsKey(path))
        }
    }

    @Test
    fun `tasks reports and write bodies still carry what the phone sends and reads`() {
        assertHasFields(
            "Task", "id", "entity_type", "entity_id", "title", "due_date", "done_at",
            "assigned_name",
        )
        assertHasFields("TaskBody", "entity_type", "entity_id", "title", "due_date")
        assertHasFields("WorkloadReport", "from", "to", "days")
        assertHasFields("WorkloadDay", "date", "placed", "completed", "in_workshop")
        assertHasFields(
            "StalledOrder", "order_id", "number", "title", "stage_label", "days_in_stage",
            "open_blockers",
        )
        assertHasFields("ContactBody", "name", "email", "phone", "position", "notes")
        assertHasFields("LeadBody", "title", "partner_id", "contact_name", "quoted_value_minor")
        assertHasFields("OrderBody", "title", "partner_id", "currency", "vehicle_plate")
        assertHasFields("BlockerBody", "what", "responsible_email", "due_date", "notes")
        assertHasFields("AddItem", "description", "quantity", "unit_price")
        assertHasFields("ComposeRequest", "order_id", "to", "subject", "body")
    }

    @Test
    fun `handover inspections still carry what the phone walks and syncs`() {
        assertHasFields(
            "Inspection", "id", "order_id", "kind", "status", "vehicle_plate",
            "inspector_name", "odometer", "fuel_level", "checkout_id", "signed_at",
        )
        assertHasFields(
            "InspectionDetail", "inspection", "photos", "damages", "verdicts",
            "signatures", "notes", "zone_titles",
        )
        assertHasFields(
            "InspectionPhoto", "image_id", "zone_key", "purpose", "taken_at",
            "thumb_url", "display_url",
        )
        assertHasFields(
            "InspectionDamage", "zone_key", "damage_type", "severity", "note", "x", "y",
        )
        assertHasFields(
            "ZoneTemplate", "set_key", "project_type_id", "kind", "zone_key", "position",
            "title", "instruction", "optional", "required",
        )
        assertHasFields("Comparison", "checkin", "checkout", "suggestions")
        assertHasFields(
            "Lookups", "damage_types", "severities", "verdicts", "walkaround_kinds",
            "fuel_levels", "heating_fuels", "defrost_modes", "order_relations",
            "task_entity_types", "currencies", "invoice_payment_methods", "annulment_codes",
            "image_categories", "email_themes", "error_texts",
        )
        assertHasFields("LookupItem", "key", "label_hu")
        assertHasFields("ImageCategoryEntry", "key", "label_hu", "immutable", "attachable")
        assertHasFields("EmailThemeEntry", "key", "label_hu", "subject", "hero", "body")
        assertHasFields("ErrorTextEntry", "code", "text_hu")
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
            "/emails", "/emails/{id}",
            "/inspections", "/inspections/{id}", "/inspections/{id}/photos",
            "/inspections/{id}/damages", "/inspections/{id}/signatures",
            "/inspections/{id}/sign", "/inspections/{id}/notes",
            "/inspections/{id}/comparison", "/inspections/{id}/verdicts",
            "/inspections/templates", "/config/lookups",
        )
        for (path in required) {
            assertTrue("the API no longer serves $path", paths.containsKey(path))
        }
    }
}
