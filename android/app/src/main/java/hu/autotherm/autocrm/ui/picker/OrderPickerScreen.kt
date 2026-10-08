package hu.autotherm.autocrm.ui.picker

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.PickerOrder
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/**
 * Choosing the van. Open orders only by default, searchable by plate ignoring spaces, dashes
 * and case — the server normalises both sides (`domain/order.rs::normalize_plate`), so a
 * fitter typing "abc123" into a phone keyboard finds "ABC-123".
 */
@OptIn(FlowPreview::class)
class OrderPickerViewModel(
    private val api: AutoCrmApi,
    private val prefs: CapturePrefs,
) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val orders: List<PickerOrder> = emptyList(),
        val error: String? = null,
        val includeFinished: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private val queryFlow = MutableStateFlow("")
    val query: StateFlow<String> = queryFlow.asStateFlow()

    /** The van chosen last time: a fitter on one job for three days taps it, not a search. */
    val last: StateFlow<CapturePrefs.CurrentOrder?> =
        prefs.currentOrder.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    init {
        // Immediate first load, then debounced typing.
        viewModelScope.launch { load(queryFlow.value) }
        viewModelScope.launch {
            // 300 ms: long enough that typing a six-character plate is one request, short
            // enough that the list feels attached to the keyboard.
            queryFlow.drop(1).debounce(300).distinctUntilChanged().collect { load(it) }
        }
    }

    fun setQuery(value: String) {
        queryFlow.value = value
    }

    fun toggleFinished() {
        _state.value = _state.value.copy(includeFinished = !_state.value.includeFinished)
        viewModelScope.launch { load(queryFlow.value) }
    }

    private suspend fun load(query: String) {
        val keepRows = _state.value.orders.isNotEmpty()
        _state.value = _state.value.copy(
            loading = !keepRows,
            refreshing = keepRows,
            error = null,
        )
        try {
            val items = api.pickerOrders(
                query = query.takeIf { it.isNotBlank() },
                all = _state.value.includeFinished,
            )
            _state.value = _state.value.copy(loading = false, refreshing = false, orders = items)
        } catch (e: Throwable) {
            // Cancellation is the ViewModel dying, not a load failure: rethrow so the
            // coroutine machinery sees it. describeError rethrows CancellationException
            // for exactly this reason.
            _state.value = _state.value.copy(
                loading = false,
                refreshing = false,
                error = when (e) {
                    is ApiException.Network -> "Nincs kapcsolat. A már kiválasztott megrendelésen tudsz fotózni."
                    else -> describeError(e)
                },
            )
        }
    }

    fun choose(order: PickerOrder, onChosen: () -> Unit) {
        viewModelScope.launch {
            prefs.setCurrentOrder(
                CapturePrefs.CurrentOrder(
                    id = order.id,
                    number = order.number,
                    title = order.title,
                    plate = order.vehiclePlate,
                ),
            )
            onChosen()
        }
    }

    fun retry() {
        viewModelScope.launch { load(queryFlow.value) }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OrderPickerScreen(
    viewModel: OrderPickerViewModel,
    onMenu: () -> Unit,
    onChoose: (orderId: Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()
    val last by viewModel.last.collectAsState()

    // Fresh search on first entry — but only once per composition instance. A bare
    // LaunchedEffect(Unit) re-runs on rotation (the ViewModel survives it) and wipes
    // the filter the fitter just typed.
    var entered by rememberSaveable { mutableStateOf(false) }
    if (!entered) {
        LaunchedEffect(Unit) {
            viewModel.setQuery("")
            entered = true
        }
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Jármű választása",
                subtitle = if (state.orders.isNotEmpty()) "${state.orders.size} találat" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::retry,
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            SearchField(
                value = query,
                onValueChange = viewModel::setQuery,
                label = "Rendszám, szám vagy ügyfél",
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            )
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterChip(
                    selected = !state.includeFinished,
                    onClick = { if (state.includeFinished) viewModel.toggleFinished() },
                    label = { Text("Nyitott") },
                )
                FilterChip(
                    selected = state.includeFinished,
                    onClick = { if (!state.includeFinished) viewModel.toggleFinished() },
                    label = { Text("Mind") },
                )
            }

            last?.takeIf { query.isBlank() }?.let { order ->
                Card(
                    modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
                    onClick = { onChoose(order.id) },
                ) {
                    Text("Legutóbbi", style = MaterialTheme.typography.labelMedium, color = Steel500)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                        order.plate?.let { hu.autotherm.autocrm.ui.common.PlateBadge(it) }
                        Text("#" + order.number, style = MonoSmall, color = Steel500)
                    }
                    Text(order.title, style = MaterialTheme.typography.titleMedium)
                }
            }

            when {
                state.loading -> ListSkeleton(Modifier.padding(top = 4.dp))
                state.error != null && state.orders.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::retry)
                state.orders.isEmpty() -> EmptyState("Nincs találat.")
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::retry,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.orders, key = { it.id }) { order ->
                            Card(
                                modifier = Modifier.animateItem(),
                                onClick = { viewModel.choose(order) { onChoose(order.id) } },
                            ) {
                                // The plate is what the fitter is looking at on the van.
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                                    order.vehiclePlate?.let { hu.autotherm.autocrm.ui.common.PlateBadge(it) }
                                    Text("#" + order.number, style = MonoSmall, color = Steel500)
                                }
                                Text(order.title, style = MaterialTheme.typography.titleMedium)
                                Text(
                                    listOfNotNull(order.partnerName, order.vehicle).joinToString(" · "),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                                StatusBadge(order.stageLabel, Tone.Cold)
                            }
                        }
                    }
                }
            }
            if (state.error != null && state.orders.isNotEmpty() && !state.refreshing) {
                ErrorState(state.error!!, onRetry = viewModel::retry)
            }
        }
    }
}
