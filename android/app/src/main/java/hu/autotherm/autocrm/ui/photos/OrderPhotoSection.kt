package hu.autotherm.autocrm.ui.photos

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.inspection.cachedLookups
import hu.autotherm.autocrm.data.inspection.downloadLookups
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.prefs.LookupsCache
import hu.autotherm.autocrm.data.upload.UploadQueue
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.ui.theme.Steel900
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.io.File

/**
 * Adding photos to a job.
 *
 * Everything attached here files as production ("Gyártás"): intake and handover
 * shots are evidence with their own flows (bevétel, átadás-átvétel) and must
 * never come from the gallery or an ad-hoc camera tap — per the client.
 *
 * Photography happens in the phone's **own camera app**, not in this one. An in-app
 * preview-and-shutter built on CameraX does not match what the manufacturer's camera
 * produces — its processing, its stabilisation, its full sensor resolution — and MEO photos
 * are evidence of a vehicle's condition. Losing quality to save a screen transition is the
 * wrong trade.
 *
 * So there are two doors, and both hand back a full-quality original:
 *
 *  - **Kamera** launches the system camera app, writing into the queue's own directory.
 *  - **Galéria** uses the system photo picker, which needs no storage permission at all.
 *
 * Everything after that is the durable queue, unchanged: hashed, written to the database
 * before this function returns, uploaded by the worker whenever there is a network.
 */
