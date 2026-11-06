package hu.autotherm.autocrm.ui.incoming

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.PhotoCamera
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.IncomingInvoice
import hu.autotherm.autocrm.data.api.patchOf
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.Toasts
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import hu.autotherm.autocrm.util.formatMoney
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlin.math.roundToLong

/** The server's bucket keys, in list order, with the office wording. Unknown keys read raw. */
fun bucketLabel(bucket: String): String = when (bucket) {
    "open_invoice" -> "Nyitott"
    "open_proforma" -> "Díjbekérő"
    "transferred" -> "Átutalt"
    "cash" -> "Készpénz"
    "partial" -> "Részben fizetett"
    "cash_receipt" -> "Nyugta"
    "booking_only" -> "Csak könyvelés"
    else -> bucket
}

fun paymentLabel(method: String): String = when (method) {
    "TRANSFER" -> "Átutalás"
    "CASH" -> "Készpénz"
    "CARD" -> "Kártya"
    else -> method
}

/**
 * What the finger typed ("12 500" / "12,5") into minor units. HUF has no fractional
 * subunit on these documents, so it rounds to whole forint; EUR goes to cents.
 * Returns null when the field is blank (means "not filled in", not zero).
 */
fun majorToMinor(text: String, currency: String): Long? {
    val clean = text.trim().replace(" ", "").replace(",", ".")
    if (clean.isEmpty()) return null
    val major = clean.toDoubleOrNull() ?: throw IllegalArgumentException("nem szám: $text")
    if (major < 0) throw IllegalArgumentException("negatív összeg")
    return if (currency == "EUR") (major * 100).roundToLong() else major.roundToLong()
}

fun minorToMajor(minor: Long?, currency: String): String {
    if (minor == null) return ""
    return if (currency == "EUR") {
        val whole = minor / 100
        val cents = (minor % 100).toString().padStart(2, '0')
        "$whole,$cents"
    } else {
        minor.toString()
    }
}

// ── List ──────────────────────────────────────────────────────────────────────

class IncomingListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val query: String = "",
        val bucket: String? = null,
        val items: List<IncomingInvoice> = emptyList(),
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    // Launched loads are tracked so a slower older request can never overwrite a newer
    // one's result (rapid bucket taps, or a bucket tap while the IME-search load runs).
    private var loadJob: kotlinx.coroutines.Job? = null

    fun load() {
        loadJob?.cancel()
        loadJob = viewModelScope.launch {
            val keepRows = _state.value.items.isNotEmpty()
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                val items = api.incomingInvoices(
                    query = _state.value.query.ifBlank { null },
                    bucket = _state.value.bucket,
                )
                _state.value = _state.value.copy(loading = false, refreshing = false, items = items)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    error = describeError(e),
                )
            }
        }
    }

    fun setQuery(query: String) {
        _state.value = _state.value.copy(query = query)
    }

    fun search() = load()

    fun setBucket(bucket: String?) {
        _state.value = _state.value.copy(bucket = bucket)
        load()
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun IncomingListScreen(
    viewModel: IncomingListViewModel,
    onOpen: (Long) -> Unit,
    onScan: (() -> Unit)?,
    onMenu: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    var query by rememberSaveable { mutableStateOf("") }

    LaunchedEffect(Unit) { viewModel.load() }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Bejövő számlák",
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load() },
            )
        },
        floatingActionButton = {
            if (onScan != null) {
                ExtendedFloatingActionButton(
                    onClick = onScan,
                    icon = { Icon(Icons.Filled.PhotoCamera, contentDescription = null) },
                    text = { Text("Szkennelés") },
                )
            }
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            SearchField(
                value = query,
                onValueChange = {
                    query = it
                    viewModel.setQuery(it)
                },
                label = "Szállító, bizonylatszám…",
                onSearch = { viewModel.search() },
            )
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                item {
                    FilterChip(
                        selected = state.bucket == null,
                        onClick = { viewModel.setBucket(null) },
                        label = { Text("Mind") },
                    )
                }
                items(listOf("open_invoice", "partial", "transferred", "cash", "booking_only")) { bucket ->
                    FilterChip(
                        selected = state.bucket == bucket,
                        onClick = { viewModel.setBucket(bucket) },
                        label = { Text(bucketLabel(bucket)) },
                    )
                }
            }
            when {
                state.loading -> ListSkeleton()
                state.error != null -> ErrorState(state.error!!) { viewModel.load() }
                state.items.isEmpty() -> EmptyState(
                    "Még nincs bejövő számla.",
                    actionLabel = if (onScan != null) "Szkennelés" else null,
                    onAction = onScan,
                )
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = { viewModel.load() },
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize(),
                        contentPadding = hu.autotherm.autocrm.ui.common.listPadding(fab = true),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.items, key = { it.id }) { invoice ->
                            IncomingRow(invoice = invoice, onOpen = { onOpen(invoice.id) })
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun IncomingRow(invoice: IncomingInvoice, onOpen: () -> Unit) {
    Card(onClick = onOpen) {
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(invoice.supplierName, style = MaterialTheme.typography.titleMedium)
                StatusBadge(bucketLabel(invoice.bucket), Tone.Steel)
            }
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    listOfNotNull(invoice.invoiceNumber, invoice.issueDate?.let { formatDate(it) })
                        .joinToString(" · ").ifEmpty { "—" },
                    style = MaterialTheme.typography.bodyMedium,
                    color = Steel500,
                )
                Text(
                    if (invoice.grossAmount != null) {
                        formatMoney(invoice.grossAmount, invoice.currency)
                    } else {
                        "összeg nélkül"
                    },
                    style = MaterialTheme.typography.titleMedium,
                )
            }
        }
    }
}

