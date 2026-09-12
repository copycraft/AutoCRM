package hu.autotherm.autocrm.ui.photos

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.upload.UploadQueue
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.ui.theme.Steel900
import hu.autotherm.autocrm.ui.theme.Surface
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.io.File

/**
 * Adding photos to a job.
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
) : ViewModel() {

    data class State(
        val category: String = CapturePrefs.CATEGORY_COMPLETION,
        val pending: Int = 0,
        val message: String? = null,
        val intakeWarning: Boolean = false,
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

    fun selectCategory(category: String) {
        viewModelScope.launch {
            prefs.setCategory(category)
            _state.value = _state.value.copy(category = category, intakeWarning = false)
            // Intake photos can never be deleted or re-filed: the database refuses both.
            // That is the one category worth interrupting for.
            if (CapturePrefs.isImmutable(category) && prefs.needsIntakeConfirmation()) {
                _state.value = _state.value.copy(intakeWarning = true)
            }
        }
    }

    fun acknowledgeIntake() {
        viewModelScope.launch {
            prefs.acknowledgeIntake()
            _state.value = _state.value.copy(intakeWarning = false)
        }
    }

    fun dismissIntakeWarning() {
        _state.value = _state.value.copy(
            intakeWarning = false,
            category = CapturePrefs.CATEGORY_PRODUCTION,
        )
        viewModelScope.launch { prefs.setCategory(CapturePrefs.CATEGORY_PRODUCTION) }
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
        add(listOf(uri), orderId, orderNumber)
    }

    fun clearMessage() {
        _state.value = _state.value.copy(message = null)
    }

    /** A file for the system camera app plus the content URI that grants it write access. */
    fun newCameraTarget(): Pair<File, Uri> = queue.newCameraTarget()

    fun loadStickyCategory() {
        viewModelScope.launch {
            _state.value = _state.value.copy(category = prefs.category.first())
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun OrderPhotoSection(
    orderId: Long,
    orderNumber: String,
    imageCounts: Map<String, Long>,
    viewModel: OrderPhotoViewModel,
) {
    val state by viewModel.state.collectAsState()
    // Held across the launcher round trip: the result callback only says whether the camera
    // app saved, not where.
    val cameraTarget = remember { mutableStateOf<Pair<File, Uri>?>(null) }

    val galleryLauncher = rememberLauncherForActivityResult(
        // The system photo picker: no storage permission, and the user only ever exposes
        // the items they choose.
        ActivityResultContracts.PickMultipleVisualMedia(20),
    ) { uris -> viewModel.add(uris, orderId, orderNumber) }

    val cameraLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture(),
    ) { saved ->
        cameraTarget.value?.let { (file, uri) ->
            viewModel.cameraResult(saved, file, uri, orderId, orderNumber)
        }
        cameraTarget.value = null
    }

    LaunchedEffect(orderId) {
        viewModel.observe(orderId)
        viewModel.loadStickyCategory()
    }

    Card {
        SectionTitle("Fotók")

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (imageCounts.isEmpty()) {
                Text("Még nincs feltöltött fotó.", style = MaterialTheme.typography.bodyLarge, color = Steel500)
            } else {
                imageCounts.forEach { (category, count) ->
                    StatusBadge("${CapturePrefs.label(category)}: $count", Tone.Steel)
                }
            }
        }

        if (state.pending > 0) {
            StatusBadge("${state.pending} feltöltésre vár", Tone.Cold)
        }

        Text(
            "Kategória",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
            modifier = Modifier.padding(top = 8.dp),
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CapturePrefs.ALL.forEach { category ->
                val active = category == state.category
                val immutable = CapturePrefs.isImmutable(category)
                Box(
                    Modifier
                        .background(
                            when {
                                active && immutable -> Signal
                                active -> Steel900
                                else -> Color.Transparent
                            },
                            RoundedCornerShape(999.dp),
                        )
                        .border(1.dp, if (active) Color.Transparent else Steel200, RoundedCornerShape(999.dp))
                        .clickable { viewModel.selectCategory(category) }
                        .padding(horizontal = 14.dp, vertical = 8.dp),
                ) {
                    Text(
                        CapturePrefs.label(category),
                        style = MaterialTheme.typography.labelLarge,
                        color = if (active) Surface else Steel900,
                    )
                }
            }
        }

        Row(
            Modifier.fillMaxWidth().padding(top = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(
                onClick = {
                    val target = viewModel.newCameraTarget()
                    cameraTarget.value = target
                    cameraLauncher.launch(target.second)
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

    if (state.intakeWarning) {
        AlertDialog(
            onDismissRequest = viewModel::dismissIntakeWarning,
            title = { Text("Bevételi fotó") },
            text = {
                Text(
                    "A bevételi fotók véglegesek: nem törölhetők és nem sorolhatók át. " +
                        "Ezek bizonyítják a jármű átvételkori állapotát.",
                )
            },
            confirmButton = {
                TextButton(onClick = viewModel::acknowledgeIntake) { Text("Értem, bevétel") }
            },
            dismissButton = {
                TextButton(onClick = viewModel::dismissIntakeWarning) { Text("Mégis gyártás") }
            },
        )
    }
}
