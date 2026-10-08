package hu.autotherm.autocrm.ui.orders

import hu.autotherm.autocrm.ui.common.clearFocusOnTap
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ManageSearch
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.OrderBody
import hu.autotherm.autocrm.ui.common.AssigneeField
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.ContactField
import hu.autotherm.autocrm.ui.common.CurrencyChoice
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.FormError
import hu.autotherm.autocrm.ui.common.FormSection
import hu.autotherm.autocrm.ui.common.PartnerChoice
import hu.autotherm.autocrm.ui.common.PartnerField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ProjectTypeField
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Panel
import hu.autotherm.autocrm.ui.theme.Steel500
import java.time.LocalDate
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * The edit form's PATCH body. A field the user emptied is sent as an explicit `null`,
 * which is how the API clears it (docs/API.md: "send null to clear"). Omitting it, as the
 * shared nulls-dropping serializer does for [OrderBody], silently keeps the old value.
 */
internal fun orderPatchJson(s: OrderEditViewModel.State): JsonObject = buildJsonObject {
    fun text(key: String, value: String) {
        val v = value.trim()
        if (v.isEmpty()) put(key, JsonNull) else put(key, v)
    }
    fun id(key: String, value: Long?) {
        if (value == null) put(key, JsonNull) else put(key, value)
    }
    put("title", s.title.trim())
    text("vehicle_make", s.vehicleMake)
    text("vehicle_model", s.vehicleModel)
    text("vehicle_plate", s.vehiclePlate)
    text("vehicle_vin", s.vehicleVin)
    text("description", s.description)
    text("due_date", s.dueDate)
    // The partner and currency are only sent when there is one: the server refuses a null
    // for either, and the form never clears them.
    s.partner?.let { put("partner_id", it.id) }
    if (s.loadedFromServer) {
        id("contact_id", s.contactId)
        id("project_type_id", s.projectTypeId)
        id("assigned_to", s.assignedTo)
        if (s.currency.isNotBlank()) put("currency", s.currency)
    }
}

class OrderEditViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val title: String = "",
        val vehicleMake: String = "",
        val vehicleModel: String = "",
        val vehiclePlate: String = "",
        val vehicleVin: String = "",
        val description: String = "",
        val dueDate: String = "",
        val partner: PartnerChoice? = null,
        val contactId: Long? = null,
        val projectTypeId: Long? = null,
        val assignedTo: Long? = null,
        val currency: String = "HUF",
        /** Items exist: the server refuses a currency change (`currency_locked`). */
        val currencyLocked: Boolean = false,
        /** Edit mode, filled from the server; the PATCH then carries every field. */
        val loadedFromServer: Boolean = false,
        val loading: Boolean = false,
        val vinNote: String? = null,
        /** The VIN lookup is running: a spinner in the field, no second request. */
        val vinDecoding: Boolean = false,
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** What the person typed, as it was when the form opened: "unsaved" means it differs. */
    private fun fingerprint(s: State): List<Any?> = listOf(s.title, s.vehicleMake, s.vehicleModel, s.vehiclePlate, s.vehicleVin, s.description, s.dueDate)
    private var baseline: List<Any?> = fingerprint(State())

    fun isDirty(s: State): Boolean = !s.busy && fingerprint(s) != baseline

    fun load(id: Long) {
        _state.value = _state.value.copy(loading = true)
        viewModelScope.launch {
            try {
                val detail = api.order(id)
                val o = detail.order
                _state.value = State(
                    title = o.title,
                    vehicleMake = o.vehicleMake.orEmpty(),
                    vehicleModel = o.vehicleModel.orEmpty(),
                    vehiclePlate = o.vehiclePlate.orEmpty(),
                    vehicleVin = o.vehicleVin.orEmpty(),
                    description = o.description.orEmpty(),
                    dueDate = o.dueDate.orEmpty(),
                    partner = PartnerChoice(detail.partner.id, detail.partner.name),
                    contactId = o.contactId,
                    projectTypeId = o.projectTypeId,
                    assignedTo = o.assignedTo,
                    currency = o.currency,
                    currencyLocked = detail.items.isNotEmpty(),
                    loadedFromServer = true,
                )
                baseline = fingerprint(_state.value)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    /** A new order started from a partner's page: the partner is already known. */
    fun preselectPartner(partnerId: Long) {
        if (_state.value.partner != null) return
        viewModelScope.launch {
            runCatching { api.partner(partnerId).partner }.onSuccess { p ->
                if (_state.value.partner == null) {
                    _state.value = _state.value.copy(
                        partner = PartnerChoice.of(p),
                        currency = p.defaultCurrency ?: _state.value.currency,
                    )
                }
            }
        }
    }

    fun set(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    fun choosePartner(p: PartnerChoice?) {
        val s = _state.value
        _state.value = s.copy(
            partner = p,
            // A new partner's people are not the old one's.
            contactId = if (p?.id != s.partner?.id) null else s.contactId,
            // The partner's invoicing currency, unless the order already has items.
            currency = if (!s.currencyLocked && p?.defaultCurrency != null) p.defaultCurrency else s.currency,
            error = null,
        )
    }

    /** A typed or dictated VIN; a complete one is looked up straight away. */
    fun setVin(raw: String) {
        val vin = raw.uppercase().filter { it.isLetterOrDigit() }.take(17)
        val before = _state.value.vehicleVin
        _state.value = _state.value.copy(vehicleVin = vin, vinNote = null)
        if (vin != before && hu.autotherm.autocrm.util.isCompleteVin(vin)) decodeVin()
    }

    /** Reads the VIN: fills in the make (and model) when the fields are still empty. */
    fun decodeVin() {
        val vin = _state.value.vehicleVin.trim()
        if (vin.length < 17) {
            _state.value = _state.value.copy(vinNote = "Az alvázszám 17 karakter.")
            return
        }
        if (_state.value.vinDecoding) return
        _state.value = _state.value.copy(vinDecoding = true, vinNote = "Alvázszám kiolvasása…")
        viewModelScope.launch {
            try {
                val info = api.decodeVin(vin)
                val maker = info.make ?: info.manufacturer?.substringBefore(" (")
                val s = _state.value
                _state.value = s.copy(
                    vinDecoding = false,
                    vehicleVin = info.vin,
                    vehicleMake = s.vehicleMake.ifBlank { maker.orEmpty() },
                    vehicleModel = s.vehicleModel.ifBlank { info.model.orEmpty() },
                    vinNote = listOfNotNull(
                        info.manufacturer ?: info.make,
                        info.model,
                        info.modelYear?.let { "$it-es modellév" },
                        info.region,
                    ).joinToString(" · ").ifBlank { "Ismeretlen gyártó" },
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(vinDecoding = false, vinNote = describeError(e))
            }
        }
    }

    /** What still stops a save, in words; null when the form can go. */
    fun missing(isNew: Boolean): String? {
        val s = _state.value
        return when {
            isNew && s.partner == null -> "Válassz vagy hozz létre partnert."
            s.title.isBlank() -> "Adj címet a munkának."
            else -> null
        }
    }

    fun save(orderId: Long?, onSaved: (Long) -> Unit) {
        val s = _state.value
        if (s.busy) return
        missing(orderId == null)?.let {
            _state.value = s.copy(error = it)
            return
        }
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
            try {
                val id = if (orderId == null) {
                    api.createOrder(
                        OrderBody(
                            title = s.title.trim(),
                            partnerId = s.partner!!.id,
                            contactId = s.contactId,
                            assignedTo = s.assignedTo,
                            projectTypeId = s.projectTypeId,
                            currency = s.currency,
                            valuationDate = LocalDate.now().toString(),
                            vehicleMake = blankToNull(s.vehicleMake),
                            vehicleModel = blankToNull(s.vehicleModel),
                            vehiclePlate = blankToNull(s.vehiclePlate),
                            vehicleVin = blankToNull(s.vehicleVin),
                            description = blankToNull(s.description),
                            dueDate = blankToNull(s.dueDate),
                        ),
                    ).id
                } else {
                    api.patchOrder(orderId, orderPatchJson(s)).id
                }
                _state.value = _state.value.copy(busy = false)
                hu.autotherm.autocrm.ui.common.Toasts.show(if (orderId == null) "Munka létrehozva" else "Munka mentve")
                onSaved(id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OrderEditScreen(
    orderId: Long?,
    partnerId: Long?,
    viewModel: OrderEditViewModel,
    onBack: () -> Unit,
    onSaved: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val guardedBack = hu.autotherm.autocrm.ui.common.rememberDiscardGuard(viewModel.isDirty(state), onBack)
    LaunchedEffect(orderId, partnerId) {
        if (orderId != null) viewModel.load(orderId)
        else if (partnerId != null) viewModel.preselectPartner(partnerId)
    }
    val isNew = orderId == null

    Scaffold(
        containerColor = Panel,
        topBar = {
            ScreenTopBar(
                title = if (isNew) "Új munka" else "Munka szerkesztése",
                onBack = guardedBack,
                // Reachable from anywhere in a long form, not only after scrolling to the end.
                actions = {
                    androidx.compose.material3.TextButton(
                        onClick = { viewModel.save(orderId, onSaved) },
                        enabled = !state.busy,
                    ) { androidx.compose.material3.Text("Mentés") }
                },
            )
        },
    ) { padding ->
        if (state.loading) {
            DetailSkeleton(Modifier.padding(padding).padding(16.dp))
            return@Scaffold
        }
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().clearFocusOnTap()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            FormSection("Ügyfél") {
                PartnerField(
                    selected = state.partner,
                    onSelect = viewModel::choosePartner,
                    supporting = if (state.partner == null) "Keress rá, vagy hozd létre egy koppintással." else null,
                )
                ContactField(
                    partnerId = state.partner?.id,
                    selected = state.contactId,
                    onSelect = { c -> viewModel.set { it.copy(contactId = c?.id) } },
                )
            }
            FormSection("Munka") {
                AutoCrmTextField(
                    value = state.title,
                    onValueChange = { v -> viewModel.set { it.copy(title = v) } },
                    label = "Megnevezés *",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    modifier = Modifier.fillMaxWidth(),
                )
                ProjectTypeField(
                    selected = state.projectTypeId,
                    onSelect = { v -> viewModel.set { it.copy(projectTypeId = v) } },
                    modifier = Modifier.fillMaxWidth(),
                )
                CurrencyChoice(
                    selected = state.currency,
                    onSelect = { c -> viewModel.set { it.copy(currency = c) } },
                    enabled = !state.currencyLocked,
                    locked = if (state.currencyLocked) "Tételek után a pénznem már nem váltható." else null,
                )
                AssigneeField(
                    selected = state.assignedTo,
                    onSelect = { v -> viewModel.set { it.copy(assignedTo = v) } },
                    modifier = Modifier.fillMaxWidth(),
                )
                DateField(
                    value = state.dueDate,
                    onValueChange = { v -> viewModel.set { it.copy(dueDate = v) } },
                    label = "Vállalt határidő",
                    modifier = Modifier.fillMaxWidth(),
                    quickPicks = listOf("1 hét" to 7L, "2 hét" to 14L, "1 hónap" to 30L),
                )
            }
            FormSection("Jármű") {
                AutoCrmTextField(
                    value = state.vehiclePlate,
                    onValueChange = { v -> viewModel.set { it.copy(vehiclePlate = v.uppercase()) } },
                    label = "Rendszám",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = state.vehicleVin,
                    onValueChange = viewModel::setVin,
                    label = "Alvázszám (VIN) – gépelve vagy diktálva",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                    trailing = {
                        androidx.compose.foundation.layout.Row {
                            // Read it out loud: "vé dé bé kilenc…" or "WDB 906…".
                            hu.autotherm.autocrm.ui.common.DictationButton(
                                onText = { spoken -> viewModel.setVin(hu.autotherm.autocrm.util.spokenToVin(spoken)) },
                            )
                            if (state.vinDecoding) {
                                androidx.compose.material3.CircularProgressIndicator(
                                    modifier = Modifier.padding(12.dp).size(24.dp),
                                    strokeWidth = 2.dp,
                                )
                            } else {
                                IconButton(onClick = viewModel::decodeVin) {
                                    Icon(Icons.Filled.ManageSearch, contentDescription = "Alvázszám kiolvasása")
                                }
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
                state.vinNote?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Steel500) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    AutoCrmTextField(
                        value = state.vehicleMake,
                        onValueChange = { v -> viewModel.set { it.copy(vehicleMake = v) } },
                        label = "Gyártmány",
                        modifier = Modifier.weight(1f),
                    )
                    AutoCrmTextField(
                        value = state.vehicleModel,
                        onValueChange = { v -> viewModel.set { it.copy(vehicleModel = v) } },
                        label = "Típus",
                        modifier = Modifier.weight(1f),
                    )
                }
            }
            FormSection("Leírás") {
                AutoCrmTextField(
                    value = state.description,
                    onValueChange = { v -> viewModel.set { it.copy(description = v) } },
                    label = "Mit kell csinálni?",
                    singleLine = false,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    modifier = Modifier.fillMaxWidth().heightIn(min = 112.dp),
                )
            }
            FormError(state.error)
            PrimaryButton(
                text = if (state.busy) "Mentés…" else if (isNew) "Munka létrehozása" else "Mentés",
                onClick = { viewModel.save(orderId, onSaved) },
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(24.dp))
        }
    }
}
