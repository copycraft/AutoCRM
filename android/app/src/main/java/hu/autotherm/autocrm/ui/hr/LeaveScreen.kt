package hu.autotherm.autocrm.ui.hr

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.Absence
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Employee
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import java.time.LocalDate

private val KINDS = listOf("annual", "sick", "unpaid", "other")

private fun kindLabel(kind: String): String = when (kind) {
    "annual" -> "Szabadság"
    "sick" -> "Betegszabadság"
    "unpaid" -> "Fizetés nélküli"
    "other" -> "Egyéb"
    else -> kind
}

private fun kindTone(kind: String): Tone = when (kind) {
    "annual" -> Tone.Cold
    "sick" -> Tone.Signal
    else -> Tone.Steel
}

/** Leave and absence for the team, from a week ago to three months ahead. HR access only. */
class LeaveViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val absences: List<Absence> = emptyList(),
        val people: List<Employee> = emptyList(),
        val busy: Boolean = false,
        val error: String? = null,
        /** An error from the add dialog, shown inside it. */
        val dialogError: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load() {
        viewModelScope.launch {
            val keep = _state.value.absences.isNotEmpty()
            _state.value = _state.value.copy(loading = !keep, refreshing = keep, error = null)
            try {
                val today = LocalDate.now()
                val absences = api.absences(today.minusDays(7).toString(), today.plusDays(90).toString())
                val people = api.employees()
                _state.value = _state.value.copy(
                    loading = false, refreshing = false, absences = absences, people = people,
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
            }
        }
    }

    fun clearDialogError() {
        _state.value = _state.value.copy(dialogError = null)
    }

    fun add(employeeId: Long, kind: String, start: String, end: String, note: String, onDone: () -> Unit) {
        if (_state.value.busy) return
        viewModelScope.launch {
            _state.value = _state.value.copy(busy = true, dialogError = null)
            try {
                api.createAbsence(
                    employeeId,
                    buildJsonObject {
                        put("kind", JsonPrimitive(kind))
                        put("start_date", JsonPrimitive(start))
                        put("end_date", JsonPrimitive(end))
                        put("note", note.trim().takeIf { it.isNotEmpty() }?.let(::JsonPrimitive) ?: JsonNull)
                    },
                )
                _state.value = _state.value.copy(busy = false)
                onDone()
                load()
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, dialogError = describeError(e))
            }
        }
    }

    fun remove(absence: Absence) {
        viewModelScope.launch {
            try {
                api.deleteAbsence(absence.id)
                load()
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LeaveScreen(viewModel: LeaveViewModel, onBack: () -> Unit) {
    val state by viewModel.state.collectAsState()
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<Absence?>(null) }
    LaunchedEffect(Unit) { viewModel.load() }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Távollétek",
                subtitle = "Egy héttel ezelőttől 3 hónapra előre",
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
            )
        },
        floatingActionButton = {
            if (state.people.isNotEmpty()) {
                FloatingActionButton(onClick = { viewModel.clearDialogError(); adding = true }) {
                    Icon(Icons.Filled.Add, contentDescription = "Új távollét")
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            if (state.error != null && state.absences.isNotEmpty()) {
                Text(
                    state.error!!,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
            }
            when {
                state.loading -> ListSkeleton()
                state.error != null && state.absences.isEmpty() && state.people.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::load)
                state.people.isEmpty() -> EmptyState("Előbb vegyen fel munkatársat.")
                state.absences.isEmpty() -> EmptyState("Nincs rögzített távollét ebben az időszakban.")
                else -> PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = viewModel::load) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.absences, key = { it.id }) { a ->
                            Card(modifier = Modifier.animateItem()) {
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                    Column(Modifier.weight(1f)) {
                                        Text(a.employeeName, style = MaterialTheme.typography.titleMedium)
                                        Text(
                                            "${formatDate(a.startDate)} – ${formatDate(a.endDate)} · ${a.workingDays} munkanap",
                                            style = MaterialTheme.typography.bodyMedium,
                                        )
                                        a.note?.let {
                                            Text(it, style = MaterialTheme.typography.labelMedium, color = Steel500)
                                        }
                                    }
                                    Column(horizontalAlignment = androidx.compose.ui.Alignment.End) {
                                        StatusBadge(kindLabel(a.kind), kindTone(a.kind))
                                        IconButton(onClick = { removing = a }) {
                                            Icon(Icons.Filled.Delete, contentDescription = "Távollét törlése: ${a.employeeName}")
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if (adding) {
        AbsenceDialog(
            people = state.people,
            busy = state.busy,
            error = state.dialogError,
            onDismiss = { adding = false },
            onSave = { emp, kind, start, end, note ->
                viewModel.add(emp, kind, start, end, note) { adding = false }
            },
        )
    }
    removing?.let { a ->
        DialogShell(
            title = "Törli a távollétet?",
            onDismiss = { removing = null },
            actions = {
                TextButton(onClick = { removing = null }) { Text("Mégse") }
                TextButton(onClick = { viewModel.remove(a); removing = null }) { Text("Törlés") }
            },
        ) {
            Text(
                "${a.employeeName}: ${formatDate(a.startDate)} – ${formatDate(a.endDate)}. A szabadságkeret újra számolódik.",
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

@Composable
private fun AbsenceDialog(
    people: List<Employee>,
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onSave: (Long, String, String, String, String) -> Unit,
) {
    var employee by remember { mutableStateOf(people.first()) }
    var kind by remember { mutableStateOf("annual") }
    var start by remember { mutableStateOf(LocalDate.now().toString()) }
    var end by remember { mutableStateOf(LocalDate.now().toString()) }
    var note by remember { mutableStateOf("") }
    var employeeMenu by remember { mutableStateOf(false) }
    var kindMenu by remember { mutableStateOf(false) }
    val valid = start.isNotBlank() && end.isNotBlank() && end >= start

    DialogShell(
        title = "Új távollét",
        onDismiss = { if (!busy) onDismiss() },
        actions = {
            TextButton(onClick = onDismiss, enabled = !busy) { Text("Mégse") }
            TextButton(
                onClick = { onSave(employee.id, kind, start, end, note) },
                enabled = valid && !busy,
            ) { Text(if (busy) "Mentés…" else "Mentés") }
        },
    ) {
        if (error != null) {
            Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
        }
        Column {
            OutlinedButton(onClick = { employeeMenu = true }, modifier = Modifier.fillMaxWidth()) {
                Text(employee.fullName)
            }
            DropdownMenu(expanded = employeeMenu, onDismissRequest = { employeeMenu = false }) {
                people.forEach { p ->
                    DropdownMenuItem(text = { Text(p.fullName) }, onClick = { employee = p; employeeMenu = false })
                }
            }
        }
        Column {
            OutlinedButton(onClick = { kindMenu = true }, modifier = Modifier.fillMaxWidth()) {
                Text(kindLabel(kind))
            }
            DropdownMenu(expanded = kindMenu, onDismissRequest = { kindMenu = false }) {
                KINDS.forEach { k ->
                    DropdownMenuItem(text = { Text(kindLabel(k)) }, onClick = { kind = k; kindMenu = false })
                }
            }
        }
        DateField(
            value = start,
            onValueChange = { v ->
                start = v
                if (v.isNotBlank() && end < v) end = v
            },
            label = "Kezdete",
            modifier = Modifier.fillMaxWidth(),
        )
        DateField(value = end, onValueChange = { end = it }, label = "Utolsó nap", modifier = Modifier.fillMaxWidth())
        AutoCrmTextField(
            value = note,
            onValueChange = { note = it },
            label = "Megjegyzés",
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
