package hu.autotherm.autocrm.data.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/**
 * The wire shapes, mirroring `openapi/openapi.json`.
 *
 * Hand-written rather than generated, and deliberately partial: the phone reads a fraction
 * of what the API returns, and a generated client would drag every schema in the document
 * into the APK. The cost is that a contract change is caught by
 * `OpenApiContractTest` rather than by the compiler — that test parses the committed
 * OpenAPI document and fails if a field named here has moved or changed type.
 *
 * `@SerialName` is spelled out even where it matches the Kotlin name, so a rename on this
 * side cannot silently change the wire format.
 */

@Serializable
data class Items<T>(@SerialName("items") val items: List<T>)

// ── Auth ────────────────────────────────────────────────────────────────────────────

@Serializable
data class LoginBody(
    @SerialName("email") val email: String,
    @SerialName("password") val password: String,
    /**
     * `mobile` returns a bearer token; `web` sets a cookie this client cannot read.
     *
     * No default value on purpose. As a Kotlin default it was silently dropped from the
     * JSON — kotlinx.serialization omits defaults unless `encodeDefaults` is on — so the
     * server applied its own default of `web`, answered with a cookie, and the app
     * reported "no session ticket". The call site states it instead, where it is visible.
     */
    @SerialName("client") val client: String,
    @SerialName("device_label") val deviceLabel: String? = null,
)

@Serializable
data class LoginResponse(
    @SerialName("user") val user: SessionUser,
    @SerialName("token") val token: String? = null,
    @SerialName("expires_at") val expiresAt: String? = null,
)

@Serializable
data class MeResponse(@SerialName("user") val user: SessionUser)

@Serializable
data class SessionUser(
    @SerialName("id") val id: Long,
    @SerialName("email") val email: String,
    @SerialName("display_name") val displayName: String,
    @SerialName("role") val role: String,
    /**
     * While true every endpoint except /auth/me, /auth/password and /auth/logout answers
     * 422. The app cannot change a password, so it says so and signs out rather than
     * showing a screen where every action fails.
     */
    @SerialName("must_change_password") val mustChangePassword: Boolean,
)

// ── The order picker ────────────────────────────────────────────────────────────────

@Serializable
data class PickerOrder(
    @SerialName("id") val id: Long,
    @SerialName("number") val number: String,
    @SerialName("title") val title: String,
    @SerialName("partner_name") val partnerName: String,
    @SerialName("vehicle") val vehicle: String? = null,
    @SerialName("vehicle_plate") val vehiclePlate: String? = null,
    @SerialName("stage_key") val stageKey: String,
    @SerialName("stage_label") val stageLabel: String,
)

// ── Orders ──────────────────────────────────────────────────────────────────────────

@Serializable
data class OrderSummary(
    @SerialName("id") val id: Long,
    @SerialName("number") val number: String,
    @SerialName("title") val title: String,
    @SerialName("partner_name") val partnerName: String,
    @SerialName("currency") val currency: String,
    @SerialName("total_minor") val totalMinor: Long,
    @SerialName("vehicle_plate") val vehiclePlate: String? = null,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("stage_key") val stageKey: String,
    @SerialName("stage_label") val stageLabel: String,
    @SerialName("stage_is_terminal") val stageIsTerminal: Boolean,
    @SerialName("open_blockers") val openBlockers: Long,
)

