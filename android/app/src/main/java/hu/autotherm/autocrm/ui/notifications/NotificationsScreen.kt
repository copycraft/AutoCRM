package hu.autotherm.autocrm.ui.notifications

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

    fun load() {
        viewModelScope.launch {
            val keep = _state.value.items.isNotEmpty()
            _state.value = _state.value.copy(loading = !keep, refreshing = keep, error = null)
            try {
                val feed = api.notifications()
                _state.value = State(loading = false, items = feed.items, unread = feed.unread)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
            }
        }
    }

    /** Marks one read and returns the in-app route its link points at, if any. */
    fun open(item: NotificationItem, onRoute: (String) -> Unit) {
        if (item.readAt == null) {
            viewModelScope.launch {
                runCatching { api.markNotificationsRead(listOf(item.id)) }
                load()
            }
        }
        NotificationPlanner.routeForLink(item.link)?.let(onRoute)
    }

    fun markAllRead() {
        viewModelScope.launch {
            runCatching { api.markAllNotificationsRead() }
            load()
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
                else -> PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = viewModel::load) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.items, key = { it.id }) { n ->
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
                                    formatDateTime(n.createdAt).orEmpty(),
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
