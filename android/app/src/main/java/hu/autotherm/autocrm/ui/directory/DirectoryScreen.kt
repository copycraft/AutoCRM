package hu.autotherm.autocrm.ui.directory

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
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
import hu.autotherm.autocrm.data.api.Contact
import hu.autotherm.autocrm.data.api.Partner
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/**
 * Névjegyzék: the whole address book in one place — every partner with their
 * kapcsolattartók, searchable in a single field.
 *
 * The two chips on top are the whole taxonomy: Ügyfél and/or Beszállító. Both on
 * means everyone; either alone filters by that side. Contacts load lazily per
 * partner the first time its row expands, and stay cached while the tab lives.
 */
@OptIn(FlowPreview::class)
class DirectoryViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class ContactsState(
        val loading: Boolean = false,
        val contacts: List<Contact> = emptyList(),
    )

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val partners: List<Partner> = emptyList(),
        val error: String? = null,
        val showCustomers: Boolean = true,
        val showSuppliers: Boolean = true,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private val _contacts = MutableStateFlow<Map<Long, ContactsState>>(emptyMap())
    val contacts: StateFlow<Map<Long, ContactsState>> = _contacts.asStateFlow()
    private val queryFlow = MutableStateFlow("")
    val query: StateFlow<String> = queryFlow.asStateFlow()

    companion object {
        const val PAGE_SIZE = 200
    }

    init {
        viewModelScope.launch { load() }
        viewModelScope.launch {
            queryFlow.drop(1).debounce(300).distinctUntilChanged().collect { load() }
        }
    }

    fun setQuery(value: String) {
        queryFlow.value = value
    }

    fun toggleCustomers() {
        _state.value = _state.value.copy(showCustomers = !_state.value.showCustomers)
        viewModelScope.launch { load() }
    }

    fun toggleSuppliers() {
        _state.value = _state.value.copy(showSuppliers = !_state.value.showSuppliers)
        viewModelScope.launch { load() }
    }

    fun refresh() = viewModelScope.launch { load() }

    fun retry() = viewModelScope.launch { load() }

    private fun roleFilter(): String? {
        val s = _state.value
        return when {
            s.showCustomers && s.showSuppliers -> null
            s.showCustomers -> "customer"
            s.showSuppliers -> "supplier"
            else -> null
        }
    }

    private suspend fun load() {
        val s = _state.value
        if (!s.showCustomers && !s.showSuppliers) {
            _state.value = s.copy(loading = false, refreshing = false, partners = emptyList(), error = null)
            return
        }
        val keepRows = s.partners.isNotEmpty()
        _state.value = s.copy(loading = !keepRows, refreshing = keepRows, error = null)
        try {
            val partners = api.partners(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                role = roleFilter(),
                limit = PAGE_SIZE,
            )
            _state.value = _state.value.copy(loading = false, refreshing = false, partners = partners)
        } catch (e: Throwable) {
            _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
        }
    }

    /** Loads a partner's contacts on first expand; cached while the tab lives. */
    fun ensureContacts(partnerId: Long) {
        if (_contacts.value[partnerId] != null) return
        _contacts.value = _contacts.value + (partnerId to ContactsState(loading = true))
        viewModelScope.launch {
            try {
                val detail = api.partner(partnerId)
                _contacts.value = _contacts.value +
                    (partnerId to ContactsState(contacts = detail.contacts))
            } catch (e: Throwable) {
                _contacts.value = _contacts.value +
                    (partnerId to ContactsState(contacts = emptyList()))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DirectoryScreen(
    viewModel: DirectoryViewModel,
    canEdit: Boolean,
    onMenu: () -> Unit,
    onOpenPartner: (Long) -> Unit,
    onNewPartner: () -> Unit,
    onEditPartner: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val contacts by viewModel.contacts.collectAsState()
    val query by viewModel.query.collectAsState()
    // Plain remember: a Set is not saveable, and rememberSaveable would throw the
    // moment the system tries to persist it (rotation/background = crash).
    var expanded by remember { mutableStateOf(setOf<Long>()) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Névjegyzék",
                subtitle = if (state.partners.isNotEmpty()) "${state.partners.size} partner" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::refresh,
            )
        },
        floatingActionButton = {
            if (canEdit) {
                FloatingActionButton(onClick = onNewPartner) {
                    Icon(Icons.Filled.Add, contentDescription = "Új partner")
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            SearchField(
                value = query,
                onValueChange = viewModel::setQuery,
                label = "Keresés név, adószám, város, kapcsolattartó szerint",
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                onSearch = viewModel::refresh,
            )
            // The taxonomy widgets: which side of the business the row is on.
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterChip(
                    selected = state.showCustomers,
                    onClick = viewModel::toggleCustomers,
                    label = { Text("Ügyfél") },
                )
                FilterChip(
                    selected = state.showSuppliers,
                    onClick = viewModel::toggleSuppliers,
                    label = { Text("Beszállító") },
                )
            }

            when {
                state.loading -> ListSkeleton(Modifier.padding(top = 4.dp))
                state.error != null && state.partners.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::retry)
                state.partners.isEmpty() -> EmptyState("Nincs találat.")
                else -> PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::refresh,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.partners, key = { it.id }) { partner ->
                            val isOpen = partner.id in expanded
                            val cs = contacts[partner.id]
                            DirectoryRow(
                                partner = partner,
                                expanded = isOpen,
                                contacts = cs?.contacts.orEmpty(),
                                contactsLoading = cs?.loading == true,
                                canEdit = canEdit,
                                modifier = Modifier.animateItem(),
                                onToggle = {
                                    val m = expanded.toMutableSet()
                                    if (partner.id in m) {
                                        m.remove(partner.id)
                                    } else {
                                        m.add(partner.id)
                                        viewModel.ensureContacts(partner.id)
                                    }
                                    expanded = m
                                },
                                onOpen = { onOpenPartner(partner.id) },
                                onEdit = { onEditPartner(partner.id) },
                            )
                        }
                    }
                }
            }
            if (state.error != null && state.partners.isNotEmpty() && !state.refreshing) {
                ErrorState(state.error!!, onRetry = viewModel::retry)
            }
        }
    }
}

