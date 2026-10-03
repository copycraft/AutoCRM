package hu.autotherm.autocrm.ui.inspection

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.location.LocationManager
import androidx.core.content.ContextCompat
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.Comparison
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.api.ZoneTemplate
import hu.autotherm.autocrm.data.db.InspectionDraft
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.inspection.DraftDamage
import hu.autotherm.autocrm.data.inspection.DraftPayload
import hu.autotherm.autocrm.data.inspection.DraftPhoto
import hu.autotherm.autocrm.data.inspection.DraftSignature
import hu.autotherm.autocrm.data.inspection.DraftVerdict
import hu.autotherm.autocrm.data.inspection.ZONE_LIST_UNAVAILABLE
import hu.autotherm.autocrm.data.inspection.cachedLookups
import hu.autotherm.autocrm.data.inspection.displayTitle
import hu.autotherm.autocrm.data.inspection.downloadLookups
import hu.autotherm.autocrm.data.inspection.downloadZoneList
import hu.autotherm.autocrm.data.inspection.inspectionDir
import hu.autotherm.autocrm.data.inspection.resolveZones
import hu.autotherm.autocrm.data.inspection.zoneListNotice
import hu.autotherm.autocrm.data.upload.sha256Hex
import hu.autotherm.autocrm.data.inspection.InspectionSyncWorker
import java.io.File
import java.time.Instant
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json

private val json = Json { ignoreUnknownKeys = true }

/** Where the walkaround is: the camera loop is driven by this, not by navigation. */
sealed class Phase {
    data object Loading : Phase()
    data object Readings : Phase()
    /** Walking zone [zoneIndex] of the template list. */
    data object Zone : Phase()
    /** Documenting one damage item (close-ups → mark → classify). */
    data class Damage(val damageLocalId: String) : Phase()
    data object Comparison : Phase()
    data object Summary : Phase()
    data object Signing : Phase()
    data class Done(val serverId: Long?) : Phase()
}

data class CaptureRequest(
    val purpose: String,
    val zoneKey: String,
    val instruction: String,
    val damageLocalId: String? = null,
    val file: File,
)

class WalkaroundViewModel(private val app: AutoCrmApp) : ViewModel() {

