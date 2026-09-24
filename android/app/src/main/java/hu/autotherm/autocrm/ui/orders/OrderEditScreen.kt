package hu.autotherm.autocrm.ui.orders

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
import hu.autotherm.autocrm.data.api.OrderBody
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Signal
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
    put("title", s.title.trim())
    text("vehicle_make", s.vehicleMake)
    text("vehicle_model", s.vehicleModel)
    text("vehicle_plate", s.vehiclePlate)
    text("vehicle_vin", s.vehicleVin)
    text("description", s.description)
    text("due_date", s.dueDate)
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
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            try {
                val o = api.order(id).order
                _state.value = State(
                    title = o.title,
                    vehicleMake = o.vehicleMake.orEmpty(),
                    vehicleModel = o.vehicleModel.orEmpty(),
                    vehiclePlate = o.vehiclePlate.orEmpty(),
                    vehicleVin = o.vehicleVin.orEmpty(),
                    description = o.description.orEmpty(),
                    dueDate = o.dueDate.orEmpty(),
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }

    fun set(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    fun save(orderId: Long?, partnerId: Long?, onSaved: (Long) -> Unit) {
        val s = _state.value
        if (s.busy || s.title.isBlank()) return
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
            try {
                val id = if (orderId == null) {
                    api.createOrder(
                        OrderBody(
                            title = s.title.trim(),
                            partnerId = partnerId,
                            currency = "HUF",
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
    LaunchedEffect(orderId) {
        if (orderId != null) viewModel.load(orderId)
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = if (orderId == null) "Új megrendelés" else "Megrendelés szerkesztése",
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
                value = state.vehiclePlate,
                onValueChange = { v -> viewModel.set { it.copy(vehiclePlate = v) } },
                label = "Rendszám",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.vehicleMake,
                onValueChange = { v -> viewModel.set { it.copy(vehicleMake = v) } },
                label = "Márka",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.vehicleModel,
                onValueChange = { v -> viewModel.set { it.copy(vehicleModel = v) } },
                label = "Típus",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.vehicleVin,
                onValueChange = { v -> viewModel.set { it.copy(vehicleVin = v) } },
                label = "Alvázszám (VIN)",
                modifier = Modifier.fillMaxWidth(),
            )
            DateField(
                value = state.dueDate,
                onValueChange = { v -> viewModel.set { it.copy(dueDate = v) } },
                label = "Határidő",
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
                onClick = { viewModel.save(orderId, partnerId, onSaved) },
                enabled = !state.busy && state.title.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
