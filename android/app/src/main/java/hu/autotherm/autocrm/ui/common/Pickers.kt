package hu.autotherm.autocrm.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Business
import androidx.compose.material.icons.filled.History
import androidx.compose.runtime.collectAsState
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.Contact
import hu.autotherm.autocrm.data.api.Partner
import hu.autotherm.autocrm.data.api.PartnerBody
import hu.autotherm.autocrm.ui.theme.Cold
import hu.autotherm.autocrm.ui.theme.Panel
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.ui.theme.Steel900
import hu.autotherm.autocrm.ui.theme.Surface
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** A chosen partner, with what the forms take from it (its currency, its country). */
data class PartnerChoice(
    val id: Long,
    val name: String,
    val defaultCurrency: String? = null,
    val country: String? = null,
) {
    companion object {
        fun of(p: Partner) = PartnerChoice(p.id, p.name, p.defaultCurrency, p.country)
    }
}

/** What a new partner starts with when created from a lead: the person who enquired. */
data class PartnerPrefill(
    val name: String? = null,
    val email: String? = null,
    val phone: String? = null,
)

/**
 * A field that looks like an input but opens a picker: the label floats, the value reads like
 * typed text, a chevron says "tap me".
 */
@Composable
fun PickerField(
    label: String,
    value: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    supporting: String? = null,
    onClear: (() -> Unit)? = null,
) {
    Column(modifier) {
        Box {
            AutoCrmTextField(
                value = value.orEmpty(),
                onValueChange = {},
                label = label,
                readOnly = true,
                enabled = enabled,
                modifier = Modifier.fillMaxWidth(),
                trailing = {
                    if (onClear != null && !value.isNullOrBlank() && enabled) {
                        IconButton(onClick = onClear) {
                            Icon(Icons.Filled.Close, contentDescription = "Törlés", tint = Steel500)
                        }
                    } else {
                        Icon(Icons.Filled.ArrowDropDown, contentDescription = null, tint = Steel500)
                    }
                },
            )
            // The whole field is the button: a read-only text field swallows taps otherwise.
            Box(
                Modifier
                    .matchParentSize()
                    .padding(end = if (onClear != null && !value.isNullOrBlank()) 48.dp else 0.dp)
                    .clickable(enabled = enabled, onClick = onClick),
            )
        }
        if (supporting != null) {
            Text(
                supporting,
                style = MaterialTheme.typography.bodySmall,
                color = Steel500,
                modifier = Modifier.padding(start = 14.dp, top = 4.dp),
            )
        }
    }
}

/** A choice from a short list, as a dropdown under the field. */
@Composable
fun <T> SelectField(
    label: String,
    options: List<Pair<T, String>>,
    selected: T?,
    onSelect: (T?) -> Unit,
    modifier: Modifier = Modifier,
    noneLabel: String? = null,
    enabled: Boolean = true,
    supporting: String? = null,
) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        PickerField(
            label = label,
            value = options.firstOrNull { it.first == selected }?.second ?: (if (selected == null) noneLabel else null),
            onClick = { open = true },
            enabled = enabled,
            supporting = supporting,
            modifier = Modifier.fillMaxWidth(),
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            if (noneLabel != null) {
                DropdownMenuItem(
                    text = { Text(noneLabel, color = Steel500) },
                    onClick = {
                        onSelect(null)
                        open = false
                    },
                )
            }
            options.forEach { (value, text) ->
                DropdownMenuItem(
                    text = { Text(text) },
                    trailingIcon = if (value == selected) ({ Icon(Icons.Filled.Check, contentDescription = null, tint = Cold) }) else null,
                    onClick = {
                        onSelect(value)
                        open = false
                    },
                )
            }
        }
    }
}

@Composable
internal fun app(): AutoCrmApp = LocalContext.current.applicationContext as AutoCrmApp

/** Project types offered for an order (active ones), with the build-spec form each asks for. */
@Composable
fun ProjectTypeField(
    selected: Long?,
    onSelect: (Long?) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val api = app().api
    val types by produceState(initialValue = emptyList<hu.autotherm.autocrm.data.api.ProjectType>()) {
        value = runCatching { api.projectTypes() }.getOrDefault(emptyList())
    }
    SelectField(
        label = "Projekt típusa",
        options = types.filter { it.isActive || it.id == selected }.map { it.id to it.labelHu },
        selected = selected,
        onSelect = onSelect,
        noneLabel = "Nincs megadva",
        enabled = enabled,
        modifier = modifier,
    )
}

