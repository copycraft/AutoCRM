package hu.autotherm.autocrm.ui.reports

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.StalledOrder
import hu.autotherm.autocrm.data.api.WorkloadReport
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import java.time.LocalDate
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * The Monday-meeting numbers in pocket form: workshop load over the last 30 days
 * and the stalled list with links into the jobs. Full chart tables stay on the
 * desktop; a phone screen is for totals and "which jobs are stuck".
 */
class ReportsViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val workload: WorkloadReport? = null,
        val stalled: List<StalledOrder> = emptyList(),
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load() {
        viewModelScope.launch {
            val keepRows = _state.value.workload != null
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                val to = LocalDate.now()
                val from = to.minusDays(29)
                val workloadDeferred = async { api.workload(from.toString(), to.toString()) }
                val stalledDeferred = async { api.stalled() }
                _state.value = State(
                    loading = false,
                    workload = workloadDeferred.await(),
                    stalled = stalledDeferred.await().sortedByDescending { it.daysInStage },
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
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReportsScreen(
    viewModel: ReportsViewModel,
    onMenu: () -> Unit,
    onOpenOrder: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.load() }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Jelentések",
                subtitle = "Utolsó 30 nap",
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
            )
        },
    ) { padding ->
        when {
            state.loading -> DetailSkeleton(
                Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            state.error != null && state.workload == null ->
                ErrorState(state.error!!, Modifier.padding(padding)) { viewModel.load() }
            else -> {
                val days = state.workload?.days.orEmpty()
                val placed = days.sumOf { it.placed }
                val completed = days.sumOf { it.completed }
                val avg = if (days.isNotEmpty()) days.sumOf { it.inWorkshop }.toDouble() / days.size else 0.0
                PullToRefreshBox(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::load,
                    modifier = Modifier.padding(padding),
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item {
                            Row(
                                Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Card(Modifier.weight(1f)) {
                                    Text("$placed", style = MaterialTheme.typography.headlineMedium)
                                    Text("Beérkezett", style = MaterialTheme.typography.labelMedium, color = Steel500)
                                }
                                Card(Modifier.weight(1f)) {
                                    Text("$completed", style = MaterialTheme.typography.headlineMedium)
                                    Text("Elkészült", style = MaterialTheme.typography.labelMedium, color = Steel500)
                                }
                                Card(Modifier.weight(1f)) {
                                    Text(
                                        "%.1f".format(avg),
                                        style = MaterialTheme.typography.headlineMedium,
                                    )
                                    Text("Átlag bent", style = MaterialTheme.typography.labelMedium, color = Steel500)
                                }
                            }
                        }
                        item { SectionTitle("Beragadt munkák", count = state.stalled.size.takeIf { it > 0 }) }
                        if (state.stalled.isEmpty()) {
                            item { EmptyState("Nincs beragadt munka.") }
                        } else {
                            items(state.stalled, key = { it.orderId }) { s ->
                                Card(modifier = Modifier.animateItem(), onClick = { onOpenOrder(s.orderId) }) {
                                    Text(
                                        "#${s.number} · ${s.title}",
                                        style = MaterialTheme.typography.titleMedium,
                                    )
                                    Text(
                                        "${s.partnerName} · ${s.stageLabel} · ${s.daysInStage} napja",
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Steel500,
                                    )
                                    if (s.openBlockers > 0) {
                                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                            StatusBadge("${s.openBlockers} akadály", Tone.Signal)
                                        }
                                    }
                                }
                            }
                        }
                        if (state.error != null) {
                            item { ErrorState(state.error!!) { viewModel.load() } }
                        }
                    }
                }
            }
        }
    }
}
