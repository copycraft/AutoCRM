package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
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
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
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
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

@OptIn(FlowPreview::class)
class OrderListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        /** Background refetch with old rows kept — shows a progress line, not a spinner. */
        val refreshing: Boolean = false,
        val loadingMore: Boolean = false,
        val orders: List<OrderSummary> = emptyList(),
        val error: String? = null,
        val openOnly: Boolean = true,
        val hasMore: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private val queryFlow = MutableStateFlow("")
    val query: StateFlow<String> = queryFlow.asStateFlow()

    companion object {
        const val PAGE_SIZE = 50
    }

    init {
        // First paint must not wait for the debounce: previously every list opened
        // 300 ms late for no reason.
        viewModelScope.launch { load() }
        viewModelScope.launch {
            queryFlow.drop(1).debounce(300).distinctUntilChanged().collect { load() }
        }
    }

    fun setQuery(value: String) {
        queryFlow.value = value
    }

    fun toggleOpenOnly() {
        _state.value = _state.value.copy(openOnly = !_state.value.openOnly)
        viewModelScope.launch { load() }
    }

    fun refresh() = viewModelScope.launch { load() }

    fun retry() = viewModelScope.launch { load() }

    fun loadMore() {
        val s = _state.value
        if (!s.hasMore || s.loading || s.refreshing || s.loadingMore) return
        viewModelScope.launch {
            _state.value = _state.value.copy(loadingMore = true)
            try {
                val next = api.orders(
                    query = queryFlow.value.takeIf { it.isNotBlank() },
                    openOnly = _state.value.openOnly,
                    limit = PAGE_SIZE,
                    offset = _state.value.orders.size,
                )
                _state.value = _state.value.copy(
                    loadingMore = false,
                    orders = _state.value.orders + next,
                    hasMore = next.size >= PAGE_SIZE,
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loadingMore = false, error = describeError(e))
            }
        }
    }

    private suspend fun load() {
        val keepRows = _state.value.orders.isNotEmpty()
        _state.value = _state.value.copy(
            loading = !keepRows,
            refreshing = keepRows,
            loadingMore = false,
            error = null,
        )
        try {
            val orders = api.orders(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                openOnly = _state.value.openOnly,
                limit = PAGE_SIZE,
                offset = 0,
            )
            _state.value = _state.value.copy(
                loading = false,
                refreshing = false,
                orders = orders,
                hasMore = orders.size >= PAGE_SIZE,
            )
        } catch (e: Throwable) {
            _state.value = _state.value.copy(
                loading = false,
                refreshing = false,
                error = describeError(e),
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OrderListScreen(
    viewModel: OrderListViewModel,
    onOpen: (Long) -> Unit,
    onMenu: () -> Unit,
    canEdit: Boolean,
    onNewOrder: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Munkák",
                subtitle = if (state.orders.isNotEmpty()) "${state.orders.size} tétel" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::refresh,
            )
        },
        floatingActionButton = {
            if (canEdit) {
                FloatingActionButton(onClick = onNewOrder) {
                    Icon(Icons.Filled.Add, contentDescription = "Új megrendelés")
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            SearchField(
                value = query,
                onValueChange = viewModel::setQuery,
                label = "Keresés rendszám, szám vagy ügyfél szerint",
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                onSearch = viewModel::refresh,
            )
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterChip(
                    selected = state.openOnly,
                    onClick = { if (!state.openOnly) viewModel.toggleOpenOnly() },
                    label = { Text("Nyitott") },
                )
                FilterChip(
                    selected = !state.openOnly,
                    onClick = { if (state.openOnly) viewModel.toggleOpenOnly() },
                    label = { Text("Mind") },
                )
            }

            when {
                state.loading -> ListSkeleton(Modifier.padding(top = 4.dp))
                state.error != null && state.orders.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::retry)
                state.orders.isEmpty() -> EmptyState("Nincs megrendelés.")
                else -> PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::refresh,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.orders, key = { it.id }) { order ->
                            OrderCard(
                                order = order,
                                onOpen = { onOpen(order.id) },
                                modifier = Modifier.animateItem(),
                            )
                        }
                        if (state.hasMore) {
                            item {
                                OutlinedButton(
                                    onClick = viewModel::loadMore,
                                    enabled = !state.loadingMore,
                                    modifier = Modifier.fillMaxWidth(),
                                ) {
                                    if (state.loadingMore) {
                                        CircularProgressIndicator(
                                            strokeWidth = 2.dp,
                                            modifier = Modifier.padding(end = 8.dp),
                                        )
                                    }
                                    Text(if (state.loadingMore) "Betöltés…" else "Továbbiak")
                                }
                            }
                        }
                    }
                }
            }
            // A background error with rows on screen: keep the rows, show the retry inline.
            if (state.error != null && state.orders.isNotEmpty() && !state.refreshing) {
                ErrorState(state.error!!, onRetry = viewModel::retry)
            }
        }
    }
}

@Composable
private fun OrderCard(order: OrderSummary, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    Card(modifier = modifier, onClick = onOpen) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                order.vehiclePlate ?: order.number,
                style = MaterialTheme.typography.titleLarge,
            )
            Text(
                formatMoney(order.totalMinor, order.currency),
                style = MonoSmall,
            )
        }
        Text(order.title, style = MaterialTheme.typography.titleMedium)
        Text(
            listOfNotNull(order.partnerName, order.number.takeIf { order.vehiclePlate != null })
                .joinToString(" · ").ifBlank { "—" },
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
