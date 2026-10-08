package hu.autotherm.autocrm.data.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

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
    /** The authenticator app's code, for an account with two-factor sign-in (0049). */
    @SerialName("totp_code") val totpCode: String? = null,
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
     * 422. The login flow routes to the change-password screen until it is false.
     */
    @SerialName("must_change_password") val mustChangePassword: Boolean,
    /** Whether the HR module is open to this user (admins, or granted by an admin). */
    @SerialName("hr_access") val hrAccess: Boolean = false,
    /** Everything this user may do: the role's defaults plus what an admin granted. */
    @SerialName("capabilities") val capabilities: List<String> = emptyList(),
)

@Serializable
data class ChangePasswordBody(
    @SerialName("current_password") val currentPassword: String,
    @SerialName("new_password") val newPassword: String,
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
    @SerialName("contact_id") val contactId: Long? = null,
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
    // Intake slip (átvételi lap). Leaving `intake` requires `mileage_in`; the rest is
    // optional but printed next to it (ORD-L6). All absent until recorded.
    @SerialName("mileage_in") val mileageIn: Int? = null,
    @SerialName("intake_condition") val intakeCondition: String? = null,
    @SerialName("fuel_level") val fuelLevel: String? = null,
    @SerialName("key_count") val keyCount: Int? = null,
    @SerialName("valuables_declared") val valuablesDeclared: Boolean? = null,
    @SerialName("valuables") val valuables: String? = null,
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
    @SerialName("nudge_count") val nudgeCount: Long,
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
    @SerialName("year") val year: Long? = null,
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
    /** Read off the unit's plate (or its barcode) on the phone (0049). */
    @SerialName("cooling_unit_serial") val coolingUnitSerial: String? = null,
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
    /** The category a new photo takes in the current stage (0048); null: production. */
    @SerialName("photo_category") val photoCategory: String? = null,
    @SerialName("vehicle_locations") val vehicleLocations: List<CurrentLocation> = emptyList(),
    @SerialName("comment_count") val commentCount: Long = 0,
    @SerialName("open_incidents") val openIncidents: Long = 0,
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
    /** Leads only: why it was lost, when moving to `lost` (an id from /lost-reasons). */
    @SerialName("lost_reason_id") val lostReasonId: Long? = null,
)

@Serializable
data class LostReason(
    @SerialName("id") val id: Long,
    @SerialName("label") val label: String,
    @SerialName("archived_at") val archivedAt: String? = null,
)

@Serializable
data class OrderNote(
    @SerialName("id") val id: Long,
    @SerialName("author_name") val authorName: String? = null,
    @SerialName("body") val body: String,
    @SerialName("occurred_at") val occurredAt: String,
)

// ── Media ───────────────────────────────────────────────────────────────────────────

/**
 * The image row (`#/components/schemas/Image`): what `complete` returns. No URLs —
 * decoding it as [ImageView] only worked because the URL fields default to null.
 */
@Serializable
data class Image(
    @SerialName("id") val id: Long,
    @SerialName("category") val category: String,
    @SerialName("captured_at") val capturedAt: String? = null,
    @SerialName("uploaded_at") val uploadedAt: String,
    @SerialName("immutable") val immutable: Boolean,
)

/** `#/components/schemas/ImageView`: the [Image] row plus presigned URLs, from `list_images`. */
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
    /** A new version of this document (0048). */
    @SerialName("replaces") val replaces: Long? = null,
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
    /**
     * `[name, value]` pairs, exactly as the API signs them. A JSON array, not an
     * object: decoding this as a Map broke every ticketed upload with
     * "expected start of the object, but had '['".
     */
    @SerialName("headers") val headerPairs: List<List<String>> = emptyList(),
) {
    /** Every header here is part of the signature: send them all, add nothing. */
    val headers: Map<String, String>
        get() = headerPairs.mapNotNull { pair ->
            if (pair.size >= 2) pair[0] to pair[1] else null
        }.toMap()
}

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
    @SerialName("image") val image: Image? = null,
    @SerialName("document") val document: CompletedDocument? = null,
    @SerialName("created") val created: Boolean = false,
)

