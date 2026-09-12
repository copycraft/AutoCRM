package hu.autotherm.autocrm.ui.leads

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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.LeadDetail
import hu.autotherm.autocrm.data.api.LeadSummary
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.LoadingState
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import hu.autotherm.autocrm.util.formatDateTime
import hu.autotherm.autocrm.util.formatMoney
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch
import java.time.LocalDate

@OptIn(FlowPreview::class)
class LeadListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val leads: List<LeadSummary> = emptyList(),
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
            val leads = api.leads(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                openOnly = _state.value.openOnly,
            )
            _state.value = _state.value.copy(loading = false, leads = leads)
        } catch (e: Throwable) {
            _state.value = _state.value.copy(loading = false, error = describeError(e))
        }
    }
}

@Composable
fun LeadListScreen(viewModel: LeadListViewModel, onOpen: (Long) -> Unit) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    Column(Modifier.fillMaxSize().padding(16.dp)) {
        OutlinedTextField(
            value = query,
            onValueChange = viewModel::setQuery,
            label = { Text("Keresés cím, kapcsolattartó, ügyfél szerint") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            if (state.openOnly) "Csak nyitott · koppints az összeshez" else "Minden lead · koppints a nyitottakhoz",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
            modifier = Modifier.padding(vertical = 8.dp).clickable { viewModel.toggleOpenOnly() },
        )

        when {
            state.loading && state.leads.isEmpty() -> LoadingState()
            state.error != null && state.leads.isEmpty() -> ErrorState(state.error!!, onRetry = viewModel::retry)
            state.leads.isEmpty() -> EmptyState("Nincs lead.")
            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(state.leads, key = { it.id }) { lead ->
                    Card(Modifier.clickable { onOpen(lead.id) }) {
                        Text(lead.title, style = MaterialTheme.typography.titleMedium)
                        Text(
                            listOfNotNull(lead.partnerName, lead.contactName).joinToString(" · ")
                                .ifBlank { "—" },
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            StatusBadge(lead.stageLabel, Tone.Steel)
                            lead.orderNumber?.let { StatusBadge(it, Tone.Done) }
                        }
                    }
                }
            }
        }
    }
}

class LeadDetailViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val detail: LeadDetail? = null,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            _state.value = State(loading = true)
            try {
                _state.value = State(loading = false, detail = api.lead(id))
            } catch (e: Throwable) {
                _state.value = State(loading = false, error = describeError(e))
            }
        }
    }
}

@Composable
fun LeadDetailScreen(leadId: Long, viewModel: LeadDetailViewModel, onOpenOrder: (Long) -> Unit) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(leadId) { viewModel.load(leadId) }

    when {
        state.loading -> LoadingState()
        state.error != null -> ErrorState(state.error!!) { viewModel.load(leadId) }
        state.detail == null -> ErrorState("Nem található.") { viewModel.load(leadId) }
        else -> {
            val detail = state.detail!!
            val lead = detail.lead
            LazyColumn(
                Modifier.fillMaxSize().padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item { Text(lead.title, style = MaterialTheme.typography.headlineMedium) }
                item {
                    Card {
                        Info("Kapcsolattartó", lead.contactName)
                        Info("E-mail", lead.contactEmail)
                        Info("Telefon", lead.contactPhone)
                        Info("Forrás", lead.source)
                        Info("Leírás", lead.description)
                    }
                }
                // V2.3: the quotation. The six weeks between "we sent a price" and "they
                // said yes" were invisible before this existed.
                item {
                    Card {
                        SectionTitle("Árajánlat")
                        Info(
                            "Összeg",
                            lead.quotedValueMinor?.let { formatMoney(it, lead.currency ?: "HUF") },
                            mono = true,
                        )
                        Row(
                            Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Info("Érvényes eddig", formatDate(lead.quoteValidUntil))
                            if (lead.quoteValidUntil?.let { isExpired(it) } == true) {
                                StatusBadge("Lejárt", Tone.Signal)
                            }
                        }
                    }
                }
                if (detail.orders.isNotEmpty()) {
                    item { SectionTitle("Megrendelések (${detail.orders.size})") }
                    items(detail.orders, key = { it.id }) { order ->
                        Card(Modifier.clickable { onOpenOrder(order.id) }) {
                            Text(order.number, style = MonoSmall)
                        }
                    }
                }
                if (detail.history.isNotEmpty()) {
                    item { SectionTitle("Előzmények") }
                    items(detail.history, key = { it.id }) { entry ->
                        Card {
                            Text(entry.labelHu, style = MaterialTheme.typography.titleMedium)
                            Text(
                                formatDateTime(entry.enteredAt).orEmpty(),
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

/** Plain `YYYY-MM-DD` from the API, compared as a date rather than a timestamp. */
internal fun isExpired(validUntil: String): Boolean =
    runCatching { LocalDate.parse(validUntil).isBefore(LocalDate.now()) }.getOrDefault(false)
