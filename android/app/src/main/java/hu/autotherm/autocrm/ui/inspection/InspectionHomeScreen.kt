package hu.autotherm.autocrm.ui.inspection

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.Inspection
import hu.autotherm.autocrm.data.api.InspectionDetail
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.db.InspectionDraft
import hu.autotherm.autocrm.data.inspection.DraftPayload
import hu.autotherm.autocrm.data.inspection.InspectionSyncWorker
import hu.autotherm.autocrm.data.inspection.cachedLookups
import hu.autotherm.autocrm.data.inspection.downloadLookups
import hu.autotherm.autocrm.data.inspection.walkaroundKindLabel
import hu.autotherm.autocrm.data.inspection.prefetchZoneLists
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

private val json = Json { ignoreUnknownKeys = true }

class InspectionHomeViewModel(private val app: AutoCrmApp) : ViewModel() {

    data class State(
        val drafts: List<InspectionDraft> = emptyList(),
        val history: List<Inspection> = emptyList(),
        val orderNumber: String = "",
        val loading: Boolean = true,
        val error: String? = null,
        val lookups: Lookups? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(orderId: Long) {
        viewModelScope.launch {
            app.database.inspectionDrafts().watchForOrder(orderId).collect { drafts ->
                _state.value = _state.value.copy(drafts = drafts)
            }
        }
        viewModelScope.launch {
            try {
                val order = app.api.order(orderId).order
                val history = app.api.inspections(orderId)
                // Same for the enumerations the walkaround renders.
                val lookups = downloadLookups(app.api, app.lookupsCache)
                    ?: cachedLookups(app.lookupsCache)
                _state.value = _state.value.copy(
                    history = history,
                    orderNumber = order.number,
                    loading = false,
                    lookups = lookups,
                )
                // Online now, so keep the current zone lists for this vehicle kind: the
                // walkaround is often started where there is no signal.
                prefetchZoneLists(app.api, app.zoneListCache, order.projectTypeId)
            } catch (e: Exception) {
                _state.value = _state.value.copy(
                    loading = false,
                    error = describeError(e),
                )
            }
        }
        viewModelScope.launch {
            // Offline open: the fetch above never ran, but the walkaround kinds
            // still render from the last downloaded document.
            val cached = cachedLookups(app.lookupsCache)
            if (_state.value.lookups == null && cached != null) {
                _state.value = _state.value.copy(lookups = cached)
            }
        }
    }

    fun discard(uuid: String) {
        viewModelScope.launch {
            InspectionSyncWorker.discard(app, uuid)
            hu.autotherm.autocrm.ui.common.Toasts.show("Piszkozat eldobva")
        }
    }

    fun syncNow() {
        InspectionSyncWorker.enqueueNow(app.applicationContext)
        hu.autotherm.autocrm.ui.common.Toasts.show("Feltöltés elindítva")
    }

    /** Back from a walkaround: the server history may have a new signed inspection. */
    fun refreshHistory(orderId: Long) {
        viewModelScope.launch {
            runCatching { app.api.inspections(orderId) }.onSuccess { history ->
                _state.value = _state.value.copy(history = history, error = null)
            }
        }
    }

    fun draftProgress(draft: InspectionDraft): String {
        val payload = runCatching {
            json.decodeFromString(DraftPayload.serializer(), draft.payloadJson)
        }.getOrNull() ?: return "piszkozat"
        if (payload.signed) return "aláírva, feltöltésre vár"
        if (draft.error != null) return draft.error
        val total = payload.templates.size.coerceAtLeast(1)
        return "${(payload.zoneIndex).coerceAtMost(total)}/$total zóna"
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InspectionHomeScreen(
    orderId: Long,
    viewModel: InspectionHomeViewModel,
    canInspect: Boolean,
    onBack: () -> Unit,
    onStart: (kind: String) -> Unit,
    onResume: (uuid: String) -> Unit,
    onOpenServer: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(orderId) { viewModel.load(orderId) }
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.refreshHistory(orderId) }
    // Discarding a walkaround throws away its photos and signatures: asked first.
    var discarding by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf<String?>(null) }
    discarding?.let { uuid ->
        hu.autotherm.autocrm.ui.common.DialogShell(
            title = "Eldobod a piszkozatot?",
            onDismiss = { discarding = null },
            actions = {
                androidx.compose.material3.TextButton(onClick = { discarding = null }) { Text("Megtartom") }
                PrimaryButton(
                    text = "Eldobás",
                    onClick = {
                        viewModel.discard(uuid)
                        discarding = null
                    },
                )
            },
        ) {
            Text(
                "A telefonon lévő fotói, sérülései és aláírásai is törlődnek. Ez nem vonható vissza.",
                style = MaterialTheme.typography.bodyLarge,
            )
        }
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Átvétel-átadás",
                subtitle = state.orderNumber.takeIf { it.isNotBlank() },
                onBack = onBack,
                refreshing = false,
                onRefresh = { viewModel.load(orderId) },
            )
        },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (canInspect) {
                item {
                    // A walk already on the phone is continued, not started a second time
                    // (two drafts of one check-out was a mess to untangle).
                    val checkoutDraft = state.drafts.firstOrNull { it.kind == "checkout" }
                    val checkinDraft = state.drafts.firstOrNull { it.kind == "checkin" }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = { checkoutDraft?.let { onResume(it.localUuid) } ?: onStart("checkout") },
                            modifier = Modifier.weight(1f),
                        ) { Text(if (checkoutDraft != null) "Átvétel folytatása" else "Átvétel indítása") }
                        OutlinedButton(
                            onClick = { checkinDraft?.let { onResume(it.localUuid) } ?: onStart("checkin") },
                            modifier = Modifier.weight(1f),
                        ) { Text(if (checkinDraft != null) "Kiadás folytatása" else "Kiadás indítása") }
                    }
                }
            }
            if (state.drafts.isNotEmpty()) {
                item { SectionTitle("Folyamatban a telefonon", count = state.drafts.size) }
                items(state.drafts, key = { it.localUuid }) { draft ->
                    Card {
                        Row(
                            Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f, fill = false)) {
                                Text(
                                    walkaroundKindLabel(draft.kind, state.lookups),
                                    style = MaterialTheme.typography.titleMedium,
                                )
                                Text(
                                    viewModel.draftProgress(draft),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                                draft.error?.let {
                                    Text(it, style = MaterialTheme.typography.labelMedium, color = hu.autotherm.autocrm.ui.theme.Signal)
                                }
                            }
                            IconButton(onClick = { discarding = draft.localUuid }) {
                                Icon(Icons.Filled.Delete, contentDescription = "Eldobás", tint = Steel500)
                            }
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            PrimaryButton(
                                text = "Folytatás",
                                onClick = { onResume(draft.localUuid) },
                            )
                            OutlinedButton(onClick = viewModel::syncNow) { Text("Feltöltés") }
                        }
                    }
                }
            }
            item { SectionTitle("Előzmények", count = state.history.size.takeIf { it > 0 }) }
            when {
                state.loading -> item { ListSkeleton() }
                state.error != null && state.history.isEmpty() ->
                    item { ErrorState(state.error!!) { viewModel.load(orderId) } }
                state.history.isEmpty() -> item { EmptyState("Még nincs átvétel ennél a munkánál.") }
                else -> items(state.history, key = { it.id }) { inspection ->
                    Card(modifier = Modifier.animateItem(), onClick = { onOpenServer(inspection.id) }) {
                        Text(
                            "${walkaroundKindLabel(inspection.kind, state.lookups)} · " +
                                inspection.vehiclePlate,
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Text(
                            "${inspection.inspectorName} · " +
                                (hu.autotherm.autocrm.util.formatDateTime(inspection.signedAt ?: inspection.createdAt) ?: ""),
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                        StatusBadge(
                            if (inspection.status == "signed") "Lezárva" else "Piszkozat",
                            if (inspection.status == "signed") Tone.Done else Tone.Steel,
                        )
                    }
                }
            }
        }
    }
}

