package hu.autotherm.autocrm.data.inspection

import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AttachPhotoBody
import hu.autotherm.autocrm.data.api.DamageBody
import hu.autotherm.autocrm.data.api.InspectionBody
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.api.SignatureBody
import hu.autotherm.autocrm.data.api.VerdictBody
import hu.autotherm.autocrm.data.db.InspectionDraft
import hu.autotherm.autocrm.data.upload.Uploader
import hu.autotherm.autocrm.data.upload.sha256Hex
import java.io.File
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.Request
import okhttp3.RequestBody.Companion.asRequestBody

private val json = Json { ignoreUnknownKeys = true }

/** Draft directory: photo JPEGs and signature PNGs, app-private, never in the gallery. */
fun inspectionDir(app: AutoCrmApp, uuid: String): File =
    File(app.filesDir, "inspections/$uuid").apply { mkdirs() }

sealed class SyncResult {
    /** On the server and signed: the local draft is gone. */
    data object Done : SyncResult()
    /** No signal or a half-drained queue: try again later, nothing recorded. */
    data object Retry : SyncResult()
    /** Will not succeed on its own (validation, conflict): recorded on the draft. */
    data class Failed(val message: String) : SyncResult()
}

/**
 * Pushes one draft to the server, in dependency order: inspection → damages →
 * photos (bytes already rode the queue) → signatures → verdicts → sign.
 * Every step is idempotent, so a dead battery mid-sync resumes cleanly.
 */
suspend fun syncDraft(app: AutoCrmApp, uuid: String): SyncResult {
    val draftDao = app.database.inspectionDrafts()
    val uploadDao = app.database.pendingUploads()
    val draft = draftDao.byUuid(uuid) ?: return SyncResult.Done
    if (draft.state == InspectionDraft.STATE_SYNCED) return SyncResult.Done
    draftDao.setState(uuid, InspectionDraft.STATE_SYNCING)

    val payload = try {
        json.decodeFromString(DraftPayload.serializer(), draft.payloadJson)
    } catch (e: Exception) {
        draftDao.setState(uuid, InspectionDraft.STATE_DRAFT, "sérült piszkozat")
        return SyncResult.Failed("sérült piszkozat")
    }

    try {
        var serverId = draft.serverId
        var current = payload
        if (serverId == null) {
            // A check-in started offline has no verified link yet: make sure a
            // signed check-out exists before creating it (the server also
            // enforces this, but its error would land mid-sync).
            if (draft.kind == "checkin" && current.checkoutServerId == null) {
                val hasCheckout = app.api.inspections(draft.orderId)
                    .any { it.kind == "checkout" && it.status == "signed" }
                if (!hasCheckout) {
                    return fail(app, uuid, "nincs lezárt átadás ehhez az összehasonlításhoz")
                }
            }
            val created = app.api.createInspection(
                InspectionBody(
                    orderId = draft.orderId,
                    kind = draft.kind,
                    vehiclePlate = current.vehiclePlate,
                    vehicleVin = current.vehicleVin?.takeIf { it.isNotBlank() },
                    inspectorName = current.inspectorName,
                    driverName = current.driverName.takeIf { it.isNotBlank() },
                    location = current.location.takeIf { it.isNotBlank() },
                    odometer = current.odometer.toIntOrNull(),
                    fuelLevel = current.fuelLevel,
                    batteryPct = current.batteryPct.toIntOrNull(),
                    warningLights = current.warningLights.takeIf { it.isNotBlank() },
                ),
            )
            serverId = created.id
            draftDao.setServerId(uuid, serverId)
        }
        val id = serverId

        // Damages first: close-up photos reference them.
        for (damage in current.damages.filter { it.serverId == null }) {
            val created = app.api.addInspectionDamage(
                id,
                DamageBody(
                    zoneKey = damage.zoneKey,
                    damageType = damage.damageType,
                    severity = damage.severity,
                    note = damage.note?.takeIf { it.isNotBlank() },
                    x = damage.x,
                    y = damage.y,
                    view = damage.view,
                ),
            )
            current = current.copy(
                damages = current.damages.map {
                    if (it.localId == damage.localId) it.copy(serverId = created.id) else it
                },
            )
            persist(app, uuid, current)
        }

        // Photos: bytes ride pending_uploads; attach once the server confirms them.
        for (photo in current.photos.filter { it.attachedPhotoId == null }) {
            var row = uploadDao.byContent(draft.orderId, photo.sha256)
            if (row == null) {
                val file = File(inspectionDir(app, uuid), photo.fileName)
                if (!file.exists()) {
                    return fail(app, uuid, "hiányzó fotófájl a telefonon")
                }
                uploadDao.insert(
                    PendingUpload(
                        orderId = draft.orderId,
                        orderNumber = draft.orderNumber,
                        category = "inspection",
                        filePath = file.absolutePath,
                        contentType = "image/jpeg",
                        byteSize = file.length(),
                        sha256 = photo.sha256,
                        filename = photo.fileName,
                    ),
                )
                row = uploadDao.byContent(draft.orderId, photo.sha256)
                    ?: return SyncResult.Retry
            }
            var imageId = row.uploadedImageId
            if (imageId == null) {
                when (Uploader(app.api, uploadDao).upload(row)) {
                    is Uploader.Outcome.Uploaded -> {
                        imageId = uploadDao.byContent(draft.orderId, photo.sha256)?.uploadedImageId
                    }
                    is Uploader.Outcome.Blocked -> {
                        return fail(app, uuid, "a fotó feltöltése meghiúsult")
                    }
                    is Uploader.Outcome.Retry -> return SyncResult.Retry
                }
            }
            val finalImageId = imageId ?: return SyncResult.Retry
            val damageServerId = photo.damageLocalId?.let { local ->
                current.damages.firstOrNull { it.localId == local }?.serverId
            }
            try {
                val attached = app.api.attachInspectionPhoto(
                    id,
                    AttachPhotoBody(
                        imageId = finalImageId,
                        zoneKey = photo.zoneKey,
                        purpose = photo.purpose,
                        damageId = damageServerId,
                        takenAt = photo.takenAt,
                        lat = photo.lat,
                        lon = photo.lon,
                    ),
                )
                current = current.copy(
                    photos = current.photos.map {
                        if (it.localId == photo.localId) it.copy(attachedPhotoId = attached.id) else it
                    },
                )
                persist(app, uuid, current)
                // Bytes are on the server and attached: free the queue row. The file
                // stays for the on-device review until the draft is swept.
                uploadDao.delete(row.id)
            } catch (e: ApiException.Rule) {
                if (e.code == "already_attached") {
                    // A previous sync died between attach and persist: recover the id.
                    val detail = app.api.inspection(id)
                    val found = detail.photos.firstOrNull { it.imageId == finalImageId }
                    current = current.copy(
                        photos = current.photos.map {
                            if (it.localId == photo.localId) it.copy(attachedPhotoId = found?.id) else it
                        },
                    )
                    persist(app, uuid, current)
                    uploadDao.delete(row.id)
                } else throw e
            }
        }

        // Signatures: finger-drawn PNGs, uploaded as `other` documents.
        for (sig in current.signatures.filter { it.fileName != null && it.serverDocumentId == null }) {
            val file = File(inspectionDir(app, uuid), sig.fileName!!)
            if (!file.exists()) return fail(app, uuid, "hiányzó aláírásfájl")
            val documentId = uploadDocument(app, draft.orderId, file, sig.fileName!!)
                ?: return SyncResult.Retry
            app.api.addInspectionSignature(
                id,
                SignatureBody(role = sig.role, name = sig.name, documentId = documentId),
            )
            current = current.copy(
                signatures = current.signatures.map {
                    if (it.role == sig.role) it.copy(serverDocumentId = documentId) else it
                },
            )
            persist(app, uuid, current)
        }

        // Check-in verdicts, then the lock.
        for (verdict in current.verdicts) {
            val damageServerId = current.damages
                .firstOrNull { it.localId == verdict.damageLocalId }?.serverId
                ?: continue
            // Already-reviewed verdicts are idempotent server-side (upsert).
            app.api.setInspectionVerdict(
                id,
                VerdictBody(
                    checkinDamageId = damageServerId,
                    checkoutDamageId = verdict.checkoutDamageId,
                    verdict = verdict.verdict,
                    note = verdict.note?.takeIf { it.isNotBlank() },
                ),
            )
        }

        app.api.signInspection(id, current.customerComment.takeIf { it.isNotBlank() })

        // Signed and stored: the phone's copy is redundant. Server history is the record.
        draftDao.delete(uuid)
        inspectionDir(app, uuid).deleteRecursively()
        return SyncResult.Done
    } catch (e: ApiException) {
        return if (e.isRetryable) {
            draftDao.setState(uuid, InspectionDraft.STATE_DRAFT)
            SyncResult.Retry
        } else {
            fail(app, uuid, e.message ?: "szinkronizálási hiba")
        }
    } catch (e: IOException) {
        draftDao.setState(uuid, InspectionDraft.STATE_DRAFT)
        return SyncResult.Retry
    }
}

