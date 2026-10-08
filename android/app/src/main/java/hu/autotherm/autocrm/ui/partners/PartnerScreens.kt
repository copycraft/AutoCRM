package hu.autotherm.autocrm.ui.partners

import androidx.compose.material.icons.filled.Call
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Contact
import hu.autotherm.autocrm.data.api.ContactBody
import hu.autotherm.autocrm.data.api.Partner
import hu.autotherm.autocrm.data.api.PartnerDetail
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.EmailInfo
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.PhoneInfo
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
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
private const val PARTNER_PAGE = 50

class PartnerListViewModel(private val api: AutoCrmApi) : ViewModel() {

    /** The next page, asked for when the end of the list scrolls into view. */
    fun loadMore() {
        val s = _state.value
        if (!s.hasMore || s.loading || s.refreshing || s.loadingMore) return
        viewModelScope.launch {
            _state.value = _state.value.copy(loadingMore = true)
            try {
                val next = api.partners(
                    query = queryFlow.value.takeIf { it.isNotBlank() },
                    role = _state.value.role,
                    limit = PARTNER_PAGE,
                    offset = _state.value.partners.size,
                )
                _state.value = _state.value.copy(
                    loadingMore = false,
                    partners = (_state.value.partners + next).distinctBy { it.id },
                    hasMore = next.size >= PARTNER_PAGE,
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loadingMore = false)
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = ::loadMore)
            }
        }
    }

    data class State(
        val loading: Boolean = true,
        /** Background refetch with old rows kept — shows a progress line, not a spinner. */
        val refreshing: Boolean = false,
        val partners: List<Partner> = emptyList(),
        val error: String? = null,
        /** null = everyone, "customer" / "supplier" = the V2.6 filter. */
        val role: String? = "customer",
        val hasMore: Boolean = false,
        val loadingMore: Boolean = false,
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

    fun setRole(role: String?) {
        _state.value = _state.value.copy(role = role)
        viewModelScope.launch { load() }
    }

    fun refresh() = viewModelScope.launch { load() }

    fun retry() = viewModelScope.launch { load() }

    private suspend fun load() {
        val keepRows = _state.value.partners.isNotEmpty()
        _state.value = _state.value.copy(
            loading = !keepRows,
            refreshing = keepRows,
            error = null,
        )
        try {
            val partners = api.partners(
                query = queryFlow.value.takeIf { it.isNotBlank() },
                role = _state.value.role,
                limit = PARTNER_PAGE,
            )
            _state.value = _state.value.copy(
                loading = false,
                refreshing = false,
                partners = partners,
                hasMore = partners.size >= PARTNER_PAGE,
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
fun PartnerListScreen(
    viewModel: PartnerListViewModel,
    onOpenPartner: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val query by viewModel.query.collectAsState()

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Ügyfelek",
                subtitle = if (state.partners.isNotEmpty()) "${state.partners.size}${if (state.hasMore) "+" else ""} ügyfél" else null,
                refreshing = state.refreshing,
                onRefresh = viewModel::refresh,
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            SearchField(
                value = query,
                onValueChange = viewModel::setQuery,
                label = "Keresés név, adószám, város szerint",
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                onSearch = viewModel::refresh,
            )
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterChip(
                    selected = state.role == "customer",
                    onClick = { viewModel.setRole("customer") },
                    label = { Text("Ügyfelek") },
                )
                FilterChip(
                    selected = state.role == "supplier",
                    onClick = { viewModel.setRole("supplier") },
                    label = { Text("Beszállítók") },
                )
                FilterChip(
                    selected = state.role == null,
                    onClick = { viewModel.setRole(null) },
                    label = { Text("Mind") },
                )
            }

            when {
                state.loading -> ListSkeleton(Modifier.padding(top = 4.dp))
                state.error != null && state.partners.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::retry)
                state.partners.isEmpty() -> EmptyState("Nincs találat.")
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::refresh,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.partners, key = { it.id }) { partner ->
                            hu.autotherm.autocrm.ui.common.ListRow(
                                title = partner.name,
                                subtitle = listOfNotNull(partner.city, hu.autotherm.autocrm.util.formatPhone(partner.phone) ?: partner.email)
                                    .joinToString(" · ").ifBlank { null },
                                leading = { hu.autotherm.autocrm.ui.common.InitialsAvatar(partner.name) },
                                trailing = if (partner.role == "supplier" || partner.role == "both") {
                                    { StatusBadge("Beszállító", Tone.Cold) }
                                } else {
                                    null
                                },
                                onClick = { onOpenPartner(partner.id) },
                            )
                        }
                        if (state.hasMore) {
                            item(key = "more") {
                                androidx.compose.runtime.LaunchedEffect(state.partners.size) { viewModel.loadMore() }
                                hu.autotherm.autocrm.ui.common.RefreshingBar()
                            }
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

class PartnerDetailViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val detail: PartnerDetail? = null,
        val error: String? = null,
        /** Non-null while the contact dialog is open; id 0 means "new". */
        val contactDialog: Contact? = null,
        val contactBusy: Boolean = false,
        val contactError: String? = null,
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
                _state.value = State(loading = false, detail = api.partner(id))
            } catch (e: Throwable) {
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    error = describeError(e),
                )
            }
        }
    }

    fun openContactDialog(existing: Contact?) {
        _state.value = _state.value.copy(
            contactDialog = existing ?: Contact(id = 0, name = ""),
            contactError = null,
        )
    }

    fun closeContactDialog() {
        _state.value = _state.value.copy(contactDialog = null, contactError = null)
    }

    fun saveContact(partnerId: Long, body: ContactBody) {
        val dialog = _state.value.contactDialog ?: return
        viewModelScope.launch {
            _state.value = _state.value.copy(contactBusy = true, contactError = null)
            try {
                if (dialog.id == 0L) {
                    api.createContact(partnerId, body)
                } else {
                    // Explicit nulls: a phone deleted in the dialog is deleted on the server.
                    api.patchContact(
                        dialog.id,
                        hu.autotherm.autocrm.data.api.patchOf(
                            "name" to body.name,
                            "email" to body.email,
                            "phone" to body.phone,
                            "position" to body.position,
                        ),
                    )
                }
                _state.value = _state.value.copy(contactBusy = false, contactDialog = null)
                hu.autotherm.autocrm.ui.common.Toasts.show("Kapcsolattartó mentve")
                load(partnerId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(contactBusy = false, contactError = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PartnerDetailScreen(
    partnerId: Long,
    viewModel: PartnerDetailViewModel,
    onOpenOrder: (Long) -> Unit,
    onOpenLead: (Long) -> Unit,
    onBack: () -> Unit,
    canEdit: Boolean,
    onEditPartner: (Long) -> Unit,
    onNewOrder: (Long) -> Unit,
    onNewLead: (Long) -> Unit,
    /** Null when the user may not send e-mail; [to] picks one contact's address. */
    onComposeEmail: ((partnerId: Long, to: String?) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(partnerId) { viewModel.load(partnerId) }
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.load(partnerId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.detail?.partner?.name ?: "Ügyfél",
                // Where they are and how much is on: "Győr · 2 nyitott munka".
                subtitle = state.detail?.let { d ->
                    val open = d.orders.count { !it.stageIsTerminal }
                    listOfNotNull(d.partner.city, if (open > 0) "$open nyitott munka" else null).joinToString(" · ").ifBlank { null }
                },
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(partnerId) },
                actions = {
                    val hasAddress = state.detail?.let { d ->
                        !d.partner.email.isNullOrBlank() || d.contacts.any { !it.email.isNullOrBlank() }
                    } == true
                    if (onComposeEmail != null && hasAddress) {
                        IconButton(onClick = { onComposeEmail(partnerId, null) }) {
                            Icon(androidx.compose.material.icons.Icons.Outlined.Email, contentDescription = "Levél")
                        }
                    }
                    if (canEdit) {
                        IconButton(onClick = { onEditPartner(partnerId) }) {
                            Icon(androidx.compose.material.icons.Icons.Outlined.Edit, contentDescription = "Szerkesztés")
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
                ErrorState(state.error!!, Modifier.padding(padding)) { viewModel.load(partnerId) }
            state.detail == null ->
                ErrorState("Nem található.", Modifier.padding(padding)) { viewModel.load(partnerId) }
            else -> {
                val detail = state.detail!!
                hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = { viewModel.load(partnerId) },
                    modifier = Modifier.padding(padding),
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item {
                            Card {
                                Text(detail.partner.name, style = MaterialTheme.typography.headlineMedium)
                                Text(
                                    if (detail.partner.kind == "business") "Vállalkozás" else "Magánszemély",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                            }
                        }
                        if (canEdit) {
                            item {
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    OutlinedButton(
                                        onClick = { onNewOrder(partnerId) },
                                        modifier = Modifier.weight(1f),
                                    ) { Text("Új munka") }
                                    OutlinedButton(
                                        onClick = { onNewLead(partnerId) },
                                        modifier = Modifier.weight(1f),
                                    ) { Text("Új lead") }
                                }
                            }
                        }
                        item {
                            Card {
                                Info("Adószám", detail.partner.taxNumber, mono = true)
                                Info("Ország", detail.partner.country, mono = true)
                                EmailInfo("E-mail", detail.partner.email)
                                PhoneInfo("Telefon", detail.partner.phone)
                                Info(
                                    "Cím",
                                    listOfNotNull(detail.partner.city, detail.partner.addressLine)
                                        .joinToString(", ").ifBlank { null },
                                )
                                val address = listOfNotNull(
                                    detail.partner.postalCode,
                                    detail.partner.city,
                                    detail.partner.addressLine,
                                ).joinToString(" ")
                                if (detail.partner.city != null || detail.partner.addressLine != null) {
                                    val ctx = androidx.compose.ui.platform.LocalContext.current
                                    androidx.compose.material3.TextButton(
                                        onClick = {
                                            hu.autotherm.autocrm.util.openNavigation(
                                                ctx,
                                                "$address ${detail.partner.country}".trim(),
                                            )
                                        },
                                    ) { Text("Útvonaltervezés") }
                                }
                            }
                        }
                        if (detail.contacts.isNotEmpty() || canEdit) {
                            item {
                                SectionTitle(
                                    "Kapcsolattartók",
                                    count = detail.contacts.size.takeIf { it > 0 },
                                    actionLabel = if (canEdit) "Új" else null,
                                    onAction = if (canEdit) ({ viewModel.openContactDialog(null) }) else null,
                                )
                            }
                            items(detail.contacts, key = { it.id }) { contact ->
                                Card {
                                    Row(
                                        Modifier.fillMaxWidth(),
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Text(
                                            contact.name,
                                            style = MaterialTheme.typography.titleMedium,
                                            modifier = Modifier.weight(1f, fill = false),
                                        )
                                        // Call or write to this person directly, the reason this
                                        // list is opened on a phone.
                                        val uri = androidx.compose.ui.platform.LocalUriHandler.current
                                        Row {
                                            contact.phone?.takeIf { it.isNotBlank() }?.let { phone ->
                                                IconButton(onClick = {
                                                    val digits = phone.trim().let { (if (it.startsWith("+")) "+" else "") + it.filter(Char::isDigit) }
                                                    uri.openUri("tel:$digits")
                                                }) {
                                                    Icon(Icons.Filled.Call, contentDescription = "Hívás: ${contact.name}", tint = hu.autotherm.autocrm.ui.theme.Cold)
                                                }
                                            }
                                            contact.email?.takeIf { it.isNotBlank() }?.let { email ->
                                                IconButton(onClick = {
                                                    if (onComposeEmail != null) onComposeEmail(partnerId, email.trim())
                                                    else uri.openUri("mailto:${email.trim()}")
                                                }) {
                                                    Icon(Icons.Outlined.Email, contentDescription = "E-mail: ${contact.name}", tint = hu.autotherm.autocrm.ui.theme.Cold)
                                                }
                                            }
                                            if (canEdit) {
                                                IconButton(onClick = { viewModel.openContactDialog(contact) }) {
                                                    Icon(
                                                        Icons.Filled.Edit,
                                                        contentDescription = "Szerkesztés",
                                                        tint = Steel500,
                                                    )
                                                }
                                            }
                                        }
                                    }
                                    Text(
                                        listOfNotNull(contact.position, hu.autotherm.autocrm.util.formatPhone(contact.phone), contact.email)
                                            .joinToString(" · ").ifBlank { "—" },
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Steel500,
                                    )
                                }
                            }
                        }
                        item { SectionTitle("Megrendelések", count = detail.orders.size) }
                        if (detail.orders.isEmpty()) {
                            item {
                                Card { Text("Nincs megrendelés.", style = MaterialTheme.typography.bodyLarge, color = Steel500) }
                            }
                        } else {
                            // Open jobs first: they are what a call about this customer is about.
                            items(detail.orders.sortedBy { it.stageIsTerminal }, key = { it.id }) { order ->
                                Card(onClick = { onOpenOrder(order.id) }) {
                                    Row(
                                        Modifier.fillMaxWidth(),
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                                    ) {
                                        Row(
                                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                                            verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                                        ) {
                                            order.vehiclePlate?.let { hu.autotherm.autocrm.ui.common.PlateBadge(it) }
                                            Text("#" + order.number, style = MonoSmall, color = Steel500)
                                        }
                                        Text(formatMoney(order.totalMinor, order.currency), style = MonoSmall)
                                    }
                                    Text(order.title, style = MaterialTheme.typography.bodyLarge, maxLines = 2)
                                    StatusBadge(order.stageLabel, if (order.stageIsTerminal) Tone.Done else Tone.Cold)
                                }
                            }
                        }
                        // V6: without this, nobody opening a partner sees they were quoted before.
                        item { SectionTitle("Leadek", count = detail.leads.size) }
                        if (detail.leads.isEmpty()) {
                            item {
                                Card { Text("Nincs lead.", style = MaterialTheme.typography.bodyLarge, color = Steel500) }
                            }
                        } else {
                            items(detail.leads, key = { it.id }) { lead ->
                                hu.autotherm.autocrm.ui.common.ListRow(
                                    title = lead.title,
                                    subtitle = listOfNotNull(
                                        hu.autotherm.autocrm.util.relativeTime(lead.createdAt),
                                        lead.orderNumber?.let { "megrendelés: #$it" },
                                    ).joinToString(" · "),
                                    trailing = {
                                        StatusBadge(lead.stageLabel, if (lead.orderNumber != null) Tone.Done else Tone.Cold)
                                    },
                                    onClick = { onOpenLead(lead.id) },
                                )
                            }
                        }
                        if (state.error != null) {
                            item {
                                ErrorState(state.error!!) { viewModel.load(partnerId) }
                            }
                        }
                    }
                }
            }
        }
    }

    state.contactDialog?.let { existing ->
        ContactDialog(
            existing = existing,
            busy = state.contactBusy,
            error = state.contactError,
            onDismiss = viewModel::closeContactDialog,
            onSave = { body -> viewModel.saveContact(partnerId, body) },
        )
    }
}

@Composable
private fun ContactDialog(
    existing: Contact,
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onSave: (ContactBody) -> Unit,
) {
    var name by rememberSaveable(existing.id) { mutableStateOf(existing.name) }
    var position by rememberSaveable(existing.id) { mutableStateOf(existing.position.orEmpty()) }
    var email by rememberSaveable(existing.id) { mutableStateOf(existing.email.orEmpty()) }
    var phone by rememberSaveable(existing.id) { mutableStateOf(existing.phone.orEmpty()) }

    DialogShell(
        title = if (existing.id == 0L) "Új kapcsolattartó" else "Kapcsolattartó szerkesztése",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Mentés",
                enabled = !busy && name.isNotBlank(),
                onClick = {
                    fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
                    onSave(
                        ContactBody(
                            name = name.trim(),
                            email = blankToNull(email),
                            phone = blankToNull(phone),
                            position = blankToNull(position),
                        ),
                    )
                },
            )
        },
    ) {
        AutoCrmTextField(
            value = name,
            onValueChange = { name = it },
            label = "Név *",
            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Words,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = position,
            onValueChange = { position = it },
            label = "Beosztás",
            modifier = Modifier.fillMaxWidth(),
        )
        val emailBad = email.isNotBlank() && !Regex("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$").matches(email.trim())
        AutoCrmTextField(
            value = email,
            onValueChange = { email = it.trim() },
            label = "E-mail",
            isError = emailBad,
            supporting = if (emailBad) "Nem érvényes e-mail cím." else null,
            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                keyboardType = androidx.compose.ui.text.input.KeyboardType.Email,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = phone,
            onValueChange = { phone = it },
            label = "Telefon",
            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                keyboardType = androidx.compose.ui.text.input.KeyboardType.Phone,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}
