package hu.autotherm.autocrm.ui.leads

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.LeadDetail
import hu.autotherm.autocrm.data.api.LeadSummary
import hu.autotherm.autocrm.data.api.OrderBody
import hu.autotherm.autocrm.data.api.TransitionOption
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.EmailInfo
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.PhoneInfo
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.StageDialog
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import hu.autotherm.autocrm.util.formatDateTime
import hu.autotherm.autocrm.util.formatMoney
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import java.time.LocalDate

@OptIn(FlowPreview::class)
class LeadListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        /** Background refetch with old rows kept — shows a progress line, not a spinner. */
        val refreshing: Boolean = false,
        val leads: List<LeadSummary> = emptyList(),
        val error: String? = null,
        val openOnly: Boolean = true,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private val queryFlow = MutableStateFlow("")
    val query: StateFlow<String> = queryFlow.asStateFlow()

    init {
        // First paint must not wait for the debounce.
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

    private suspend fun load() {
        val keepRows = _state.value.leads.isNotEmpty()
        _state.value = _state.value.copy(
            loading = !keepRows,
            refreshing = keepRows,
            error = null,
        )
        try {
            val leads = api.leads(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                openOnly = _state.value.openOnly,
            )
            _state.value = _state.value.copy(loading = false, refreshing = false, leads = leads)
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
fun LeadListScreen(
    viewModel: LeadListViewModel,
    onOpen: (Long) -> Unit,
    onMenu: () -> Unit,
    canEdit: Boolean,
    onNewLead: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Leadek",
                subtitle = if (state.leads.isNotEmpty()) "${state.leads.size} tétel" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::refresh,
            )
        },
        floatingActionButton = {
            if (canEdit) {
                FloatingActionButton(onClick = onNewLead) {
                    Icon(Icons.Filled.Add, contentDescription = "Új lead")
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            SearchField(
                value = query,
                onValueChange = viewModel::setQuery,
                label = "Keresés cím, kapcsolattartó, ügyfél szerint",
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
                state.error != null && state.leads.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::retry)
                state.leads.isEmpty() -> EmptyState("Nincs lead.")
                else -> PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::refresh,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.leads, key = { it.id }) { lead ->
                            Card(modifier = Modifier.animateItem(), onClick = { onOpen(lead.id) }) {
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
            if (state.error != null && state.leads.isNotEmpty() && !state.refreshing) {
                ErrorState(state.error!!, onRetry = viewModel::retry)
            }
        }
    }
}

class LeadDetailViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val detail: LeadDetail? = null,
        val error: String? = null,
        val stageDialog: List<TransitionOption>? = null,
        val stageError: String? = null,
        val busy: Boolean = false,
        val convertOpen: Boolean = false,
        val convertBusy: Boolean = false,
        val convertError: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            val keepRows = _state.value.detail != null
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                _state.value = State(loading = false, detail = api.lead(id))
            } catch (e: Throwable) {
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    error = describeError(e),
                )
            }
        }
    }

    fun openStageDialog(id: Long) {
        viewModelScope.launch {
            try {
                val options = api.leadTransitions(id)
                _state.value = _state.value.copy(stageDialog = options, stageError = null)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(stageDialog = emptyList(), stageError = describeError(e))
            }
        }
    }

    fun closeStageDialog() {
        _state.value = _state.value.copy(stageDialog = null, stageError = null)
    }

    fun changeStage(id: Long, stage: String, note: String?) {
        viewModelScope.launch {
            _state.value = _state.value.copy(busy = true, stageError = null)
            try {
                api.changeLeadStage(id, stage, note?.takeIf { it.isNotBlank() })
                _state.value = _state.value.copy(busy = false, stageDialog = null)
                load(id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, stageError = describeError(e))
            }
        }
    }

    fun openConvert() {
        _state.value = _state.value.copy(convertOpen = true, convertError = null)
    }

    fun closeConvert() {
        _state.value = _state.value.copy(convertOpen = false, convertError = null)
    }

    fun convert(id: Long, title: String, onDone: (Long) -> Unit) {
        viewModelScope.launch {
            _state.value = _state.value.copy(convertBusy = true, convertError = null)
            try {
                val order = api.convertLead(id, OrderBody(title = title, currency = "HUF"))
                _state.value = _state.value.copy(convertBusy = false, convertOpen = false)
                onDone(order.id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(convertBusy = false, convertError = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LeadDetailScreen(
    leadId: Long,
    viewModel: LeadDetailViewModel,
    onOpenOrder: (Long) -> Unit,
    onBack: () -> Unit,
    canEdit: Boolean,
    onEditLead: (Long) -> Unit,
    onConverted: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(leadId) { viewModel.load(leadId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.detail?.lead?.title ?: "Lead",
                subtitle = state.detail?.lead?.contactName,
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(leadId) },
                actions = {
                    if (canEdit) {
                        TextButton(onClick = { onEditLead(leadId) }) { Text("Szerkesztés") }
                    }
                },
            )
        },
    ) { padding ->
        when {
            state.loading -> DetailSkeleton(
                Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            state.error != null && state.detail == null ->
                ErrorState(state.error!!, Modifier.padding(padding)) { viewModel.load(leadId) }
            state.detail == null ->
                ErrorState("Nem található.", Modifier.padding(padding)) { viewModel.load(leadId) }
            else -> {
                val detail = state.detail!!
                val lead = detail.lead
                PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = { viewModel.load(leadId) },
                    modifier = Modifier.padding(padding),
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item {
                            Card {
                                Text(lead.title, style = MaterialTheme.typography.headlineMedium)
                                Text(
                                    lead.contactName?.takeIf { it.isNotBlank() } ?: "—",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    detail.history.lastOrNull()?.let {
                                        StatusBadge(it.labelHu, Tone.Steel)
                                    }
                                    if (lead.quoteValidUntil?.let { isExpired(it) } == true) {
                                        StatusBadge("Lejárt", Tone.Signal)
                                    }
                                }
                            }
                        }
                        if (canEdit) {
                            item {
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    OutlinedButton(
                                        onClick = { viewModel.openStageDialog(leadId) },
                                        modifier = Modifier.weight(1f),
                                    ) { Text("Fázisváltás") }
                                    OutlinedButton(
                                        onClick = viewModel::openConvert,
                                        modifier = Modifier.weight(1f),
                                    ) { Text("Megrendeléssé") }
                                }
                            }
                        }
                        item {
                            Card {
                                Info("Kapcsolattartó", lead.contactName)
                                EmailInfo("E-mail", lead.contactEmail)
                                PhoneInfo("Telefon", lead.contactPhone)
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
                                Info("Érvényes eddig", formatDate(lead.quoteValidUntil))
                            }
                        }
                        if (detail.orders.isNotEmpty()) {
                            item { SectionTitle("Megrendelések", count = detail.orders.size) }
                            items(detail.orders, key = { it.id }) { order ->
                                Card(onClick = { onOpenOrder(order.id) }) {
                                    Text(order.number, style = MonoSmall)
                                }
                            }
                        }
                        if (detail.history.isNotEmpty()) {
                            item { SectionTitle("Előzmények", count = detail.history.size) }
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
                        if (state.error != null) {
                            item {
                                ErrorState(state.error!!) { viewModel.load(leadId) }
                            }
                        }
                    }
                }
            }
        }
    }

    state.stageDialog?.let { options ->
        StageDialog(
            options = options,
            error = state.stageError,
            busy = state.busy,
            onDismiss = viewModel::closeStageDialog,
            onConfirm = { stage, note -> viewModel.changeStage(leadId, stage, note) },
        )
    }
    if (state.convertOpen) {
        ConvertDialog(
            busy = state.convertBusy,
            error = state.convertError,
            initialTitle = state.detail?.lead?.title.orEmpty(),
            onDismiss = viewModel::closeConvert,
            onConfirm = { title -> viewModel.convert(leadId, title, onConverted) },
        )
    }
}

/** Plain `YYYY-MM-DD` from the API, compared as a date rather than a timestamp. */
internal fun isExpired(validUntil: String): Boolean =
    runCatching { LocalDate.parse(validUntil).isBefore(LocalDate.now()) }.getOrDefault(false)

/** Turns the lead into an order: the new job opens when the server answers. */
@Composable
private fun ConvertDialog(
    busy: Boolean,
    error: String?,
    initialTitle: String,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    var title by rememberSaveable(initialTitle) { mutableStateOf(initialTitle) }
    DialogShell(
        title = "Megrendeléssé alakítás",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Létrehozás",
                enabled = !busy && title.isNotBlank(),
                onClick = { onConfirm(title.trim()) },
            )
        },
    ) {
        AutoCrmTextField(
            value = title,
            onValueChange = { title = it },
            label = "Megrendelés címe *",
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            "Az új megrendelés a lead partnerével és HUF pénznemmel jön létre.",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}