@Serializable
data class Order(
    @SerialName("id") val id: Long,
    @SerialName("number") val number: String,
    @SerialName("title") val title: String,
    @SerialName("partner_id") val partnerId: Long,
    @SerialName("project_type_id") val projectTypeId: Long? = null,
    @SerialName("currency") val currency: String,
    @SerialName("valuation_date") val valuationDate: String,
    @SerialName("vehicle_make") val vehicleMake: String? = null,
    @SerialName("vehicle_model") val vehicleModel: String? = null,
    @SerialName("vehicle_plate") val vehiclePlate: String? = null,
    @SerialName("vehicle_vin") val vehicleVin: String? = null,
    @SerialName("description") val description: String? = null,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("assigned_to") val assignedTo: Long? = null,
    @SerialName("minicrm_id") val minicrmId: Long? = null,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class PartnerRef(
    @SerialName("id") val id: Long,
    @SerialName("name") val name: String,
)

@Serializable
data class StageView(
    @SerialName("key") val key: String,
    @SerialName("label_hu") val labelHu: String,
    @SerialName("entered_at") val enteredAt: String,
    @SerialName("days_in_stage") val daysInStage: Long,
    @SerialName("is_terminal") val isTerminal: Boolean,
)

@Serializable
data class OrderValue(
    @SerialName("currency") val currency: String,
    @SerialName("total_minor") val totalMinor: Long,
    @SerialName("valuation_date") val valuationDate: String,
    @SerialName("total_huf_minor") val totalHufMinor: Long? = null,
)

@Serializable
data class ItemView(
    @SerialName("id") val id: Long,
    @SerialName("description") val description: String,
    @SerialName("quantity") val quantity: String,
    @SerialName("unit_price") val unitPrice: Long,
    @SerialName("currency") val currency: String,
    @SerialName("line_total_minor") val lineTotalMinor: Long,
)

@Serializable
data class Blocker(
    @SerialName("id") val id: Long,
    @SerialName("order_id") val orderId: Long,
    @SerialName("order_number") val orderNumber: String,
    @SerialName("what") val what: String,
    @SerialName("responsible_partner_name") val responsiblePartnerName: String? = null,
    @SerialName("responsible_email") val responsibleEmail: String? = null,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("notes") val notes: String? = null,
    @SerialName("nudge_count") val nudgeCount: Int,
    @SerialName("resolved_at") val resolvedAt: String? = null,
    @SerialName("resolution_note") val resolutionNote: String? = null,
    @SerialName("is_overdue") val isOverdue: Boolean,
)

@Serializable
data class Vehicle(
    @SerialName("id") val id: Long,
    @SerialName("vin") val vin: String? = null,
    @SerialName("plate") val plate: String? = null,
    @SerialName("make") val make: String? = null,
    @SerialName("model") val model: String? = null,
    @SerialName("year") val year: Int? = null,
)

@Serializable
data class OrderSpec(
    @SerialName("order_id") val orderId: Long,
    @SerialName("form") val form: String,
    @SerialName("target_temp_c") val targetTempC: String? = null,
    @SerialName("insulation_mm") val insulationMm: Int? = null,
    @SerialName("cooling_unit_make") val coolingUnitMake: String? = null,
    @SerialName("cooling_unit_model") val coolingUnitModel: String? = null,
    @SerialName("atp_class") val atpClass: String? = null,
    @SerialName("compartments") val compartments: Int? = null,
    @SerialName("defrost") val defrost: String? = null,
    @SerialName("electric_standby") val electricStandby: Boolean? = null,
    @SerialName("heater_make") val heaterMake: String? = null,
    @SerialName("heater_model") val heaterModel: String? = null,
    @SerialName("heat_output_kw") val heatOutputKw: String? = null,
    @SerialName("fuel") val fuel: String? = null,
    @SerialName("thermostat") val thermostat: Boolean? = null,
    @SerialName("notes") val notes: String? = null,
)

@Serializable
data class RelatedOrder(
    @SerialName("id") val id: Long,
    @SerialName("number") val number: String,
    @SerialName("title") val title: String,
    @SerialName("relation") val relation: String,
)

@Serializable
data class OrderDetail(
    @SerialName("order") val order: Order,
    @SerialName("partner") val partner: PartnerRef,
    @SerialName("stage") val stage: StageView,
    @SerialName("items") val items: List<ItemView> = emptyList(),
    @SerialName("value") val value: OrderValue,
    @SerialName("blockers") val blockers: List<Blocker> = emptyList(),
    @SerialName("related") val related: RelatedOrder? = null,
    @SerialName("vehicles") val vehicles: List<Vehicle> = emptyList(),
    @SerialName("spec") val spec: OrderSpec? = null,
    @SerialName("image_counts") val imageCounts: Map<String, Long> = emptyMap(),
)

@Serializable
data class StageEntry(
    @SerialName("id") val id: Long,
    @SerialName("stage_key") val stageKey: String,
    @SerialName("label_hu") val labelHu: String,
    @SerialName("entered_at") val enteredAt: String,
    @SerialName("entered_by_name") val enteredByName: String? = null,
    @SerialName("note") val note: String? = null,
)

@Serializable
data class TransitionOption(
    @SerialName("stage_key") val stageKey: String,
    @SerialName("label_hu") val labelHu: String,
    /** False for targets no manual move can reach — a lead's `won` needs a conversion. */
    @SerialName("manual") val manual: Boolean,
    /** Backward moves and reopening a terminal stage need a note. */
    @SerialName("requires_note") val requiresNote: Boolean,
    /** False when an image gate between the current stage and this one is unmet. */
    @SerialName("gates_met") val gatesMet: Boolean,
) {
    val allowed: Boolean get() = manual && gatesMet

    /**
     * Why a move is refused, in the fitter's words. The server sends the facts rather than
     * a sentence, and the two cases mean very different things: a missing photo is
     * something the person holding the phone can fix in ten seconds.
     */
    val reason: String?
        get() = when {
            !gatesMet -> "Hiányzik a kötelező fotó ehhez a fázishoz."
            !manual -> "Ez a fázis nem érhető el kézi váltással."
            else -> null
        }
}

@Serializable
data class StageBody(
    @SerialName("stage") val stage: String,
    @SerialName("note") val note: String? = null,
)

@Serializable
data class OrderNote(
    @SerialName("id") val id: Long,
    @SerialName("author_name") val authorName: String? = null,
    @SerialName("body") val body: String,
    @SerialName("occurred_at") val occurredAt: String,
)

// ── Media ───────────────────────────────────────────────────────────────────────────

@Serializable
data class ImageView(
    @SerialName("id") val id: Long,
    @SerialName("category") val category: String,
    @SerialName("thumb_url") val thumbUrl: String? = null,
    @SerialName("display_url") val displayUrl: String? = null,
    @SerialName("captured_at") val capturedAt: String? = null,
    @SerialName("uploaded_at") val uploadedAt: String,
    @SerialName("immutable") val immutable: Boolean,
)

@Serializable
data class UploadTarget(
    @SerialName("type") val type: String,
    @SerialName("category") val category: String? = null,
    @SerialName("kind") val kind: String? = null,
)

@Serializable
data class UploadRequest(
    @SerialName("target") val target: UploadTarget,
    @SerialName("filename") val filename: String? = null,
    @SerialName("content_type") val contentType: String,
    @SerialName("byte_size") val byteSize: Long,
    @SerialName("sha256") val sha256: String,
)

@Serializable
data class PresignedRequest(
    @SerialName("method") val method: String,
    @SerialName("url") val url: String,
    /** Every header here is part of the signature: send them all, add nothing. */
    @SerialName("headers") val headers: Map<String, String> = emptyMap(),
)

/**
 * `status` is the discriminator: `upload` means do the PUT, `already_uploaded` means this
 * exact file is already on the order and there is nothing to do. The second case is what
 * makes retrying a whole batch free.
 */
@Serializable
data class UploadResponse(
    @SerialName("status") val status: String,
    @SerialName("ticket") val ticket: String? = null,
    @SerialName("upload") val upload: PresignedRequest? = null,
    @SerialName("expires_at") val expiresAt: String? = null,
    @SerialName("image_id") val imageId: Long? = null,
    @SerialName("document_id") val documentId: Long? = null,
)

@Serializable
data class CompleteBody(@SerialName("ticket") val ticket: String)

@Serializable
data class Completed(
    @SerialName("type") val type: String,
    @SerialName("image") val image: ImageView? = null,
    @SerialName("created") val created: Boolean = false,
)

// ── Email ───────────────────────────────────────────────────────────────────────────

@Serializable
data class EmailSummary(
    @SerialName("id") val id: Long,
    @SerialName("order_id") val orderId: Long? = null,
    @SerialName("trigger") val trigger: String,
    @SerialName("is_automatic") val isAutomatic: Boolean,
    @SerialName("sent_by_name") val sentByName: String? = null,
    @SerialName("to_address") val toAddress: String,
    @SerialName("subject") val subject: String,
    @SerialName("status") val status: String,
    @SerialName("error") val error: String? = null,
    @SerialName("queued_at") val queuedAt: String,
    @SerialName("sent_at") val sentAt: String? = null,
)

@Serializable
data class AttachmentRef(
    @SerialName("document_id") val documentId: Long,
    @SerialName("filename") val filename: String? = null,
    @SerialName("byte_size") val byteSize: Long? = null,
)

@Serializable
data class EmailMessage(
    @SerialName("id") val id: Long,
    @SerialName("order_id") val orderId: Long? = null,
    @SerialName("lead_id") val leadId: Long? = null,
    @SerialName("trigger") val trigger: String,
    @SerialName("is_automatic") val isAutomatic: Boolean,
    @SerialName("to_address") val toAddress: String,
    @SerialName("cc") val cc: List<String> = emptyList(),
    @SerialName("from_address") val fromAddress: String,
    @SerialName("subject") val subject: String,
    /** The text part, not the HTML: it is what was actually sent, and it reads on a phone. */
    @SerialName("body_text") val bodyText: String,
    @SerialName("attachments") val attachments: List<AttachmentRef> = emptyList(),
    @SerialName("status") val status: String,
    @SerialName("error") val error: String? = null,
    @SerialName("attempts") val attempts: Int,
    @SerialName("queued_at") val queuedAt: String,
    @SerialName("sent_at") val sentAt: String? = null,
)

// ── Partners and leads ──────────────────────────────────────────────────────────────

@Serializable
data class Partner(
    @SerialName("id") val id: Long,
    @SerialName("kind") val kind: String,
    @SerialName("name") val name: String,
    @SerialName("tax_number") val taxNumber: String? = null,
    @SerialName("country") val country: String,
    @SerialName("email") val email: String? = null,
    @SerialName("phone") val phone: String? = null,
    @SerialName("city") val city: String? = null,
    @SerialName("address_line") val addressLine: String? = null,
    @SerialName("role") val role: String? = null,
    @SerialName("archived_at") val archivedAt: String? = null,
)

@Serializable
data class Contact(
    @SerialName("id") val id: Long,
    @SerialName("name") val name: String,
    @SerialName("email") val email: String? = null,
    @SerialName("phone") val phone: String? = null,
    @SerialName("position") val position: String? = null,
)

@Serializable
data class PartnerDetail(
    @SerialName("partner") val partner: Partner,
    @SerialName("contacts") val contacts: List<Contact> = emptyList(),
    @SerialName("orders") val orders: List<OrderSummary> = emptyList(),
    @SerialName("leads") val leads: List<LeadSummary> = emptyList(),
)

@Serializable
data class LeadSummary(
    @SerialName("id") val id: Long,
    @SerialName("title") val title: String,
    @SerialName("partner_name") val partnerName: String? = null,
    @SerialName("contact_name") val contactName: String? = null,
    @SerialName("stage_key") val stageKey: String,
    @SerialName("stage_label") val stageLabel: String,
    @SerialName("order_number") val orderNumber: String? = null,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class Lead(
    @SerialName("id") val id: Long,
    @SerialName("title") val title: String,
    @SerialName("partner_id") val partnerId: Long? = null,
    @SerialName("contact_name") val contactName: String? = null,
    @SerialName("contact_email") val contactEmail: String? = null,
    @SerialName("contact_phone") val contactPhone: String? = null,
    @SerialName("source") val source: String? = null,
    @SerialName("description") val description: String? = null,
    @SerialName("quoted_value_minor") val quotedValueMinor: Long? = null,
    @SerialName("currency") val currency: String? = null,
    @SerialName("quote_valid_until") val quoteValidUntil: String? = null,
)

@Serializable
data class CurrentStage(
    @SerialName("stage_key") val stageKey: String,
    @SerialName("entered_at") val enteredAt: String,
)

@Serializable
data class OrderRef(
    @SerialName("id") val id: Long,
    @SerialName("number") val number: String,
)

@Serializable
data class LeadDetail(
    @SerialName("lead") val lead: Lead,
    @SerialName("stage") val stage: CurrentStage? = null,
    @SerialName("history") val history: List<StageEntry> = emptyList(),
    @SerialName("orders") val orders: List<OrderRef> = emptyList(),
)

// ── Configuration ───────────────────────────────────────────────────────────────────

@Serializable
data class StageDefinition(
    @SerialName("id") val id: Long,
    @SerialName("entity") val entity: String,
    @SerialName("key") val key: String,
    @SerialName("label_hu") val labelHu: String,
    @SerialName("position") val position: Int,
    @SerialName("min_images") val minImages: Int,
    @SerialName("required_image_category") val requiredImageCategory: String? = null,
    @SerialName("is_terminal") val isTerminal: Boolean,
    @SerialName("is_exit") val isExit: Boolean,
    @SerialName("is_active") val isActive: Boolean,
)

@Serializable
data class ProjectType(
    @SerialName("id") val id: Long,
    @SerialName("key") val key: String,
    @SerialName("label_hu") val labelHu: String,
    @SerialName("is_active") val isActive: Boolean,
    @SerialName("spec_form") val specForm: String? = null,
)

/** The API's error envelope. `details` shape varies by code, so it stays raw JSON. */
@Serializable
data class ApiErrorBody(
    @SerialName("error") val error: String,
    @SerialName("message") val message: String? = null,
    @SerialName("details") val details: JsonElement? = null,
)
