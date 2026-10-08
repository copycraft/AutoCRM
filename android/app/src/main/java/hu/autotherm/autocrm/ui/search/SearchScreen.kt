package hu.autotherm.autocrm.ui.search

import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Business
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.automirrored.filled.TrendingUp
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.SearchResults
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** One tappable result: what to show and which screen it opens. */
data class SearchHit(val group: String, val title: String, val sub: String, val route: String)

/** Flattens the server's groups, in the same order as the web client's search box. */
fun searchHits(r: SearchResults): List<SearchHit> {
    fun join(vararg parts: String?) = parts.filter { !it.isNullOrBlank() }.joinToString(" · ")
    return buildList {
        r.orders.forEach {
            add(SearchHit("Munkák", "#${it.number} · ${it.plate ?: "—"}", join(it.title, it.stageLabel), "order/${it.id}"))
        }
        r.partners.forEach {
            add(
                SearchHit(
                    "Partnerek", it.name,
                    join(if (it.kind == "business") "Üzleti" else "Magán", it.city), "partner/${it.id}",
                ),
            )
        }
        r.leads.forEach {
            add(SearchHit("Leadek", it.title, join(it.contactName, it.stageLabel), "lead/${it.id}"))
        }
        r.contacts.forEach {
            // A contact has no screen of its own: it opens the company it works for.
            add(SearchHit("Kapcsolattartók", it.name, join(it.partnerName, it.phone, it.email), "partner/${it.partnerId}"))
        }
        r.emails.forEach {
            add(SearchHit("E-mailek", it.subject, it.toAddress, "email/${it.id}"))
        }
        r.employees.forEach {
            add(SearchHit("Munkatársak", it.fullName, join(it.companyPhone, it.email, if (it.archived) "Kilépett" else null), "hr"))
        }
    }
}

/**
 * Search from the phone: every word typed must match, accents and spacing are forgiven, and
 * a plate or phone number in any spelling finds its record. It asks the server each time, so
 * unlike the lists it does not work offline; it says so instead of showing nothing.
 */
class SearchViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val query: String = "",
        val loading: Boolean = false,
        val hits: List<SearchHit> = emptyList(),
        val offline: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private var job: Job? = null

    fun setQuery(q: String) {
        _state.value = _state.value.copy(query = q)
        job?.cancel()
        if (q.trim().length < 2) {
            _state.value = _state.value.copy(loading = false, hits = emptyList(), offline = false, error = null)
            return
        }
        job = viewModelScope.launch {
            delay(300) // typing, not searching, until the thumb pauses
            _state.value = _state.value.copy(loading = true, offline = false, error = null)
            try {
                _state.value = _state.value.copy(loading = false, hits = searchHits(api.search(q.trim())))
            } catch (e: ApiException.Network) {
                _state.value = _state.value.copy(loading = false, hits = emptyList(), offline = true)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun SearchScreen(viewModel: SearchViewModel, onMenu: () -> Unit, onOpen: (String) -> Unit) {
    val state by viewModel.state.collectAsState()
    // The keyboard is up on arrival: this screen is only ever opened to type.
    val focus = androidx.compose.runtime.remember { androidx.compose.ui.focus.FocusRequester() }
    androidx.compose.runtime.LaunchedEffect(Unit) {
        if (state.query.isBlank()) runCatching { focus.requestFocus() }
    }
    // The last few searches that led somewhere, newest first, one tap to repeat.
    val context = androidx.compose.ui.platform.LocalContext.current
    val prefs = androidx.compose.runtime.remember {
        context.getSharedPreferences("search", android.content.Context.MODE_PRIVATE)
    }
    var recent by androidx.compose.runtime.remember {
        androidx.compose.runtime.mutableStateOf(
            prefs.getString("recent", null).orEmpty().lines().filter { it.isNotBlank() },
        )
    }
    val open: (String) -> Unit = { route ->
        val q = state.query.trim()
        if (q.length >= 2) {
            recent = (listOf(q) + recent.filter { !it.equals(q, ignoreCase = true) }).take(8)
            prefs.edit().putString("recent", recent.joinToString("\n")).apply()
        }
        onOpen(route)
    }
    // Searched in a dead spot: asked again by itself once there is a network.
    val online by hu.autotherm.autocrm.data.net.NetworkState.online.collectAsState()
    androidx.compose.runtime.LaunchedEffect(online) {
        if (online && state.offline) viewModel.setQuery(state.query)
    }
    Scaffold(topBar = { ScreenTopBar(title = "Keresés", onMenu = onMenu) }) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            SearchField(
                value = state.query,
                onValueChange = viewModel::setQuery,
                label = "Rendszám, név, telefonszám, megrendelés…",
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
            when {
                state.query.trim().length < 2 && recent.isNotEmpty() -> Column(
                    Modifier.padding(top = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    hu.autotherm.autocrm.ui.common.FieldLabel("Legutóbbi keresések")
                    androidx.compose.foundation.layout.FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        recent.forEach { q ->
                            androidx.compose.material3.AssistChip(
                                onClick = { viewModel.setQuery(q) },
                                label = { Text(q) },
                            )
                        }
                    }
                }
                state.query.trim().length < 2 ->
                    EmptyState("Írjon be legalább 2 karaktert. Több szó esetén mindegyiknek egyeznie kell.")
                state.loading -> ListSkeleton()
                state.offline -> EmptyState("A keresés hálózatot igényel. Csatlakozzon, és próbálja újra.")
                state.error != null -> ErrorState(state.error!!, onRetry = { viewModel.setQuery(state.query) })
                state.hits.isEmpty() -> EmptyState("Nincs találat.")
                else -> {
                    // Group headings between runs of the same group.
                    val rows = buildList {
                        var last: String? = null
                        for (h in state.hits) {
                            if (h.group != last) add(h.group to null)
                            add(h.group to h)
                            last = h.group
                        }
                    }
                    LazyColumn(
                        modifier = Modifier.fillMaxSize().padding(top = 8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(rows) { (group, hit) ->
                            if (hit == null) {
                                SectionTitle(group)
                            } else {
                                // An icon per kind: a job, a person and a letter look different
                                // at a glance in a mixed result list.
                                hu.autotherm.autocrm.ui.common.ListRow(
                                    title = hit.title,
                                    subtitle = hit.sub.ifBlank { null },
                                    leading = {
                                        androidx.compose.material3.Icon(
                                            when (hit.group) {
                                                "Munkák" -> androidx.compose.material.icons.Icons.Filled.Build
                                                "Partnerek" -> androidx.compose.material.icons.Icons.Filled.Business
                                                "Leadek" -> androidx.compose.material.icons.Icons.AutoMirrored.Filled.TrendingUp
                                                "E-mailek" -> androidx.compose.material.icons.Icons.Filled.Email
                                                else -> androidx.compose.material.icons.Icons.Filled.Person
                                            },
                                            contentDescription = null,
                                            tint = hu.autotherm.autocrm.ui.theme.Cold,
                                        )
                                    },
                                    onClick = { open(hit.route) },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
