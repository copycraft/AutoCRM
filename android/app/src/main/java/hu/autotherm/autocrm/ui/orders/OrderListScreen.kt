package hu.autotherm.autocrm.ui.orders

import androidx.compose.material.icons.filled.Sort
import kotlinx.coroutines.flow.map
import androidx.compose.runtime.setValue
import androidx.compose.material.icons.filled.PhotoCamera
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
class OrderListViewModel(
    private val api: AutoCrmApi,
    /** Where the "open only" choice is kept between launches; null in tests. */
    private val prefs: android.content.SharedPreferences? = null,
) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        /** Background refetch with old rows kept — shows a progress line, not a spinner. */
        val refreshing: Boolean = false,
        val loadingMore: Boolean = false,
        val orders: List<OrderSummary> = emptyList(),
        val error: String? = null,
        val openOnly: Boolean = true,
        /** Newest first, nearest deadline first, or longest in its stage first. */
        val sort: String = "-created_at",
        val hasMore: Boolean = false,
    )

    // The list opens filtered the way it was left, not reset to the default every launch.
    private val _state = MutableStateFlow(
        State(
            openOnly = prefs?.getBoolean("orders_open_only", true) ?: true,
            sort = prefs?.getString("orders_sort", null) ?: "-created_at",
        ),
    )
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

    /**
     * Back from a job: the rows already on screen are fetched again in one go (as many as
     * were loaded, up to 200), so a changed stage shows and the scroll position holds.
     */
    fun refreshInPlace() {
        val s = _state.value
        if (s.loading || s.refreshing || s.orders.isEmpty()) return
        viewModelScope.launch {
            val count = s.orders.size.coerceAtLeast(PAGE_SIZE).coerceAtMost(200)
            runCatching {
                api.orders(
                    query = queryFlow.value.takeIf { it.isNotBlank() },
                    openOnly = _state.value.openOnly,
                    limit = count,
                    offset = 0,
                    sort = _state.value.sort,
                )
            }.onSuccess { rows ->
                _state.value = _state.value.copy(orders = rows, hasMore = rows.size >= count)
            }
        }
    }

    fun setSort(sort: String) {
        if (sort == _state.value.sort) return
        _state.value = _state.value.copy(sort = sort)
        prefs?.edit()?.putString("orders_sort", sort)?.apply()
        viewModelScope.launch { load() }
    }

    fun toggleOpenOnly() {
        _state.value = _state.value.copy(openOnly = !_state.value.openOnly)
        prefs?.edit()?.putBoolean("orders_open_only", _state.value.openOnly)?.apply()
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
                    sort = _state.value.sort,
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
                sort = _state.value.sort,
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
    /** Straight to the camera for an order (long-press menu); null without upload rights. */
    onCapture: ((Long) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    // Photos still on this phone, per order: a van whose pictures have not gone yet says so.
    val context = androidx.compose.ui.platform.LocalContext.current
    val pendingByOrder by androidx.compose.runtime.remember {
        (context.applicationContext as hu.autotherm.autocrm.AutoCrmApp).database.pendingUploads().watchQueue()
            .map { rows -> rows.filter { it.state != "blocked" }.groupingBy { it.orderId }.eachCount() }
    }.collectAsState(initial = emptyMap())
    var actionsFor by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf<OrderSummary?>(null) }
    // Press and hold a job: the few things done to it without opening it.
    actionsFor?.let { order ->
        val clipboard = androidx.compose.ui.platform.LocalClipboardManager.current
        hu.autotherm.autocrm.ui.common.DialogShell(
            title = listOfNotNull(order.vehiclePlate, "#" + order.number).joinToString(" · "),
            onDismiss = { actionsFor = null },
            actions = { androidx.compose.material3.TextButton(onClick = { actionsFor = null }) { Text("Mégse") } },
        ) {
            Text(order.title, style = MaterialTheme.typography.bodyLarge, color = Steel500, maxLines = 2)
            hu.autotherm.autocrm.ui.common.SecondaryButton(
                text = "Megnyitás",
                onClick = {
                    actionsFor = null
                    onOpen(order.id)
                },
                modifier = Modifier.fillMaxWidth(),
            )
            if (onCapture != null) {
                hu.autotherm.autocrm.ui.common.SecondaryButton(
                    text = "Fotózás",
                    onClick = {
                        actionsFor = null
                        onCapture(order.id)
                    },
                    icon = androidx.compose.material.icons.Icons.Filled.PhotoCamera,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            order.vehiclePlate?.let { plate ->
                hu.autotherm.autocrm.ui.common.SecondaryButton(
                    text = "Rendszám másolása",
                    onClick = {
                        clipboard.setText(androidx.compose.ui.text.AnnotatedString(plate))
                        actionsFor = null
                        hu.autotherm.autocrm.ui.common.Toasts.show("Vágólapra másolva: $plate")
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
    val topScroll = hu.autotherm.autocrm.ui.common.rememberTopScrollableListState("orders")
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.refreshInPlace() }
    val query by viewModel.query.collectAsState()

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Munkák",
                // The header answers "how bad is it": how many, and how many are late.
                subtitle = if (state.orders.isNotEmpty()) {
                    val today = java.time.LocalDate.now().toString()
                    val late = state.orders.count { !it.stageIsTerminal && it.dueDate != null && it.dueDate < today }
                    "${state.orders.size}${if (state.hasMore) "+" else ""} munka" + if (late > 0) " · $late lejárt" else ""
                } else {
                    null
                },
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::refresh,
            )
        },
        floatingActionButton = {
            if (canEdit) {
                hu.autotherm.autocrm.ui.common.NewFab("Új munka", onNewOrder)
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
            // What comes first: the newest job, the nearest deadline, or the van stuck longest.
            androidx.compose.foundation.lazy.LazyRow(
                Modifier.fillMaxWidth().padding(bottom = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val sorts = listOf("-created_at" to "Legújabb", "due_date" to "Határidő szerint", "stage_entered_at" to "Régóta áll")
                items(sorts.size) { i ->
                    val (key, label) = sorts[i]
                    FilterChip(
                        selected = state.sort == key,
                        onClick = { viewModel.setSort(key) },
                        label = { Text(label) },
                        leadingIcon = if (state.sort == key) {
                            { androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.Filled.Sort, contentDescription = null) }
                        } else {
                            null
                        },
                    )
                }
            }

            when {
                state.loading -> ListSkeleton(Modifier.padding(top = 4.dp))
                state.error != null && state.orders.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::retry)
                state.orders.isEmpty() -> run {
                    val q = viewModel.query.collectAsState().value
                    if (q.isNotBlank()) {
                        EmptyState(
                            "Nincs találat erre: „${q.trim()}”.",
                            actionLabel = "Keresés törlése",
                            onAction = { viewModel.setQuery("") },
                        )
                    } else {
                        EmptyState(
                            "Még nincs munka.",
                            actionLabel = if (canEdit) "Új munka" else null,
                            onAction = onNewOrder,
                        )
                    }
                }
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::refresh,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        state = topScroll,
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.orders, key = { it.id }) { order ->
                            OrderCard(
                                order = order,
                                onOpen = { onOpen(order.id) },
                                pendingPhotos = pendingByOrder[order.id] ?: 0,
                                onLongPress = { actionsFor = order },
                                modifier = Modifier.animateItem(),
                            )
                        }
                        if (state.hasMore) {
                            item {
                                // Scrolling to the end loads the next page by itself; the
                                // button stays for a page that failed.
                                androidx.compose.runtime.LaunchedEffect(state.orders.size) { viewModel.loadMore() }
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
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
private fun OrderCard(
    order: OrderSummary,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
    pendingPhotos: Int = 0,
    onLongPress: (() -> Unit)? = null,
) {
    Card(modifier = modifier, onClick = onOpen, onLongClick = onLongPress) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                order.vehiclePlate?.let { hu.autotherm.autocrm.ui.common.PlateBadge(it) }
                Text(order.number, style = MonoSmall, color = Steel500)
            }
            Text(
                formatMoney(order.totalMinor, order.currency),
                style = MonoSmall,
            )
        }
        Text(order.title, style = MaterialTheme.typography.titleMedium, maxLines = 2)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            hu.autotherm.autocrm.ui.common.InitialsAvatar(order.partnerName, size = 24.dp)
            Text(order.partnerName, style = MaterialTheme.typography.bodySmall, color = Steel500, maxLines = 1)
        }
        // Wraps instead of cutting off: a late job with blockers and no value has four badges.
        androidx.compose.foundation.layout.FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            if (pendingPhotos > 0) {
                StatusBadge("$pendingPhotos fotó feltöltésre vár", Tone.Cold)
            }
            order.dueDate?.let { due ->
                val late = !order.stageIsTerminal && runCatching { java.time.LocalDate.parse(due).isBefore(java.time.LocalDate.now()) }.getOrDefault(false)
                StatusBadge(
                    (if (late) "Lejárt: " else "Határidő: ") + hu.autotherm.autocrm.util.relativeDay(due),
                    if (late) Tone.Signal else Tone.Cold,
                )
            }
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
