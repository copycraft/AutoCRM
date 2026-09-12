package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Blocker
import hu.autotherm.autocrm.data.api.OrderDetail
import hu.autotherm.autocrm.data.api.OrderNote
import hu.autotherm.autocrm.data.api.StageEntry
import hu.autotherm.autocrm.data.api.TransitionOption
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.LoadingState
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.photos.OrderPhotoSection
import hu.autotherm.autocrm.ui.photos.OrderPhotoViewModel
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import hu.autotherm.autocrm.util.formatDateTime
import hu.autotherm.autocrm.util.formatMoney
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class OrderDetailViewModel(
    private val api: AutoCrmApi,
    private val sessionStore: SessionStore,
) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val detail: OrderDetail? = null,
        val stages: List<StageEntry> = emptyList(),
        val notes: List<OrderNote> = emptyList(),
        val canChangeStage: Boolean = false,
        val error: String? = null,
        /** Non-null while the stage dialog is open. */
        val stageDialog: List<TransitionOption>? = null,
        val stageError: String? = null,
        val busy: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(orderId: Long) {
        viewModelScope.launch {
            _state.value = _state.value.copy(loading = true, error = null)
            try {
                val detail = api.order(orderId)
                val stages = api.orderStages(orderId)
                // Imported MiniCRM history. For a migrated order this is usually the only
                // record of what happened, since MiniCRM exposes no stage history.
                val notes = runCatching { api.orderNotes(orderId) }.getOrDefault(emptyList())
                val account = sessionStore.currentAccount()
                _state.value = State(
                    loading = false,
                    detail = detail,
                    stages = stages,
                    notes = notes,
                    canChangeStage = account?.canChangeStage == true,
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    fun openStageDialog(orderId: Long) {
        viewModelScope.launch {
            try {
                // The server decides what is allowed, including the MEO photo gate. The app
                // never computes a transition itself: the gate lives in domain/stage.rs and
                // a second implementation here would be a second thing to get wrong.
                val options = api.orderTransitions(orderId)
                _state.value = _state.value.copy(stageDialog = options, stageError = null)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(stageError = describeError(e))
            }
        }
    }

    fun closeStageDialog() {
        _state.value = _state.value.copy(stageDialog = null, stageError = null)
    }

    fun changeStage(orderId: Long, stage: String, note: String?) {
        viewModelScope.launch {
            _state.value = _state.value.copy(busy = true, stageError = null)
            try {
                api.changeOrderStage(orderId, stage, note?.takeIf { it.isNotBlank() })
                _state.value = _state.value.copy(busy = false, stageDialog = null)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(
                    busy = false,
                    stageError = when {
                        e is ApiException.Rule && e.code == "stage_gate" ->
                            // The single most useful error in the system: it means the MEO
                            // photo is missing, which this app can fix on the spot.
                            e.detail ?: "Hiányzik a kötelező fotó ehhez a fázishoz."
                        else -> describeError(e)
                    },
                )
            }
        }
    }
}

@Composable
fun OrderDetailScreen(
    orderId: Long,
    viewModel: OrderDetailViewModel,
    photoViewModel: OrderPhotoViewModel,
    onOpenOrder: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(orderId) { viewModel.load(orderId) }

    when {
        state.loading -> LoadingState()
        state.error != null -> ErrorState(state.error!!) { viewModel.load(orderId) }
        state.detail == null -> ErrorState("Nem található.") { viewModel.load(orderId) }
        else -> {
            val detail = state.detail!!
            LazyColumn(
                Modifier.fillMaxSize().padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    Column {
                        Text(
                            "#${detail.order.number}",
                            style = MonoSmall,
                            color = Steel500,
                        )
                        Text(detail.order.title, style = MaterialTheme.typography.headlineMedium)
                        Text(
                            "${detail.partner.name} · ${detail.stage.labelHu} · ${detail.stage.daysInStage} napja",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                }

                if (state.canChangeStage) {
                    item {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedButton(onClick = { viewModel.openStageDialog(orderId) }) {
                                Text("Fázisváltás")
                            }
                        }
                    }
                }

                item {
                    Card {
                        SectionTitle("Érték")
                        Text(
                            formatMoney(detail.value.totalMinor, detail.value.currency),
                            style = MaterialTheme.typography.headlineMedium,
                        )
                        // V3: an order with no line items reports as zero and silently
                        // undercounts every report it appears in. Say so where the work is.
                        if (detail.value.totalMinor == 0L) {
                            StatusBadge("Nincs rögzített érték", Tone.Signal)
                        }
                        if (detail.value.totalHufMinor == null && detail.value.currency != "HUF") {
                            StatusBadge("Hiányzó árfolyam", Tone.Signal)
                        }
                    }
                }

                item {
                    Card {
                        SectionTitle("Jármű")
                        if (detail.vehicles.isEmpty()) {
                            Info("Rendszám", detail.order.vehiclePlate, mono = true)
                            Info(
                                "Típus",
                                listOfNotNull(detail.order.vehicleMake, detail.order.vehicleModel)
                                    .joinToString(" ").ifBlank { null },
                            )
                        } else {
                            detail.vehicles.forEach { vehicle ->
                                Info(
                                    vehicle.plate ?: vehicle.vin ?: "#${vehicle.id}",
                                    listOfNotNull(vehicle.make, vehicle.model, vehicle.year?.toString())
                                        .joinToString(" ").ifBlank { null },
                                    mono = true,
                                )
                            }
                        }
                        Info("VIN", detail.order.vehicleVin, mono = true)
                        Info("Határidő", formatDate(detail.order.dueDate))
                    }
                }

                detail.spec?.let { spec ->
                    item {
                        Card {
                            SectionTitle(
                                if (spec.form == "cooling") "Hűtési specifikáció" else "Fűtési specifikáció",
                            )
                            Info("Kívánt hőmérséklet", spec.targetTempC?.let { "$it °C" }, mono = true)
                            Info("Szigetelés", spec.insulationMm?.let { "$it mm" }, mono = true)
                            if (spec.form == "cooling") {
                                Info("Hűtőgép", listOfNotNull(spec.coolingUnitMake, spec.coolingUnitModel).joinToString(" ").ifBlank { null })
                                Info("ATP osztály", spec.atpClass, mono = true)
                            } else {
                                Info("Fűtőkészülék", listOfNotNull(spec.heaterMake, spec.heaterModel).joinToString(" ").ifBlank { null })
                                Info("Teljesítmény", spec.heatOutputKw?.let { "$it kW" }, mono = true)
                            }
                        }
                    }
                }

                item {
                    OrderPhotoSection(
                        orderId = orderId,
                        orderNumber = detail.order.number,
                        imageCounts = detail.imageCounts,
                        viewModel = photoViewModel,
                    )
                }

                if (detail.blockers.any { it.resolvedAt == null }) {
                    item { SectionTitle("Akadályok") }
                    items(detail.blockers.filter { it.resolvedAt == null }, key = { it.id }) { blocker ->
                        BlockerCard(blocker)
                    }
                }

                if (state.notes.isNotEmpty()) {
                    item { SectionTitle("MiniCRM előzmények") }
                    items(state.notes, key = { it.id }) { note ->
                        Card {
                            Text(
                                "${note.authorName ?: "—"} · ${formatDateTime(note.occurredAt)}",
                                style = MaterialTheme.typography.labelMedium,
                                color = Steel500,
                            )
                            Text(note.body, style = MaterialTheme.typography.bodyLarge)
                        }
                    }
                }

                if (state.stages.isNotEmpty()) {
                    item { SectionTitle("Fázisok") }
                    items(state.stages, key = { it.id }) { entry ->
                        Card {
                            Text(entry.labelHu, style = MaterialTheme.typography.titleMedium)
                            Text(
                                listOfNotNull(formatDateTime(entry.enteredAt), entry.enteredByName)
                                    .joinToString(" · "),
                                style = MaterialTheme.typography.labelMedium,
                                color = Steel500,
                            )
                            entry.note?.let { Text(it, style = MaterialTheme.typography.bodyLarge) }
                        }
                    }
                }
            }
        }
    }

    state.stageDialog?.let { options ->
        StageDialog(
            options = options,
            error = state.stageError,
            busy = state.busy,
            onDismiss = viewModel::closeStageDialog,
            onConfirm = { stage, note -> viewModel.changeStage(orderId, stage, note) },
        )
    }
}

@Composable
private fun BlockerCard(blocker: Blocker) {
    Card {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(blocker.what, style = MaterialTheme.typography.titleMedium)
            if (blocker.isOverdue) StatusBadge("Lejárt", Tone.Signal)
        }
        Text(
            listOfNotNull(
                blocker.responsiblePartnerName,
                formatDate(blocker.dueDate)?.let { "határidő: $it" },
                blocker.nudgeCount.takeIf { it > 0 }?.let { "$it emlékeztető" },
            ).joinToString(" · ").ifBlank { "—" },
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
        )
        blocker.notes?.let { Text(it, style = MaterialTheme.typography.bodyLarge) }
    }
}

/**
 * The transition list comes from the server with `allowed` and `reason` already decided.
 * A disallowed option is shown greyed with its reason rather than hidden: "why can't I move
 * this to Kész" is the question, and "because there is no completion photo" is the answer.
 */
@Composable
private fun StageDialog(
    options: List<TransitionOption>,
    error: String?,
    busy: Boolean,
    onDismiss: () -> Unit,
    onConfirm: (String, String?) -> Unit,
) {
    var selected by remember { mutableStateOf<TransitionOption?>(null) }
    var note by remember { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Fázisváltás") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                options.forEach { option ->
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .clickable(enabled = option.allowed) { selected = option }
                            .padding(vertical = 6.dp),
                    ) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(
                                option.labelHu,
                                style = MaterialTheme.typography.bodyLarge,
                                color = if (option.allowed) MaterialTheme.colorScheme.onSurface else Steel500,
                            )
                            if (selected?.stageKey == option.stageKey) StatusBadge("Kiválasztva", Tone.Cold)
                        }
                        val reason = option.reason
                        if (!option.allowed && reason != null) {
                            Text(reason, style = MaterialTheme.typography.labelMedium, color = Signal)
                        }
                    }
                }
                if (selected?.requiresNote == true) {
                    OutlinedTextField(
                        value = note,
                        onValueChange = { note = it },
                        label = { Text("Indoklás (kötelező)") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
            }
        },
        confirmButton = {
            val choice = selected
            TextButton(
                enabled = choice != null && !busy && (choice.requiresNote.not() || note.isNotBlank()),
                onClick = { choice?.let { onConfirm(it.stageKey, note) } },
            ) { Text(if (busy) "Mentés…" else "Váltás") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Mégse") } },
    )
}