@Serializable
data class CompletedDocument(
    @SerialName("id") val id: Long,
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
data class DownloadUrl(@SerialName("url") val url: String)

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
    @SerialName("attempts") val attempts: Long,
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
    @SerialName("default_currency") val defaultCurrency: String? = null,
    @SerialName("email") val email: String? = null,
    @SerialName("phone") val phone: String? = null,
    @SerialName("postal_code") val postalCode: String? = null,
    @SerialName("city") val city: String? = null,
    @SerialName("address_line") val addressLine: String? = null,
    @SerialName("role") val role: String? = null,
    @SerialName("archived_at") val archivedAt: String? = null,
    @SerialName("eu_tax_number") val euTaxNumber: String? = null,
    @SerialName("website") val website: String? = null,
    @SerialName("notes") val notes: String? = null,
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
    @SerialName("contact_id") val contactId: Long? = null,
    @SerialName("assigned_to") val assignedTo: Long? = null,
    @SerialName("contact_name") val contactName: String? = null,
    @SerialName("contact_email") val contactEmail: String? = null,
    @SerialName("contact_phone") val contactPhone: String? = null,
    @SerialName("source") val source: String? = null,
    @SerialName("source_detail") val sourceDetail: String? = null,
    @SerialName("description") val description: String? = null,
    @SerialName("quoted_value_minor") val quotedValueMinor: Long? = null,
    @SerialName("currency") val currency: String? = null,
    @SerialName("quote_valid_until") val quoteValidUntil: String? = null,
    /** An EUR quote's frozen MNB rate (decimal string) and its day. */
    @SerialName("quote_fx_rate") val quoteFxRate: String? = null,
    @SerialName("quote_fx_day") val quoteFxDay: String? = null,
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
    /** Where the lead came from; only leads the website filed have it. */
    @SerialName("attribution") val attribution: LeadAttribution? = null,
)

// ── Write bodies ────────────────────────────────────────────────────────────────
// PATCH semantics (backend): a field omitted is kept, explicit null clears it. The
// client's `explicitNulls = false` omits every null, so "clear" is expressed by
// simply not sending — clearing a field from the phone is not supported, by design.

@Serializable
data class PartnerBody(
    @SerialName("kind") val kind: String? = null,
    @SerialName("name") val name: String? = null,
    @SerialName("tax_number") val taxNumber: String? = null,
    /** Community VAT number (DE123456789) for EU customers. */
    @SerialName("eu_tax_number") val euTaxNumber: String? = null,
    /** ISO two letters; the server defaults to HU. */
    @SerialName("country") val country: String? = null,
    /** HUF or EUR; the server defaults to HUF. */
    @SerialName("default_currency") val defaultCurrency: String? = null,
    @SerialName("email") val email: String? = null,
    @SerialName("phone") val phone: String? = null,
    @SerialName("website") val website: String? = null,
    @SerialName("postal_code") val postalCode: String? = null,
    @SerialName("city") val city: String? = null,
    @SerialName("address_line") val addressLine: String? = null,
    @SerialName("notes") val notes: String? = null,
    @SerialName("role") val role: String? = null,
)

@Serializable
data class ContactBody(
    @SerialName("name") val name: String? = null,
    @SerialName("email") val email: String? = null,
    @SerialName("phone") val phone: String? = null,
    @SerialName("position") val position: String? = null,
    @SerialName("notes") val notes: String? = null,
)

@Serializable
data class LeadBody(
    @SerialName("title") val title: String? = null,
    @SerialName("partner_id") val partnerId: Long? = null,
    @SerialName("contact_id") val contactId: Long? = null,
    @SerialName("assigned_to") val assignedTo: Long? = null,
    @SerialName("contact_name") val contactName: String? = null,
    @SerialName("contact_email") val contactEmail: String? = null,
    @SerialName("contact_phone") val contactPhone: String? = null,
    @SerialName("source") val source: String? = null,
    @SerialName("source_detail") val sourceDetail: String? = null,
    @SerialName("description") val description: String? = null,
    @SerialName("quoted_value_minor") val quotedValueMinor: Long? = null,
    @SerialName("currency") val currency: String? = null,
    @SerialName("quote_valid_until") val quoteValidUntil: String? = null,
)

