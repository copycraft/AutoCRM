package hu.autotherm.autocrm.ui.leads

import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Call
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.Alignment
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
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Email
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
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.api.LookupItem
import hu.autotherm.autocrm.data.api.OrderBody
import hu.autotherm.autocrm.data.api.OrderRef
import hu.autotherm.autocrm.data.api.TransitionOption
import hu.autotherm.autocrm.data.inspection.cachedLookups
import hu.autotherm.autocrm.data.inspection.downloadLookups
import hu.autotherm.autocrm.data.prefs.LookupsCache
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
import java.time.ZoneId

private const val LEAD_PAGE = 50

@OptIn(FlowPreview::class)
class LeadListViewModel(
    private val api: AutoCrmApi,
    /** Where the "open only" choice is kept between launches; null in tests. */
    private val prefs: android.content.SharedPreferences? = null,
) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        /** Background refetch with old rows kept — shows a progress line, not a spinner. */
        val refreshing: Boolean = false,
        val leads: List<LeadSummary> = emptyList(),
        val error: String? = null,
        val openOnly: Boolean = true,
        /** The last page was full: there may be more beyond it. */
        val hasMore: Boolean = false,
        val loadingMore: Boolean = false,
    )

    // The list opens filtered the way it was left, not reset to the default every launch.
    private val _state = MutableStateFlow(State(openOnly = prefs?.getBoolean("leads_open_only", true) ?: true))
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
        prefs?.edit()?.putBoolean("leads_open_only", _state.value.openOnly)?.apply()
        viewModelScope.launch { load() }
    }

    fun refresh() = viewModelScope.launch { load() }

    fun retry() = viewModelScope.launch { load() }

    /** Back from a lead: the loaded rows again in one request, scroll position kept. */
    fun refreshInPlace() {
        val s = _state.value
        if (s.loading || s.refreshing || s.leads.isEmpty()) return
        viewModelScope.launch {
            val count = s.leads.size.coerceAtLeast(LEAD_PAGE).coerceAtMost(200)
            runCatching {
                api.leads(
                    query = queryFlow.value.takeIf { it.isNotBlank() },
                    openOnly = _state.value.openOnly,
                    limit = count,
                )
            }.onSuccess { rows ->
                _state.value = _state.value.copy(leads = rows, hasMore = rows.size >= count)
            }
        }
    }

    /** The next page, asked for when the end of the list scrolls into view. */
    fun loadMore() {
        val s = _state.value
        if (!s.hasMore || s.loading || s.refreshing || s.loadingMore) return
        viewModelScope.launch {
            _state.value = _state.value.copy(loadingMore = true)
            try {
                val next = api.leads(
                    query = queryFlow.value.takeIf { it.isNotBlank() },
                    openOnly = _state.value.openOnly,
                    limit = LEAD_PAGE,
                    offset = _state.value.leads.size,
                )
                _state.value = _state.value.copy(
                    loadingMore = false,
                    leads = (_state.value.leads + next).distinctBy { it.id },
                    hasMore = next.size >= LEAD_PAGE,
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loadingMore = false)
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = ::loadMore)
            }
        }
    }

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
                limit = LEAD_PAGE,
            )
            _state.value = _state.value.copy(
                loading = false,
                refreshing = false,
                leads = leads,
                hasMore = leads.size >= LEAD_PAGE,
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
fun LeadListScreen(
    viewModel: LeadListViewModel,
    onOpen: (Long) -> Unit,
    onMenu: () -> Unit,
    canEdit: Boolean,
    onNewLead: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val topScroll = hu.autotherm.autocrm.ui.common.rememberTopScrollableListState("leads")
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.refreshInPlace() }
    val query by viewModel.query.collectAsState()

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Leadek",
                subtitle = if (state.leads.isNotEmpty()) "${state.leads.size}${if (state.hasMore) "+" else ""} érdeklődés" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::refresh,
            )
        },
        floatingActionButton = {
            if (canEdit) {
                hu.autotherm.autocrm.ui.common.NewFab("Új érdeklődés", onNewLead)
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
                state.leads.isEmpty() -> run {
                    val q = viewModel.query.collectAsState().value
                    if (q.isNotBlank()) {
                        EmptyState(
                            "Nincs találat erre: „${q.trim()}”.",
                            actionLabel = "Keresés törlése",
                            onAction = { viewModel.setQuery("") },
                        )
                    } else {
                        EmptyState(
                            "Még nincs érdeklődés.",
                            actionLabel = if (canEdit) "Új érdeklődés" else null,
                            onAction = onNewLead,
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
                        items(state.leads, key = { it.id }) { lead ->
                            val who = lead.partnerName ?: lead.contactName
                            hu.autotherm.autocrm.ui.common.ListRow(
                                title = lead.title,
                                subtitle = who ?: "Nincs ügyfél megadva",
                                leading = { hu.autotherm.autocrm.ui.common.InitialsAvatar(who ?: lead.title) },
                                trailing = {
                                    Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                        StatusBadge(lead.stageLabel, if (lead.orderNumber != null) Tone.Done else Tone.Cold)
                                        lead.orderNumber?.let { Text(it, style = hu.autotherm.autocrm.ui.theme.MonoSmall, color = Steel500) }
                                    }
                                },
                                onClick = { onOpen(lead.id) },
                                modifier = Modifier.animateItem(),
                            )
                        }
                        if (state.hasMore) {
                            item(key = "more") {
                                // Reaching the end asks for the next page by itself.
                                LaunchedEffect(state.leads.size) { viewModel.loadMore() }
                                hu.autotherm.autocrm.ui.common.RefreshingBar()
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

class LeadDetailViewModel(
    private val api: AutoCrmApi,
    private val lookupsCache: LookupsCache,
    private val sessionStore: hu.autotherm.autocrm.data.auth.SessionStore? = null,
) : ViewModel() {

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
        /** Order currency for the conversion: the partner's default, changeable. */
        val convertCurrency: String = "HUF",
        /** The server's enumerations; cached for offline opens. */
        val lookups: Lookups? = null,
        /** Source names for the source key (0048). */
        val sources: List<hu.autotherm.autocrm.data.api.LeadSource> = emptyList(),
        /** Comments need a capability viewers do not have. */
        val canComment: Boolean = false,
        /** The conversion's partner: the lead's own, or one picked/created in the dialog. */
        val convertPartner: hu.autotherm.autocrm.ui.common.PartnerChoice? = null,
        val convertProjectType: Long? = null,
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
                val lookups = downloadLookups(api, lookupsCache) ?: cachedLookups(lookupsCache)
                val sources = runCatching { api.leadSources() }.getOrDefault(_state.value.sources)
                _state.value = State(
                    loading = false,
                    detail = api.lead(id),
                    lookups = lookups,
                    sources = sources,
                    canComment = sessionStore?.currentAccount()?.canComment == true,
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

    fun changeStage(id: Long, stage: String, note: String?, lostReasonId: Long? = null) {
        viewModelScope.launch {
            _state.value = _state.value.copy(busy = true, stageError = null)
            try {
                api.changeLeadStage(id, stage, note?.takeIf { it.isNotBlank() }, lostReasonId)
                val label = _state.value.stageDialog?.firstOrNull { it.stageKey == stage }?.labelHu ?: stage
                _state.value = _state.value.copy(busy = false, stageDialog = null)
                hu.autotherm.autocrm.ui.common.Toasts.show("Új fázis: $label")
                load(id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, stageError = describeError(e))
            }
        }
    }

    fun openConvert() {
        _state.value = _state.value.copy(
            convertOpen = true,
            convertError = null,
            convertCurrency = _state.value.detail?.lead?.currency ?: "HUF",
            convertPartner = null,
            convertProjectType = null,
        )
        // Same prefill as the web dialog: the partner's default currency (MAJOR-03),
        // when the server lists it; else the first listed currency; HUF (the business
        // default) when offline with nothing cached.
        val partnerId = _state.value.detail?.lead?.partnerId ?: return
        viewModelScope.launch {
            val lookups = downloadLookups(api, lookupsCache) ?: cachedLookups(lookupsCache)
            val partner = runCatching { api.partner(partnerId).partner }.getOrNull()
            if (partner != null) {
                _state.value = _state.value.copy(convertPartner = hu.autotherm.autocrm.ui.common.PartnerChoice.of(partner))
            }
            val default = partner?.defaultCurrency
            val listed = lookups?.currencies.orEmpty()
            val currency = when {
                default != null && (listed.isEmpty() || listed.any { it.key == default }) -> default
                listed.isNotEmpty() -> listed.first().key
                else -> "HUF"
            }
            _state.value = _state.value.copy(convertCurrency = currency, lookups = lookups)
        }
    }

    fun setConvertCurrency(currency: String) {
        _state.value = _state.value.copy(convertCurrency = currency)
    }

    fun setConvertPartner(p: hu.autotherm.autocrm.ui.common.PartnerChoice?) {
        _state.value = _state.value.copy(
            convertPartner = p,
            convertCurrency = p?.defaultCurrency ?: _state.value.convertCurrency,
            convertError = null,
        )
    }

    fun setConvertProjectType(id: Long?) {
        _state.value = _state.value.copy(convertProjectType = id)
    }

    fun closeConvert() {
        _state.value = _state.value.copy(convertOpen = false, convertError = null)
    }

    fun convert(id: Long, title: String, onDone: (Long) -> Unit) {
        val s = _state.value
        // The server needs a partner for the order: the lead's, or one chosen here.
        if (s.convertPartner == null) {
            _state.value = s.copy(convertError = "Válassz vagy hozz létre partnert a megrendeléshez.")
            return
        }
        viewModelScope.launch {
            _state.value = _state.value.copy(convertBusy = true, convertError = null)
            try {
                val order = api.convertLead(
                    id,
                    convertBody(
                        title,
                        _state.value.convertCurrency,
                        partnerId = s.convertPartner.id,
                        projectTypeId = s.convertProjectType,
                    ),
                )
                _state.value = _state.value.copy(convertBusy = false, convertOpen = false)
                hu.autotherm.autocrm.ui.common.Toasts.show("Megrendelés létrehozva: #" + order.number)
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
    /** Conversion creates an order: the server wants edit_orders as well as edit_leads. */
    canConvert: Boolean = canEdit,
    onConverted: (Long) -> Unit,
    /** Null when the user may not send e-mail. */
    onComposeEmail: ((Long) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(leadId) { viewModel.load(leadId) }
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.load(leadId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.detail?.lead?.title ?: "Lead",
                subtitle = state.detail?.lead?.contactName,
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(leadId) },
                actions = {
                    // Answering an enquiry is the next step more often than editing it.
                    if (onComposeEmail != null && !state.detail?.lead?.contactEmail.isNullOrBlank()) {
                        androidx.compose.material3.IconButton(onClick = { onComposeEmail(leadId) }) {
                            androidx.compose.material3.Icon(
                                androidx.compose.material.icons.Icons.Outlined.Email,
                                contentDescription = "Levél",
                            )
                        }
                    }
                    if (canEdit) {
                        androidx.compose.material3.IconButton(onClick = { onEditLead(leadId) }) {
                            androidx.compose.material3.Icon(
                                androidx.compose.material.icons.Icons.Outlined.Edit,
                                contentDescription = "Szerkesztés",
                            )
                        }
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
                hu.autotherm.autocrm.ui.common.AppPullToRefresh(
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
                        // An enquiry is answered by phone first: the call is one big button away.
                        lead.contactPhone?.takeIf { it.isNotBlank() }?.let { phone ->
                            item {
                                val uri = androidx.compose.ui.platform.LocalUriHandler.current
                                hu.autotherm.autocrm.ui.common.SecondaryButton(
                                    text = "Hívás: " + hu.autotherm.autocrm.util.formatPhone(phone.trim()),
                                    onClick = {
                                        val digits = phone.trim().let { (if (it.startsWith("+")) "+" else "") + it.filter(Char::isDigit) }
                                        uri.openUri("tel:$digits")
                                    },
                                    icon = androidx.compose.material.icons.Icons.Filled.Call,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                            }
                        }
                        if (canEdit) {
                            item {
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    // A converted lead cannot change stage (lead_converted).
                                    if (canChangeLeadStage(detail.orders)) {
                                        OutlinedButton(
                                            onClick = { viewModel.openStageDialog(leadId) },
                                            modifier = Modifier.weight(1f),
                                        ) { Text("Fázisváltás") }
                                    }
                                    if (canConvert) {
                                        OutlinedButton(
                                            onClick = viewModel::openConvert,
                                            modifier = Modifier.weight(1f),
                                        ) { Text("Megrendeléssé") }
                                    }
                                }
                            }
                        }
                        item {
                            Card {
                                Info("Kapcsolattartó", lead.contactName)
                                EmailInfo("E-mail", lead.contactEmail)
                                PhoneInfo("Telefon", lead.contactPhone)
                                Info(
                                    "Forrás",
                                    listOfNotNull(
                                        state.sources.firstOrNull { it.key == lead.source }?.label ?: lead.source,
                                        lead.sourceDetail,
                                    ).joinToString(" · ").ifBlank { null },
                                )
                                detail.attribution?.let { a ->
                                    Info("Csatorna", channelLabel(a.channel))
                                    a.utmCampaign?.let { Info("Kampány", it) }
                                    Info("Első oldal", a.landingPage)
                                    Info("Hivatkozó oldal", a.referrer)
                                }
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
                                val rate = lead.quoteFxRate?.toDoubleOrNull()
                                if (rate != null && lead.quotedValueMinor != null) {
                                    Info(
                                        "Forintban (MNB ${formatDate(lead.quoteFxDay).orEmpty()})",
                                        "≈ " + formatMoney(Math.round(lead.quotedValueMinor * rate), "HUF") +
                                            " · " + String.format(java.util.Locale.ROOT, "%.2f", rate) + " Ft/EUR",
                                        mono = true,
                                    )
                                }
                            }
                        }
                        item {
                            hu.autotherm.autocrm.ui.common.CommentsSection(
                                entity = "lead",
                                id = leadId,
                                canComment = state.canComment,
                            )
                        }
                        if (state.canComment) {
                            item {
                                hu.autotherm.autocrm.ui.common.Card {
                                    hu.autotherm.autocrm.ui.common.VoiceNoteRecorder(orderId = 0, leadId = leadId)
                                }
                            }
                        }
                        if (detail.orders.isNotEmpty()) {
                            item { SectionTitle("Megrendelések", count = detail.orders.size) }
                            items(detail.orders, key = { it.id }) { order ->
                                // Reads as a link to the job, not a bare number in a box.
                                hu.autotherm.autocrm.ui.common.ListRow(
                                    title = "#" + order.number,
                                    subtitle = "Megrendelés megnyitása",
                                    leading = {
                                        Icon(
                                            androidx.compose.material.icons.Icons.Filled.Build,
                                            contentDescription = null,
                                            tint = hu.autotherm.autocrm.ui.theme.Done,
                                        )
                                    },
                                    trailing = {
                                        Icon(
                                            androidx.compose.material.icons.Icons.AutoMirrored.Filled.KeyboardArrowRight,
                                            contentDescription = null,
                                            tint = Steel500,
                                        )
                                    },
                                    onClick = { onOpenOrder(order.id) },
                                )
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
        val reasonApi = hu.autotherm.autocrm.ui.common.app().api
        val lostReasons by androidx.compose.runtime.produceState(
            initialValue = emptyList<hu.autotherm.autocrm.data.api.LostReason>(),
        ) {
            value = runCatching { reasonApi.lostReasons() }
                .getOrDefault(emptyList()).filter { it.archivedAt == null }
        }
        StageDialog(
            options = options,
            error = state.stageError,
            busy = state.busy,
            currentLabel = state.detail?.let { d -> d.history.lastOrNull { it.stageKey == d.stage?.stageKey }?.labelHu },
            onDismiss = viewModel::closeStageDialog,
            lostReasons = lostReasons,
            onConfirm = { stage, note, reason -> viewModel.changeStage(leadId, stage, note, reason) },
        )
    }
    if (state.convertOpen) {
        ConvertDialog(
            busy = state.convertBusy,
            error = state.convertError,
            initialTitle = state.detail?.lead?.title.orEmpty(),
            currency = state.convertCurrency,
            currencies = state.lookups?.currencies?.takeIf { it.isNotEmpty() }
                ?: listOf(LookupItem(state.convertCurrency, state.convertCurrency)),
            partner = state.convertPartner,
            partnerFixed = state.detail?.lead?.partnerId != null,
            prefill = state.detail?.lead?.let { l ->
                hu.autotherm.autocrm.ui.common.PartnerPrefill(l.contactName, l.contactEmail, l.contactPhone)
            },
            onPartner = viewModel::setConvertPartner,
            projectTypeId = state.convertProjectType,
            onProjectType = viewModel::setConvertProjectType,
            onCurrency = viewModel::setConvertCurrency,
            onDismiss = viewModel::closeConvert,
            onConfirm = { title -> viewModel.convert(leadId, title, onConverted) },
        )
    }
}

/** Plain `YYYY-MM-DD` from the API, compared as a date rather than a timestamp. */
internal fun isExpired(
    validUntil: String,
    today: LocalDate = LocalDate.now(ZoneId.of("Europe/Budapest")),
): Boolean = runCatching { LocalDate.parse(validUntil).isBefore(today) }.getOrDefault(false)

/**
 * The conversion body. The chosen currency travels as-is; the server refuses
 * what it does not know. Blank means the business default (HUF) — the web
 * dialog's rule (docs/history/REMEDIATION.md MAJOR-03).
 */
internal fun convertBody(
    title: String,
    currency: String?,
    partnerId: Long? = null,
    projectTypeId: Long? = null,
): OrderBody =
    OrderBody(
        title = title,
        currency = currency?.takeIf { it.isNotBlank() } ?: "HUF",
        partnerId = partnerId,
        projectTypeId = projectTypeId,
    )

/** A lead that became an order is worked on the order; its stage is fixed. */
internal fun canChangeLeadStage(orders: List<OrderRef>): Boolean = orders.isEmpty()

/** Turns the lead into an order: the new job opens when the server answers. */
@Composable
private fun ConvertDialog(
    busy: Boolean,
    error: String?,
    initialTitle: String,
    currency: String,
    /** The server's currencies; offline with nothing cached, the current choice alone. */
    currencies: List<LookupItem>,
    partner: hu.autotherm.autocrm.ui.common.PartnerChoice?,
    /** The lead already has a partner: the order takes it. */
    partnerFixed: Boolean,
    prefill: hu.autotherm.autocrm.ui.common.PartnerPrefill?,
    onPartner: (hu.autotherm.autocrm.ui.common.PartnerChoice?) -> Unit,
    projectTypeId: Long?,
    onProjectType: (Long?) -> Unit,
    onCurrency: (String) -> Unit,
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
        hu.autotherm.autocrm.ui.common.PartnerField(
            selected = partner,
            onSelect = onPartner,
            enabled = !partnerFixed,
            prefill = prefill,
            supporting = when {
                partnerFixed -> "A lead partnere."
                partner == null -> "A megrendeléshez partner kell: keresd meg, vagy hozd létre a lead adataiból."
                else -> null
            },
            modifier = Modifier.fillMaxWidth(),
        )
        hu.autotherm.autocrm.ui.common.ProjectTypeField(
            selected = projectTypeId,
            onSelect = onProjectType,
            modifier = Modifier.fillMaxWidth(),
        )
        hu.autotherm.autocrm.ui.common.FieldLabel("Pénznem")
        hu.autotherm.autocrm.ui.common.SegmentedChoice(
            options = currencies.map { it.key to it.key },
            selected = currency,
            onSelect = onCurrency,
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}

/** The server's channel code in words. */
internal fun channelLabel(channel: String): String = when (channel) {
    "paid" -> "Fizetett hirdetés"
    "organic" -> "Keresőből (ingyenes)"
    "social" -> "Közösségi média"
    "email" -> "E-mail / hírlevél"
    "referral" -> "Másik oldalról"
    "direct" -> "Közvetlen"
    else -> channel
}
