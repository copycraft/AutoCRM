package hu.autotherm.autocrm.ui.partners

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.PartnerBody
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Signal
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class PartnerEditViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = false,
        val name: String = "",
        val kind: String = "business",
        val role: String = "customer",
        val taxNumber: String = "",
        val email: String = "",
        val phone: String = "",
        val city: String = "",
        val address: String = "",
        val notes: String = "",
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            _state.value = _state.value.copy(loading = true, error = null)
            try {
                val p = api.partner(id).partner
                _state.value = State(
                    name = p.name,
                    kind = p.kind,
                    role = p.role ?: "customer",
                    taxNumber = p.taxNumber.orEmpty(),
                    email = p.email.orEmpty(),
                    phone = p.phone.orEmpty(),
                    city = p.city.orEmpty(),
                    address = p.addressLine.orEmpty(),
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    fun set(field: (State) -> State) {
        _state.value = field(_state.value)
    }

    fun save(partnerId: Long?, onSaved: (Long) -> Unit) {
        val s = _state.value
        if (s.busy || s.name.isBlank()) return
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
            try {
                val body = PartnerBody(
                    kind = s.kind,
                    name = s.name.trim(),
                    taxNumber = blankToNull(s.taxNumber),
                    email = blankToNull(s.email),
                    phone = blankToNull(s.phone),
                    city = blankToNull(s.city),
                    addressLine = blankToNull(s.address),
                    notes = blankToNull(s.notes),
                    role = s.role,
                )
                val id = if (partnerId == null) api.createPartner(body).id
                else api.patchPartner(partnerId, body).id
                _state.value = _state.value.copy(busy = false)
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
    LaunchedEffect(partnerId) {
        if (partnerId != null) viewModel.load(partnerId)
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = if (partnerId == null) "Új partner" else "Partner szerkesztése",
                onBack = onBack,
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text("Típus", style = MaterialTheme.typography.labelMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = state.kind == "business",
                    onClick = { viewModel.set { it.copy(kind = "business") } },
                    label = { Text("Vállalkozás") },
                )
                FilterChip(
                    selected = state.kind == "person",
                    onClick = { viewModel.set { it.copy(kind = "person") } },
                    label = { Text("Magánszemély") },
                )
            }
            AutoCrmTextField(
                value = state.name,
                onValueChange = { v -> viewModel.set { it.copy(name = v) } },
                label = "Név *",
                modifier = Modifier.fillMaxWidth(),
            )
            Text("Szerep", style = MaterialTheme.typography.labelMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = state.role == "customer",
                    onClick = { viewModel.set { it.copy(role = "customer") } },
                    label = { Text("Ügyfél") },
                )
                FilterChip(
                    selected = state.role == "supplier",
                    onClick = { viewModel.set { it.copy(role = "supplier") } },
                    label = { Text("Beszállító") },
                )
                FilterChip(
                    selected = state.role == "both",
                    onClick = { viewModel.set { it.copy(role = "both") } },
                    label = { Text("Mindkettő") },
                )
            }
            AutoCrmTextField(
                value = state.taxNumber,
                onValueChange = { v -> viewModel.set { it.copy(taxNumber = v) } },
                label = "Adószám",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.email,
                onValueChange = { v -> viewModel.set { it.copy(email = v) } },
                label = "E-mail",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.phone,
                onValueChange = { v -> viewModel.set { it.copy(phone = v) } },
                label = "Telefon",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.city,
                onValueChange = { v -> viewModel.set { it.copy(city = v) } },
                label = "Város",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.address,
                onValueChange = { v -> viewModel.set { it.copy(address = v) } },
                label = "Cím",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.notes,
                onValueChange = { v -> viewModel.set { it.copy(notes = v) } },
                label = "Megjegyzés",
                singleLine = false,
                modifier = Modifier.fillMaxWidth(),
            )
            state.error?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal)
            }
            PrimaryButton(
                text = if (state.busy) "Mentés…" else "Mentés",
                onClick = { viewModel.save(partnerId, onSaved) },
                enabled = !state.busy && state.name.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