private suspend fun persist(app: AutoCrmApp, uuid: String, payload: DraftPayload) {
    app.database.inspectionDrafts()
        .setPayload(uuid, json.encodeToString(DraftPayload.serializer(), payload))
}

private suspend fun fail(app: AutoCrmApp, uuid: String, message: String): SyncResult {
    app.database.inspectionDrafts().setState(uuid, InspectionDraft.STATE_DRAFT, message)
    return SyncResult.Failed(message)
}

/** Direct three-step document upload (signatures skip the photo queue). */
private suspend fun uploadDocument(
    app: AutoCrmApp,
    orderId: Long,
    file: File,
    filename: String,
): Long? {
    val sha = file.sha256Hex()
    val response = app.api.requestDocumentUpload(
        orderId = orderId,
        filename = filename,
        contentType = "image/png",
        byteSize = file.length(),
        sha256 = sha,
    )
    when (response.status) {
        "already_uploaded" -> return response.documentId
        "upload" -> {
            val presigned = response.upload ?: return null
            putBytes(app, presigned.url, presigned.headers, file, "image/png")
            val ticket = response.ticket ?: return null
            return app.api.completeUpload(ticket).document?.id
        }
        else -> return null
    }
}

private suspend fun putBytes(
    app: AutoCrmApp,
    url: String,
    headers: Map<String, String>,
    file: File,
    contentType: String,
) = withContext(Dispatchers.IO) {
    val builder = Request.Builder().url(url)
    headers.forEach { (name, value) -> builder.header(name, value) }
    try {
        app.api.http.newCall(
            builder.put(file.asRequestBody(contentType.toMediaTypeOrNull())).build(),
        ).execute()
    } catch (e: IOException) {
        throw ApiException.Network(e)
    }.use {
        if (!it.isSuccessful) {
            throw if (it.code in 500..599 || it.code == 403) {
                ApiException.Server(it.code, it.body?.string()?.take(300).orEmpty())
            } else {
                ApiException.Rule("upload_rejected", "tárhely: ${it.code}")
            }
        }
    }
}
