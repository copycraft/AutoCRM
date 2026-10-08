package hu.autotherm.autocrm.ui.capture

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.photos.OrderPhotoSection
import hu.autotherm.autocrm.ui.photos.OrderPhotoViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class CaptureViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val orderNumber: String = "",
        val imageCounts: Map<String, Long> = emptyMap(),
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(orderId: Long) {
        viewModelScope.launch {
            // A reload keeps what is shown (back from the camera): no skeleton flash.
            _state.value = _state.value.copy(loading = _state.value.orderNumber.isBlank(), error = null)
            try {
                val detail = api.order(orderId)
                _state.value = State(
                    loading = false,
                    orderNumber = detail.order.number,
                    imageCounts = detail.imageCounts,
                )
            } catch (e: Throwable) {
                _state.value = State(loading = false, error = describeError(e))
            }
        }
    }
}

/**
 * Capture-first photography: the picker chose the van, this screen attaches the
 * photos. Same photo section as order detail, without the rest of the record —
 * a fitter photographing six vans in a row never opens six order pages.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CaptureScreen(
    orderId: Long,
    viewModel: CaptureViewModel,
    photoViewModel: OrderPhotoViewModel,
    onBack: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(orderId) { viewModel.load(orderId) }
    // Back from the camera app: the counts and thumbnails catch up.
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.load(orderId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Fotózás",
                subtitle = state.orderNumber.takeIf { it.isNotBlank() },
                onBack = onBack,
                refreshing = false,
                onRefresh = { viewModel.load(orderId) },
            )
        },
    ) { padding ->
        when {
            state.loading -> DetailSkeleton(
                Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            state.error != null -> ErrorState(
                state.error!!,
                Modifier.padding(padding),
            ) { viewModel.load(orderId) }
            else -> Column(Modifier.fillMaxSize().padding(padding).padding(16.dp)) {
                OrderPhotoSection(
                    orderId = orderId,
                    orderNumber = state.orderNumber,
                    imageCounts = state.imageCounts,
                    viewModel = photoViewModel,
                )
            }
        }
    }
}
