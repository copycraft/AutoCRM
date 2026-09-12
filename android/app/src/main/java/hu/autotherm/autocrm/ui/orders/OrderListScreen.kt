package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.OrderSummary
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.LoadingState
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatMoney
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch

@OptIn(FlowPreview::class)
class OrderListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val orders: List<OrderSummary> = emptyList(),
        val error: String? = null,
        val openOnly: Boolean = true,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private val queryFlow = MutableStateFlow("")
    val query: StateFlow<String> = queryFlow.asStateFlow()

    init {
        viewModelScope.launch { queryFlow.debounce(300).collect { load() } }
    }

    fun setQuery(value: String) {
        queryFlow.value = value
    }

    fun toggleOpenOnly() {
        _state.value = _state.value.copy(openOnly = !_state.value.openOnly)
        viewModelScope.launch { load() }
    }

    fun retry() = viewModelScope.launch { load() }

    private suspend fun load() {
        _state.value = _state.value.copy(loading = true, error = null)
        try {
            val orders = api.orders(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                openOnly = _state.value.openOnly,
            )
            _state.value = _state.value.copy(loading = false, orders = orders)
        } catch (e: Throwable) {
            _state.value = _state.value.copy(loading = false, error = describeError(e))
        }
    }
}

@Composable
fun OrderListScreen(viewModel: OrderListViewModel, onOpen: (Long) -> Unit) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    Column(Modifier.fillMaxSize().padding(16.dp)) {
        OutlinedTextField(
            value = query,
            onValueChange = viewModel::setQuery,
            label = { Text("Keresés rendszám, szám vagy ügyfél szerint") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            if (state.openOnly) "Csak nyitott · koppints az összeshez" else "Minden megrendelés · koppints a nyitottakhoz",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
            modifier = Modifier.padding(vertical = 8.dp).clickable { viewModel.toggleOpenOnly() },
        )

        when {
            state.loading && state.orders.isEmpty() -> LoadingState()
            state.error != null && state.orders.isEmpty() -> ErrorState(state.error!!, onRetry = viewModel::retry)
            state.orders.isEmpty() -> EmptyState("Nincs megrendelés.")
            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(state.orders, key = { it.id }) { order ->
                    Card(Modifier.clickable { onOpen(order.id) }) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(order.number, style = MonoSmall)
                            Text(
                                formatMoney(order.totalMinor, order.currency),
                                style = MonoSmall,
                            )
                        }
                        Text(order.title, style = MaterialTheme.typography.titleMedium)
                        Text(
                            listOfNotNull(order.partnerName, order.vehiclePlate).joinToString(" · "),
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            StatusBadge(order.stageLabel, if (order.stageIsTerminal) Tone.Done else Tone.Steel)
                            if (order.openBlockers > 0) {
                                StatusBadge("${order.openBlockers} akadály", Tone.Signal)
                            }
                            // The number the whole project exists to report on. Zero is
                            // shown as a problem, not as a value (V3.2).
                            if (order.totalMinor == 0L) {
                                StatusBadge("Nincs érték", Tone.Signal)
                            }
                        }
                    }
                }
            }
        }
    }
}
