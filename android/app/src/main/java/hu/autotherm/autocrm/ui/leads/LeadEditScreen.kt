package hu.autotherm.autocrm.ui.leads

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.LeadBody
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Signal
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class LeadEditViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val title: String = "",
        val contactName: String = "",
        val contactEmail: String = "",
        val contactPhone: String = "",
        val source: String = "",
        val sourceDetail: String = "",
        /** The list the source picker offers (0048). */
        val sources: List<hu.autotherm.autocrm.data.api.LeadSource> = emptyList(),
        val description: String = "",
        val quotedValue: String = "",
        val quoteValidUntil: String = "",
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            try {
                val l = api.lead(id).lead
                val sources = runCatching { api.leadSources() }.getOrDefault(emptyList())
                _state.value = State(
                    sources = sources,
                    sourceDetail = l.sourceDetail.orEmpty(),
                    title = l.title,
                    contactName = l.contactName.orEmpty(),
                    contactEmail = l.contactEmail.orEmpty(),
                    contactPhone = l.contactPhone.orEmpty(),
                    source = l.source.orEmpty(),
                    description = l.description.orEmpty(),
                    quotedValue = l.quotedValueMinor?.let { (it / 100).toString() }.orEmpty(),
                    quoteValidUntil = l.quoteValidUntil.orEmpty(),
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }

    fun loadSources() {
        viewModelScope.launch {
            runCatching { api.leadSources() }.onSuccess { list ->
                _state.value = _state.value.copy(sources = list)
            }
        }
    }

    fun set(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    fun save(leadId: Long?, partnerId: Long?, onSaved: (Long) -> Unit) {
        val s = _state.value
        if (s.busy || s.title.isBlank()) return
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
            try {
                val body = LeadBody(
                    title = s.title.trim(),
                    partnerId = partnerId,
                    contactName = blankToNull(s.contactName),
                    contactEmail = blankToNull(s.contactEmail),
                    contactPhone = blankToNull(s.contactPhone),
                    source = blankToNull(s.source),
                    sourceDetail = blankToNull(s.sourceDetail),
                    description = blankToNull(s.description),
                    quotedValueMinor = s.quotedValue.trim().toLongOrNull()?.let { it * 100 },
                    currency = "HUF",
                    quoteValidUntil = blankToNull(s.quoteValidUntil),
                )
                val id = if (leadId == null) api.createLead(body).id
                else api.patchLead(leadId, body).id
                _state.value = _state.value.copy(busy = false)
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
    LaunchedEffect(leadId) {
        if (leadId != null) viewModel.load(leadId) else viewModel.loadSources()
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = if (leadId == null) "Új lead" else "Lead szerkesztése",
                onBack = onBack,
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            AutoCrmTextField(
                value = state.title,
                onValueChange = { v -> viewModel.set { it.copy(title = v) } },
                label = "Cím *",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.contactName,
                onValueChange = { v -> viewModel.set { it.copy(contactName = v) } },
                label = "Kapcsolattartó neve",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.contactEmail,
                onValueChange = { v -> viewModel.set { it.copy(contactEmail = v) } },
                label = "Kapcsolattartó e-mail",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.contactPhone,
                onValueChange = { v -> viewModel.set { it.copy(contactPhone = v) } },
                label = "Kapcsolattartó telefon",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone),
                modifier = Modifier.fillMaxWidth(),
            )
            Text("Forrás", style = MaterialTheme.typography.labelMedium)
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
                label = "Forrás részletei (melyik vásár, ki ajánlotta…)",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.quotedValue,
                onValueChange = { v -> viewModel.set { it.copy(quotedValue = v.filter { c -> c.isDigit() }) } },
                label = "Ajánlati összeg (Ft)",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.fillMaxWidth(),
            )
            DateField(
                value = state.quoteValidUntil,
                onValueChange = { v -> viewModel.set { it.copy(quoteValidUntil = v) } },
                label = "Ajánlat érvényes",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.description,
                onValueChange = { v -> viewModel.set { it.copy(description = v) } },
                label = "Leírás",
                singleLine = false,
                modifier = Modifier.fillMaxWidth(),
            )
            state.error?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal)
            }
            PrimaryButton(
                text = if (state.busy) "Mentés…" else "Mentés",
                onClick = { viewModel.save(leadId, partnerId, onSaved) },
                enabled = !state.busy && state.title.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