@Serializable
data class OrderBody(
    @SerialName("title") val title: String? = null,
    @SerialName("partner_id") val partnerId: Long? = null,
    @SerialName("contact_id") val contactId: Long? = null,
    @SerialName("assigned_to") val assignedTo: Long? = null,
    @SerialName("project_type_id") val projectTypeId: Long? = null,
    @SerialName("currency") val currency: String? = null,
    @SerialName("valuation_date") val valuationDate: String? = null,
    @SerialName("vehicle_make") val vehicleMake: String? = null,
    @SerialName("vehicle_model") val vehicleModel: String? = null,
    @SerialName("vehicle_plate") val vehiclePlate: String? = null,
    @SerialName("vehicle_vin") val vehicleVin: String? = null,
    @SerialName("description") val description: String? = null,
    @SerialName("due_date") val dueDate: String? = null,
)

@Serializable
data class TaskBody(
    @SerialName("entity_type") val entityType: String,
    @SerialName("entity_id") val entityId: Long,
    @SerialName("title") val title: String,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("assigned_to") val assignedTo: Long? = null,
)

@Serializable
data class Task(
    @SerialName("id") val id: Long,
    @SerialName("entity_type") val entityType: String,
    @SerialName("entity_id") val entityId: Long,
    @SerialName("title") val title: String,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("done_at") val doneAt: String? = null,
    @SerialName("assigned_name") val assignedName: String? = null,
) {
    val isDone: Boolean get() = doneAt != null
}

@Serializable
data class DoneBody(@SerialName("done") val done: Boolean)

@Serializable
data class BlockerBody(
    @SerialName("what") val what: String? = null,
    @SerialName("responsible_email") val responsibleEmail: String? = null,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("notes") val notes: String? = null,
)

@Serializable
data class ResolveBody(@SerialName("note") val note: String? = null)

@Serializable
data class AddItemBody(
    @SerialName("description") val description: String,
    @SerialName("quantity") val quantity: String,
    @SerialName("unit_price") val unitPrice: Long,
)

@Serializable
data class ComposeBody(
    @SerialName("order_id") val orderId: Long? = null,
    @SerialName("lead_id") val leadId: Long? = null,
    /** About the partner itself (not one job): it lands on the partner's timeline. */
    @SerialName("partner_id") val partnerId: Long? = null,
    @SerialName("to") val to: String,
    @SerialName("subject") val subject: String? = null,
    @SerialName("body") val body: String? = null,
)

// ── Reports ───────────────────────────────────────────────────────────────────

@Serializable
data class WorkloadDay(
    @SerialName("date") val date: String,
    @SerialName("placed") val placed: Long,
    @SerialName("completed") val completed: Long,
    @SerialName("in_workshop") val inWorkshop: Long,
)

@Serializable
data class WorkloadReport(
    @SerialName("from") val from: String,
    @SerialName("to") val to: String,
    @SerialName("days") val days: List<WorkloadDay> = emptyList(),
)

@Serializable
data class StalledOrder(
    @SerialName("order_id") val orderId: Long,
    @SerialName("number") val number: String,
    @SerialName("title") val title: String,
    @SerialName("partner_name") val partnerName: String,
    @SerialName("stage_key") val stageKey: String,
    @SerialName("stage_label") val stageLabel: String,
    @SerialName("days_in_stage") val daysInStage: Long,
    @SerialName("open_blockers") val openBlockers: Long = 0,
)

// ── Handover inspections (átadás-átvétel) ───────────────────────────────────────
// Order-linked check-out / check-in damage record. Created on the phone (guided
// walkaround), read everywhere. Photos ride the normal upload queue with category
// `inspection` and are attached with zone + purpose + capture metadata.

