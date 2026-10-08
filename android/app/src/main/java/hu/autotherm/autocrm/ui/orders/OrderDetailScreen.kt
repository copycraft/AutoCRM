package hu.autotherm.autocrm.ui.orders

import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.KeyboardArrowUp
import hu.autotherm.autocrm.ui.common.copyOnLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Email
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material3.HorizontalDivider
import hu.autotherm.autocrm.ui.common.InitialsAvatar
import hu.autotherm.autocrm.ui.common.PlateBadge
import hu.autotherm.autocrm.ui.common.SecondaryButton
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
        /** Who to ring about this job: its contact's number, else the customer's. */
        val callNumber: String? = null,
        val canManageBlockers: Boolean = false,
        val canSendEmail: Boolean = false,
        val canComment: Boolean = false,
        val canUploadMedia: Boolean = false,
        val error: String? = null,
        /** Non-null while the stage dialog is open. */
        val stageDialog: List<TransitionOption>? = null,
        val stageError: String? = null,
        val busy: Boolean = false,
        /** Write-dialog flags for the sections below. */
        val itemDialog: Boolean = false,
        /** Bumped after "Még egy": the dialog starts over empty for the next line. */
        val itemNonce: Int = 0,
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
                    canManageBlockers = account?.canManageBlockers == true,
                    canSendEmail = account?.canSendEmail == true,
                    canComment = account?.canComment == true,
                    canUploadMedia = account?.canUploadMedia == true,
                    lookups = lookupsDeferred.await(),
                    callNumber = _state.value.callNumber,
                )
                // The number to ring comes from the partner record; fetched after the page
                // is up so it never holds the job back.
                launch {
                    runCatching { api.partner(detail.order.partnerId) }.getOrNull()?.let { p ->
                        val number = p.contacts.firstOrNull { it.id == detail.order.contactId }?.phone?.takeIf { it.isNotBlank() }
                            ?: p.partner.phone?.takeIf { it.isNotBlank() }
                        _state.value = _state.value.copy(callNumber = number)
                    }
                }
            } catch (e: Throwable) {
                val hadRows = _state.value.detail != null
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    error = if (hadRows) null else describeError(e),
                )
                if (hadRows) hu.autotherm.autocrm.ui.common.Toasts.error("Frissítés sikertelen: " + describeError(e), retry = { load(orderId) })
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
                val label = _state.value.stageDialog?.firstOrNull { it.stageKey == stage }?.labelHu ?: stage
                _state.value = _state.value.copy(busy = false, stageDialog = null)
                hu.autotherm.autocrm.ui.common.Toasts.show("Új fázis: $label")
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

    fun addItem(
        orderId: Long,
        description: String,
        quantity: String,
        unitPriceMinor: Long,
        /** Keep the dialog open, emptied, for the next line of a longer quote. */
        another: Boolean = false,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(itemBusy = true, itemError = null)
            try {
                api.addItem(orderId, AddItemBody(description, quantity, unitPriceMinor))
                _state.value = _state.value.copy(
                    itemBusy = false,
                    itemDialog = another,
                    itemNonce = _state.value.itemNonce + if (another) 1 else 0,
                )
                hu.autotherm.autocrm.ui.common.Toasts.show("Tétel hozzáadva")
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(itemBusy = false, itemError = describeError(e))
            }
        }
    }

    /**
     * The line leaves the list at once; the server call follows. "Visszavonás" puts the same
     * line back (as a new row at the end — the server has no undelete).
     */
    fun deleteItem(orderId: Long, itemId: Long) {
        val detail = _state.value.detail ?: return
        val item = detail.items.firstOrNull { it.id == itemId } ?: return
        _state.value = _state.value.copy(detail = detail.copy(items = detail.items.filter { it.id != itemId }))
        viewModelScope.launch {
            try {
                api.deleteItem(itemId)
                hu.autotherm.autocrm.ui.common.Toasts.show("Tétel törölve: ${item.description}", "Visszavonás") {
                    addItem(orderId, item.description, item.quantity, item.unitPrice)
                }
                load(orderId)
            } catch (e: Throwable) {
                hu.autotherm.autocrm.ui.common.Toasts.error("Nem sikerült törölni: " + describeError(e), retry = { deleteItem(orderId, itemId) })
                load(orderId)
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
                hu.autotherm.autocrm.ui.common.Toasts.show("Akadály rögzítve")
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
                hu.autotherm.autocrm.ui.common.Toasts.show("Akadály megoldva")
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
                hu.autotherm.autocrm.ui.common.Toasts.show("Akadály újranyitva")
                load(orderId)
            } catch (e: Throwable) {
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = { reopenBlocker(orderId, blockerId) })
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

    fun createTask(orderId: Long, title: String, dueDate: String?, assignedTo: Long?) {
        viewModelScope.launch {
            _state.value = _state.value.copy(taskBusy = true, taskError = null)
            try {
                val created = api.createTask(TaskBody("order", orderId, title, dueDate?.takeIf { it.isNotBlank() }, assignedTo))
                // Shown at once from the answer; no full reload for one new row.
                _state.value = _state.value.copy(
                    taskBusy = false,
                    taskDialog = false,
                    tasks = _state.value.tasks + created,
                )
                hu.autotherm.autocrm.ui.common.Toasts.show("Feladat létrehozva")
            } catch (e: Throwable) {
                _state.value = _state.value.copy(taskBusy = false, taskError = describeError(e))
            }
        }
    }

    /** The tick shows at once; the server's answer replaces it, a refusal undoes it. */
    fun toggleTask(orderId: Long, task: Task) {
        fun replace(with: Task) {
            _state.value = _state.value.copy(tasks = _state.value.tasks.map { if (it.id == task.id) with else it })
        }
        replace(task.copy(doneAt = if (task.isDone) null else "pending"))
        viewModelScope.launch {
            try {
                replace(api.setTaskDone(task.id, !task.isDone))
            } catch (e: Throwable) {
                replace(task)
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = { toggleTask(orderId, task) })
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
                hu.autotherm.autocrm.ui.common.Toasts.show("Átvételi lap mentve")
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(intakeBusy = false, intakeError = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
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
    /** The customer's page: their contacts and numbers, one tap from the job. */
    onOpenPartner: ((Long) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(orderId) { viewModel.load(orderId) }
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.load(orderId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.detail?.let { "#${it.order.number}" } ?: "Munka",
                subtitle = state.detail?.let { "${it.partner.name} · ${it.stage.labelHu}" },
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(orderId) },
                actions = {
                    // Icons, not three words fighting the title for a phone's width.
                    if (state.canSendEmail && state.detail != null) {
                        IconButton(onClick = { onComposeEmail(orderId) }) {
                            Icon(Icons.Outlined.Email, contentDescription = "Levél")
                        }
                    }
                    if (state.canEdit && state.detail != null) {
                        IconButton(onClick = { onEditOrder(orderId) }) {
                            Icon(Icons.Outlined.Edit, contentDescription = "Szerkesztés")
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
                hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = { viewModel.load(orderId) },
                    modifier = Modifier.padding(padding),
                ) {
                    val detailList = androidx.compose.foundation.lazy.rememberLazyListState()
                    LazyColumn(
                        Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                        state = detailList,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item {
                            // One card answers "which van, whose, how far along, what is it worth".
                            Card {
                                Row(
                                    Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                        // Press and hold to copy, for the parts supplier or an e-mail.
                                        detail.order.vehiclePlate?.let { plate ->
                                            androidx.compose.foundation.layout.Box(Modifier.copyOnLongPress(plate)) { PlateBadge(plate) }
                                        }
                                        Text(
                                            "#${detail.order.number}",
                                            style = MonoSmall,
                                            color = Steel500,
                                            modifier = Modifier.copyOnLongPress(detail.order.number),
                                        )
                                    }
                                    StatusBadge(
                                        detail.stage.labelHu,
                                        if (detail.stage.isTerminal) Tone.Done else Tone.Cold,
                                    )
                                }
                                Text(detail.order.title, style = MaterialTheme.typography.headlineSmall)
                                Row(
                                    Modifier.fillMaxWidth().then(
                                        if (onOpenPartner != null) {
                                            Modifier.clickable { onOpenPartner(detail.partner.id) }
                                        } else {
                                            Modifier
                                        },
                                    ),
                                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    InitialsAvatar(detail.partner.name, size = 32.dp)
                                    Column(Modifier.weight(1f)) {
                                        Text(detail.partner.name, style = MaterialTheme.typography.titleSmall, maxLines = 1)
                                        Text(
                                            if (detail.stage.daysInStage == 0L) "Ma került ebbe a fázisba"
                                            else "${detail.stage.daysInStage} napja ebben a fázisban",
                                            style = MaterialTheme.typography.bodySmall,
                                            color = Steel500,
                                        )
                                    }
                                    // One tap to ring the customer about this van.
                                    state.callNumber?.let { number ->
                                        val uri = androidx.compose.ui.platform.LocalUriHandler.current
                                        IconButton(onClick = {
                                            val digits = number.trim().let { (if (it.startsWith("+")) "+" else "") + it.filter(Char::isDigit) }
                                            uri.openUri("tel:$digits")
                                        }) {
                                            Icon(
                                                androidx.compose.material.icons.Icons.Filled.Call,
                                                contentDescription = "Hívás: " + (hu.autotherm.autocrm.util.formatPhone(number) ?: number),
                                                tint = hu.autotherm.autocrm.ui.theme.Cold,
                                            )
                                        }
                                    }
                                    // Says the row leads somewhere: the customer's page.
                                    if (onOpenPartner != null) {
                                        Icon(
                                            androidx.compose.material.icons.Icons.AutoMirrored.Filled.KeyboardArrowRight,
                                            contentDescription = "Partner adatlapja",
                                            tint = Steel500,
                                        )
                                    }
                                }
                                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                    Column {
                                        Text("Érték", style = MaterialTheme.typography.labelMedium, color = Steel500)
                                        Text(
                                            formatMoney(detail.value.totalMinor, detail.value.currency),
                                            style = MaterialTheme.typography.titleLarge,
                                        )
                                    }
                                    detail.order.dueDate?.let { due ->
                                        val late = !detail.stage.isTerminal && runCatching {
                                            java.time.LocalDate.parse(due).isBefore(java.time.LocalDate.now())
                                        }.getOrDefault(false)
                                        Column(horizontalAlignment = Alignment.End) {
                                            Text("Határidő", style = MaterialTheme.typography.labelMedium, color = Steel500)
                                            Text(
                                                formatDate(due).orEmpty() +
                                                    (hu.autotherm.autocrm.util.relativeDay(due)?.takeIf { !it.contains('.') }?.let { " ($it)" } ?: ""),
                                                style = MaterialTheme.typography.titleMedium,
                                                color = if (late) Signal else MaterialTheme.colorScheme.onSurface,
                                            )
                                        }
                                    }
                                }
                                val open = detail.blockers.count { it.resolvedAt == null }
                                // V3: an order with no line items reports as zero and silently
                                // undercounts every report it appears in. Say so where the work is.
                                val warnings = listOfNotNull(
                                    "$open nyitott akadály".takeIf { open > 0 },
                                    "Nincs rögzített érték".takeIf { detail.value.totalMinor == 0L },
                                    "Hiányzó árfolyam".takeIf { detail.value.totalHufMinor == null && detail.value.currency != "HUF" },
                                )
                                if (warnings.isNotEmpty()) {
                                    FlowRow(
                                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                                        verticalArrangement = Arrangement.spacedBy(6.dp),
                                    ) {
                                        warnings.forEach { StatusBadge(it, Tone.Signal) }
                                    }
                                }
                            }
                        }

                        item {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                if (state.canChangeStage) {
                                    PrimaryButton(
                                        text = "Fázisváltás",
                                        onClick = { viewModel.openStageDialog(orderId) },
                                        modifier = Modifier.weight(1f),
                                    )
                                }
                                SecondaryButton(
                                    text = "Átvétel-átadás",
                                    onClick = { onInspections(orderId) },
                                    modifier = Modifier.weight(1f),
                                )
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
                                            canEdit = state.canUploadMedia,
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
                                canComment = state.canComment,
                            )
                        }

                        if (state.canUploadMedia) {
                            item {
                                Card {
                                    hu.autotherm.autocrm.ui.common.VoiceNoteRecorder(orderId = orderId)
                                }
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
                            // The lines add up to something: say what, under the last one.
                            if (detail.items.size > 1) {
                                item(key = "items-total") {
                                    Row(
                                        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                    ) {
                                        Text(
                                            "${detail.items.size} tétel összesen (nettó)",
                                            style = MaterialTheme.typography.labelLarge,
                                            color = Steel500,
                                        )
                                        Text(
                                            formatMoney(detail.items.sumOf { it.lineTotalMinor }, detail.value.currency),
                                            style = MaterialTheme.typography.titleMedium,
                                        )
                                    }
                                }
                            }
                        }

                        if (detail.blockers.isNotEmpty() || state.canManageBlockers) {
                            item {
                                SectionTitle(
                                    "Akadályok",
                                    count = detail.blockers.count { it.resolvedAt == null }.takeIf { it > 0 },
                                    actionLabel = if (state.canManageBlockers) "Új" else null,
                                    onAction = if (state.canManageBlockers) viewModel::openBlockerDialog else null,
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
                                            hu.autotherm.autocrm.util.relativeDay(blocker.dueDate)?.let { "határidő: $it" },
                                            blocker.nudgeCount.takeIf { it > 0 }?.let { "$it emlékeztető" },
                                        ).joinToString(" · ").ifBlank { "—" },
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Steel500,
                                    )
                                    // Chase the person it waits on: their address opens a letter.
                                    blocker.responsibleEmail?.takeIf { it.isNotBlank() && blocker.resolvedAt == null }?.let { email ->
                                        hu.autotherm.autocrm.ui.common.EmailInfo("Felelős", email)
                                    }
                                    blocker.notes?.let { Text(it, style = MaterialTheme.typography.bodyLarge) }
                                    blocker.resolutionNote?.let {
                                        Text(it, style = MaterialTheme.typography.labelMedium, color = Steel500)
                                    }
                                    if (state.canManageBlockers) {
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
                                if (task.isDone) {
                                    TaskRow(task = task, onToggle = { viewModel.toggleTask(orderId, task) })
                                } else {
                                    hu.autotherm.autocrm.ui.tasks.SwipeToDone(onDone = { viewModel.toggleTask(orderId, task) }) {
                                        TaskRow(task = task, onToggle = { viewModel.toggleTask(orderId, task) })
                                    }
                                }
                            }
                        }

                        if (state.notes.isNotEmpty()) {
                            item { SectionTitle("MiniCRM előzmények", count = state.notes.size) }
                            items(state.notes, key = { it.id }) { note ->
                                Card {
                                    Text(
                                        "${note.authorName ?: "—"} · ${hu.autotherm.autocrm.util.relativeTime(note.occurredAt)}",
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
                    // A long job page: once well down it, one tap back to the header.
                    val scrolledDown by androidx.compose.runtime.remember {
                        androidx.compose.runtime.derivedStateOf { detailList.firstVisibleItemIndex > 2 }
                    }
                    val topScope = androidx.compose.runtime.rememberCoroutineScope()
                    androidx.compose.animation.AnimatedVisibility(
                        visible = scrolledDown,
                        modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
                        enter = androidx.compose.animation.fadeIn() + androidx.compose.animation.scaleIn(),
                        exit = androidx.compose.animation.fadeOut() + androidx.compose.animation.scaleOut(),
                    ) {
                        androidx.compose.material3.SmallFloatingActionButton(
                            onClick = { topScope.launch { detailList.animateScrollToItem(0) } },
                            containerColor = hu.autotherm.autocrm.ui.theme.Steel900,
                            contentColor = hu.autotherm.autocrm.ui.theme.Surface,
                        ) {
                            Icon(androidx.compose.material.icons.Icons.Filled.KeyboardArrowUp, contentDescription = "Vissza a tetejére")
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
            currentLabel = state.detail?.stage?.labelHu,
            onDismiss = viewModel::closeStageDialog,
            onConfirm = { stage, note, _ -> viewModel.changeStage(orderId, stage, note) },
        )
    }
    if (state.itemDialog) {
        androidx.compose.runtime.key(state.itemNonce) {
            ItemDialog(
                currency = state.detail?.order?.currency ?: "HUF",
                busy = state.itemBusy,
                error = state.itemError,
                onDismiss = viewModel::closeItemDialog,
                onConfirm = { desc, qty, price -> viewModel.addItem(orderId, desc, qty, price) },
                onConfirmAndNext = { desc, qty, price -> viewModel.addItem(orderId, desc, qty, price, another = true) },
            )
        }
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
            onConfirm = { title, due, who -> viewModel.createTask(orderId, title, due, who) },
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
    onConfirmAndNext: (description: String, quantity: String, unitPriceMinor: Long) -> Unit,
) {
    var description by rememberSaveable { mutableStateOf("") }
    var quantity by rememberSaveable { mutableStateOf("1") }
    var unitPrice by rememberSaveable { mutableStateOf("") }
    // "2,5" and "12 500,50" as a fitter types them; a leading minus makes a discount line.
    val qty = hu.autotherm.autocrm.util.parseQuantity(quantity.ifBlank { "1" })
    val priceMinor = hu.autotherm.autocrm.util.parseSignedMajorToMinor(unitPrice)
    val symbol = when (currency.uppercase()) { "HUF" -> "Ft"; "EUR" -> "€"; else -> currency }
    val lineTotal = if (qty != null && priceMinor != null) {
        runCatching {
            java.math.BigDecimal(qty).multiply(java.math.BigDecimal(priceMinor))
                .setScale(0, java.math.RoundingMode.HALF_UP).toLong()
        }.getOrNull()
    } else null
    DialogShell(
        title = "Új tétel",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            // Saves this line and opens an empty one: a quote is rarely a single line.
            TextButton(
                onClick = {
                    if (priceMinor != null && qty != null) onConfirmAndNext(description.trim(), qty, priceMinor)
                },
                enabled = !busy && description.isNotBlank() && priceMinor != null && qty != null,
            ) { Text("+ Még egy") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Hozzáadás",
                enabled = !busy && description.isNotBlank() && priceMinor != null && qty != null,
                onClick = {
                    if (priceMinor != null && qty != null) onConfirm(description.trim(), qty, priceMinor)
                },
            )
        },
    ) {
        AutoCrmTextField(
            value = description,
            onValueChange = { description = it },
            label = "Megnevezés *",
            keyboardOptions = KeyboardOptions(capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Sentences),
            modifier = Modifier.fillMaxWidth(),
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            AutoCrmTextField(
                value = quantity,
                onValueChange = { v -> quantity = v.filter { it.isDigit() || it == ',' || it == '.' } },
                label = "Mennyiség",
                isError = quantity.isNotBlank() && qty == null,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.weight(0.4f),
            )
            AutoCrmTextField(
                value = unitPrice,
                onValueChange = { v ->
                    unitPrice = v.filterIndexed { i, c -> c.isDigit() || c == ',' || c == '.' || (i == 0 && c == '-') }
                },
                visualTransformation = hu.autotherm.autocrm.util.GroupedNumberTransformation,
                label = "Egységár, nettó ($symbol) *",
                isError = unitPrice.isNotBlank() && priceMinor == null,
                // The last field: the keyboard's tick adds the line.
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Decimal,
                    imeAction = androidx.compose.ui.text.input.ImeAction.Done,
                ),
                keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = {
                    if (!busy && description.isNotBlank() && priceMinor != null && qty != null) {
                        onConfirm(description.trim(), qty, priceMinor)
                    }
                }),
                modifier = Modifier.weight(0.6f),
            )
        }
        Text(
            when {
                quantity.isNotBlank() && qty == null -> "A mennyiség pozitív szám, legfeljebb 3 tizedesjeggyel."
                lineTotal != null -> "Sor összesen: ${formatMoney(lineTotal, currency)}" +
                    if (lineTotal < 0) " (kedvezmény)" else ""
                else -> "Kedvezményhez írj mínusz jelet az ár elé (pl. -5000)."
            },
            style = MaterialTheme.typography.bodySmall,
            color = if (quantity.isNotBlank() && qty == null) Signal else Steel500,
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
        // What usually holds a job up, one tap each.
        hu.autotherm.autocrm.ui.common.QuickPicks(
            options = listOf("Alkatrészre vár", "Ügyfél jóváhagyására vár", "Előlegre vár", "Járműre vár", "Gyártói anyagra vár"),
            current = what,
            onPick = { what = it },
        )
        val emailBad = email.isNotBlank() && !Regex("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$").matches(email.trim())
        AutoCrmTextField(
            value = email,
            onValueChange = { email = it.trim() },
            label = "Felelős e-mail",
            isError = emailBad,
            supporting = if (emailBad) "Nem érvényes e-mail cím." else "Ide megy az emlékeztető, ha lejár.",
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
            modifier = Modifier.fillMaxWidth(),
        )
        DateField(
            value = due,
            onValueChange = { due = it },
            label = "Határidő",
            modifier = Modifier.fillMaxWidth(),
            quickPicks = listOf("Holnap" to 1L, "3 nap" to 3L, "1 hét" to 7L),
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
