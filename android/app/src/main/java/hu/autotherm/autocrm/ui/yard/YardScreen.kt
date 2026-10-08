package hu.autotherm.autocrm.ui.yard

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.CurrentLocation
import hu.autotherm.autocrm.data.api.MoveBody
import hu.autotherm.autocrm.data.api.Vehicle
import hu.autotherm.autocrm.data.api.YardBoard
import hu.autotherm.autocrm.data.api.YardLocation
import hu.autotherm.autocrm.data.api.YardVehicle
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDateTime
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class YardViewModel(private val api: AutoCrmApi) : ViewModel() {
    data class State(
        val loading: Boolean = true,
        val board: YardBoard? = null,
        val locations: List<YardLocation> = emptyList(),
        val error: String? = null,
        val message: String? = null,
        /** Pull-to-refresh with the board kept on screen. */
        val refreshing: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(withBoard: Boolean = true) {
        viewModelScope.launch {
            if (withBoard && _state.value.board != null) _state.value = _state.value.copy(refreshing = true)
            try {
                if (withBoard) {
                    val board = api.yardBoard()
                    _state.value = _state.value.copy(
                        loading = false,
                        refreshing = false,
                        board = board,
                        locations = board.locations,
                        error = null,
                    )
                } else {
                    _state.value = _state.value.copy(loading = false, locations = api.yardLocations(), error = null)
                }
            } catch (e: Throwable) {
                val hadBoard = _state.value.board != null
                _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
                if (hadBoard) hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e))
            }
        }
    }

    /** [label] names the van in the confirmation ("ABC-123 → Műhely 2"). */
    fun move(
        vehicleId: Long,
        locationId: Long?,
        orderId: Long?,
        reloadBoard: Boolean,
        onDone: () -> Unit = {},
        label: String? = null,
    ) {
        viewModelScope.launch {
            try {
                val r = api.moveVehicle(MoveBody(vehicleId, locationId, orderId))
                val where = locationId?.let { id -> _state.value.locations.firstOrNull { it.id == id }?.name }
                    ?: if (locationId == null) "elvitték" else "áthelyezve"
                val what = label?.let { "$it → " } ?: ""
                if (r.overCapacity) {
                    hu.autotherm.autocrm.ui.common.Toasts.error("$what$where — de ez a hely már tele van.")
                } else {
                    hu.autotherm.autocrm.ui.common.Toasts.show("$what$where")
                }
                _state.value = _state.value.copy(message = null, error = null)
                onDone()
                if (reloadBoard) load(true)
            } catch (e: Throwable) {
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = { move(vehicleId, locationId, orderId, reloadBoard, onDone, label) })
            }
        }
    }
}

@Composable
private fun MoveDialog(
    title: String,
    locations: List<YardLocation>,
    current: Long?,
    onPick: (Long?) -> Unit,
    onDismiss: () -> Unit,
) {
    DialogShell(
        title = title,
        onDismiss = onDismiss,
        actions = { TextButton(onClick = onDismiss) { Text("Mégse") } },
    ) {
        locations.forEach { l ->
            TextButton(onClick = { onPick(l.id) }, modifier = Modifier.fillMaxWidth(), enabled = l.id != current) {
                Text(l.name + if (l.id == current) " (itt áll)" else "")
            }
        }
        TextButton(onClick = { onPick(null) }, modifier = Modifier.fillMaxWidth()) {
            Text("Elvitték a telephelyről")
        }
    }
}

/** Where an order's vehicles stand, with a button to move one (order detail). */
@Composable
fun VehicleLocationCard(
    orderId: Long,
    vehicles: List<Vehicle>,
    current: List<CurrentLocation>,
    canMove: Boolean,
    onMoved: () -> Unit,
) {
    if (vehicles.isEmpty()) return
    val context = LocalContext.current
    val vm: YardViewModel = viewModel(key = "yard-order-$orderId") {
        YardViewModel((context.applicationContext as AutoCrmApp).api)
    }
    val state by vm.state.collectAsState()
    var moving by remember { mutableStateOf<Vehicle?>(null) }
    LaunchedEffect(orderId) { if (canMove) vm.load(withBoard = false) }

    Card {
        SectionTitle("Hol áll?")
        vehicles.forEach { v ->
            val here = current.firstOrNull { it.vehicleId == v.id }
            Text(v.plate ?: v.vin ?: "#${v.id}", style = MaterialTheme.typography.titleMedium)
            Text(
                when {
                    here == null -> "Nincs elhelyezve"
                    here.locationId == null -> "Elvitték · ${hu.autotherm.autocrm.util.relativeTime(here.movedAt).orEmpty()}"
                    else -> "${here.locationName} · ${hu.autotherm.autocrm.util.relativeTime(here.movedAt).orEmpty()}" +
                        (here.movedByName?.let { " · $it" } ?: "")
                },
                style = MaterialTheme.typography.bodyLarge,
                color = Steel500,
            )
            if (canMove) {
                TextButton(onClick = { moving = v }) { Text("Áthelyezés") }
            }
        }
        state.message?.let { Text(it, style = MaterialTheme.typography.labelMedium) }
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
    }

    moving?.let { v ->
        MoveDialog(
            title = "${v.plate ?: v.vin ?: "#${v.id}"} áthelyezése",
            locations = state.locations,
            current = current.firstOrNull { it.vehicleId == v.id }?.locationId,
            onPick = { loc ->
                moving = null
                vm.move(v.id, loc, orderId, reloadBoard = false, onDone = onMoved, label = v.plate ?: v.vin)
            },
            onDismiss = { moving = null },
        )
    }
}

