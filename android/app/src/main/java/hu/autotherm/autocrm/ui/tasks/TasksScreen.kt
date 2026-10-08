package hu.autotherm.autocrm.ui.tasks

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.background
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
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = { toggle(task) })
                load()
            }
        }
    }

    fun delete(task: Task) {
        viewModelScope.launch {
            try {
                api.deleteTask(task.id)
                _state.value = _state.value.copy(tasks = _state.value.tasks.filter { it.id != task.id })
                hu.autotherm.autocrm.ui.common.Toasts.show("Feladat törölve")
            } catch (e: Throwable) {
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = { delete(task) })
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
    val topScroll = hu.autotherm.autocrm.ui.common.rememberTopScrollableListState("tasks")
    // Back from the job a task belongs to: what was done there shows here.
    hu.autotherm.autocrm.ui.common.RefreshOnReturn { viewModel.load() }
    LaunchedEffect(Unit) { viewModel.load() }

    val open = state.tasks.filter { !it.isDone }
    val done = state.tasks.filter { it.isDone }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Feladatok",
                subtitle = if (open.isNotEmpty()) {
                    val today = java.time.LocalDate.now().toString()
                    val late = open.count { it.dueDate != null && it.dueDate < today }
                    "${open.size} nyitott" + if (late > 0) " · $late lejárt" else ""
                } else {
                    null
                },
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
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::load,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        state = topScroll,
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        // Grouped by what needs doing first; a short list needs no headings.
                        val groups = taskGroups(open)
                        groups.forEach { (heading, tasks) ->
                            if (groups.size > 1 || heading == "Lejárt") {
                                item(key = "h-$heading") { SectionTitle(heading, count = tasks.size) }
                            }
                            items(tasks, key = { it.id }) { task ->
                                SwipeToDone(onDone = { viewModel.toggle(task) }, modifier = Modifier.animateItem()) {
                                    TaskRow(
                                        task = task,
                                        onToggle = { viewModel.toggle(task) },
                                        onDelete = { viewModel.delete(task) },
                                        onOpen = { onOpenTask(task.entityType, task.entityId) },
                                    )
                                }
                            }
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
    // The whole row opens the record; the checkbox and the bin keep their own taps.
    Card(modifier, onClick = onOpen) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
            Checkbox(
                checked = task.isDone,
                onCheckedChange = {
                    haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove)
                    onToggle()
                },
            )
            Column(Modifier.weight(1f)) {
                Text(
                    task.title,
                    style = MaterialTheme.typography.bodyLarge.copy(
                        textDecoration = if (task.isDone) TextDecoration.LineThrough else null,
                    ),
                    color = if (task.isDone) Steel500 else MaterialTheme.colorScheme.onSurface,
                )
                val overdue = !task.isDone && task.dueDate?.let {
                    runCatching { java.time.LocalDate.parse(it).isBefore(java.time.LocalDate.now()) }.getOrDefault(false)
                } == true
                val meta = listOfNotNull(
                    task.assignedName,
                    task.dueDate?.let { hu.autotherm.autocrm.util.relativeDay(it)?.let { d -> (if (overdue) "lejárt: " else "határidő: ") + d } },
                ).joinToString(" · ")
                if (meta.isNotBlank()) {
                    Text(
                        meta,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (overdue) hu.autotherm.autocrm.ui.theme.Signal else Steel500,
                    )
                }
            }
            if (onDelete != null) {
                IconButton(onClick = onDelete) {
                    Icon(Icons.Filled.Delete, contentDescription = "Törlés", tint = Steel500)
                }
            }
        }
    }
}

/** Title, optional due date and assignee; the record link is fixed by the caller. */
@Composable
fun TaskDialog(
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (title: String, dueDate: String?, assignedTo: Long?) -> Unit,
) {
    var title by rememberSaveable { mutableStateOf("") }
    var due by rememberSaveable { mutableStateOf("") }
    var assignee by rememberSaveable { mutableStateOf<Long?>(null) }
    DialogShell(
        title = "Új feladat",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Mentés",
                enabled = !busy && title.isNotBlank(),
                onClick = { onConfirm(title.trim(), due.trim().takeIf { it.isNotBlank() }, assignee) },
            )
        },
    ) {
        AutoCrmTextField(
            value = title,
            onValueChange = { title = it },
            label = "Cím *",
            modifier = Modifier.fillMaxWidth(),
        )
        hu.autotherm.autocrm.ui.common.QuickPicks(
            options = listOf("Visszahívni az ügyfelet", "Árajánlatot küldeni", "Alkatrészt rendelni", "Számlát kiállítani"),
            current = title,
            onPick = { title = it },
        )
        DateField(
            value = due,
            onValueChange = { due = it },
            label = "Határidő",
            modifier = Modifier.fillMaxWidth(),
        )
        // The usual deadlines, one tap each instead of a calendar.
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val today = java.time.LocalDate.now()
            listOf("Ma" to today, "Holnap" to today.plusDays(1), "1 hét" to today.plusWeeks(1)).forEach { (label, date) ->
                androidx.compose.material3.FilterChip(
                    selected = due == date.toString(),
                    onClick = { due = if (due == date.toString()) "" else date.toString() },
                    label = { Text(label) },
                )
            }
        }
        hu.autotherm.autocrm.ui.common.AssigneeField(
            selected = assignee,
            onSelect = { assignee = it },
            label = "Felelős (nem kötelező)",
            modifier = Modifier.fillMaxWidth(),
        )
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}

/**
 * Open tasks in the order they need attention: overdue, today, tomorrow, later (soonest
 * first), then those with no deadline. Empty groups are left out.
 */
internal fun taskGroups(
    open: List<Task>,
    today: java.time.LocalDate = java.time.LocalDate.now(),
): List<Pair<String, List<Task>>> {
    fun day(t: Task) = t.dueDate?.let { runCatching { java.time.LocalDate.parse(it) }.getOrNull() }
    val dated = open.filter { day(it) != null }.sortedBy { day(it) }
    return listOf(
        "Lejárt" to dated.filter { day(it)!!.isBefore(today) },
        "Ma" to dated.filter { day(it) == today },
        "Holnap" to dated.filter { day(it) == today.plusDays(1) },
        "Később" to dated.filter { day(it)!!.isAfter(today.plusDays(1)) },
        "Nincs határidő" to open.filter { day(it) == null },
    ).filter { it.second.isNotEmpty() }
}

/** Swipe a row right to tick it off — the checkbox still works too. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun SwipeToDone(onDone: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val swipe = androidx.compose.material3.rememberSwipeToDismissBoxState(
        confirmValueChange = { value ->
            if (value == androidx.compose.material3.SwipeToDismissBoxValue.StartToEnd) onDone()
            false
        },
    )
    androidx.compose.material3.SwipeToDismissBox(
        state = swipe,
        enableDismissFromEndToStart = false,
        modifier = modifier,
        backgroundContent = {
            androidx.compose.foundation.layout.Box(
                Modifier.fillMaxSize()
                    .background(hu.autotherm.autocrm.ui.theme.Done.copy(alpha = 0.18f), androidx.compose.foundation.shape.RoundedCornerShape(16.dp))
                    .padding(horizontal = 20.dp),
                contentAlignment = androidx.compose.ui.Alignment.CenterStart,
            ) {
                Text("✓ Kész", style = MaterialTheme.typography.titleMedium, color = hu.autotherm.autocrm.ui.theme.Done)
            }
        },
    ) { content() }
}