class OrderPhotoViewModel(
    private val queue: UploadQueue,
    private val prefs: CapturePrefs,
    private val api: AutoCrmApi,
    private val lookupsCache: LookupsCache,
) : ViewModel() {

    data class State(
        val category: String = CapturePrefs.CATEGORY_PRODUCTION,
        /** What may be picked: the attachable categories plus the stage's default (0048). */
        val choices: List<String> = listOf(CapturePrefs.CATEGORY_PRODUCTION),
        val pending: Int = 0,
        val message: String? = null,
        val lookups: Lookups? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** Set once by the screen; the queue count is per order. */
    fun observe(orderId: Long) {
        viewModelScope.launch {
            queue.forOrder(orderId).collect { rows ->
                _state.value = _state.value.copy(pending = rows.size)
            }
        }
    }

    /**
     * The category new photos take. Manual attaching is production-only (client rule):
     * intake and handover shots come from their own flows. The one addition is the order's
     * current stage default (0048), so in MEO the completion photo the gate needs can be
     * taken here; it is also preselected. Evidence categories are never offered.
     */
    fun loadStickyCategory(stageCategory: String? = null) {
        viewModelScope.launch {
            val lookups = downloadLookups(api, lookupsCache) ?: cachedLookups(lookupsCache)
            val stored = prefs.category.first()
            val allowed = CapturePrefs.attachable(lookups).toMutableList()
            val stageOk = stageCategory != null &&
                stageCategory != CapturePrefs.CATEGORY_INTAKE &&
                stageCategory != CapturePrefs.CATEGORY_INSPECTION
            if (stageOk && stageCategory !in allowed) allowed.add(0, stageCategory!!)
            val category = when {
                stageOk -> stageCategory!!
                stored in allowed -> stored
                else -> allowed.firstOrNull() ?: CapturePrefs.CATEGORY_PRODUCTION
            }
            if (category != stored && category in CapturePrefs.attachable(lookups)) prefs.setCategory(category)
            _state.value = _state.value.copy(category = category, choices = allowed, lookups = lookups)
        }
    }

    fun pickCategory(category: String) {
        if (category !in _state.value.choices) return
        _state.value = _state.value.copy(category = category)
    }

    fun add(uris: List<Uri>, orderId: Long, orderNumber: String) {
        if (uris.isEmpty()) return
        viewModelScope.launch {
            var queued = 0
            var duplicates = 0
            var unreadable = 0
            for (uri in uris) {
                when (queue.enqueueFromUri(uri, orderId, orderNumber, _state.value.category)) {
                    is UploadQueue.Enqueued.Queued -> queued++
                    is UploadQueue.Enqueued.Duplicate -> duplicates++
                    is UploadQueue.Enqueued.Unreadable -> unreadable++
                }
            }
            _state.value = _state.value.copy(
                message = buildString {
                    if (queued > 0) append("$queued fotó sorba állítva. ")
                    if (duplicates > 0) append("$duplicates már sorban állt. ")
                    if (unreadable > 0) append("$unreadable nem olvasható.")
                }.trim().ifBlank { null },
            )
        }
    }

    /** The camera app wrote (or did not write) into the file we handed it. */
    fun cameraResult(saved: Boolean, file: File, uri: Uri, orderId: Long, orderNumber: String) {
        if (!saved || file.length() == 0L) {
            file.delete()
            return
        }
        // The camera already wrote into the queue's own directory: move that file into the
        // queue (enqueue), rather than copying it through its URI and leaving the original
        // behind as orphaned bytes nothing ever deletes.
        viewModelScope.launch {
            val message = when (queue.enqueue(file, orderId, orderNumber, _state.value.category)) {
                is UploadQueue.Enqueued.Queued -> "1 fotó sorba állítva."
                is UploadQueue.Enqueued.Duplicate -> "1 már sorban állt."
                is UploadQueue.Enqueued.Unreadable -> "1 nem olvasható."
            }
            _state.value = _state.value.copy(message = message)
        }
    }

    fun clearMessage() {
        _state.value = _state.value.copy(message = null)
    }

    /** The system camera could not be opened at all. Shown, not thrown. */
    fun cameraUnavailable() {
        _state.value = _state.value.copy(message = "Nincs elérhető kameraalkalmazás ezen a telefonon.")
    }

    /** A file for the system camera app plus the content URI that grants it write access. */
    fun newCameraTarget(): Pair<File, Uri> = queue.newCameraTarget()
}

@Composable
fun OrderPhotoSection(
    orderId: Long,
    orderNumber: String,
    imageCounts: Map<String, Long>,
    viewModel: OrderPhotoViewModel,
    /** The order's current stage default (`photo_category` on the order detail). */
    stageCategory: String? = null,
) {
    val state by viewModel.state.collectAsState()
    // Held across the launcher round trip: the result callback only says whether the camera
    // app saved, not where. Saveable as path strings — a plain remember dies with the process
    // when the system camera foregrounds, and the full-quality JPEG would sit orphaned with
    // no queue row.
    var cameraPath by rememberSaveable { mutableStateOf<String?>(null) }
    var cameraUri by rememberSaveable { mutableStateOf<String?>(null) }

    val galleryLauncher = rememberLauncherForActivityResult(
        // The system photo picker: no storage permission, and the user only ever exposes
        // the items they choose.
        ActivityResultContracts.PickMultipleVisualMedia(20),
    ) { uris -> viewModel.add(uris, orderId, orderNumber) }

    val cameraLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture(),
    ) { saved ->
        val path = cameraPath
        val uri = cameraUri
        if (path != null && uri != null) {
            viewModel.cameraResult(saved, File(path), Uri.parse(uri), orderId, orderNumber)
        }
        cameraPath = null
        cameraUri = null
    }

    LaunchedEffect(orderId) { viewModel.observe(orderId) }
    LaunchedEffect(orderId, stageCategory) { viewModel.loadStickyCategory(stageCategory) }

    Card {
        SectionTitle(
            "Fotók",
            count = imageCounts.values.sum().toInt().takeIf { it > 0 },
        )

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (imageCounts.isEmpty()) {
                Text("Még nincs feltöltött fotó.", style = MaterialTheme.typography.bodyLarge, color = Steel500)
            } else {
                imageCounts.forEach { (category, count) ->
                    StatusBadge("${CapturePrefs.label(category, state.lookups)}: $count", Tone.Steel)
                }
            }
        }

        if (state.pending > 0) {
            StatusBadge("${state.pending} feltöltésre vár", Tone.Cold)
        }

        if (state.choices.size > 1) {
            Row(
                Modifier.padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                state.choices.forEach { key ->
                    androidx.compose.material3.FilterChip(
                        selected = state.category == key,
                        onClick = { viewModel.pickCategory(key) },
                        label = { Text(CapturePrefs.label(key, state.lookups)) },
                    )
                }
            }
        }
        Text(
            if (state.category == CapturePrefs.CATEGORY_PRODUCTION) {
                "Ide gyártás közbeni fotók kerülnek. A bevételi és az átadási fotók a saját folyamatukban készülnek."
            } else {
                "A fotók ide kerülnek: ${CapturePrefs.label(state.category, state.lookups)} (a jelenlegi fázis alapértéke)."
            },
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
            modifier = Modifier.padding(top = 8.dp),
        )

        Row(
            Modifier.fillMaxWidth().padding(top = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(
                onClick = {
                    try {
                        val target = viewModel.newCameraTarget()
                        cameraPath = target.first.absolutePath
                        cameraUri = target.second.toString()
                        cameraLauncher.launch(target.second)
                    } catch (e: Exception) {
                        // No camera app, or one that refuses the handoff: a button that
                        // kills the process reads as "the app is broken", a line does not.
                        viewModel.cameraUnavailable()
                    }
                },
                modifier = Modifier.weight(1f),
            ) { Text("Kamera") }
            OutlinedButton(
                onClick = {
                    galleryLauncher.launch(
                        PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                    )
                },
                modifier = Modifier.weight(1f),
            ) { Text("Galéria") }
        }

        state.message?.let {
            Text(it, style = MaterialTheme.typography.bodyLarge, color = Steel900)
        }
    }
}
