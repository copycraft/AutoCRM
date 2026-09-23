package hu.autotherm.autocrm.ui.tasks

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
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Task
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DateField
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDate
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Saját feladatok: what is assigned to the signed-in user, open first.
 * Done rows stay visible until the next load so a tap has visible feedback.
 */
class TasksViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val tasks: List<Task> = emptyList(),
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load() {
        viewModelScope.launch {
            val keepRows = _state.value.tasks.isNotEmpty()
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                val tasks = api.tasksMine().sortedWith(
                    compareBy({ it.isDone }, { it.dueDate ?: "9" }),
                )
                _state.value = _state.value.copy(loading = false, refreshing = false, tasks = tasks)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    error = describeError(e),
                )
            }
        }
    }

    fun toggle(task: Task) {
        viewModelScope.launch {
            _state.value = _state.value.copy(
                tasks = _state.value.tasks.map {
                    if (it.id == task.id) it.copy(doneAt = if (it.isDone) null else "pending") else it
                },
            )
            try {
                val updated = api.setTaskDone(task.id, !task.isDone)
                _state.value = _state.value.copy(
                    tasks = _state.value.tasks.map { if (it.id == task.id) updated else it },
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
                load()
            }
        }
    }

    fun delete(task: Task) {
        viewModelScope.launch {
            try {
                api.deleteTask(task.id)
                _state.value = _state.value.copy(tasks = _state.value.tasks.filter { it.id != task.id })
            } catch (e: Throwable) {
                _state.value = _state.value.copy(error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TasksScreen(
    viewModel: TasksViewModel,
    onMenu: () -> Unit,
    onOpenTask: (entityType: String, entityId: Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.load() }

    val open = state.tasks.filter { !it.isDone }
    val done = state.tasks.filter { it.isDone }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Feladatok",
                subtitle = if (open.isNotEmpty()) "${open.size} nyitott" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            when {
                state.loading -> ListSkeleton()
                state.error != null && state.tasks.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::load)
                state.tasks.isEmpty() -> EmptyState("Nincs feladat.")
                else -> PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::load,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(open, key = { it.id }) { task ->
                            TaskRow(
                                task = task,
                                onToggle = { viewModel.toggle(task) },
                                onDelete = { viewModel.delete(task) },
                                onOpen = { onOpenTask(task.entityType, task.entityId) },
                                modifier = Modifier.animateItem(),
                            )
                        }
                        if (done.isNotEmpty()) {
                            item { SectionTitle("Kész", count = done.size) }
                            items(done, key = { it.id }) { task ->
                                TaskRow(
                                    task = task,
                                    onToggle = { viewModel.toggle(task) },
                                    onDelete = { viewModel.delete(task) },
                                    onOpen = { onOpenTask(task.entityType, task.entityId) },
                                    modifier = Modifier.animateItem(),
                                )
                            }
                        }
                    }
                }
            }
            if (state.error != null && state.tasks.isNotEmpty() && !state.refreshing) {
                ErrorState(state.error!!, onRetry = viewModel::load)
            }
        }
    }
}

/**
 * One task row, shared with the order detail's task section. Tapping the row
 * opens the record; the checkbox toggles done; the bin deletes.
 */
@Composable
fun TaskRow(
    task: Task,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
    onDelete: (() -> Unit)? = null,
    onOpen: (() -> Unit)? = null,
) {
    Card(modifier) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Checkbox(checked = task.isDone, onCheckedChange = { onToggle() })
            Column(Modifier.weight(1f, fill = false)) {
                Text(
                    task.title,
                    style = MaterialTheme.typography.bodyLarge.copy(
                        textDecoration = if (task.isDone) TextDecoration.LineThrough else null,
                    ),
                    color = if (task.isDone) Steel500 else MaterialTheme.colorScheme.onSurface,
                )
                val meta = listOfNotNull(
                    task.assignedName,
                    task.dueDate?.let { formatDate(it)?.let { d -> "határidő: $d" } },
                ).joinToString(" · ")
                if (meta.isNotBlank()) {
                    Text(meta, style = MaterialTheme.typography.labelMedium, color = Steel500)
                }
            }
            if (onDelete != null) {
                IconButton(onClick = onDelete) {
                    Icon(Icons.Filled.Delete, contentDescription = "Törlés", tint = Steel500)
                }
            }
        }
        if (onOpen != null) {
            TextButton(onClick = onOpen) { Text("Megnyitás") }
        }
    }
}

/** Title + optional due date; the record link is fixed by the caller. */
@Composable
fun TaskDialog(
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (title: String, dueDate: String?) -> Unit,
) {
    var title by rememberSaveable { mutableStateOf("") }
    var due by rememberSaveable { mutableStateOf("") }
    DialogShell(
        title = "Új feladat",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Mentés",
                enabled = !busy && title.isNotBlank(),
                onClick = { onConfirm(title.trim(), due.trim().takeIf { it.isNotBlank() }) },
            )
        },
    ) {
        AutoCrmTextField(
            value = title,
            onValueChange = { title = it },
            label = "Cím *",
            modifier = Modifier.fillMaxWidth(),
        )
        DateField(
            value = due,
            onValueChange = { due = it },
            label = "Határidő",
            modifier = Modifier.fillMaxWidth(),
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}
