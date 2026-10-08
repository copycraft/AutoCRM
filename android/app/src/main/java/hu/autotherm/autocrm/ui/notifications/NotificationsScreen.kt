package hu.autotherm.autocrm.ui.notifications

import hu.autotherm.autocrm.ui.common.SectionTitle
import androidx.lifecycle.repeatOnLifecycle
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.NotificationItem
import hu.autotherm.autocrm.data.notifications.NotificationPlanner
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDateTime
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** The user's notification feed: what the system notifications announce, kept to read later. */
class NotificationsViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val items: List<NotificationItem> = emptyList(),
        val unread: Long = 0,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** [silent]: the minute tick — no progress line, and a failure keeps the list as is. */
    fun load(silent: Boolean = false) {
        viewModelScope.launch {
            val keep = _state.value.items.isNotEmpty()
            if (!silent) _state.value = _state.value.copy(loading = !keep, refreshing = keep, error = null)
            try {
                val feed = api.notifications()
                _state.value = State(loading = false, items = feed.items, unread = feed.unread)
            } catch (e: Throwable) {
                if (silent) return@launch
                _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
            }
        }
    }

    /** Marks one read and returns the in-app route its link points at, if any. */
    fun open(item: NotificationItem, onRoute: (String) -> Unit) {
        if (item.readAt == null) {
            // Read at once on screen; the server is told in the background.
            _state.value = _state.value.copy(
                items = _state.value.items.map { if (it.id == item.id) it.copy(readAt = "most") else it },
                unread = (_state.value.unread - 1).coerceAtLeast(0L),
            )
            viewModelScope.launch {
                runCatching { api.markNotificationsRead(listOf(item.id)) }
                load(silent = true)
            }
        }
        val route = NotificationPlanner.routeForLink(item.link)
        if (route != null) {
            onRoute(route)
        } else if (item.link != null) {
            // A tap that does nothing reads as a broken app; say where it opens instead.
            hu.autotherm.autocrm.ui.common.Toasts.show("Ez a webes felületen nyitható meg.")
        }
    }

    fun markAllRead() {
        // All read on screen at once; the server catches up in the background.
        _state.value = _state.value.copy(
            items = _state.value.items.map { if (it.readAt == null) it.copy(readAt = "most") else it },
            unread = 0,
        )
        viewModelScope.launch {
            runCatching { api.markAllNotificationsRead() }
                .onSuccess { hu.autotherm.autocrm.ui.common.Toasts.show("Minden értesítés olvasott") }
            load(silent = true)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotificationsScreen(
    viewModel: NotificationsViewModel,
    onMenu: () -> Unit,
    onRoute: (String) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.load() }
    // While the feed is on screen it keeps itself current, and "5 perce" stays true.
    val owner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    LaunchedEffect(owner) {
        owner.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.RESUMED) {
            while (true) {
                kotlinx.coroutines.delay(60_000)
                viewModel.load(silent = true)
            }
        }
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Értesítések",
                subtitle = if (state.unread > 0) "${state.unread} olvasatlan" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
                actions = {
                    if (state.unread > 0) {
                        TextButton(onClick = viewModel::markAllRead) { Text("Mind olvasott") }
                    }
                },
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            when {
                state.loading -> ListSkeleton()
                state.error != null && state.items.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::load)
                state.items.isEmpty() -> EmptyState("Nincs értesítés.")
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(isRefreshing = state.refreshing, onRefresh = viewModel::load) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        // Today, yesterday, earlier: a feed reads by day.
                        val zone = java.time.ZoneId.of("Europe/Budapest")
                        val today = java.time.LocalDate.now(zone)
                        val groups = state.items.groupBy { item ->
                            val day = runCatching { java.time.Instant.parse(item.createdAt).atZone(zone).toLocalDate() }.getOrNull()
                            when (day) {
                                today -> "Ma"
                                today.minusDays(1) -> "Tegnap"
                                else -> "Korábban"
                            }
                        }
                        groups.forEach { (heading, dayItems) ->
                            if (groups.size > 1) {
                                item(key = "h-$heading") { SectionTitle(heading, count = dayItems.size) }
                            }
                            items(dayItems, key = { it.id }) { n ->
                            Card(
                                modifier = Modifier.animateItem(),
                                onClick = { viewModel.open(n, onRoute) },
                            ) {
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                    Text(
                                        n.title,
                                        style = MaterialTheme.typography.titleMedium,
                                        fontWeight = if (n.readAt == null) FontWeight.Bold else FontWeight.Normal,
                                        modifier = Modifier.weight(1f, fill = false),
                                    )
                                    if (n.readAt == null) StatusBadge("Új", Tone.Cold)
                                }
                                n.body?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
                                Text(
                                    hu.autotherm.autocrm.util.relativeTime(n.createdAt).orEmpty(),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                            }
                        }
                        }
                    }
                }
            }
        }
    }
}