@Serializable
data class Inspection(
    @SerialName("id") val id: Long,
    @SerialName("order_id") val orderId: Long,
    @SerialName("kind") val kind: String,
    @SerialName("status") val status: String,
    @SerialName("vehicle_plate") val vehiclePlate: String,
    @SerialName("vehicle_vin") val vehicleVin: String? = null,
    @SerialName("inspector_name") val inspectorName: String,
    @SerialName("driver_name") val driverName: String? = null,
    @SerialName("location") val location: String? = null,
    @SerialName("odometer") val odometer: Int? = null,
    @SerialName("fuel_level") val fuelLevel: String? = null,
    @SerialName("battery_pct") val batteryPct: Int? = null,
    @SerialName("warning_lights") val warningLights: String? = null,
    @SerialName("checkout_id") val checkoutId: Long? = null,
    @SerialName("customer_comment") val customerComment: String? = null,
    @SerialName("signed_at") val signedAt: String? = null,
    @SerialName("created_by") val createdBy: Long,
    @SerialName("created_at") val createdAt: String,
    @SerialName("updated_at") val updatedAt: String,
)

@Serializable
data class InspectionPhoto(
    @SerialName("id") val id: Long,
    @SerialName("inspection_id") val inspectionId: Long,
    @SerialName("image_id") val imageId: Long,
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("purpose") val purpose: String,
    @SerialName("damage_id") val damageId: Long? = null,
    @SerialName("taken_at") val takenAt: String,
    @SerialName("lat") val lat: Double? = null,
    @SerialName("lon") val lon: Double? = null,
    @SerialName("created_at") val createdAt: String,
    @SerialName("thumb_url") val thumbUrl: String? = null,
    @SerialName("display_url") val displayUrl: String? = null,
)