// ── Detail / edit ─────────────────────────────────────────────────────────────

class IncomingDetailViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val invoice: IncomingInvoice? = null,
        val supplierName: String = "",
        val taxNumber: String = "",
        val number: String = "",
        val issueDate: String = "",
        val dueDate: String = "",
        val currency: String = "HUF",
        val gross: String = "",
        val paymentMethod: String = "TRANSFER",
        val notes: String = "",
        val saving: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            _state.value = State()
            try {
                val invoice = api.incomingInvoice(id)
                _state.value = State(
                    loading = false,
                    invoice = invoice,
                    supplierName = invoice.supplierName,
                    taxNumber = invoice.supplierTaxNumber.orEmpty(),
                    number = invoice.invoiceNumber.orEmpty(),
                    issueDate = invoice.issueDate.orEmpty(),
                    dueDate = invoice.dueDate.orEmpty(),
                    currency = invoice.currency,
                    gross = minorToMajor(invoice.grossAmount, invoice.currency),
                    paymentMethod = invoice.paymentMethod,
                    notes = invoice.notes.orEmpty(),
                )
            } catch (e: Throwable) {
                _state.value = State(loading = false, error = describeError(e))
            }
        }
    }

    fun edit(block: State.() -> State) {
        _state.value = _state.value.block()
    }

    fun save(id: Long, onSaved: () -> Unit) {
        val s = _state.value
        val grossMinor = try {
            majorToMinor(s.gross, s.currency)
        } catch (e: IllegalArgumentException) {
            Toasts.error(e.message ?: "hibás összeg")
            return
        }
        viewModelScope.launch {
            _state.value = s.copy(saving = true)
            try {
                val updated = api.patchIncomingInvoice(
                    id,
                    patchOf(
                        "supplier_name" to s.supplierName.trim().ifEmpty { "Ismeretlen szállító" },
                        "supplier_tax_number" to s.taxNumber.trim().ifBlank { null },
                        "invoice_number" to s.number.trim().ifBlank { null },
                        "issue_date" to s.issueDate.trim().ifBlank { null },
                        "due_date" to s.dueDate.trim().ifBlank { null },
                        "currency" to s.currency,
                        "gross_amount" to grossMinor,
                        "payment_method" to s.paymentMethod,
                        "notes" to s.notes.trim().ifBlank { null },
                    ),
                )
                _state.value = _state.value.copy(saving = false, invoice = updated)
                Toasts.show("Mentve.")
                onSaved()
            } catch (e: Throwable) {
                _state.value = _state.value.copy(saving = false)
                Toasts.error(describeError(e))
            }
        }
    }

    fun markPaid(id: Long) {
        val gross = _state.value.invoice?.grossAmount
        if (gross == null) {
            Toasts.error("Előbb írd be a bruttó összeget és mentsd.")
            return
        }
        viewModelScope.launch {
            try {
                val today = hu.autotherm.autocrm.util.businessTodayIso()
                val updated = api.patchIncomingInvoice(
                    id,
                    patchOf("paid_amount" to gross, "paid_on" to today),
                )
                _state.value = _state.value.copy(invoice = updated)
                Toasts.show("Fizetve jelölve.")
            } catch (e: Throwable) {
                Toasts.error(describeError(e))
            }
        }
    }

    fun delete(id: Long, onDeleted: () -> Unit) {
        viewModelScope.launch {
            try {
                api.deleteIncomingInvoice(id)
                Toasts.show("Törölve.")
                onDeleted()
            } catch (e: Throwable) {
                Toasts.error(describeError(e))
            }
        }
    }

    fun openFile(id: Long, openUrl: (String) -> Unit) {
        viewModelScope.launch {
            try {
                openUrl(api.incomingFileUrl(id))
            } catch (e: Throwable) {
                Toasts.error(describeError(e))
            }
        }
    }
}