/** Who is responsible: the staff accounts that can be named. */
@Composable
fun AssigneeField(
    selected: Long?,
    onSelect: (Long?) -> Unit,
    modifier: Modifier = Modifier,
    label: String = "Felelős",
    enabled: Boolean = true,
) {
    val application = app()
    val api = application.api
    val people by produceState(initialValue = emptyList<hu.autotherm.autocrm.data.api.Mentionable>()) {
        value = runCatching { api.mentionable() }.getOrDefault(emptyList())
    }
    val me by application.sessionStore.account.collectAsState(initial = null)
    val myId = me?.userId
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        SelectField(
            label = label,
            // "Én" first: most jobs and tasks are taken by whoever is holding the phone.
            options = people.sortedByDescending { it.id == myId }
                .map { it.id to if (it.id == myId) "${it.displayName} (én)" else it.displayName },
            selected = selected,
            onSelect = onSelect,
            noneLabel = "Nincs felelős",
            enabled = enabled,
            modifier = Modifier.fillMaxWidth(),
        )
        if (enabled && myId != null && selected != myId && people.any { it.id == myId }) {
            androidx.compose.material3.AssistChip(
                onClick = { onSelect(myId) },
                label = { Text("Én vállalom") },
                leadingIcon = {
                    Icon(Icons.Filled.Person, contentDescription = null, modifier = Modifier.size(18.dp))
                },
            )
        }
    }
}

/** The contact person at the chosen partner (reloaded when the partner changes). */
@Composable
fun ContactField(
    partnerId: Long?,
    selected: Long?,
    onSelect: (Contact?) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val api = app().api
    val contacts by produceState(initialValue = emptyList<Contact>(), partnerId) {
        value = if (partnerId == null) emptyList() else runCatching { api.partner(partnerId).contacts }.getOrDefault(emptyList())
    }
    SelectField(
        label = "Kapcsolattartó",
        options = contacts.map { it.id to listOfNotNull(it.name, it.position).joinToString(" · ") },
        selected = selected,
        onSelect = { id -> onSelect(contacts.firstOrNull { it.id == id }) },
        noneLabel = if (partnerId == null) "Előbb válassz partnert" else if (contacts.isEmpty()) "Nincs rögzített kapcsolattartó" else "Nincs kiválasztva",
        enabled = enabled && partnerId != null,
        modifier = modifier,
    )
}

/**
 * The partner of an order or lead: search by name, tax number, city, or create a new one on
 * the spot. Creating is the common case for a first job, so it is one tap away, prefilled
 * from what is known (a lead's contact).
 */
@Composable
fun PartnerField(
    selected: PartnerChoice?,
    onSelect: (PartnerChoice?) -> Unit,
    modifier: Modifier = Modifier,
    label: String = "Partner *",
    required: Boolean = true,
    canCreate: Boolean = true,
    prefill: PartnerPrefill? = null,
    enabled: Boolean = true,
    supporting: String? = null,
) {
    var open by remember { mutableStateOf(false) }
    PickerField(
        label = label,
        value = selected?.name,
        onClick = { open = true },
        enabled = enabled,
        supporting = supporting,
        onClear = if (!required) ({ onSelect(null) }) else null,
        modifier = modifier,
    )
    if (open) {
        PartnerPickerDialog(
            canCreate = canCreate,
            prefill = prefill,
            onPick = {
                onSelect(it)
                open = false
            },
            onDismiss = { open = false },
        )
    }
}

