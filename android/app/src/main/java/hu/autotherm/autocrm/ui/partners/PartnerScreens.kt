package hu.autotherm.autocrm.ui.partners

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
import androidx.compose.material3.OutlinedButton
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
import hu.autotherm.autocrm.data.api.Partner
import hu.autotherm.autocrm.data.api.PartnerDetail
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.LoadingState
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.orders.describe
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
class PartnerListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val partners: List<Partner> = emptyList(),
        val error: String? = null,
        /** null = everyone, "customer" / "supplier" = the V2.6 filter. */
        val role: String? = "customer",
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

    fun setRole(role: String?) {
        _state.value = _state.value.copy(role = role)
        viewModelScope.launch { load() }
    }

    fun retry() = viewModelScope.launch { load() }

    private suspend fun load() {
        _state.value = _state.value.copy(loading = true, error = null)
        try {
            val partners = api.partners(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                role = _state.value.role,
            )
            _state.value = _state.value.copy(loading = false, partners = partners)
        } catch (e: ApiException) {
            _state.value = _state.value.copy(loading = false, error = describe(e))
        }
    }
}

@Composable
fun PartnerListScreen(
    viewModel: PartnerListViewModel,
    onOpenPartner: (Long) -> Unit,
    onOpenLeads: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    Column(Modifier.fillMaxSize().padding(16.dp)) {
        OutlinedTextField(
            value = query,
            onValueChange = viewModel::setQuery,
            label = { Text("Keresés név, adószám, város szerint") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Row(
            Modifier.fillMaxWidth().padding(vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            RoleChip("Ügyfelek", state.role == "customer") { viewModel.setRole("customer") }
            RoleChip("Beszállítók", state.role == "supplier") { viewModel.setRole("supplier") }
            RoleChip("Mind", state.role == null) { viewModel.setRole(null) }
            OutlinedButton(onClick = onOpenLeads) { Text("Leadek") }
        }

        when {
            state.loading && state.partners.isEmpty() -> LoadingState()
            state.error != null && state.partners.isEmpty() -> ErrorState(state.error!!, onRetry = viewModel::retry)
            state.partners.isEmpty() -> EmptyState("Nincs találat.")
            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(state.partners, key = { it.id }) { partner ->
                    Card(Modifier.clickable { onOpenPartner(partner.id) }) {
                        Text(partner.name, style = MaterialTheme.typography.titleMedium)
                        Text(
                            listOfNotNull(partner.city, partner.email, partner.phone).joinToString(" · ")
                                .ifBlank { "—" },
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                        if (partner.role == "supplier" || partner.role == "both") {
                            StatusBadge("Beszállító", Tone.Cold)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun RoleChip(label: String, selected: Boolean, onClick: () -> Unit) {
    StatusBadge(
        label,
        if (selected) Tone.Cold else Tone.Steel,
        Modifier.clickable(onClick = onClick).padding(vertical = 4.dp),
    )
}

class PartnerDetailViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val detail: PartnerDetail? = null,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            _state.value = State(loading = true)
            try {
                _state.value = State(loading = false, detail = api.partner(id))
            } catch (e: ApiException) {
                _state.value = State(loading = false, error = describe(e))
            }
        }
    }
}

@Composable
fun PartnerDetailScreen(
    partnerId: Long,
    viewModel: PartnerDetailViewModel,
    onOpenOrder: (Long) -> Unit,
    onOpenLead: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(partnerId) { viewModel.load(partnerId) }

    when {
        state.loading -> LoadingState()
        state.error != null -> ErrorState(state.error!!) { viewModel.load(partnerId) }
        state.detail == null -> ErrorState("Nem található.") { viewModel.load(partnerId) }
        else -> {
            val detail = state.detail!!
            LazyColumn(
                Modifier.fillMaxSize().padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    Column {
                        Text(detail.partner.name, style = MaterialTheme.typography.headlineMedium)
                        Text(
                            if (detail.partner.kind == "business") "Vállalkozás" else "Magánszemély",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                }
                item {
                    Card {
                        Info("Adószám", detail.partner.taxNumber, mono = true)
                        Info("Ország", detail.partner.country, mono = true)
                        Info("E-mail", detail.partner.email)
                        Info("Telefon", detail.partner.phone)
                        Info(
                            "Cím",
                            listOfNotNull(detail.partner.city, detail.partner.addressLine)
                                .joinToString(", ").ifBlank { null },
                        )
                    }
                }
                if (detail.contacts.isNotEmpty()) {
                    item { SectionTitle("Kapcsolattartók (${detail.contacts.size})") }
                    items(detail.contacts, key = { it.id }) { contact ->
                        Card {
                            Text(contact.name, style = MaterialTheme.typography.titleMedium)
                            Text(
                                listOfNotNull(contact.position, contact.email, contact.phone)
                                    .joinToString(" · ").ifBlank { "—" },
                                style = MaterialTheme.typography.labelMedium,
                                color = Steel500,
                            )
                        }
                    }
                }
                item { SectionTitle("Megrendelések (${detail.orders.size})") }
                items(detail.orders, key = { it.id }) { order ->
                    Card(Modifier.clickable { onOpenOrder(order.id) }) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(order.number, style = MonoSmall)
                            Text(formatMoney(order.totalMinor, order.currency), style = MonoSmall)
                        }
                        Text(order.title, style = MaterialTheme.typography.bodyLarge)
                        StatusBadge(order.stageLabel, Tone.Steel)
                    }
                }
                // V6: without this, nobody opening a partner sees they were quoted before.
                item { SectionTitle("Leadek (${detail.leads.size})") }
                items(detail.leads, key = { it.id }) { lead ->
                    Card(Modifier.clickable { onOpenLead(lead.id) }) {
                        Text(lead.title, style = MaterialTheme.typography.bodyLarge)
                        StatusBadge(lead.stageLabel, Tone.Steel)
                    }
                }
            }
        }
    }
}
