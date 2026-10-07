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
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(withBoard: Boolean = true) {
        viewModelScope.launch {
            try {
                if (withBoard) {
                    val board = api.yardBoard()
                    _state.value = _state.value.copy(loading = false, board = board, locations = board.locations, error = null)
                } else {
                    _state.value = _state.value.copy(loading = false, locations = api.yardLocations(), error = null)
                }
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    fun move(vehicleId: Long, locationId: Long?, orderId: Long?, reloadBoard: Boolean, onDone: () -> Unit = {}) {
        viewModelScope.launch {
            try {
                val r = api.moveVehicle(MoveBody(vehicleId, locationId, orderId))
                _state.value = _state.value.copy(
                    message = if (r.overCapacity) "Áthelyezve, de ez a hely már tele van." else "Áthelyezve.",
                    error = null,
                )
                onDone()
                if (reloadBoard) load(true)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
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
                    here.locationId == null -> "Elvitték · ${formatDateTime(here.movedAt).orEmpty()}"
                    else -> "${here.locationName} · ${formatDateTime(here.movedAt).orEmpty()}" +
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
                vm.move(v.id, loc, orderId, reloadBoard = false, onDone = onMoved)
            },
            onDismiss = { moving = null },
        )
    }
}

/** The yard from the phone: every place with what stands on it; tap a van to move it. */
@Composable
fun YardScreen(canMove: Boolean, onOpenOrder: (Long) -> Unit, onMenu: () -> Unit) {
    val context = LocalContext.current
    val vm: YardViewModel = viewModel(key = "yard-board") {
        YardViewModel((context.applicationContext as AutoCrmApp).api)
    }
    val state by vm.state.collectAsState()
    var moving by remember { mutableStateOf<YardVehicle?>(null) }
    LaunchedEffect(Unit) { vm.load() }

    Scaffold(topBar = { ScreenTopBar(title = "Udvar", onMenu = onMenu, onRefresh = { vm.load() }) }) { padding ->
        val board = state.board
        when {
            state.loading -> ListSkeleton(Modifier.padding(padding).padding(16.dp))
            board == null -> ErrorState(state.error ?: "Nem tölthető be.", Modifier.padding(padding)) { vm.load() }
            else -> LazyColumn(
                Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                state.message?.let { msg -> item { Text(msg, style = MaterialTheme.typography.labelMedium) } }
                val groups = listOf<Pair<YardLocation?, List<YardVehicle>>>(
                    null to board.vehicles.filter { it.locationId == null && it.orderId != null },
                ) + board.locations.map { l -> l to board.vehicles.filter { it.locationId == l.id } }
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
                                Text(v.plate ?: v.vin ?: "#${v.vehicleId}", style = MaterialTheme.typography.titleMedium)
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

    moving?.let { v ->
        MoveDialog(
            title = "${v.plate ?: v.vin ?: "#${v.vehicleId}"} áthelyezése",
            locations = state.board?.locations.orEmpty(),
            current = v.locationId,
            onPick = { loc ->
                moving = null
                vm.move(v.vehicleId, loc, v.orderId, reloadBoard = true)
            },
            onDismiss = { moving = null },
        )
    }
}