class InspectionServerViewModel(private val app: AutoCrmApp) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val detail: InspectionDetail? = null,
        val error: String? = null,
        val note: String = "",
        val noteBusy: Boolean = false,
        val lookups: Lookups? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            val keep = _state.value.detail != null
            _state.value = _state.value.copy(
                loading = !keep,
                refreshing = keep,
                error = null,
            )
            try {
                val lookups = downloadLookups(app.api, app.lookupsCache)
                    ?: cachedLookups(app.lookupsCache)
                _state.value = State(
                    loading = false,
                    detail = app.api.inspection(id),
                    lookups = lookups,
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    error = describeError(e),
                )
            }
        }
    }

    fun addNote(id: Long) {
        val body = _state.value.note.trim()
        if (body.isEmpty()) return
        viewModelScope.launch {
            _state.value = _state.value.copy(noteBusy = true)
            try {
                app.api.addInspectionNote(id, body)
                _state.value = _state.value.copy(note = "", noteBusy = false)
                load(id)
            } catch (e: Exception) {
                _state.value = _state.value.copy(noteBusy = false, error = describeError(e))
            }
        }
    }

    fun setNote(value: String) {
        _state.value = _state.value.copy(note = value)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InspectionServerScreen(
    inspectionId: Long,
    viewModel: InspectionServerViewModel,
    canAnnotate: Boolean,
    onBack: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    var noteOpen by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(inspectionId) { viewModel.load(inspectionId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.detail?.let {
                    walkaroundKindLabel(it.inspection.kind, state.lookups) +
                        " · ${it.inspection.vehiclePlate}"
                } ?: "Átvétel",
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(inspectionId) },
            )
        },
    ) { padding ->
        when {
            state.loading -> DetailSkeleton(
                Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            state.error != null && state.detail == null ->
                ErrorState(state.error!!, Modifier.padding(padding)) { viewModel.load(inspectionId) }
            state.detail == null ->
                ErrorState("Nem található.", Modifier.padding(padding)) { viewModel.load(inspectionId) }
            else -> {
                Column(Modifier.fillMaxSize().padding(padding)) {
                    ServerInspectionDetail(
                        detail = state.detail!!,
                        lookups = state.lookups,
                        modifier = Modifier.weight(1f, fill = false)
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                    )
                    if (canAnnotate) {
                        if (noteOpen) {
                            Card(Modifier.padding(16.dp)) {
                                AutoCrmTextField(
                                    value = state.note,
                                    onValueChange = viewModel::setNote,
                                    label = "Időbélyegzett megjegyzés",
                                    singleLine = false,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    PrimaryButton(
                                        text = if (state.noteBusy) "Mentés…" else "Hozzáfűzés",
                                        enabled = !state.noteBusy && state.note.isNotBlank(),
                                        onClick = { viewModel.addNote(inspectionId) },
                                    )
                                }
                            }
                        } else {
                            TextButton(onClick = { noteOpen = true }) {
                                Text("Megjegyzés hozzáfűzése", modifier = Modifier.padding(16.dp))
                            }
                        }
                    }
                }
            }
        }
    }
}