@Serializable
data class InspectionDamage(
    @SerialName("id") val id: Long,
    @SerialName("inspection_id") val inspectionId: Long,
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("damage_type") val damageType: String,
    @SerialName("severity") val severity: String,
    @SerialName("note") val note: String? = null,
    @SerialName("x") val x: Double? = null,
    @SerialName("y") val y: Double? = null,
    @SerialName("view") val view: String,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class InspectionVerdict(
    @SerialName("id") val id: Long,
    @SerialName("checkin_id") val checkinId: Long,
    @SerialName("checkin_damage_id") val checkinDamageId: Long,
    @SerialName("checkout_damage_id") val checkoutDamageId: Long? = null,
    @SerialName("verdict") val verdict: String,
    @SerialName("note") val note: String? = null,
    @SerialName("reviewed_by") val reviewedBy: Long,
    @SerialName("reviewed_at") val reviewedAt: String,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class InspectionSignature(
    @SerialName("id") val id: Long,
    @SerialName("inspection_id") val inspectionId: Long,
    @SerialName("role") val role: String,
    @SerialName("name") val name: String,
    @SerialName("document_id") val documentId: Long,
    @SerialName("signed_at") val signedAt: String,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class InspectionNote(
    @SerialName("id") val id: Long,
    @SerialName("inspection_id") val inspectionId: Long,
    @SerialName("body") val body: String,
    @SerialName("created_by") val createdBy: Long,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class InspectionDetail(
    @SerialName("inspection") val inspection: Inspection,
    @SerialName("photos") val photos: List<InspectionPhoto> = emptyList(),
    @SerialName("damages") val damages: List<InspectionDamage> = emptyList(),
    @SerialName("verdicts") val verdicts: List<InspectionVerdict> = emptyList(),
    @SerialName("signatures") val signatures: List<InspectionSignature> = emptyList(),
    @SerialName("notes") val notes: List<InspectionNote> = emptyList(),
    /** Zone heading by zone key, from the list this inspection was walked with. */
    @SerialName("zone_titles") val zoneTitles: Map<String, String> = emptyMap(),
    @SerialName("tyres") val tyres: List<InspectionTyre> = emptyList(),
    @SerialName("videos") val videos: List<InspectionVideo> = emptyList(),
)

/** One tyre's tread (decimal string, mm) and condition (0048). */
@Serializable
data class InspectionTyre(
    @SerialName("position") val position: String,
    @SerialName("tread_mm") val treadMm: String? = null,
    @SerialName("condition") val condition: String,
    @SerialName("note") val note: String? = null,
)

@Serializable
data class TyresBody(@SerialName("tyres") val tyres: List<InspectionTyre>)

@Serializable
data class InspectionVideo(
    @SerialName("id") val id: Long,
    @SerialName("document_id") val documentId: Long,
    @SerialName("zone_key") val zoneKey: String? = null,
    @SerialName("duration_ms") val durationMs: Long? = null,
    @SerialName("taken_at") val takenAt: String,
    @SerialName("url") val url: String? = null,
)

@Serializable
data class VideoBody(
    @SerialName("document_id") val documentId: Long,
    @SerialName("zone_key") val zoneKey: String? = null,
    @SerialName("duration_ms") val durationMs: Long? = null,
    @SerialName("taken_at") val takenAt: String,
)

@Serializable
data class InspectionBody(
    @SerialName("order_id") val orderId: Long? = null,
    @SerialName("kind") val kind: String? = null,
    @SerialName("vehicle_plate") val vehiclePlate: String? = null,
    @SerialName("vehicle_vin") val vehicleVin: String? = null,
    @SerialName("inspector_name") val inspectorName: String? = null,
    @SerialName("driver_name") val driverName: String? = null,
    @SerialName("location") val location: String? = null,
    @SerialName("odometer") val odometer: Int? = null,
    @SerialName("fuel_level") val fuelLevel: String? = null,
    @SerialName("battery_pct") val batteryPct: Int? = null,
    @SerialName("warning_lights") val warningLights: String? = null,
    @SerialName("customer_comment") val customerComment: String? = null,
    // Idempotency key: the phone's stable local draft UUID. A retried create whose
    // first response was lost returns the already-created inspection (200) instead
    // of a second row or `checkout_open` (INSP-L10).
    @SerialName("client_key") val clientKey: String? = null,
)

@Serializable
data class AttachPhotoBody(
    @SerialName("image_id") val imageId: Long,
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("purpose") val purpose: String,
    @SerialName("damage_id") val damageId: Long? = null,
    @SerialName("taken_at") val takenAt: String,
    @SerialName("lat") val lat: Double? = null,
    @SerialName("lon") val lon: Double? = null,
)

@Serializable
data class DamageBody(
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("damage_type") val damageType: String,
    @SerialName("severity") val severity: String,
    @SerialName("note") val note: String? = null,
    @SerialName("x") val x: Double? = null,
    @SerialName("y") val y: Double? = null,
    @SerialName("view") val view: String = "top",
)

@Serializable
data class SignatureBody(
    @SerialName("role") val role: String,
    @SerialName("name") val name: String,
    @SerialName("document_id") val documentId: Long,
)

@Serializable
data class SignBody(
    @SerialName("customer_comment") val customerComment: String? = null,
)

@Serializable
data class VerdictBody(
    @SerialName("checkin_damage_id") val checkinDamageId: Long,
    @SerialName("checkout_damage_id") val checkoutDamageId: Long? = null,
    @SerialName("verdict") val verdict: String,
    @SerialName("note") val note: String? = null,
)

@Serializable
data class InspectionNoteBody(
    @SerialName("body") val body: String,
)

@Serializable
data class ZoneTemplate(
    @SerialName("id") val id: Long,
    @SerialName("set_key") val setKey: String,
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("position") val position: Long,
    @SerialName("instruction") val instruction: String,
    @SerialName("optional") val optional: Boolean,
    @SerialName("required") val required: Boolean,
    // Added with lists per vehicle kind. Defaults, because a draft saved on the phone before
    // them holds zones without these.
    /** The heading of the zone; `instruction` is the sentence under it. */
    @SerialName("title") val title: String = "",
    /** The project type that owns the list; null for the general list. */
    @SerialName("project_type_id") val projectTypeId: Long? = null,
    /** `checkout` (the first walkaround, átvétel) or `checkin` (the second, kiadás). */
    @SerialName("kind") val kind: String = "",
)

@Serializable
data class DamageSuggestion(
    @SerialName("checkin_damage_id") val checkinDamageId: Long,
    @SerialName("checkout_damage_id") val checkoutDamageId: Long? = null,
    @SerialName("suggested") val suggested: String,
)

// ── Client-facing enumerations ──────────────────────────────────────────────
// `GET /config/lookups`: every list the phone renders (damage types, fuel
// marks, currencies, image categories, ...) in one document. Nothing in it is
// duplicated in the app; unknown keys read as themselves (see lookupLabel).

@Serializable
data class LookupItem(
    @SerialName("key") val key: String,
    @SerialName("label_hu") val labelHu: String = "",
)

@Serializable
data class ImageCategoryEntry(
    @SerialName("key") val key: String,
    @SerialName("label_hu") val labelHu: String = "",
    @SerialName("immutable") val immutable: Boolean = false,
    @SerialName("attachable") val attachable: Boolean = false,
)

@Serializable
data class EmailThemeEntry(
    @SerialName("key") val key: String,
    @SerialName("label_hu") val labelHu: String = "",
    @SerialName("subject") val subject: String = "",
    @SerialName("hero") val hero: String = "",
    @SerialName("body") val body: String = "",
)

@Serializable
data class ErrorTextEntry(
    @SerialName("code") val code: String,
    @SerialName("text_hu") val textHu: String = "",
)

@Serializable
data class Lookups(
    @SerialName("damage_types") val damageTypes: List<LookupItem> = emptyList(),
    @SerialName("severities") val severities: List<LookupItem> = emptyList(),
    @SerialName("verdicts") val verdicts: List<LookupItem> = emptyList(),
    @SerialName("walkaround_kinds") val walkaroundKinds: List<LookupItem> = emptyList(),
    @SerialName("fuel_levels") val fuelLevels: List<LookupItem> = emptyList(),
    @SerialName("heating_fuels") val heatingFuels: List<LookupItem> = emptyList(),
    @SerialName("defrost_modes") val defrostModes: List<LookupItem> = emptyList(),
    @SerialName("order_relations") val orderRelations: List<LookupItem> = emptyList(),
    @SerialName("task_entity_types") val taskEntityTypes: List<LookupItem> = emptyList(),
    @SerialName("currencies") val currencies: List<LookupItem> = emptyList(),
    @SerialName("invoice_payment_methods") val invoicePaymentMethods: List<LookupItem> = emptyList(),
    @SerialName("annulment_codes") val annulmentCodes: List<LookupItem> = emptyList(),
    @SerialName("image_categories") val imageCategories: List<ImageCategoryEntry> = emptyList(),
    @SerialName("email_themes") val emailThemes: List<EmailThemeEntry> = emptyList(),
    @SerialName("error_texts") val errorTexts: List<ErrorTextEntry> = emptyList(),
    @SerialName("tyre_positions") val tyrePositions: List<LookupItem> = emptyList(),
    @SerialName("tyre_conditions") val tyreConditions: List<LookupItem> = emptyList(),
    @SerialName("yard_kinds") val yardKinds: List<LookupItem> = emptyList(),
)
@Serializable
data class Comparison(
    @SerialName("checkin") val checkin: InspectionDetail,
    @SerialName("checkout") val checkout: InspectionDetail,
    @SerialName("suggestions") val suggestions: List<DamageSuggestion> = emptyList(),
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

/** The API's error envelope (backend/src/error.rs): `{"error": {"code", "message"}}`. */
@Serializable
data class ApiErrorBody(
    @SerialName("error") val error: ErrorDetail,
)

@Serializable
data class ErrorDetail(
    @SerialName("code") val code: String,
    @SerialName("message") val message: String? = null,
)

// ── HR and users ────────────────────────────────────────────────────────────────────

@Serializable
data class Employee(
    @SerialName("id") val id: Long,
    @SerialName("full_name") val fullName: String,
    @SerialName("email") val email: String? = null,
    @SerialName("company_phone") val companyPhone: String? = null,
    @SerialName("personal_phone") val personalPhone: String? = null,
    /** A signed link that expires in about an hour; never stored. */
    @SerialName("photo_url") val photoUrl: String? = null,
    /** Paid annual leave per calendar year. */
    @SerialName("annual_leave_days") val annualLeaveDays: Int = 20,
    @SerialName("archived_at") val archivedAt: String? = null,
)

/** A staff account as the admin users list shows it. */
@Serializable
data class StaffUser(
    @SerialName("id") val id: Long,
    @SerialName("email") val email: String,
    @SerialName("display_name") val displayName: String,
    @SerialName("role") val role: String,
    @SerialName("is_active") val isActive: Boolean,
    @SerialName("hr_access") val hrAccess: Boolean = false,
)

/** One period of leave or absence, with working days already counted by the server. */
@Serializable
data class Absence(
    @SerialName("id") val id: Long,
    @SerialName("employee_id") val employeeId: Long,
    @SerialName("employee_name") val employeeName: String,
    /** `annual` | `sick` | `unpaid` | `other`. */
    @SerialName("kind") val kind: String,
    @SerialName("start_date") val startDate: String,
    @SerialName("end_date") val endDate: String,
    @SerialName("working_days") val workingDays: Int,
    @SerialName("note") val note: String? = null,
)

// ── Notifications ───────────────────────────────────────────────────────────────────

@Serializable
data class NotificationItem(
    @SerialName("id") val id: Long,
    @SerialName("kind") val kind: String,
    @SerialName("title") val title: String,
    @SerialName("body") val body: String? = null,
    /** An app path such as `/leads/42`; see `routeForLink`. */
    @SerialName("link") val link: String? = null,
    @SerialName("created_at") val createdAt: String,
    @SerialName("read_at") val readAt: String? = null,
)

@Serializable
data class NotificationFeed(
    /** Newest first. */
    @SerialName("items") val items: List<NotificationItem>,
    @SerialName("unread") val unread: Long,
)

@Serializable
data class MarkReadBody(@SerialName("ids") val ids: List<Long>)

/** What the website told us about where a lead came from. */
@Serializable
data class LeadAttribution(
    /** `paid` | `organic` | `social` | `email` | `referral` | `direct`. */
    @SerialName("channel") val channel: String,
    @SerialName("utm_source") val utmSource: String? = null,
    @SerialName("utm_medium") val utmMedium: String? = null,
    @SerialName("utm_campaign") val utmCampaign: String? = null,
    @SerialName("referrer") val referrer: String? = null,
    @SerialName("landing_page") val landingPage: String? = null,
)

// ── Search ──────────────────────────────────────────────────────────────────────────

@Serializable
data class OrderHit(
    @SerialName("id") val id: Long,
    @SerialName("number") val number: String,
    @SerialName("title") val title: String,
    @SerialName("plate") val plate: String? = null,
    @SerialName("stage_label") val stageLabel: String,
)

@Serializable
data class PartnerHit(
    @SerialName("id") val id: Long,
    @SerialName("name") val name: String,
    @SerialName("kind") val kind: String,
    @SerialName("city") val city: String? = null,
)

@Serializable
data class LeadHit(
    @SerialName("id") val id: Long,
    @SerialName("title") val title: String,
    @SerialName("contact_name") val contactName: String? = null,
    @SerialName("stage_label") val stageLabel: String,
)

@Serializable
data class ContactHit(
    @SerialName("id") val id: Long,
    @SerialName("partner_id") val partnerId: Long,
    @SerialName("name") val name: String,
    @SerialName("partner_name") val partnerName: String,
    @SerialName("email") val email: String? = null,
    @SerialName("phone") val phone: String? = null,
)

@Serializable
data class EmailHit(
    @SerialName("id") val id: Long,
    @SerialName("subject") val subject: String,
    @SerialName("to_address") val toAddress: String,
    @SerialName("status") val status: String,
)

@Serializable
data class EmployeeHit(
    @SerialName("id") val id: Long,
    @SerialName("full_name") val fullName: String,
    @SerialName("email") val email: String? = null,
    @SerialName("company_phone") val companyPhone: String? = null,
    @SerialName("archived") val archived: Boolean = false,
)

@Serializable
data class SearchResults(
    @SerialName("orders") val orders: List<OrderHit> = emptyList(),
    @SerialName("partners") val partners: List<PartnerHit> = emptyList(),
    @SerialName("leads") val leads: List<LeadHit> = emptyList(),
    @SerialName("contacts") val contacts: List<ContactHit> = emptyList(),
    @SerialName("emails") val emails: List<EmailHit> = emptyList(),
    /** Empty unless the caller has HR access. */
    @SerialName("employees") val employees: List<EmployeeHit> = emptyList(),
)

// ── 0048: lead sources, comments, the yard ─────────────────────────────────────

@Serializable
data class LeadSource(
    @SerialName("key") val key: String,
    @SerialName("label") val label: String,
    @SerialName("is_system") val isSystem: Boolean = false,
    @SerialName("archived_at") val archivedAt: String? = null,
)

@Serializable
data class Mention(
    @SerialName("user_id") val userId: Long,
    @SerialName("name") val name: String,
)

@Serializable
data class Comment(
    @SerialName("id") val id: Long,
    @SerialName("body") val body: String,
    @SerialName("created_by") val createdBy: Long,
    @SerialName("author_name") val authorName: String,
    @SerialName("created_at") val createdAt: String,
    @SerialName("edited_at") val editedAt: String? = null,
    @SerialName("mentions") val mentions: List<Mention> = emptyList(),
)

@Serializable
data class CommentBody(
    @SerialName("entity_type") val entityType: String,
    @SerialName("entity_id") val entityId: Long,
    @SerialName("body") val body: String,
    @SerialName("mention_ids") val mentionIds: List<Long> = emptyList(),
)

@Serializable
data class Mentionable(
    @SerialName("id") val id: Long,
    @SerialName("display_name") val displayName: String,
)

@Serializable
data class YardLocation(
    @SerialName("id") val id: Long,
    @SerialName("name") val name: String,
    @SerialName("kind") val kind: String,
    @SerialName("capacity") val capacity: Int? = null,
)

@Serializable
data class CurrentLocation(
    @SerialName("vehicle_id") val vehicleId: Long,
    @SerialName("location_id") val locationId: Long? = null,
    @SerialName("location_name") val locationName: String? = null,
    @SerialName("moved_at") val movedAt: String,
    @SerialName("moved_by_name") val movedByName: String? = null,
)

@Serializable
data class YardVehicle(
    @SerialName("vehicle_id") val vehicleId: Long,
    @SerialName("plate") val plate: String? = null,
    @SerialName("vin") val vin: String? = null,
    @SerialName("make") val make: String? = null,
    @SerialName("model") val model: String? = null,
    @SerialName("order_id") val orderId: Long? = null,
    @SerialName("order_number") val orderNumber: String? = null,
    @SerialName("partner_name") val partnerName: String? = null,
    @SerialName("stage_label") val stageLabel: String? = null,
    @SerialName("location_id") val locationId: Long? = null,
)

@Serializable
data class YardBoard(
    @SerialName("locations") val locations: List<YardLocation> = emptyList(),
    @SerialName("vehicles") val vehicles: List<YardVehicle> = emptyList(),
)

/** Explicit null for `location_id` means "left the site", so it is always sent. */
@Serializable
data class MoveBody(
    @SerialName("vehicle_id") val vehicleId: Long,
    @SerialName("location_id") val locationId: Long?,
    @SerialName("order_id") val orderId: Long? = null,
)

@Serializable
data class MoveResult(
    @SerialName("id") val id: Long,
    @SerialName("over_capacity") val overCapacity: Boolean = false,
)

// ── 0049 ──────────────────────────────────────────────────────────────────────────

@Serializable
data class CoolingSerialBody(
    @SerialName("serial") val serial: String?,
)

/** What the VIN itself says (GET /vehicles/decode/{vin}). */
@Serializable
data class VinInfo(
    @SerialName("vin") val vin: String,
    @SerialName("check_digit_ok") val checkDigitOk: Boolean = false,
    @SerialName("manufacturer") val manufacturer: String? = null,
    @SerialName("region") val region: String? = null,
    @SerialName("model_year") val modelYear: Int? = null,
    @SerialName("make") val make: String? = null,
    @SerialName("model") val model: String? = null,
)
