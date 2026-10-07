package hu.autotherm.autocrm.data.inspection

import hu.autotherm.autocrm.data.api.ZoneTemplate
import java.util.UUID
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The whole walkaround as the phone owns it: readings, zone order, photo
 * references by content hash, damages with local ids, verdicts, signatures.
 * Serialised into `inspection_drafts.payload_json` after every mutation, so a
 * dead battery mid-zone resumes exactly where the walkaround stopped.
 *
 * Server ids fill in during sync (`serverId` / `attachedPhotoId` / ...); a null
 * means "not on the server yet".
 */
@Serializable
data class DraftPhoto(
    @SerialName("local_id") val localId: String = UUID.randomUUID().toString(),
    /** Matches `pending_uploads.sha256`: the bytes ride the normal queue. */
    @SerialName("sha256") val sha256: String,
    @SerialName("file_name") val fileName: String,
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("purpose") val purpose: String,
    @SerialName("damage_local_id") val damageLocalId: String? = null,
    @SerialName("taken_at") val takenAt: String,
    @SerialName("lat") val lat: Double? = null,
    @SerialName("lon") val lon: Double? = null,
    @SerialName("attached_photo_id") val attachedPhotoId: Long? = null,
)

@Serializable
data class DraftDamage(
    @SerialName("local_id") val localId: String = UUID.randomUUID().toString(),
    @SerialName("zone_key") val zoneKey: String,
    @SerialName("damage_type") val damageType: String,
    @SerialName("severity") val severity: String,
    @SerialName("note") val note: String? = null,
    @SerialName("x") val x: Double? = null,
    @SerialName("y") val y: Double? = null,
    @SerialName("view") val view: String = "top",
    @SerialName("server_id") val serverId: Long? = null,
)

@Serializable
data class DraftSignature(
    @SerialName("role") val role: String,
    @SerialName("name") val name: String = "",
    /** PNG in the draft directory, drawn on the signature pad. */
    @SerialName("file_name") val fileName: String? = null,
    @SerialName("server_document_id") val serverDocumentId: Long? = null,
)

@Serializable
data class DraftVerdict(
    @SerialName("damage_local_id") val damageLocalId: String,
    @SerialName("checkout_damage_id") val checkoutDamageId: Long? = null,
    @SerialName("verdict") val verdict: String,
    @SerialName("note") val note: String? = null,
)

/** One tyre's reading (0048). Recorded only for the positions the inspector touched. */
@Serializable
data class DraftTyre(
    @SerialName("position") val position: String,
    /** Millimetres as typed; parsed at sync. */
    @SerialName("tread_mm") val treadMm: String = "",
    @SerialName("condition") val condition: String = "ok",
    @SerialName("note") val note: String = "",
)

/** A walkaround clip (0048): bytes in the draft directory, uploaded at sync as a video document. */
@Serializable
data class DraftVideo(
    @SerialName("local_id") val localId: String = UUID.randomUUID().toString(),
    @SerialName("file_name") val fileName: String,
    @SerialName("zone_key") val zoneKey: String? = null,
    @SerialName("duration_ms") val durationMs: Long? = null,
    @SerialName("taken_at") val takenAt: String,
    @SerialName("server_document_id") val serverDocumentId: Long? = null,
    @SerialName("attached") val attached: Boolean = false,
)

@Serializable
data class DraftPayload(
    @SerialName("vehicle_plate") val vehiclePlate: String,
    @SerialName("vehicle_vin") val vehicleVin: String? = null,
    @SerialName("inspector_name") val inspectorName: String = "",
    @SerialName("driver_name") val driverName: String = "",
    @SerialName("location") val location: String = "",
    @SerialName("odometer") val odometer: String = "",
    @SerialName("fuel_level") val fuelLevel: String? = null,
    @SerialName("battery_pct") val batteryPct: String = "",
    @SerialName("warning_lights") val warningLights: String = "",
    @SerialName("customer_comment") val customerComment: String = "",
    @SerialName("checkout_server_id") val checkoutServerId: Long? = null,
    @SerialName("templates") val templates: List<ZoneTemplate> = emptyList(),
    @SerialName("zone_index") val zoneIndex: Int = 0,
    @SerialName("readings_done") val readingsDone: Boolean = false,
    @SerialName("photos") val photos: List<DraftPhoto> = emptyList(),
    @SerialName("damages") val damages: List<DraftDamage> = emptyList(),
    @SerialName("signatures") val signatures: List<DraftSignature> = listOf(
        DraftSignature("inspector"),
        DraftSignature("customer"),
    ),
    @SerialName("verdicts") val verdicts: List<DraftVerdict> = emptyList(),
    @SerialName("signed") val signed: Boolean = false,
    @SerialName("tyres") val tyres: List<DraftTyre> = emptyList(),
    @SerialName("tyres_synced") val tyresSynced: Boolean = false,
    @SerialName("videos") val videos: List<DraftVideo> = emptyList(),
)

/**
 * Damage-type, severity and verdict labels come from the server's lookups
 * (`GET /config/lookups`); see `damageTypeLabel`, `severityLabel` and
 * `verdictLabel` in Lookups.kt. Nothing about which values exist is kept here.
 */

/**
 * What the two walkarounds are called: `checkout` is the first (átvétel, the vehicle
 * arriving), `checkin` the second (kiadás, leaving). The keys are API values and stay.
 */
fun inspectionKindLabel(kind: String): String = if (kind == "checkin") "Kiadás" else "Átvétel"

/**
 * A zone's heading comes from the server: the list's own `title`. A key with no title (a draft
 * saved by an older build, or a zone removed from the list since) reads as the key itself,
 * spaced out: nothing about which zones exist is kept in the app.
 */
fun humanizeZoneKey(key: String): String =
    key.replace('_', ' ').trim().replaceFirstChar { it.uppercase() }

fun ZoneTemplate.displayTitle(): String = title.ifBlank { humanizeZoneKey(zoneKey) }

/** Headings of the zones of a draft's frozen list. */
fun zoneTitle(key: String, templates: List<ZoneTemplate>): String =
    templates.firstOrNull { it.zoneKey == key }?.displayTitle() ?: humanizeZoneKey(key)

/** Headings the server sent with an inspection (`zone_titles`). */
fun zoneTitle(key: String, titles: Map<String, String>): String =
    titles[key]?.takeIf { it.isNotBlank() } ?: humanizeZoneKey(key)