    data class State(
        val uuid: String = "",
        val orderId: Long = 0,
        val orderNumber: String = "",
        val kind: String = "checkout",
        val payload: DraftPayload? = null,
        val phase: Phase = Phase.Loading,
        val capture: CaptureRequest? = null,
        /** Captured file awaiting Keep / Retake. */
        val review: Pair<CaptureRequest, File>? = null,
        val comparison: Comparison? = null,
        val comparisonLoading: Boolean = false,
        val busy: Boolean = false,
        val error: String? = null,
        /** Not an error: where the zone list came from when it is not the server's own (offline start). */
        val notice: String? = null,
        /** The server's enumerations (damage types, fuel marks, ...); cached for offline starts. */
        val lookups: Lookups? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    private var openArgs: Triple<String?, Long, String>? = null

    /** Re-runs the last open after a load failure (order fetch, templates, …). */
    fun retryOpen() {
        val (uuid, orderId, kind) = openArgs ?: return
        update { it.copy(error = null, phase = Phase.Loading) }
        open(uuid, orderId, kind)
    }

    private fun update(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    private suspend fun persist() {
        val s = _state.value
        val payload = s.payload ?: return
        withContext(Dispatchers.IO) {
            app.database.inspectionDrafts().upsert(
                InspectionDraft(
                    localUuid = s.uuid,
                    orderId = s.orderId,
                    orderNumber = s.orderNumber,
                    kind = s.kind,
                    payloadJson = json.encodeToString(DraftPayload.serializer(), payload),
                ),
            )
        }
    }

    /** Opens an existing draft (resume) or starts a fresh walkaround. */
    fun open(uuid: String?, orderId: Long, kind: String) {
        openArgs = Triple(uuid, orderId, kind)
        viewModelScope.launch {
            // The enumerations the form renders (damage types, fuel marks, ...):
            // fresh when online, last downloaded when not.
            val lookups = downloadLookups(app.api, app.lookupsCache)
                ?: cachedLookups(app.lookupsCache)
            if (uuid != null) {
                val row = withContext(Dispatchers.IO) {
                    app.database.inspectionDrafts().byUuid(uuid)
                } ?: return@launch
                val payload = json.decodeFromString(DraftPayload.serializer(), row.payloadJson)
                _state.value = State(
                    uuid = uuid,
                    orderId = row.orderId,
                    orderNumber = row.orderNumber,
                    kind = row.kind,
                    payload = payload,
                    phase = if (payload.signed) Phase.Done(null) else Phase.Zone,
                    lookups = lookups,
                )
                return@launch
            }
            val order = try {
                app.api.order(orderId).order
            } catch (e: Exception) {
                _state.value = State(error = "a megrendelés nem tölthető be")
                return@launch
            }
            val inspectorDefault = try {
                app.sessionStore.currentAccount()?.displayName.orEmpty()
            } catch (e: Exception) {
                ""
            }
            // Fresh draft: the list for this vehicle kind and walkaround comes from the server;
            // if it cannot be reached, the last one downloaded for the same. There is no list
            // built into the app: with neither, the walkaround does not start.
            val fetched = downloadZoneList(app.api, app.zoneListCache, order.projectTypeId, kind)
            val cached = if (fetched == null) {
                runCatching { app.zoneListCache.get(order.projectTypeId, kind) }.getOrNull()
            } else null
            val resolved = resolveZones(fetched, cached)
            if (resolved == null) {
                _state.value = State(error = ZONE_LIST_UNAVAILABLE)
                return@launch
            }
            val templates = resolved.zones
            // Check-in links the latest signed check-out when online; otherwise the
            // sync resolves it (and fails visibly if none exists by then).
            val checkoutId = if (kind == "checkin") {
                try {
                    app.api.inspections(orderId)
                        .filter { it.kind == "checkout" && it.status == "signed" }
                        .maxByOrNull { it.signedAt.orEmpty() }?.id
                } catch (e: Exception) {
                    null
                }
            } else null
            val newUuid = UUID.randomUUID().toString()
            _state.value = State(
                uuid = newUuid,
                orderId = orderId,
                orderNumber = order.number,
                kind = kind,
                payload = DraftPayload(
                    vehiclePlate = order.vehiclePlate ?: "",
                    vehicleVin = order.vehicleVin,
                    inspectorName = inspectorDefault,
                    checkoutServerId = checkoutId,
                    templates = templates,
                ),
                phase = Phase.Readings,
                notice = zoneListNotice(resolved.source),
                lookups = lookups,
            )
            persist()
        }
    }

    fun setReadings(next: (DraftPayload) -> DraftPayload) {
        update { it.copy(payload = it.payload?.let(next), error = null) }
        viewModelScope.launch { persist() }
    }

    fun readingsDone() {
        update { it.copy(phase = Phase.Zone) }
    }

    // ── Camera loop ──

    private fun lastLocation(): Pair<Double, Double>? {
        val ctx = app.applicationContext
        if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) !=
            PackageManager.PERMISSION_GRANTED &&
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_COARSE_LOCATION) !=
                PackageManager.PERMISSION_GRANTED
        ) {
            return null
        }
        val manager = ctx.getSystemService(Context.LOCATION_SERVICE) as? LocationManager
            ?: return null
        val providers = manager.getProviders(true)
        for (name in listOf(LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER)) {
            if (name !in providers) continue
            try {
                manager.getLastKnownLocation(name)?.let { return it.latitude to it.longitude }
            } catch (e: SecurityException) {
                return null
            }
        }
        return null
    }

    /** Opens the camera for one shot. The file is reserved up front in the draft dir. */
    fun requestCapture(purpose: String, zoneKey: String, instruction: String, damageLocalId: String? = null) {
        val s = _state.value
        val file = File(
            inspectionDir(app, s.uuid),
            "${purpose}_${zoneKey}_${System.currentTimeMillis()}.jpg",
        )
        update {
            it.copy(
                capture = CaptureRequest(purpose, zoneKey, instruction, damageLocalId, file),
                review = null,
                error = null,
            )
        }
    }

    fun cancelCapture() {
        update { it.copy(capture = null) }
    }

    /** A shot landed: hash it, queue the bytes, record the photo, show Keep/Retake. */
    fun onCaptured(request: CaptureRequest) {
        viewModelScope.launch {
            update { it.copy(capture = null, busy = true) }
            try {
                val (sha, lat, lon) = withContext(Dispatchers.IO) {
                    val sha = request.file.sha256Hex()
                    app.database.pendingUploads().insert(
                        PendingUpload(
                            orderId = _state.value.orderId,
                            orderNumber = _state.value.orderNumber,
                            category = "inspection",
                            filePath = request.file.absolutePath,
                            contentType = "image/jpeg",
                            byteSize = request.file.length(),
                            sha256 = sha,
                            filename = request.file.name,
                        ),
                    )
                    Triple(sha, null as Double?, null as Double?)
                }
                // Location is best-effort and never blocks the loop.
                val loc = lastLocation()
                val photo = DraftPhoto(
                    sha256 = sha,
                    fileName = request.file.name,
                    zoneKey = request.zoneKey,
                    purpose = request.purpose,
                    damageLocalId = request.damageLocalId,
                    takenAt = Instant.now().toString(),
                    lat = loc?.first,
                    lon = loc?.second,
                )
                update {
                    val payload = it.payload ?: return@update it
                    it.copy(
                        payload = payload.copy(photos = payload.photos + photo),
                        review = request to request.file,
                        busy = false,
                    )
                }
                persist()
            } catch (e: Exception) {
                request.file.delete()
                update { it.copy(busy = false, error = "a fotó mentése nem sikerült") }
            }
        }
    }

    /** Keep: continue the loop. Retake: delete bytes + queue row + entry, shoot again. */
    fun keepPhoto() {
        val (request, _) = _state.value.review ?: return
        update { it.copy(review = null) }
        afterPhoto(request)
    }

    fun retakePhoto() {
        val (request, file) = _state.value.review ?: return
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                val sha = runCatching { file.sha256Hex() }.getOrNull()
                file.delete()
                if (sha != null) {
                    app.database.pendingUploads()
                        .byContent(_state.value.orderId, sha)?.let {
                            app.database.pendingUploads().delete(it.id)
                        }
                }
            }
            update {
                val payload = it.payload ?: return@update it
                it.copy(
                    // Only the shot under review is replaced. Earlier kept shots of the
                    // same zone (several overviews, "Még egy közeli") stay: their files
                    // and queue rows were not deleted above.
                    payload = payload.copy(
                        photos = payload.photos.filterNot {
                            it.fileName == file.name && it.attachedPhotoId == null
                        },
                    ),
                    review = null,
                )
            }
            persist()
            requestCapture(request.purpose, request.zoneKey, request.instruction, request.damageLocalId)
        }
    }

    private fun afterPhoto(request: CaptureRequest) {
        // The walkaround screen advances from the review state; damage close-ups
        // return to their damage item, everything else to the zone step.
        update { it.copy(capture = null) }
    }

    // ── Damage loop ──

    fun startDamage(zoneKey: String): String {
        val damage = DraftDamage(zoneKey = zoneKey, damageType = "", severity = "")
        update {
            val payload = it.payload ?: return@update it
            it.copy(
                payload = payload.copy(damages = payload.damages + damage),
                phase = Phase.Damage(damage.localId),
            )
        }
        viewModelScope.launch { persist() }
        return damage.localId
    }

    fun updateDamage(localId: String, next: (DraftDamage) -> DraftDamage) {
        update {
            val payload = it.payload ?: return@update it
            it.copy(
                payload = payload.copy(
                    damages = payload.damages.map { if (it.localId == localId) next(it) else it },
                ),
            )
        }
        viewModelScope.launch { persist() }
    }

    fun removeDamage(localId: String) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                val payload = _state.value.payload
                payload?.photos?.filter { it.damageLocalId == localId }?.forEach { photo ->
                    File(inspectionDir(app, _state.value.uuid), photo.fileName).delete()
                    app.database.pendingUploads()
                        .byContent(_state.value.orderId, photo.sha256)?.let {
                            app.database.pendingUploads().delete(it.id)
                        }
                }
            }
            update {
                val payload = it.payload ?: return@update it
                it.copy(
                    payload = payload.copy(
                        damages = payload.damages.filterNot { d -> d.localId == localId },
                        photos = payload.photos.filterNot { p -> p.damageLocalId == localId },
                    ),
                    phase = Phase.Zone,
                )
            }
            persist()
        }
    }

    fun damageDone() {
        update { it.copy(phase = Phase.Zone) }
    }

    fun nextZone() {
        update {
            val payload = it.payload ?: return@update it
            val next = (it.payload?.zoneIndex ?: 0) + 1
            if (next >= payload.templates.size) {
                it.copy(
                    payload = payload.copy(zoneIndex = next),
                    phase = if (it.kind == "checkin") Phase.Comparison else Phase.Summary,
                )
            } else {
                it.copy(payload = payload.copy(zoneIndex = next), phase = Phase.Zone)
            }
        }
        viewModelScope.launch { persist() }
    }

    fun gotoSummary() {
        update { it.copy(phase = Phase.Summary) }
    }

    fun gotoSigning() {
        update { it.copy(phase = Phase.Signing, error = null) }
    }

    // ── Check-in comparison ──

    fun loadComparison() {
        val s = _state.value
        val payload = s.payload ?: return
        viewModelScope.launch {
            update { it.copy(comparisonLoading = true, error = null) }
            try {
                // A check-in started offline has no check-out link yet: resolve it now
                // (latest signed check-out, as the server links it) instead of leaving
                // "Újrapróbálás" a button that can never succeed.
                val checkoutId = payload.checkoutServerId ?: app.api.inspections(s.orderId)
                    .filter { it.kind == "checkout" && it.status == "signed" }
                    .maxByOrNull { it.signedAt.orEmpty() }?.id
                if (checkoutId == null) {
                    update {
                        it.copy(
                            comparisonLoading = false,
                            error = "nincs lezárt átvétel ehhez az összehasonlításhoz",
                        )
                    }
                    return@launch
                }
                if (payload.checkoutServerId == null) {
                    update { st -> st.copy(payload = st.payload?.copy(checkoutServerId = checkoutId)) }
                    persist()
                }
                // The review needs the checkout's damages and photos: fetch its detail.
                // Suggestions pair same-zone + same-type items as pre-existing.
                val checkoutDetail = app.api.inspection(checkoutId)
                _comparisonCheckout.value = checkoutDetail
                update { it.copy(comparisonLoading = false) }
            } catch (e: Exception) {
                update { it.copy(comparisonLoading = false, error = "nincs kapcsolat az összehasonlításhoz") }
            }
        }
    }

    private val _comparisonCheckout: MutableStateFlow<hu.autotherm.autocrm.data.api.InspectionDetail?> =
        MutableStateFlow(null)
    val comparisonCheckout: StateFlow<hu.autotherm.autocrm.data.api.InspectionDetail?> =
        _comparisonCheckout.asStateFlow()

    fun setVerdict(damageLocalId: String, checkoutDamageId: Long?, verdict: String, note: String?) {
        update {
            val payload = it.payload ?: return@update it
            val rest = payload.verdicts.filterNot { v -> v.damageLocalId == damageLocalId }
            it.copy(
                payload = payload.copy(
                    verdicts = rest + DraftVerdict(damageLocalId, checkoutDamageId, verdict, note),
                ),
            )
        }
        viewModelScope.launch { persist() }
    }

    // ── Signatures + sign ──

    fun setSignatureName(role: String, name: String) {
        update {
            val payload = it.payload ?: return@update it
            it.copy(
                payload = payload.copy(
                    signatures = payload.signatures.map {
                        if (it.role == role) it.copy(name = name) else it
                    },
                ),
            )
        }
        viewModelScope.launch { persist() }
    }

    fun saveSignatureFile(role: String, file: File) {
        update {
            val payload = it.payload ?: return@update it
            it.copy(
                payload = payload.copy(
                    signatures = payload.signatures.map {
                        if (it.role == role) it.copy(fileName = file.name) else it
                    },
                ),
            )
        }
        viewModelScope.launch { persist() }
    }

    /** Local validation, then the sync worker performs the server sign when online. */
    fun signNow() {
        val s = _state.value
        val payload = s.payload ?: return
        val missingOverview = payload.templates
            .filter { !it.optional }
            .filter { zone ->
                payload.photos.none { it.zoneKey == zone.zoneKey && it.purpose == "overview" }
            }
        if (missingOverview.isNotEmpty()) {
            update {
                it.copy(
                    error = "hiányzó zónafotó: " +
                        missingOverview.joinToString { it.displayTitle() },
                    phase = Phase.Zone,
                    payload = payload.copy(zoneIndex = payload.templates.indexOf(missingOverview.first())),
                )
            }
            return
        }
        val badSig = payload.signatures.firstOrNull { it.name.isBlank() || it.fileName == null }
        if (badSig != null) {
            update { it.copy(error = "mindkét aláírás és név kötelező") }
            return
        }
        if (s.kind == "checkin") {
            // Same set the comparison screen reviews: an unfinished damage (no type)
            // has no verdict chip and is not synced, so it cannot block signing.
            val pending = payload.damages.filter { d ->
                d.damageType.isNotBlank() && d.severity.isNotBlank() &&
                    payload.verdicts.none { it.damageLocalId == d.localId }
            }
            if (pending.isNotEmpty() && s.comparison == null) {
                update { it.copy(error = "az összehasonlításhoz jel kell", phase = Phase.Comparison) }
                return
            }
            if (pending.isNotEmpty()) {
                update { it.copy(error = "${pending.size} sérüléshez még kell döntés") }
                return
            }
        }
        update { it.copy(payload = payload.copy(signed = true), busy = false, error = null) }
        viewModelScope.launch {
            persist()
            InspectionSyncWorker.enqueue(app)
            update { it.copy(phase = Phase.Done(null)) }
        }
    }

    fun setError(message: String?) {
        update { it.copy(error = message) }
    }
}