@Composable
private fun DirectoryRow(
    partner: Partner,
    expanded: Boolean,
    contacts: List<Contact>,
    contactsLoading: Boolean,
    canEdit: Boolean,
    modifier: Modifier = Modifier,
    onToggle: () -> Unit,
    onOpen: () -> Unit,
    onEdit: () -> Unit,
) {
    Card(modifier = modifier) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f, fill = false)) {
                Text(partner.name, style = MaterialTheme.typography.titleMedium)
                Text(
                    listOfNotNull(partner.city, partner.phone).joinToString(" · ").ifBlank { "—" },
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (canEdit) {
                    IconButton(onClick = onEdit) {
                        Icon(Icons.Filled.Edit, contentDescription = "Szerkesztés", tint = Steel500)
                    }
                }
                IconButton(onClick = onToggle) {
                    Icon(
                        if (expanded) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                        contentDescription = if (expanded) "Bezárás" else "Kapcsolattartók",
                    )
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (partner.role == "customer" || partner.role == "both" || partner.role == null) {
                StatusBadge("Ügyfél", Tone.Cold)
            }
            if (partner.role == "supplier" || partner.role == "both") {
                StatusBadge("Beszállító", Tone.Steel)
            }
        }
        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically(tween(220)) + fadeIn(tween(180)),
            exit = shrinkVertically(tween(180)) + fadeOut(tween(150)),
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                when {
                    contactsLoading -> CircularProgressIndicator(
                        strokeWidth = 2.dp,
                        modifier = Modifier.size(24.dp),
                    )
                    contacts.isEmpty() -> Text(
                        "Nincs kapcsolattartó. Részletek a partner adatlapján.",
                        style = MaterialTheme.typography.labelMedium,
                        color = Steel500,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    else -> Column(
                        Modifier.fillMaxWidth().padding(top = 4.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        contacts.forEach { contact ->
                            Column {
                                Text(contact.name, style = MaterialTheme.typography.bodyLarge)
                                Text(
                                    listOfNotNull(contact.position, contact.email, contact.phone)
                                        .joinToString(" · ").ifBlank { "—" },
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                            }
                        }
                    }
                }
                Row(
                    Modifier.fillMaxWidth().padding(top = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    TextButton(onClick = onOpen) { Text("Adatlap") }
                }
            }
        }
    }
}
