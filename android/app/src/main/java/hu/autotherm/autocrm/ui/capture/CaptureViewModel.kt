package hu.autotherm.autocrm.ui.capture

import androidx.camera.core.ImageCapture
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.upload.UploadQueue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors

/**
 * Capture is deliberately fire-and-forget from the shutter's point of view: the photo is
 * written, hashed, queued and the shutter is live again. Nothing here waits for a network.
 *
 * That is the whole reliability argument. A fitter photographing the inside of a box body
 * has no signal; if the shutter blocked on an upload, they would stop using the app by the
 * third van.
 */
class CaptureViewModel(
    private val queue: UploadQueue,
    private val prefs: CapturePrefs,
) : ViewModel() {

    data class State(
        val order: CapturePrefs.CurrentOrder? = null,
        val category: String = CapturePrefs.CATEGORY_PRODUCTION,
        val capturing: Boolean = false,
        val shotsThisSession: Int = 0,
        val outstanding: Int = 0,
        val blocked: Int = 0,
        val intakeWarning: Boolean = false,
        val toast: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** One thread for image writes: CameraX needs an executor and two would serve nothing. */
    private val captureExecutor = Executors.newSingleThreadExecutor()
    private var imageCapture: ImageCapture? = null

    init {
        viewModelScope.launch {
            combine(
                prefs.currentOrder,
                prefs.category,
                queue.outstanding,
                queue.blocked,
            ) { order, category, outstanding, blocked ->
                _state.value.copy(
                    order = order,
                    category = category,
                    outstanding = outstanding,
                    blocked = blocked,
                )
            }.collect { _state.value = it }
        }
        viewModelScope.launch {
            // On arrival, not on every recomposition: the banner is about the state the
            // fitter is starting in, not about a category they just chose deliberately.
            if (prefs.category.let { _state.value.category } == CapturePrefs.CATEGORY_INTAKE &&
                prefs.needsIntakeConfirmation()
            ) {
                _state.value = _state.value.copy(intakeWarning = true)
            }
        }
    }

    fun attachCamera(capture: ImageCapture?) {
        imageCapture = capture
    }

    fun selectCategory(category: String) {
        viewModelScope.launch {
            prefs.setCategory(category)
            _state.value = _state.value.copy(intakeWarning = false)
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
        _state.value = _state.value.copy(intakeWarning = false)
    }

    fun capture() {
        val capture = imageCapture ?: return
        val order = _state.value.order ?: return
        if (_state.value.capturing) return

        _state.value = _state.value.copy(capturing = true)
        viewModelScope.launch {
            val file = queue.newCaptureFile()
            val result = capture.takePictureTo(file, captureExecutor)
            result.fold(
                onSuccess = {
                    val outcome = withContext(Dispatchers.IO) {
                        queue.enqueue(
                            captured = file,
                            orderId = order.id,
                            orderNumber = order.number,
                            category = _state.value.category,
                        )
                    }
                    _state.value = _state.value.copy(
                        capturing = false,
                        shotsThisSession = _state.value.shotsThisSession + 1,
                        toast = when (outcome) {
                            is UploadQueue.Enqueued.Duplicate -> "Ez a kép már sorban áll"
                            is UploadQueue.Enqueued.Queued -> null
                        },
                    )
                },
                onFailure = { error ->
                    file.delete()
                    _state.value = _state.value.copy(
                        capturing = false,
                        toast = "A fotó nem készült el: ${error.message}",
                    )
                },
            )
        }
    }

    fun clearToast() {
        _state.value = _state.value.copy(toast = null)
    }

    /** Called by the picker. Resets the session counter: a new van is a new count. */
    fun selectOrder(order: CapturePrefs.CurrentOrder) {
        viewModelScope.launch {
            prefs.setCurrentOrder(order)
            _state.value = _state.value.copy(shotsThisSession = 0)
        }
    }

    override fun onCleared() {
        captureExecutor.shutdown()
        super.onCleared()
    }
}
