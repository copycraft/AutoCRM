package hu.autotherm.autocrm.ui.partners

import androidx.compose.ui.focus.focusRequester
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
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
import hu.autotherm.autocrm.data.api.PartnerBody
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.COUNTRIES
import hu.autotherm.autocrm.ui.common.CurrencyChoice
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.FieldLabel
import hu.autotherm.autocrm.ui.common.FormError
import hu.autotherm.autocrm.ui.common.FormSection
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SegmentedChoice
import hu.autotherm.autocrm.ui.common.SelectField
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Panel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Hungarian tax numbers as the server normalises them: 8-1-2 digits. */
private val HU_TAX = Regex("^\\d{8}-?\\d-?\\d{2}$")

class PartnerEditViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = false,
        val name: String = "",
        val kind: String = "business",
        val role: String = "customer",
        val country: String = "HU",
        val currency: String = "HUF",
        val taxNumber: String = "",
        val euTaxNumber: String = "",
        val email: String = "",
        val phone: String = "",
        val website: String = "",
        val postalCode: String = "",
        val city: String = "",
        val address: String = "",
        val notes: String = "",
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** What the person typed, as it was when the form opened: "unsaved" means it differs. */
    private fun fingerprint(s: State): List<Any?> = listOf(s.name, s.taxNumber, s.euTaxNumber, s.email, s.phone, s.website, s.postalCode, s.city, s.address, s.notes)
    private var baseline: List<Any?> = fingerprint(State())

    fun isDirty(s: State): Boolean = !s.busy && fingerprint(s) != baseline

    fun load(id: Long) {
        viewModelScope.launch {
            _state.value = _state.value.copy(loading = true, error = null)
            try {
                val p = api.partner(id).partner
                _state.value = State(
                    name = p.name,
                    kind = p.kind,
                    role = p.role ?: "customer",
                    country = p.country,
                    currency = p.defaultCurrency ?: "HUF",
                    // Digits only for a Hungarian number: the field draws the dashes itself.
                    taxNumber = p.taxNumber.orEmpty().let { t ->
                        if (p.country == "HU" && HU_TAX.matches(t)) t.filter { it.isDigit() } else t
                    },
                    euTaxNumber = p.euTaxNumber.orEmpty(),
                    email = p.email.orEmpty(),
                    phone = p.phone.orEmpty(),
                    website = p.website.orEmpty(),
                    postalCode = p.postalCode.orEmpty(),
                    city = p.city.orEmpty(),
                    address = p.addressLine.orEmpty(),
                    notes = p.notes.orEmpty(),
                )
                baseline = fingerprint(_state.value)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    fun set(field: (State) -> State) {
        _state.value = field(_state.value)
    }

    /** The first thing the server would refuse, said before asking it. */
    private fun problem(s: State): String? = when {
        s.name.isBlank() -> "A név kötelező."
        s.country == "HU" && s.taxNumber.isNotBlank() && !HU_TAX.matches(s.taxNumber.trim()) ->
            "A magyar adószám formája 12345678-1-23."
        s.email.isNotBlank() && !s.email.trim().contains('@') -> "Az e-mail cím nem érvényes."
        else -> null
    }

    fun save(partnerId: Long?, onSaved: (Long) -> Unit) {
        val s = _state.value
        if (s.busy) return
        problem(s)?.let {
            _state.value = s.copy(error = it)
            return
        }
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
            try {
                val body = PartnerBody(
                    kind = s.kind,
                    name = s.name.trim(),
                    country = s.country,
                    defaultCurrency = s.currency,
                    taxNumber = blankToNull(s.taxNumber),
                    euTaxNumber = blankToNull(s.euTaxNumber)?.uppercase(),
                    email = blankToNull(s.email),
                    phone = blankToNull(s.phone),
                    website = blankToNull(s.website),
                    postalCode = blankToNull(s.postalCode),
                    city = blankToNull(s.city),
                    addressLine = blankToNull(s.address),
                    notes = blankToNull(s.notes),
                    role = s.role,
                )
                val id = if (partnerId == null) {
                    api.createPartner(body).id
                } else {
                    // Every field this form shows, nulls included: emptying a field clears it.
                    api.patchPartner(
                        partnerId,
                        hu.autotherm.autocrm.data.api.patchOf(
                            "kind" to body.kind,
                            "name" to body.name,
                            "role" to body.role,
                            "country" to body.country,
                            "default_currency" to body.defaultCurrency,
                            "tax_number" to body.taxNumber,
                            "eu_tax_number" to body.euTaxNumber,
                            "email" to body.email,
                            "phone" to body.phone,
                            "website" to body.website,
                            "postal_code" to body.postalCode,
                            "city" to body.city,
                            "address_line" to body.addressLine,
                            "notes" to body.notes,
                        ),
                    ).id
                }
                _state.value = _state.value.copy(busy = false)
                hu.autotherm.autocrm.ui.common.Toasts.show(if (partnerId == null) "Partner létrehozva" else "Partner mentve")
                onSaved(id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PartnerEditScreen(
    partnerId: Long?,
    viewModel: PartnerEditViewModel,
    onBack: () -> Unit,
    onSaved: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    // A new record starts with its name: the keyboard opens on it.
    val firstField = androidx.compose.runtime.remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(Unit) {
        if (partnerId == null) {
            kotlinx.coroutines.delay(250) // after the slide-in, or the keyboard fights the animation
            runCatching { firstField.requestFocus() }
        }
    }
    val guardedBack = hu.autotherm.autocrm.ui.common.rememberDiscardGuard(viewModel.isDirty(state), onBack)
    LaunchedEffect(partnerId) {
        if (partnerId != null) viewModel.load(partnerId)
    }

    Scaffold(
        containerColor = Panel,
        topBar = {
            ScreenTopBar(
                title = if (partnerId == null) "Új partner" else "Partner szerkesztése",
                onBack = guardedBack,
                // Reachable from anywhere in a long form, not only after scrolling to the end.
                actions = {
                    androidx.compose.material3.TextButton(
                        onClick = { viewModel.save(partnerId, onSaved) },
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
            SegmentedChoice(
                options = listOf("business" to "Cég", "person" to "Magánszemély"),
                selected = state.kind,
                onSelect = { k -> viewModel.set { it.copy(kind = k) } },
            )
            FormSection("Alapadatok") {
                AutoCrmTextField(
                    value = state.name,
                    onValueChange = { v -> viewModel.set { it.copy(name = v) } },
                    label = if (state.kind == "business") "Cégnév *" else "Név *",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                    modifier = Modifier.fillMaxWidth().focusRequester(firstField),
                )
                FieldLabel("Szerep")
                SegmentedChoice(
                    options = listOf("customer" to "Ügyfél", "supplier" to "Beszállító", "both" to "Mindkettő"),
                    selected = state.role,
                    onSelect = { r -> viewModel.set { it.copy(role = r) } },
                )
                SelectField(
                    label = "Ország",
                    options = COUNTRIES.let { list ->
                        if (list.any { it.first == state.country }) list else list + (state.country to state.country)
                    },
                    selected = state.country,
                    onSelect = { c ->
                        viewModel.set {
                            val country = c ?: "HU"
                            it.copy(
                                country = country,
                                // A new foreign partner is usually invoiced in euros.
                                currency = if (partnerId == null) (if (country == "HU") "HUF" else "EUR") else it.currency,
                            )
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
                CurrencyChoice(
                    selected = state.currency,
                    onSelect = { c -> viewModel.set { it.copy(currency = c) } },
                )
            }
            if (state.kind == "business") {
                FormSection("Adószám", hint = "Számlához kell.") {
                    AutoCrmTextField(
                        value = state.taxNumber,
                        onValueChange = { v ->
                            viewModel.set {
                                it.copy(taxNumber = if (state.country == "HU") v.filter { c -> c.isDigit() }.take(11) else v)
                            }
                        },
                        visualTransformation = if (state.country == "HU") hu.autotherm.autocrm.util.HuTaxNumberTransformation else androidx.compose.ui.text.input.VisualTransformation.None,
                        label = if (state.country == "HU") "Adószám (12345678-1-23)" else "Helyi adószám",
                        isError = state.country == "HU" && state.taxNumber.isNotEmpty() && state.taxNumber.length != 11,
                        supporting = if (state.country == "HU" && state.taxNumber.isNotEmpty() && state.taxNumber.length != 11) "${state.taxNumber.length}/11 számjegy" else null,
                        keyboardOptions = KeyboardOptions(
                            keyboardType = if (state.country == "HU") KeyboardType.Number else KeyboardType.Text,
                        ),
                        modifier = Modifier.fillMaxWidth(),
                    )
                    AutoCrmTextField(
                        value = state.euTaxNumber,
                        onValueChange = { v -> viewModel.set { it.copy(euTaxNumber = v.uppercase()) } },
                        label = "Közösségi adószám (HU12345678, DE123456789)",
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
            FormSection("Elérhetőség") {
                AutoCrmTextField(
                    value = state.email,
                    onValueChange = { v -> viewModel.set { it.copy(email = v.trim()) } },
                    label = "E-mail",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = state.phone,
                    onValueChange = { v -> viewModel.set { it.copy(phone = v) } },
                    label = "Telefon",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = state.website,
                    onValueChange = { v -> viewModel.set { it.copy(website = v.trim()) } },
                    label = "Weboldal",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            FormSection("Cím") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    AutoCrmTextField(
                        value = state.postalCode,
                        // Hungarian postcodes are four digits; abroad, anything goes.
                        onValueChange = { v ->
                            viewModel.set {
                                it.copy(postalCode = if (state.country == "HU") v.filter { c -> c.isDigit() }.take(4) else v)
                            }
                        },
                        isError = state.country == "HU" && state.postalCode.isNotEmpty() && state.postalCode.length != 4,
                        label = "Irsz.",
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        modifier = Modifier.weight(0.35f),
                    )
                    AutoCrmTextField(
                        value = state.city,
                        onValueChange = { v -> viewModel.set { it.copy(city = v) } },
                        label = "Város",
                        modifier = Modifier.weight(0.65f),
                    )
                }
                AutoCrmTextField(
                    value = state.address,
                    onValueChange = { v -> viewModel.set { it.copy(address = v) } },
                    label = "Utca, házszám",
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            FormSection("Megjegyzés") {
                AutoCrmTextField(
                    value = state.notes,
                    onValueChange = { v -> viewModel.set { it.copy(notes = v) } },
                    label = "Megjegyzés",
                    singleLine = false,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 112.dp),
                )
            }
            FormError(state.error)
            PrimaryButton(
                text = if (state.busy) "Mentés…" else if (partnerId == null) "Partner létrehozása" else "Mentés",
                onClick = { viewModel.save(partnerId, onSaved) },
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(24.dp))
        }
    }
}