@Composable
private fun PartnerPickerDialog(
    canCreate: Boolean,
    prefill: PartnerPrefill?,
    onPick: (PartnerChoice) -> Unit,
    onDismiss: () -> Unit,
) {
    val api = app().api
    val context = androidx.compose.ui.platform.LocalContext.current
    val recents = remember { hu.autotherm.autocrm.data.prefs.RecentPartners.load(context) }
    // Every pick (found, recent or just created) moves to the top of the recent list.
    val pick: (PartnerChoice) -> Unit = { choice ->
        hu.autotherm.autocrm.data.prefs.RecentPartners.remember(
            context,
            hu.autotherm.autocrm.data.prefs.RecentPartners.Entry(choice.id, choice.name, choice.defaultCurrency, choice.country),
        )
        onPick(choice)
    }
    var query by remember { mutableStateOf("") }
    var results by remember { mutableStateOf<List<Partner>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var creating by remember { mutableStateOf(false) }

    LaunchedEffect(query) {
        loading = true
        delay(if (query.isBlank()) 0 else 250)
        try {
            results = api.partners(query.trim().ifBlank { null }, role = "customer", limit = 40)
            error = null
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            error = describeError(e)
        }
        loading = false
    }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(Panel).imePadding()) {
            Row(
                Modifier.fillMaxWidth().background(Surface).padding(horizontal = 4.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = { if (creating) creating = false else onDismiss() }) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Vissza")
                }
                Text(
                    if (creating) "Új partner" else "Partner kiválasztása",
                    style = MaterialTheme.typography.titleLarge,
                    color = Steel900,
                )
            }
            HorizontalDivider(color = Steel200)
            if (creating) {
                QuickPartnerForm(
                    prefill = prefill?.copy(name = prefill.name ?: query.takeIf { it.isNotBlank() }) ?: PartnerPrefill(name = query.takeIf { it.isNotBlank() }),
                    onCreated = { pick(PartnerChoice.of(it)) },
                )
            } else {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    SearchField(
                        value = query,
                        onValueChange = { query = it },
                        label = "Név, adószám, város…",
                        modifier = Modifier.fillMaxWidth(),
                    )
                    if (canCreate) {
                        SecondaryButton(
                            text = if (query.isBlank()) "Új partner létrehozása" else "Új partner: „${query.trim()}”",
                            onClick = { creating = true },
                            icon = Icons.Filled.Add,
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                    if (loading && results.isEmpty()) RefreshingBar()
                    error?.let { FormError(it) }
                }
                LazyColumn(
                    Modifier.fillMaxSize(),
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    val showRecents = query.isBlank() && recents.isNotEmpty()
                    if (showRecents) {
                        item { FieldLabel("Legutóbbiak") }
                        items(recents, key = { "recent-" + it.id }) { r ->
                            ListRow(
                                title = r.name,
                                leading = { InitialsAvatar(r.name) },
                                trailing = {
                                    Icon(Icons.Filled.History, contentDescription = null, tint = Steel500)
                                },
                                onClick = { pick(PartnerChoice(r.id, r.name, r.defaultCurrency, r.country)) },
                            )
                        }
                        if (results.isNotEmpty()) item { FieldLabel("Összes partner") }
                    }
                    if (!loading && results.isEmpty() && error == null) {
                        item { EmptyState("Nincs ilyen partner.", icon = Icons.Filled.Business) }
                    }
                    items(results, key = { it.id }) { p ->
                        ListRow(
                            title = p.name,
                            subtitle = listOfNotNull(
                                p.city,
                                p.taxNumber,
                                p.country.takeIf { it != "HU" },
                            ).joinToString(" · ").ifBlank { null },
                            leading = { InitialsAvatar(p.name) },
                            trailing = {
                                if (p.kind == "person") StatusBadge("Magánszemély") else StatusBadge("Cég", Tone.Cold)
                            },
                            onClick = { pick(PartnerChoice.of(p)) },
                        )
                    }
                }
            }
        }
    }
}

/** EU members (plus the EEA neighbours) offered first in the country choice. */
val COUNTRIES: List<Pair<String, String>> = listOf(
    "HU" to "Magyarország", "AT" to "Ausztria", "DE" to "Németország", "SK" to "Szlovákia",
    "RO" to "Románia", "HR" to "Horvátország", "SI" to "Szlovénia", "RS" to "Szerbia",
    "CZ" to "Csehország", "PL" to "Lengyelország", "IT" to "Olaszország", "NL" to "Hollandia",
    "BE" to "Belgium", "FR" to "Franciaország", "UA" to "Ukrajna", "CH" to "Svájc",
)

/**
 * The short form for a new customer: enough for an order and an invoice later, nothing that
 * can wait. A company needs a tax number for invoicing; a private person does not.
 */
