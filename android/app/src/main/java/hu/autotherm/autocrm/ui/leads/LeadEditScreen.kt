package hu.autotherm.autocrm.ui.leads

import androidx.compose.ui.focus.focusRequester
import hu.autotherm.autocrm.ui.common.clearFocusOnTap
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.LeadBody
import hu.autotherm.autocrm.ui.common.AssigneeField
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.CurrencyChoice
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.FieldLabel
import hu.autotherm.autocrm.ui.common.FormError
import hu.autotherm.autocrm.ui.common.FormSection
import hu.autotherm.autocrm.ui.common.PartnerChoice
import hu.autotherm.autocrm.ui.common.PartnerField
import hu.autotherm.autocrm.ui.common.PartnerPrefill
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Panel
import hu.autotherm.autocrm.util.minorToMajorInput
import hu.autotherm.autocrm.util.parseMajorToMinor
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class LeadEditViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val title: String = "",
        val partner: PartnerChoice? = null,
        val contactName: String = "",
        val contactEmail: String = "",
        val contactPhone: String = "",
        val source: String = "",
        val sourceDetail: String = "",
        /** The list the source picker offers (0048). */
        val sources: List<hu.autotherm.autocrm.data.api.LeadSource> = emptyList(),
        val assignedTo: Long? = null,
        val description: String = "",
        val quotedValue: String = "",
        val currency: String = "HUF",
        val quoteValidUntil: String = "",
        /** The partner and contact the lead had when loaded: a new partner drops the contact. */
        val loadedPartnerId: Long? = null,
        val loadedContactId: Long? = null,
        val loading: Boolean = false,
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** What the person typed, as it was when the form opened: "unsaved" means it differs. */
    private fun fingerprint(s: State): List<Any?> = listOf(s.title, s.contactName, s.contactEmail, s.contactPhone, s.sourceDetail, s.description, s.quotedValue, s.quoteValidUntil)
    private var baseline: List<Any?> = fingerprint(State())

    fun isDirty(s: State): Boolean = !s.busy && fingerprint(s) != baseline

    fun load(id: Long) {
        _state.value = _state.value.copy(loading = true)
        viewModelScope.launch {
            try {
                val l = api.lead(id).lead
                val sources = runCatching { api.leadSources() }.getOrDefault(emptyList())
                // A partner that cannot be fetched (offline, no access) is still the lead's:
                // shown by number rather than dropped, so saving cannot unlink it.
                val partner = l.partnerId?.let { pid ->
                    runCatching { PartnerChoice.of(api.partner(pid).partner) }.getOrNull()
                        ?: PartnerChoice(pid, "Partner #$pid")
                }
                _state.value = State(
                    sources = sources,
                    sourceDetail = l.sourceDetail.orEmpty(),
                    title = l.title,
                    partner = partner,
                    contactName = l.contactName.orEmpty(),
                    contactEmail = l.contactEmail.orEmpty(),
                    contactPhone = l.contactPhone.orEmpty(),
                    source = l.source.orEmpty(),
                    assignedTo = l.assignedTo,
                    description = l.description.orEmpty(),
                    quotedValue = l.quotedValueMinor?.let(::minorToMajorInput).orEmpty(),
                    currency = l.currency ?: "HUF",
                    quoteValidUntil = l.quoteValidUntil.orEmpty(),
                    loadedPartnerId = l.partnerId,
                    loadedContactId = l.contactId,
                )
                baseline = fingerprint(_state.value)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    fun loadSources(partnerId: Long?) {
        viewModelScope.launch {
            runCatching { api.leadSources() }.onSuccess { list ->
                _state.value = _state.value.copy(sources = list)
            }
            if (partnerId != null && _state.value.partner == null) {
                runCatching { api.partner(partnerId).partner }.onSuccess { p ->
                    _state.value = _state.value.copy(partner = PartnerChoice.of(p), currency = p.defaultCurrency ?: "HUF")
                }
            }
        }
    }

    fun set(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    fun choosePartner(p: PartnerChoice?) {
        _state.value = _state.value.copy(
            partner = p,
            currency = p?.defaultCurrency ?: _state.value.currency,
            error = null,
        )
    }

    fun save(leadId: Long?, onSaved: (Long) -> Unit) {
        val s = _state.value
        if (s.busy) return
        if (s.title.isBlank()) {
            _state.value = s.copy(error = "Adj címet az érdeklődésnek (mit szeretne?).")
            return
        }
        val quoted = if (s.quotedValue.isBlank()) null else parseMajorToMinor(s.quotedValue)
        if (s.quotedValue.isNotBlank() && quoted == null) {
            _state.value = s.copy(error = "Az ajánlati összeg nem szám.")
            return
        }
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
            try {
                val body = LeadBody(
                    title = s.title.trim(),
                    partnerId = s.partner?.id,
                    assignedTo = s.assignedTo,
                    contactName = blankToNull(s.contactName),
                    contactEmail = blankToNull(s.contactEmail),
                    contactPhone = blankToNull(s.contactPhone),
                    source = blankToNull(s.source),
                    sourceDetail = blankToNull(s.sourceDetail),
                    description = blankToNull(s.description),
                    quotedValueMinor = quoted,
                    // A quote needs its currency (the database insists); the choice is kept
                    // either way so an EUR lead stays EUR.
                    currency = s.currency,
                    quoteValidUntil = blankToNull(s.quoteValidUntil),
                )
                val id = if (leadId == null) {
                    api.createLead(body).id
                } else {
                    // Explicit nulls for what the form shows: an emptied field is cleared.
                    val fields = mutableListOf<Pair<String, Any?>>(
                        "title" to body.title,
                        "partner_id" to body.partnerId,
                        "assigned_to" to body.assignedTo,
                        "contact_name" to body.contactName,
                        "contact_email" to body.contactEmail,
                        "contact_phone" to body.contactPhone,
                        "source" to body.source,
                        "source_detail" to body.sourceDetail,
                        "description" to body.description,
                        "quoted_value_minor" to body.quotedValueMinor,
                        "currency" to body.currency,
                        "quote_valid_until" to body.quoteValidUntil,
                    )
                    // The stored contact belongs to the old partner; the server refuses the pair.
                    if (s.loadedContactId != null && body.partnerId != s.loadedPartnerId) {
                        fields += "contact_id" to null
                    }
                    api.patchLead(leadId, hu.autotherm.autocrm.data.api.patchOf(*fields.toTypedArray())).id
                }
                _state.value = _state.value.copy(busy = false)
                hu.autotherm.autocrm.ui.common.Toasts.show(if (leadId == null) "Érdeklődés rögzítve" else "Érdeklődés mentve")
                onSaved(id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun LeadEditScreen(
    leadId: Long?,
    partnerId: Long?,
    viewModel: LeadEditViewModel,
    onBack: () -> Unit,
    onSaved: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    // A new record starts with its name: the keyboard opens on it.
    val firstField = androidx.compose.runtime.remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(Unit) {
        if (leadId == null) {
            kotlinx.coroutines.delay(250) // after the slide-in, or the keyboard fights the animation
            runCatching { firstField.requestFocus() }
        }
    }
    val guardedBack = hu.autotherm.autocrm.ui.common.rememberDiscardGuard(viewModel.isDirty(state), onBack)
    LaunchedEffect(leadId) {
        if (leadId != null) viewModel.load(leadId) else viewModel.loadSources(partnerId)
    }

    Scaffold(
        containerColor = Panel,
        topBar = {
            ScreenTopBar(
                title = if (leadId == null) "Új érdeklődés" else "Érdeklődés szerkesztése",
                onBack = guardedBack,
                // Reachable from anywhere in a long form, not only after scrolling to the end.
                actions = {
                    androidx.compose.material3.TextButton(
                        onClick = { viewModel.save(leadId, onSaved) },
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
            FormSection("Érdeklődés") {
                AutoCrmTextField(
                    value = state.title,
                    onValueChange = { v -> viewModel.set { it.copy(title = v) } },
                    label = "Mit szeretne? *",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    modifier = Modifier.fillMaxWidth().focusRequester(firstField),
                )
                FieldLabel("Forrás")
                androidx.compose.foundation.layout.FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    state.sources
                        .filter { (!it.isSystem && it.archivedAt == null) || it.key == state.source }
                        .forEach { src ->
                            androidx.compose.material3.FilterChip(
                                selected = state.source == src.key,
                                enabled = !src.isSystem,
                                onClick = {
                                    viewModel.set { it.copy(source = if (it.source == src.key) "" else src.key) }
                                },
                                label = { Text(src.label) },
                            )
                        }
                }
                AutoCrmTextField(
                    value = state.sourceDetail,
                    onValueChange = { v -> viewModel.set { it.copy(sourceDetail = v) } },
                    label = "Forrás részletei (vásár, ki ajánlotta…)",
                    modifier = Modifier.fillMaxWidth(),
                )
                AssigneeField(
                    selected = state.assignedTo,
                    onSelect = { v -> viewModel.set { it.copy(assignedTo = v) } },
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = state.description,
                    onValueChange = { v -> viewModel.set { it.copy(description = v) } },
                    label = "Részletek",
                    singleLine = false,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    modifier = Modifier.fillMaxWidth().heightIn(min = 112.dp),
                )
            }
            FormSection("Ügyfél", hint = "Partner nélkül is menthető; megrendeléshez kell majd.") {
                PartnerField(
                    selected = state.partner,
                    onSelect = viewModel::choosePartner,
                    label = "Partner",
                    required = false,
                    prefill = PartnerPrefill(
                        name = state.contactName.ifBlank { null },
                        email = state.contactEmail.ifBlank { null },
                        phone = state.contactPhone.ifBlank { null },
                    ),
                )
                AutoCrmTextField(
                    value = state.contactName,
                    onValueChange = { v -> viewModel.set { it.copy(contactName = v) } },
                    label = "Kapcsolattartó neve",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = state.contactEmail,
                    onValueChange = { v -> viewModel.set { it.copy(contactEmail = v.trim()) } },
                    label = "E-mail",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = state.contactPhone,
                    onValueChange = { v -> viewModel.set { it.copy(contactPhone = v) } },
                    label = "Telefon",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            FormSection("Ajánlat") {
                CurrencyChoice(
                    selected = state.currency,
                    onSelect = { c -> viewModel.set { it.copy(currency = c) } },
                )
                AutoCrmTextField(
                    value = state.quotedValue,
                    onValueChange = { v -> viewModel.set { it.copy(quotedValue = v.filter { c -> c.isDigit() || c == ',' || c == '.' }) } },
                    visualTransformation = hu.autotherm.autocrm.util.GroupedNumberTransformation,
                    label = "Ajánlati összeg (${if (state.currency == "EUR") "€" else "Ft"}, nettó)",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    modifier = Modifier.fillMaxWidth(),
                )
                DateField(
                    value = state.quoteValidUntil,
                    onValueChange = { v -> viewModel.set { it.copy(quoteValidUntil = v) } },
                    label = "Ajánlat érvényes eddig",
                    modifier = Modifier.fillMaxWidth(),
                    quickPicks = listOf("2 hét" to 14L, "1 hónap" to 30L, "3 hónap" to 90L),
                )
            }
            FormError(state.error)
            PrimaryButton(
                text = if (state.busy) "Mentés…" else if (leadId == null) "Érdeklődés rögzítése" else "Mentés",
                onClick = { viewModel.save(leadId, onSaved) },
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(24.dp))
        }
    }
}
