package hu.autotherm.autocrm.ui.picker

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
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
import hu.autotherm.autocrm.ui.common.LoadingState
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
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
        val orders: List<PickerOrder> = emptyList(),
        val error: String? = null,
        val includeFinished: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private val queryFlow = MutableStateFlow("")
    val query: StateFlow<String> = queryFlow.asStateFlow()

    init {
        viewModelScope.launch {
            // 300 ms: long enough that typing a six-character plate is one request, short
            // enough that the list feels attached to the keyboard.
            queryFlow.debounce(300).collect { load(it) }
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
        _state.value = _state.value.copy(loading = true, error = null)
        try {
            val items = api.pickerOrders(
                query = query.takeIf { it.isNotBlank() },
                all = _state.value.includeFinished,
            )
            _state.value = _state.value.copy(loading = false, orders = items)
        } catch (e: Throwable) {
            _state.value = _state.value.copy(
                loading = false,
                error = when (e) {
                    is ApiException.Network -> "Nincs kapcsolat. A már kiválasztott megrendelésen tudsz fotózni."
                    else -> e.message ?: "Nem sikerült betölteni."
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

@Composable
fun OrderPickerScreen(viewModel: OrderPickerViewModel, onChosen: () -> Unit) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    LaunchedEffect(Unit) { viewModel.setQuery("") }

    Column(Modifier.fillMaxSize().padding(16.dp)) {
        OutlinedTextField(
            value = query,
            onValueChange = viewModel::setQuery,
            label = { Text("Rendszám, szám vagy ügyfél") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            if (state.includeFinished) "Minden megrendelés · koppints a nyitottakhoz" else "Nyitott megrendelések · koppints az összeshez",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
            modifier = Modifier
                .padding(vertical = 8.dp)
                .clickable { viewModel.toggleFinished() },
        )

        when {
            state.loading && state.orders.isEmpty() -> LoadingState()
            state.error != null && state.orders.isEmpty() ->
                ErrorState(state.error!!, onRetry = viewModel::retry)
            state.orders.isEmpty() -> EmptyState("Nincs találat.")
            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(state.orders, key = { it.id }) { order ->
                    Card(Modifier.clickable { viewModel.choose(order) { onChosen() } }) {
                        Text(
                            "${order.number} · ${order.vehiclePlate ?: "—"}",
                            style = MonoSmall.copy(fontSize = MaterialTheme.typography.bodyLarge.fontSize),
                        )
                        Text(order.title, style = MaterialTheme.typography.titleMedium)
                        Text(
                            listOfNotNull(order.partnerName, order.vehicle).joinToString(" · "),
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                        StatusBadge(order.stageLabel, Tone.Steel)
                    }
                }
            }
        }
    }
}