@Composable
private fun QuickPartnerForm(prefill: PartnerPrefill, onCreated: (Partner) -> Unit) {
    val api = app().api
    val scope = rememberCoroutineScope()
    var kind by remember { mutableStateOf(if (prefill.name.isNullOrBlank()) "business" else "person") }
    var name by remember { mutableStateOf(prefill.name.orEmpty()) }
    var country by remember { mutableStateOf("HU") }
    var currency by remember { mutableStateOf("HUF") }
    var taxNumber by remember { mutableStateOf("") }
    var euTax by remember { mutableStateOf("") }
    var email by remember { mutableStateOf(prefill.email.orEmpty()) }
    var phone by remember { mutableStateOf(prefill.phone.orEmpty()) }
    var postal by remember { mutableStateOf("") }
    var city by remember { mutableStateOf("") }
    var address by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    fun save() {
        if (name.isBlank()) {
            error = "A név kötelező."
            return
        }
        busy = true
        error = null
        scope.launch {
            try {
                val created = api.createPartner(
                    PartnerBody(
                        kind = kind,
                        name = name.trim(),
                        country = country,
                        defaultCurrency = currency,
                        taxNumber = taxNumber.trim().ifBlank { null },
                        euTaxNumber = euTax.trim().ifBlank { null },
                        email = email.trim().ifBlank { null },
                        phone = phone.trim().ifBlank { null },
                        postalCode = postal.trim().ifBlank { null },
                        city = city.trim().ifBlank { null },
                        addressLine = address.trim().ifBlank { null },
                        role = "customer",
                    ),
                )
                Toasts.show("Partner létrehozva: " + created.name)
                onCreated(created)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = describeError(e)
            } finally {
                busy = false
            }
        }
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SegmentedChoice(
            options = listOf("business" to "Cég", "person" to "Magánszemély"),
            selected = kind,
            onSelect = { kind = it },
        )
        FormSection("Alapadatok") {
            AutoCrmTextField(
                value = name,
                onValueChange = { name = it },
                label = if (kind == "business") "Cégnév *" else "Név *",
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                modifier = Modifier.fillMaxWidth(),
            )
            SelectField(
                label = "Ország",
                options = COUNTRIES,
                selected = country,
                onSelect = { c ->
                    country = c ?: "HU"
                    // Abroad, invoices are usually in euros.
                    currency = if (country == "HU") "HUF" else "EUR"
                },
                modifier = Modifier.fillMaxWidth(),
            )
            Text("Számlázási pénznem", style = MaterialTheme.typography.labelMedium, color = Steel500)
            SegmentedChoice(
                options = listOf("HUF" to "Forint (HUF)", "EUR" to "Euró (EUR)"),
                selected = currency,
                onSelect = { currency = it },
            )
            if (kind == "business") {
                AutoCrmTextField(
                    value = taxNumber,
                    onValueChange = { v -> taxNumber = if (country == "HU") v.filter { it.isDigit() }.take(11) else v },
                    visualTransformation = if (country == "HU") hu.autotherm.autocrm.util.HuTaxNumberTransformation else androidx.compose.ui.text.input.VisualTransformation.None,
                    label = if (country == "HU") "Adószám (12345678-1-23)" else "Helyi adószám",
                    isError = country == "HU" && taxNumber.isNotEmpty() && taxNumber.length != 11,
                    supporting = if (country == "HU" && taxNumber.isNotEmpty() && taxNumber.length != 11) "${taxNumber.length}/11 számjegy" else null,
                    keyboardOptions = KeyboardOptions(keyboardType = if (country == "HU") KeyboardType.Number else KeyboardType.Text),
                    modifier = Modifier.fillMaxWidth(),
                )
                if (country != "HU") {
                    AutoCrmTextField(
                        value = euTax,
                        onValueChange = { euTax = it.uppercase() },
                        label = "Közösségi adószám (pl. DE123456789)",
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        FormSection("Elérhetőség") {
            AutoCrmTextField(
                value = email,
                onValueChange = { email = it },
                label = "E-mail",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = phone,
                onValueChange = { phone = it },
                label = "Telefon",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        FormSection("Cím", hint = "Számlához kell; később is megadható.") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                AutoCrmTextField(
                    value = postal,
                    onValueChange = { v -> postal = if (country == "HU") v.filter { it.isDigit() }.take(4) else v },
                    isError = country == "HU" && postal.isNotEmpty() && postal.length != 4,
                    label = "Irsz.",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.weight(0.35f),
                )
                AutoCrmTextField(
                    value = city,
                    onValueChange = { city = it },
                    label = "Város",
                    modifier = Modifier.weight(0.65f),
                )
            }
            AutoCrmTextField(
                value = address,
                onValueChange = { address = it },
                label = "Utca, házszám",
                modifier = Modifier.fillMaxWidth(),
            )
        }
        FormError(error)
        PrimaryButton(
            text = if (busy) "Mentés…" else "Partner létrehozása",
            onClick = ::save,
            enabled = !busy && name.isNotBlank(),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(24.dp))
    }
}

/** Small helper for forms: the label above a group of segmented options. */
@Composable
fun FieldLabel(text: String) {
    Text(text, style = MaterialTheme.typography.labelMedium, color = Steel500)
}

/** Currency as two big buttons; EUR orders are valued at the MNB rate of their day. */
@Composable
fun CurrencyChoice(selected: String, onSelect: (String) -> Unit, enabled: Boolean = true, locked: String? = null) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        FieldLabel("Pénznem")
        SegmentedChoice(
            options = listOf("HUF" to "Forint", "EUR" to "Euró"),
            selected = selected,
            onSelect = onSelect,
            enabled = enabled,
        )
        if (locked != null) {
            Text(locked, style = MaterialTheme.typography.bodySmall, color = Steel500)
        }
    }
}

/** A small leading icon tile for form rows and list rows. */
@Composable
fun IconTile(icon: androidx.compose.ui.graphics.vector.ImageVector, modifier: Modifier = Modifier) {
    Box(
        modifier.size(40.dp).background(Cold.copy(alpha = 0.12f), RoundedCornerShape(12.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = Cold, modifier = Modifier.size(22.dp))
    }
}

/** The label of an option list item, ellipsised. */
@Composable
internal fun OptionText(text: String) {
    Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis)
}