@Composable
fun IncomingDetailScreen(
    invoiceId: Long,
    viewModel: IncomingDetailViewModel,
    canEdit: Boolean,
    onBack: () -> Unit,
    onDeleted: () -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    var confirmDelete by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(invoiceId) { viewModel.load(invoiceId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.invoice?.supplierName ?: "Bejövő számla",
                subtitle = state.invoice?.let { "#${it.id} · ${bucketLabel(it.bucket)}" },
                onBack = onBack,
                refreshing = state.loading,
                onRefresh = { viewModel.load(invoiceId) },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            when {
                state.loading -> ListSkeleton()
                state.error != null -> ErrorState(state.error!!) { viewModel.load(invoiceId) }
                state.invoice == null -> EmptyState("Nincs ilyen számla.")
                else -> {
                    val invoice = state.invoice!!
                    Card {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            SectionTitle("Bizonylat")
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                StatusBadge(bucketLabel(invoice.bucket), Tone.Steel)
                                StatusBadge(paymentLabel(invoice.paymentMethod), Tone.Cold)
                                if (invoice.paidAmount > 0 && invoice.grossAmount != null &&
                                    invoice.paidAmount >= invoice.grossAmount
                                ) {
                                    StatusBadge("Fizetve", Tone.Done)
                                }
                            }
                            Text(
                                "Fájl: ${invoice.fileName ?: "—"}",
                                style = MaterialTheme.typography.bodyMedium,
                                color = Steel500,
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                TextButton(onClick = { viewModel.openFile(invoiceId, onOpenUrl) }) {
                                    Icon(Icons.Filled.Description, contentDescription = null)
                                    Text("Megnyitás")
                                }
                            }
                        }
                    }
                    Card {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            SectionTitle("Adatok a bizonylatról")
                            AutoCrmTextField(
                                value = state.supplierName,
                                onValueChange = { viewModel.edit({ copy(supplierName = it) }) },
                                label = "Szállító",
                                modifier = Modifier.fillMaxWidth(),
                                enabled = canEdit,
                            )
                            AutoCrmTextField(
                                value = state.taxNumber,
                                onValueChange = { viewModel.edit({ copy(taxNumber = it) }) },
                                label = "Adószám",
                                modifier = Modifier.fillMaxWidth(),
                                enabled = canEdit,
                            )
                            AutoCrmTextField(
                                value = state.number,
                                onValueChange = { viewModel.edit({ copy(number = it) }) },
                                label = "Bizonylatszám",
                                modifier = Modifier.fillMaxWidth(),
                                enabled = canEdit,
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                AutoCrmTextField(
                                    value = state.issueDate,
                                    onValueChange = { viewModel.edit({ copy(issueDate = it) }) },
                                    label = "Kelte (ÉÉÉÉ-HH-NN)",
                                    modifier = Modifier.weight(1f),
                                    enabled = canEdit,
                                )
                                AutoCrmTextField(
                                    value = state.dueDate,
                                    onValueChange = { viewModel.edit({ copy(dueDate = it) }) },
                                    label = "Határidő",
                                    modifier = Modifier.weight(1f),
                                    enabled = canEdit,
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                listOf("HUF", "EUR").forEach { currency ->
                                    androidx.compose.material3.FilterChip(
                                        selected = state.currency == currency,
                                        onClick = {
                                            viewModel.edit({
                                                copy(
                                                    currency = currency,
                                                    gross = minorToMajor(
                                                        try {
                                                            majorToMinor(gross, this.currency)
                                                        } catch (e: IllegalArgumentException) {
                                                            null
                                                        },
                                                        currency,
                                                    ),
                                                )
                                            })
                                        },
                                        label = { Text(currency) },
                                        enabled = canEdit,
                                    )
                                }
                            }
                            AutoCrmTextField(
                                value = state.gross,
                                onValueChange = { viewModel.edit({ copy(gross = it) }) },
                                label = if (state.currency == "EUR") "Bruttó (euró)" else "Bruttó (Ft)",
                                modifier = Modifier.fillMaxWidth(),
                                enabled = canEdit,
                                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                                    keyboardType = KeyboardType.Decimal,
                                ),
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                listOf("TRANSFER", "CASH", "CARD").forEach { method ->
                                    androidx.compose.material3.FilterChip(
                                        selected = state.paymentMethod == method,
                                        onClick = { viewModel.edit({ copy(paymentMethod = method) }) },
                                        label = { Text(paymentLabel(method)) },
                                        enabled = canEdit,
                                    )
                                }
                            }
                            AutoCrmTextField(
                                value = state.notes,
                                onValueChange = { viewModel.edit({ copy(notes = it) }) },
                                label = "Megjegyzés",
                                modifier = Modifier.fillMaxWidth(),
                                enabled = canEdit,
                                singleLine = false,
                            )
                            if (canEdit) {
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    PrimaryButton(
                                        text = if (state.saving) "Mentés…" else "Mentés",
                                        onClick = { viewModel.save(invoiceId) {} },
                                        enabled = !state.saving,
                                    )
                                    TextButton(
                                        onClick = { viewModel.markPaid(invoiceId) },
                                        enabled = invoice.grossAmount != null,
                                    ) {
                                        Text("Fizetve")
                                    }
                                }
                                TextButton(onClick = { confirmDelete = true }) {
                                    Icon(Icons.Filled.Delete, contentDescription = null)
                                    Text("Törlés")
                                }
                            }
                        }
                    }
                    if (confirmDelete) {
                        DialogShell(
                            title = "Törlöd a számlát?",
                            onDismiss = { confirmDelete = false },
                            actions = {
                                TextButton(onClick = { confirmDelete = false }) { Text("Mégse") }
                                TextButton(onClick = {
                                    confirmDelete = false
                                    viewModel.delete(invoiceId, onDeleted)
                                }) { Text("Törlés") }
                            },
                        ) {
                            Text("A bizonylat lekerül a listáról. A fájl a tárhelyen marad.")
                        }
                    }
                }
            }
        }
    }
}
