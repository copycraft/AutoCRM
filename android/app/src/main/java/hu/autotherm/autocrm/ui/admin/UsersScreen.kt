package hu.autotherm.autocrm.ui.admin

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.StaffUser
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

private val ROLES = listOf("admin", "office", "designer", "viewer")

private fun roleLabel(role: String): String = when (role) {
    "admin" -> "Adminisztrátor"
    "office" -> "Irodai"
    "designer" -> "Tervező"
    "viewer" -> "Megtekintő"
    else -> role
}

/**
 * Every registered user and what they may do. Admin only: the server refuses anyone else,
 * and the drawer does not show the entry. Creating accounts and resetting passwords stay on
 * the web client, where a temporary password is typed on a real keyboard.
 */
class UsersViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val users: List<StaffUser> = emptyList(),
        val savingId: Long? = null,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load() {
        viewModelScope.launch {
            val keepRows = _state.value.users.isNotEmpty()
            _state.value = _state.value.copy(loading = !keepRows, refreshing = keepRows, error = null)
            try {
                _state.value = _state.value.copy(loading = false, refreshing = false, users = api.users())
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
            }
        }
    }

    /** One PATCH per change; the row is replaced by the server's answer, refusals included. */
    private fun patch(user: StaffUser, field: String, value: JsonElement) {
        if (_state.value.savingId != null) return
        viewModelScope.launch {
            _state.value = _state.value.copy(savingId = user.id, error = null)
            try {
                val updated = api.patchUser(user.id, buildJsonObject { put(field, value) })
                _state.value = _state.value.copy(
                    savingId = null,
                    users = _state.value.users.map { if (it.id == updated.id) updated else it },
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(savingId = null, error = describeError(e))
            }
        }
    }

    fun setRole(user: StaffUser, role: String) = patch(user, "role", JsonPrimitive(role))
    fun setActive(user: StaffUser, active: Boolean) = patch(user, "is_active", JsonPrimitive(active))
    fun setHrAccess(user: StaffUser, on: Boolean) = patch(user, "hr_access", JsonPrimitive(on))
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UsersScreen(viewModel: UsersViewModel, myUserId: Long, onMenu: () -> Unit) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.load() }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Felhasználók",
                subtitle = if (state.users.isNotEmpty()) "${state.users.size} fiók" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            // A refused change (the last admin, say) keeps the list on screen.
            if (state.error != null && state.users.isNotEmpty()) {
                Text(
                    state.error!!,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
            }
            when {
                state.loading -> ListSkeleton()
                state.error != null && state.users.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::load)
                state.users.isEmpty() -> EmptyState("Még nincs felhasználó.")
                else -> PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = viewModel::load) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.users, key = { it.id }) { user ->
                            UserCard(
                                user = user,
                                self = user.id == myUserId,
                                busy = state.savingId != null,
                                viewModel = viewModel,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun UserCard(user: StaffUser, self: Boolean, busy: Boolean, viewModel: UsersViewModel) {
    var menu by remember { mutableStateOf(false) }
    Card {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Column(Modifier.weight(1f)) {
                Text(user.displayName, style = MaterialTheme.typography.titleMedium)
                Text(user.email, style = MaterialTheme.typography.labelMedium, color = Steel500)
            }
            if (self) StatusBadge("Ön", Tone.Cold)
        }
        // Not your own role: demoting yourself is the quickest way to lock out.
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text("Szerepkör", Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
            Column {
                OutlinedButton(onClick = { menu = true }, enabled = !self && !busy) {
                    Text(roleLabel(user.role))
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    ROLES.forEach { role ->
                        DropdownMenuItem(
                            text = { Text(roleLabel(role)) },
                            onClick = {
                                menu = false
                                if (role != user.role) viewModel.setRole(user, role)
                            },
                        )
                    }
                }
            }
        }
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text("Aktív", Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
            Switch(
                checked = user.isActive,
                onCheckedChange = { viewModel.setActive(user, it) },
                enabled = !self && !busy,
            )
        }
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text("HR-hozzáférés", Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
            if (user.role == "admin") {
                Text("Mindig", style = MaterialTheme.typography.bodyMedium, color = Steel500)
            } else {
                Switch(
                    checked = user.hrAccess,
                    onCheckedChange = { viewModel.setHrAccess(user, it) },
                    enabled = !busy,
                )
            }
        }
    }
}
