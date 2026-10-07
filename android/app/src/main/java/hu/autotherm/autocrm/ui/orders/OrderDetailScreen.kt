package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AddItemBody
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Blocker
import hu.autotherm.autocrm.data.api.BlockerBody
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.api.OrderDetail
import hu.autotherm.autocrm.data.api.OrderNote
import hu.autotherm.autocrm.data.api.ResolveBody
import hu.autotherm.autocrm.data.api.StageEntry
import hu.autotherm.autocrm.data.api.Task
import hu.autotherm.autocrm.data.api.TaskBody
import hu.autotherm.autocrm.data.api.TransitionOption
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.StageDialog
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.tasks.TaskDialog
import hu.autotherm.autocrm.ui.tasks.TaskRow
import hu.autotherm.autocrm.ui.photos.OrderPhotoSection
import hu.autotherm.autocrm.ui.photos.OrderPhotoViewModel
import hu.autotherm.autocrm.data.inspection.cachedLookups
import hu.autotherm.autocrm.data.inspection.downloadLookups
import hu.autotherm.autocrm.data.prefs.LookupsCache
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import hu.autotherm.autocrm.util.formatDateTime
import hu.autotherm.autocrm.util.formatMoney
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject

class OrderDetailViewModel(
    private val api: AutoCrmApi,
    private val sessionStore: SessionStore,
    private val lookupsCache: LookupsCache,
) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        /** Pull-to-refresh with the detail kept — progress line, not a full spinner. */
        val refreshing: Boolean = false,
        val detail: OrderDetail? = null,
        val stages: List<StageEntry> = emptyList(),
        val notes: List<OrderNote> = emptyList(),
        val tasks: List<Task> = emptyList(),
        val canEdit: Boolean = false,
        val canChangeStage: Boolean = false,
        val error: String? = null,
        /** Non-null while the stage dialog is open. */
        val stageDialog: List<TransitionOption>? = null,
        val stageError: String? = null,
        val busy: Boolean = false,
        /** Write-dialog flags for the sections below. */
        val itemDialog: Boolean = false,
        val itemBusy: Boolean = false,
        val itemError: String? = null,
        val blockerDialog: Boolean = false,
        val blockerBusy: Boolean = false,
        val blockerError: String? = null,
        /** Blocker awaiting a resolve note; null while the resolve dialog is closed. */
        val resolveTarget: Blocker? = null,
        val taskDialog: Boolean = false,
        val taskBusy: Boolean = false,
        val taskError: String? = null,
        /** True while the intake-slip dialog is open (ORD-L6). */
        val intakeDialog: Boolean = false,
        val intakeBusy: Boolean = false,
        val intakeError: String? = null,
        /** The server's enumerations (fuel marks, ...); cached for offline opens. */
        val lookups: Lookups? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(orderId: Long) {
        viewModelScope.launch {
            val keepRows = _state.value.detail != null
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                // The reads are independent — fire them together. Previously they
                // ran sequentially and the detail paid full latency × 3 on every open.
                val detailDeferred = async { api.order(orderId) }
                val stagesDeferred = async { api.orderStages(orderId) }
                val notesDeferred = async {
                    // Imported MiniCRM history. For a migrated order this is usually the
                    // only record of what happened, since MiniCRM exposes no stage history.
                    runCatching { api.orderNotes(orderId) }.getOrDefault(emptyList())
                }
                val tasksDeferred = async {
                    runCatching { api.tasksFor("order", orderId) }.getOrDefault(emptyList())
                }
                val accountDeferred = async { sessionStore.currentAccount() }
                val lookupsDeferred = async {
                    downloadLookups(api, lookupsCache) ?: cachedLookups(lookupsCache)
                }
                val detail = detailDeferred.await()
                val account = accountDeferred.await()
                _state.value = State(
                    loading = false,
                    detail = detail,
                    stages = stagesDeferred.await(),
                    notes = notesDeferred.await(),
                    tasks = tasksDeferred.await(),
                    canEdit = account?.canEdit == true,
                    canChangeStage = account?.canChangeStage == true,
                    lookups = lookupsDeferred.await(),
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

    fun openStageDialog(orderId: Long) {
        viewModelScope.launch {
            try {
                // The server decides what is allowed, including the MEO photo gate. The app
                // never computes a transition itself: the gate lives in domain/stage.rs and
                // a second implementation here would be a second thing to get wrong.
                val options = api.orderTransitions(orderId)
                _state.value = _state.value.copy(stageDialog = options, stageError = null)
            } catch (e: Throwable) {
                // Open the dialog anyway with an empty option list: stageError only renders
                // inside it, and a tap that visibly does nothing reads as a dead button.
                _state.value = _state.value.copy(stageDialog = emptyList(), stageError = describeError(e))
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

    // ── Line items ──

    fun openItemDialog() {
        _state.value = _state.value.copy(itemDialog = true, itemError = null)
    }

    fun closeItemDialog() {
        _state.value = _state.value.copy(itemDialog = false, itemError = null)
    }

    fun addItem(orderId: Long, description: String, quantity: String, unitPriceMinor: Long) {
        viewModelScope.launch {
            _state.value = _state.value.copy(itemBusy = true, itemError = null)
            try {
                api.addItem(orderId, AddItemBody(description, quantity, unitPriceMinor))
                _state.value = _state.value.copy(itemBusy = false, itemDialog = false)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(itemBusy = false, itemError = describeError(e))
            }
        }
    }

    fun deleteItem(orderId: Long, itemId: Long) {
        viewModelScope.launch {
            try {
                api.deleteItem(itemId)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }

    // ── Blockers ──

    fun openBlockerDialog() {
        _state.value = _state.value.copy(blockerDialog = true, blockerError = null)
    }

    fun closeBlockerDialog() {
        _state.value = _state.value.copy(blockerDialog = false, blockerError = null)
    }

    fun createBlocker(orderId: Long, body: BlockerBody) {
        viewModelScope.launch {
            _state.value = _state.value.copy(blockerBusy = true, blockerError = null)
            try {
                api.createBlocker(orderId, body)
                _state.value = _state.value.copy(blockerBusy = false, blockerDialog = false)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(blockerBusy = false, blockerError = describeError(e))
            }
        }
    }

    fun openResolve(blocker: Blocker) {
        _state.value = _state.value.copy(resolveTarget = blocker, blockerError = null)
    }

    fun closeResolve() {
        _state.value = _state.value.copy(resolveTarget = null, blockerError = null)
    }

    fun resolveBlocker(orderId: Long, note: String?) {
        val target = _state.value.resolveTarget ?: return
        viewModelScope.launch {
            _state.value = _state.value.copy(blockerBusy = true, blockerError = null)
            try {
                api.resolveBlocker(target.id, note?.takeIf { it.isNotBlank() })
                _state.value = _state.value.copy(blockerBusy = false, resolveTarget = null)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(blockerBusy = false, blockerError = describeError(e))
            }
        }
    }

    fun reopenBlocker(orderId: Long, blockerId: Long) {
        viewModelScope.launch {
            try {
                api.reopenBlocker(blockerId)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }

    // ── Tasks ──

    fun openTaskDialog() {
        _state.value = _state.value.copy(taskDialog = true, taskError = null)
    }

    fun closeTaskDialog() {
        _state.value = _state.value.copy(taskDialog = false, taskError = null)
    }

    fun createTask(orderId: Long, title: String, dueDate: String?) {
        viewModelScope.launch {
            _state.value = _state.value.copy(taskBusy = true, taskError = null)
            try {
                api.createTask(TaskBody("order", orderId, title, dueDate?.takeIf { it.isNotBlank() }))
                _state.value = _state.value.copy(taskBusy = false, taskDialog = false)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(taskBusy = false, taskError = describeError(e))
            }
        }
    }

    fun toggleTask(orderId: Long, task: Task) {
        viewModelScope.launch {
            try {
                api.setTaskDone(task.id, !task.isDone)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }

    // ── Intake slip (ORD-L6) ──

    fun openIntakeDialog() {
        _state.value = _state.value.copy(intakeDialog = true, intakeError = null)
    }

    fun closeIntakeDialog() {
        _state.value = _state.value.copy(intakeDialog = false, intakeError = null)
    }

    fun saveIntakeSlip(orderId: Long, body: JsonObject) {
        viewModelScope.launch {
            _state.value = _state.value.copy(intakeBusy = true, intakeError = null)
            try {
                api.patchOrder(orderId, body)
                _state.value = _state.value.copy(intakeBusy = false, intakeDialog = false)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(intakeBusy = false, intakeError = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OrderDetailScreen(
    orderId: Long,
    viewModel: OrderDetailViewModel,
    photoViewModel: OrderPhotoViewModel,
    onOpenOrder: (Long) -> Unit,
    onBack: () -> Unit,
    onEditOrder: (Long) -> Unit,
    onComposeEmail: (Long) -> Unit,
    onInspections: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(orderId) { viewModel.load(orderId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.detail?.let { "#${it.order.number}" } ?: "Munka",
                subtitle = state.detail?.let { "${it.partner.name} · ${it.stage.labelHu}" },
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(orderId) },
                actions = {
                    if (state.canEdit && state.detail != null) {
                        TextButton(onClick = { onComposeEmail(orderId) }) {
                            Text("Levél")
                        }
                        TextButton(onClick = { onEditOrder(orderId) }) {
                            Text("Szerkesztés")
                        }
                    }
                    if (state.canChangeStage && state.detail != null) {
                        TextButton(onClick = { viewModel.openStageDialog(orderId) }) {
                            Text("Fázisváltás")
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
                ErrorState(state.error!!, Modifier.padding(padding)) { viewModel.load(orderId) }
            state.detail == null ->
                ErrorState("Nem található.", Modifier.padding(padding)) { viewModel.load(orderId) }
            else -> {
                val detail = state.detail!!
                PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = { viewModel.load(orderId) },
                    modifier = Modifier.padding(padding),
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item {
                            Card {
                                Text(
                                    detail.order.vehiclePlate ?: "#${detail.order.number}",
                                    style = MaterialTheme.typography.displaySmall,
                                )
                                Text(
                                    detail.order.title,
                                    style = MaterialTheme.typography.titleLarge,
                                )
                                Text(
                                    "${detail.partner.name} · ${detail.stage.labelHu} · ${detail.stage.daysInStage} napja",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    StatusBadge(
                                        detail.stage.labelHu,
                                        if (detail.stage.isTerminal) Tone.Done else Tone.Steel,
                                    )
                                    val open = detail.blockers.count { it.resolvedAt == null }
                                    if (open > 0) StatusBadge("$open akadály", Tone.Signal)
                                }
                            }
                        }

                        if (state.canChangeStage) {
                            item {
                                Button(
                                    onClick = { viewModel.openStageDialog(orderId) },
                                    modifier = Modifier.fillMaxWidth(),
                                ) { Text("Fázisváltás") }
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
                                        CoolingSerialRow(
                                            orderId = orderId,
                                            current = spec.coolingUnitSerial,
                                            canEdit = state.canChangeStage,
                                            onSaved = { viewModel.load(orderId) },
                                        )
                                    } else {
                                        Info("Fűtőkészülék", listOfNotNull(spec.heaterMake, spec.heaterModel).joinToString(" ").ifBlank { null })
                                        Info("Teljesítmény", spec.heatOutputKw?.let { "$it kW" }, mono = true)
                                    }
                                }
                            }
                        }

                        // The intake slip unblocks leaving `intake` (ORD-L6): without it the
                        // stage dialog answers `intake_slip_missing` and the phone cannot fix it.
                        item {
                            val order = detail.order
                            val filled = order.mileageIn != null ||
                                order.intakeCondition != null ||
                                order.fuelLevel != null ||
                                order.keyCount != null ||
                                order.valuablesDeclared != null
                            Card {
                                SectionTitle(
                                    "Átvételi lap",
                                    actionLabel = if (state.canEdit) {
                                        if (filled) "Szerkesztés" else "Rögzítés"
                                    } else {
                                        null
                                    },
                                    onAction = if (state.canEdit) {
                                        { viewModel.openIntakeDialog() }
                                    } else {
                                        null
                                    },
                                )
                                if (filled) {
                                    Info(
                                        "Km-óra",
                                        order.mileageIn?.let { "$it km" },
                                        mono = true,
                                    )
                                    Info("Üzemanyag", order.fuelLevel, mono = true)
                                    Info(
                                        "Kulcsok",
                                        order.keyCount?.toString(),
                                        mono = true,
                                    )
                                    Info("Állapot", order.intakeCondition)
                                    Info(
                                        "Értéktárgy",
                                        when (order.valuablesDeclared) {
                                            null -> null
                                            true -> order.valuables?.takeIf { it.isNotBlank() }
                                                ?: "van, listázatlan"
                                            false -> "nincs"
                                        },
                                    )
                                } else {
                                    val hint = if (state.canEdit) {
                                        "Még nincs rögzítve. Enélkül az átvétel nem zárható le."
                                    } else {
                                        "Még nincs rögzítve."
                                    }
                                    Text(
                                        hint,
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Steel500,
                                    )
                                }
                            }
                        }

                        item {
                            OrderPhotoSection(
                                orderId = orderId,
                                orderNumber = detail.order.number,
                                imageCounts = detail.imageCounts,
                                viewModel = photoViewModel,
                                stageCategory = detail.photoCategory,
                            )
                        }

                        item {
                            hu.autotherm.autocrm.ui.yard.VehicleLocationCard(
                                orderId = orderId,
                                vehicles = detail.vehicles,
                                current = detail.vehicleLocations,
                                canMove = state.canChangeStage,
                                onMoved = { viewModel.load(orderId) },
                            )
                        }

                        item {
                            hu.autotherm.autocrm.ui.common.CommentsSection(
                                entity = "order",
                                id = orderId,
                                canComment = state.canChangeStage,
                            )
                        }

                        if (state.canChangeStage) {
                            item {
                                Card {
                                    hu.autotherm.autocrm.ui.common.VoiceNoteRecorder(orderId = orderId)
                                }
                            }
                        }

                        item {
                            Card(onClick = { onInspections(orderId) }) {
                                SectionTitle("Átvétel-átadás")
                                Text(
                                    "Átvétel és kiadás sérülésvizsgálattal, " +
                                        "aláírással, csak telefonról.",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                            }
                        }

                        if (detail.items.isNotEmpty() || state.canEdit) {
                            item {
                                SectionTitle(
                                    "Tételek",
                                    count = detail.items.size.takeIf { it > 0 },
                                    actionLabel = if (state.canEdit) "Hozzáadás" else null,
                                    onAction = if (state.canEdit) viewModel::openItemDialog else null,
                                )
                            }
                            items(detail.items, key = { it.id }) { item ->
                                Card {
                                    Row(
                                        Modifier.fillMaxWidth(),
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Column(Modifier.weight(1f, fill = false)) {
                                            Text(item.description, style = MaterialTheme.typography.bodyLarge)
                                            Text(
                                                "${item.quantity} × ${formatMoney(item.unitPrice, item.currency)}",
                                                style = MaterialTheme.typography.labelMedium,
                                                color = Steel500,
                                            )
                                        }
                                        Text(
                                            formatMoney(item.lineTotalMinor, item.currency),
                                            style = MonoSmall,
                                        )
                                        if (state.canEdit) {
                                            IconButton(onClick = { viewModel.deleteItem(orderId, item.id) }) {
                                                Icon(
                                                    Icons.Filled.Delete,
                                                    contentDescription = "Törlés",
                                                    tint = Steel500,
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if (detail.blockers.isNotEmpty() || state.canChangeStage) {
                            item {
                                SectionTitle(
                                    "Akadályok",
                                    count = detail.blockers.count { it.resolvedAt == null }.takeIf { it > 0 },
                                    actionLabel = if (state.canChangeStage) "Új" else null,
                                    onAction = if (state.canChangeStage) viewModel::openBlockerDialog else null,
                                )
                            }
                            items(detail.blockers, key = { it.id }) { blocker ->
                                val open = blocker.resolvedAt == null
                                Card {
                                    Row(
                                        Modifier.fillMaxWidth(),
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Text(
                                            blocker.what,
                                            style = MaterialTheme.typography.titleMedium,
                                            modifier = Modifier.weight(1f, fill = false),
                                        )
                                        if (open && blocker.isOverdue) StatusBadge("Lejárt", Tone.Signal)
                                        if (!open) StatusBadge("Megoldva", Tone.Done)
                                    }
                                    Text(
                                        listOfNotNull(
                                            blocker.responsiblePartnerName,
                                            blocker.responsibleEmail,
                                            formatDate(blocker.dueDate)?.let { "határidő: $it" },
                                            blocker.nudgeCount.takeIf { it > 0 }?.let { "$it emlékeztető" },
                                        ).joinToString(" · ").ifBlank { "—" },
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Steel500,
                                    )
                                    blocker.notes?.let { Text(it, style = MaterialTheme.typography.bodyLarge) }
                                    blocker.resolutionNote?.let {
                                        Text(it, style = MaterialTheme.typography.labelMedium, color = Steel500)
                                    }
                                    if (state.canChangeStage) {
                                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                            if (open) {
                                                TextButton(onClick = { viewModel.openResolve(blocker) }) {
                                                    Text("Megoldva")
                                                }
                                            } else {
                                                TextButton(onClick = { viewModel.reopenBlocker(orderId, blocker.id) }) {
                                                    Text("Újranyitás")
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        item {
                            SectionTitle(
                                "Feladatok",
                                count = state.tasks.count { !it.isDone }.takeIf { it > 0 },
                                actionLabel = "Új",
                                onAction = viewModel::openTaskDialog,
                            )
                        }
                        if (state.tasks.isEmpty()) {
                            item {
                                Text(
                                    "Nincs feladat.",
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = Steel500,
                                )
                            }
                        } else {
                            items(state.tasks, key = { it.id }) { task ->
                                TaskRow(
                                    task = task,
                                    onToggle = { viewModel.toggleTask(orderId, task) },
                                )
                            }
                        }

                        if (state.notes.isNotEmpty()) {
                            item { SectionTitle("MiniCRM előzmények", count = state.notes.size) }
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
                            item {
                                Card {
                                    StageRail(
                                        history = state.stages,
                                        daysInStage = detail.stage.daysInStage,
                                        openBlockers = detail.blockers.count { it.resolvedAt == null },
                                    )
                                }
                            }
                        }
                        if (state.error != null) {
                            item {
                                ErrorState(state.error!!) { viewModel.load(orderId) }
                            }
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
    if (state.itemDialog) {
        ItemDialog(
            currency = state.detail?.order?.currency ?: "HUF",
            busy = state.itemBusy,
            error = state.itemError,
            onDismiss = viewModel::closeItemDialog,
            onConfirm = { desc, qty, price -> viewModel.addItem(orderId, desc, qty, price) },
        )
    }
    if (state.blockerDialog) {
        BlockerDialog(
            busy = state.blockerBusy,
            error = state.blockerError,
            onDismiss = viewModel::closeBlockerDialog,
            onConfirm = { body -> viewModel.createBlocker(orderId, body) },
        )
    }
    state.resolveTarget?.let { blocker ->
        ResolveDialog(
            blockerWhat = blocker.what,
            busy = state.blockerBusy,
            error = state.blockerError,
            onDismiss = viewModel::closeResolve,
            onConfirm = { note -> viewModel.resolveBlocker(orderId, note) },
        )
    }
    if (state.taskDialog) {
        TaskDialog(
            busy = state.taskBusy,
            error = state.taskError,
            onDismiss = viewModel::closeTaskDialog,
            onConfirm = { title, due -> viewModel.createTask(orderId, title, due) },
        )
    }
    if (state.intakeDialog && state.detail != null) {
        IntakeSlipDialog(
            order = state.detail!!.order,
            fuelLevels = state.lookups?.fuelLevels.orEmpty(),
            busy = state.intakeBusy,
            error = state.intakeError,
            onDismiss = viewModel::closeIntakeDialog,
            onSave = { body -> viewModel.saveIntakeSlip(orderId, body) },
        )
    }
}

@Composable
private fun ItemDialog(
    currency: String,
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (description: String, quantity: String, unitPriceMinor: Long) -> Unit,
) {
    var description by rememberSaveable { mutableStateOf("") }
    var quantity by rememberSaveable { mutableStateOf("1") }
    var unitPrice by rememberSaveable { mutableStateOf("") }
    val priceMinor = unitPrice.filter { it.isDigit() }.toLongOrNull()?.times(100)
    DialogShell(
        title = "Új tétel",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Hozzáadás",
                enabled = !busy && description.isNotBlank() && priceMinor != null,
                onClick = {
                    priceMinor?.let { onConfirm(description.trim(), quantity.trim().ifBlank { "1" }, it) }
                },
            )
        },
    ) {
        AutoCrmTextField(
            value = description,
            onValueChange = { description = it },
            label = "Megnevezés *",
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = quantity,
            onValueChange = { quantity = it },
            label = "Mennyiség",
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = unitPrice,
            onValueChange = { unitPrice = it.filter { c -> c.isDigit() } },
            label = "Egységár ($currency, Ft)",
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}

@Composable
private fun BlockerDialog(
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (BlockerBody) -> Unit,
) {
    var what by rememberSaveable { mutableStateOf("") }
    var email by rememberSaveable { mutableStateOf("") }
    var due by rememberSaveable { mutableStateOf("") }
    var notes by rememberSaveable { mutableStateOf("") }
    DialogShell(
        title = "Új akadály",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Létrehozás",
                enabled = !busy && what.isNotBlank(),
                onClick = {
                    fun blankToNull(v: String): String? = v.trim().takeIf { it.isNotBlank() }
                    onConfirm(
                        BlockerBody(
                            what = what.trim(),
                            responsibleEmail = blankToNull(email),
                            dueDate = blankToNull(due),
                            notes = blankToNull(notes),
                        ),
                    )
                },
            )
        },
    ) {
        AutoCrmTextField(
            value = what,
            onValueChange = { what = it },
            label = "Mi akadt el? *",
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = email,
            onValueChange = { email = it },
            label = "Felelős e-mail",
            modifier = Modifier.fillMaxWidth(),
        )
        DateField(
            value = due,
            onValueChange = { due = it },
            label = "Határidő",
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = notes,
            onValueChange = { notes = it },
            label = "Megjegyzés",
            modifier = Modifier.fillMaxWidth(),
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}

@Composable
private fun ResolveDialog(
    blockerWhat: String,
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (note: String?) -> Unit,
) {
    var note by rememberSaveable { mutableStateOf("") }
    DialogShell(
        title = "Akadály megoldva",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Lezárás",
                enabled = !busy,
                onClick = { onConfirm(note.trim().takeIf { it.isNotBlank() }) },
            )
        },
    ) {
        Text(blockerWhat, style = MaterialTheme.typography.bodyLarge)
        AutoCrmTextField(
            value = note,
            onValueChange = { note = it },
            label = "Megoldás leírása",
            modifier = Modifier.fillMaxWidth(),
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}
