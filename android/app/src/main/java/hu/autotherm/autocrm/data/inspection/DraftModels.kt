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
)

val DAMAGE_TYPES = listOf(
    "scratch" to "Karcolás",
    "dent" to "Horpadás",
    "crack" to "Repedés",
    "chip" to "Lepattanás",
    "broken" to "Törött alkatrész",
    "missing" to "Hiányzó alkatrész",
    "stain" to "Folt",
    "tear" to "Szakadás",
    "other" to "Egyéb",
)

val SEVERITIES = listOf(
    "minor" to "Enyhe",
    "moderate" to "Közepes",
    "severe" to "Súlyos",
)

fun damageTypeLabel(type: String): String = DAMAGE_TYPES.firstOrNull { it.first == type }?.second ?: type
fun severityLabel(severity: String): String = SEVERITIES.firstOrNull { it.first == severity }?.second ?: severity

/** Hungarian zone titles for keys the server does not label (instructions carry the detail). */
fun zoneTitle(key: String): String = when (key) {
    "front" -> "Elöl"
    "front_left" -> "Bal első sarok"
    "left_side" -> "Bal oldal"
    "rear_left" -> "Bal hátsó sarok"
    "rear" -> "Hátul"
    "rear_right" -> "Jobb hátsó sarok"
    "right_side" -> "Jobb oldal"
    "front_right" -> "Jobb első sarok"
    "roof" -> "Tető"
    "wheels" -> "Kerekek, gumik"
    "glass" -> "Szélvédő, üvegek"
    "interior_front" -> "Belső: első ülések"
    "interior_rear" -> "Belső: hátsó ülések"
    "interior_dashboard" -> "Belső: műszerfal"
    "interior_boot" -> "Belső: csomagtartó"
    "cargo_box" -> "Rakodótér"
    "cargo_doors" -> "Rakodótér ajtók"
    "refrigeration_unit" -> "Hűtőaggregát"
    else -> key
}