/** The yard from the phone: every place with what stands on it; tap a van to move it. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun YardScreen(canMove: Boolean, onOpenOrder: (Long) -> Unit, onMenu: () -> Unit) {
    val context = LocalContext.current
    val vm: YardViewModel = viewModel(key = "yard-board") {
        YardViewModel((context.applicationContext as AutoCrmApp).api)
    }
    val state by vm.state.collectAsState()
    var moving by remember { mutableStateOf<YardVehicle?>(null) }
    // "Where is the white Ducato?" — typed, not scrolled for.
    var query by androidx.compose.runtime.saveable.rememberSaveable { mutableStateOf("") }
    LaunchedEffect(Unit) { vm.load() }

    Scaffold(topBar = { ScreenTopBar(title = "Udvar", onMenu = onMenu, refreshing = state.refreshing, onRefresh = { vm.load() }) }) { padding ->
        val board = state.board
        when {
            state.loading -> ListSkeleton(Modifier.padding(padding).padding(16.dp))
            board == null -> ErrorState(state.error ?: "Nem tölthető be.", Modifier.padding(padding)) { vm.load() }
            else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                isRefreshing = state.refreshing,
                onRefresh = { vm.load() },
                modifier = Modifier.padding(padding),
            ) {
            LazyColumn(
                Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    hu.autotherm.autocrm.ui.common.SearchField(
                        value = query,
                        onValueChange = { query = it },
                        label = "Rendszám, munkaszám, ügyfél…",
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                val needle = query.trim().lowercase().replace(" ", "").replace("-", "")
                fun matches(v: YardVehicle) = needle.isEmpty() || listOfNotNull(v.plate, v.vin, v.orderNumber, v.partnerName)
                    .any { it.lowercase().replace(" ", "").replace("-", "").contains(needle) }
                val groups = (
                    listOf<Pair<YardLocation?, List<YardVehicle>>>(
                        null to board.vehicles.filter { it.locationId == null && it.orderId != null },
                    ) + board.locations.map { l -> l to board.vehicles.filter { it.locationId == l.id } }
                    )
                    .map { (l, vans) -> l to vans.filter(::matches) }
                    // While searching, only the places where something matched.
                    .filter { needle.isEmpty() || it.second.isNotEmpty() }
                if (needle.isNotEmpty() && groups.isEmpty()) {
                    item { hu.autotherm.autocrm.ui.common.EmptyState("Nincs ilyen jármű az udvaron.") }
                }
                items(groups, key = { it.first?.id ?: -1L }) { (location, vans) ->
                    Card {
                        val full = location?.capacity != null && vans.size > location.capacity
                        SectionTitle(location?.name ?: "Nincs elhelyezve", count = vans.size)
                        location?.capacity?.let {
                            StatusBadge("${vans.size} / $it", if (full) Tone.Signal else Tone.Steel)
                        }
                        if (vans.isEmpty()) {
                            Text("Üres.", style = MaterialTheme.typography.bodyLarge, color = Steel500)
                        }
                        vans.forEach { v ->
                            Card(onClick = { if (canMove) moving = v else v.orderId?.let(onOpenOrder) }) {
                                val plate = v.plate
                                if (plate != null) {
                                    hu.autotherm.autocrm.ui.common.PlateBadge(plate)
                                } else {
                                    Text(v.vin ?: "#${v.vehicleId}", style = MaterialTheme.typography.titleMedium)
                                }
                                Text(
                                    listOfNotNull(v.orderNumber?.let { "#$it" }, v.partnerName, v.stageLabel).joinToString(" · "),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                                if (v.orderId != null) {
                                    TextButton(onClick = { onOpenOrder(v.orderId) }) { Text("Munka megnyitása") }
                                }
                            }
                        }
                    }
                }
            }
            }
        }
    }

    moving?.let { v ->
        MoveDialog(
            title = "${v.plate ?: v.vin ?: "#${v.vehicleId}"} áthelyezése",
            locations = state.board?.locations.orEmpty(),
            current = v.locationId,
            onPick = { loc ->
                moving = null
                vm.move(v.vehicleId, loc, v.orderId, reloadBoard = true, label = v.plate ?: v.vin)
            },
            onDismiss = { moving = null },
        )
    }
}
