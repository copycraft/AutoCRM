package hu.autotherm.autocrm.ui.emails

import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.clickable
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.EmailMessage
import hu.autotherm.autocrm.data.api.EmailSummary
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDateTime
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * The correspondence log, read-only.
 *
 * `email_messages` is a permanent, per-order record of everything the system sent — nudges
 * to suppliers, stalled-order alerts, anything a person composed. Being able to answer
 * "did the paint shop actually get chased, and when" from the shop floor is the point;
 * composing a new one from a phone is not, and that stays on the desktop where a body of
 * text and an attachment picker belong.
 */
class EmailListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val emails: List<EmailSummary> = emptyList(),
        val error: String? = null,
        /** Searched by recipient or subject; the list used to show only the latest 50. */
        val query: String = "",
        val hasMore: Boolean = false,
        val loadingMore: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private var searchJob: kotlinx.coroutines.Job? = null

    fun setQuery(q: String) {
        _state.value = _state.value.copy(query = q)
        searchJob?.cancel()
        searchJob = viewModelScope.launch {
            kotlinx.coroutines.delay(300) // typing, not searching
            load()
        }
    }

    fun loadMore() {
        val s = _state.value
        if (!s.hasMore || s.loading || s.refreshing || s.loadingMore) return
        viewModelScope.launch {
            _state.value = _state.value.copy(loadingMore = true)
            try {
                val next = api.emails(limit = 50, offset = s.emails.size, query = s.query.trim().ifBlank { null })
                _state.value = _state.value.copy(
                    loadingMore = false,
                    emails = (_state.value.emails + next).distinctBy { it.id },
                    hasMore = next.size >= 50,
                )
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loadingMore = false)
                hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e), retry = ::loadMore)
            }
        }
    }

    fun load() {
        viewModelScope.launch {
            val keepRows = _state.value.emails.isNotEmpty()
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                val rows = api.emails(query = _state.value.query.trim().ifBlank { null })
                _state.value = _state.value.copy(
                    loading = false,
                    refreshing = false,
                    emails = rows,
                    hasMore = rows.size >= 50,
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

/** `EmailStatus` from the API, in the palette's language. */
private fun statusTone(status: String): Tone = when (status) {
    "sent" -> Tone.Done
    "failed", "needs_review" -> Tone.Signal
    "cancelled" -> Tone.Steel
    else -> Tone.Cold
}

private fun statusLabel(status: String): String = when (status) {
    "queued" -> "Várakozik"
    "sending" -> "Küldés"
    "sent" -> "Elküldve"
    "failed" -> "Sikertelen"
    "cancelled" -> "Visszavonva"
    "needs_review" -> "Ellenőrzendő"
    else -> status
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EmailListScreen(
    viewModel: EmailListViewModel,
    onOpen: (Long) -> Unit,
    onMenu: () -> Unit,
    canEdit: Boolean,
    onCompose: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.load() }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "E-mailek",
                subtitle = if (state.emails.isNotEmpty()) "${state.emails.size} levél" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
            )
        },
        floatingActionButton = {
            if (canEdit) {
                // Says what it makes, like every other list in the app.
                hu.autotherm.autocrm.ui.common.NewFab("Új levél", onCompose)
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            hu.autotherm.autocrm.ui.common.SearchField(
                value = state.query,
                onValueChange = viewModel::setQuery,
                label = "Címzett vagy tárgy…",
                modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
            )
            when {
                state.loading -> ListSkeleton()
                state.error != null && state.emails.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::load)
                state.emails.isEmpty() && state.query.isNotBlank() -> EmptyState(
                    "Nincs találat erre: „${state.query.trim()}”.",
                    actionLabel = "Keresés törlése",
                    onAction = { viewModel.setQuery("") },
                )
                state.emails.isEmpty() -> EmptyState("Nincs elküldött e-mail.")
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = viewModel::load,
                ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.emails, key = { it.id }) { email ->
                            Card(modifier = Modifier.animateItem(), onClick = { onOpen(email.id) }) {
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                    Text(
                                        email.subject,
                                        style = MaterialTheme.typography.titleMedium,
                                        modifier = Modifier.weight(1f, fill = false),
                                    )
                                    StatusBadge(statusLabel(email.status), statusTone(email.status))
                                }
                                Text(email.toAddress, style = MonoSmall, color = Steel500)
                                Text(
                                    listOfNotNull(
                                        hu.autotherm.autocrm.util.relativeTime(email.sentAt ?: email.queuedAt),
                                        email.sentByName ?: if (email.isAutomatic) "automatikus" else null,
                                    ).joinToString(" · "),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = Steel500,
                                )
                                // A letter that did not go is the one thing on this list to act on.
                                email.error?.let {
                                    Text(
                                        "Nem ment el: $it",
                                        style = MaterialTheme.typography.labelMedium,
                                        color = hu.autotherm.autocrm.ui.theme.Signal,
                                        maxLines = 2,
                                    )
                                }
                            }
                        }
                        if (state.hasMore) {
                            item(key = "more") {
                                LaunchedEffect(state.emails.size) { viewModel.loadMore() }
                                hu.autotherm.autocrm.ui.common.RefreshingBar()
                            }
                        }
                    }
                }
            }
            if (state.error != null && state.emails.isNotEmpty() && !state.refreshing) {
                ErrorState(state.error!!, onRetry = viewModel::load)
            }
        }
    }
}

class EmailDetailViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val email: EmailMessage? = null,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(id: Long) {
        viewModelScope.launch {
            val keepRows = _state.value.email != null
            _state.value = _state.value.copy(
                loading = !keepRows,
                refreshing = keepRows,
                error = null,
            )
            try {
                _state.value = State(loading = false, email = api.email(id))
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
fun EmailDetailScreen(
    emailId: Long,
    viewModel: EmailDetailViewModel,
    onOpenOrder: (Long) -> Unit,
    onBack: () -> Unit,
    /** Write to the same person again (a follow-up, or a resend after a failure). */
    onFollowUp: ((hu.autotherm.autocrm.data.api.EmailMessage) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(emailId) { viewModel.load(emailId) }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = state.email?.subject ?: "E-mail",
                onBack = onBack,
                refreshing = state.refreshing,
                onRefresh = { viewModel.load(emailId) },
                actions = {
                    val email = state.email
                    if (onFollowUp != null && email != null) {
                        androidx.compose.material3.IconButton(onClick = { onFollowUp(email) }) {
                            androidx.compose.material3.Icon(
                                androidx.compose.material.icons.Icons.AutoMirrored.Filled.Reply,
                                contentDescription = "Új levél neki",
                            )
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
            state.error != null && state.email == null ->
                ErrorState(state.error!!, Modifier.padding(padding)) { viewModel.load(emailId) }
            state.email == null ->
                ErrorState("Nem található.", Modifier.padding(padding)) { viewModel.load(emailId) }
            else -> {
                val email = state.email!!
                hu.autotherm.autocrm.ui.common.AppPullToRefresh(
                    isRefreshing = state.refreshing,
                    onRefresh = { viewModel.load(emailId) },
                    modifier = Modifier.padding(padding),
                ) {
                    LazyColumn(
                        Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item {
                            Card {
                                Text(email.subject, style = MaterialTheme.typography.headlineMedium)
                                StatusBadge(statusLabel(email.status), statusTone(email.status))
                            }
                        }
                        item {
                            Card {
                                Info("Címzett", email.toAddress, mono = true)
                                if (email.cc.isNotEmpty()) Info("Másolat", email.cc.joinToString(", "), mono = true)
                                Info("Feladó", email.fromAddress, mono = true)
                                Info("Sorba állítva", formatDateTime(email.queuedAt))
                                Info("Elküldve", formatDateTime(email.sentAt))
                                Info("Kísérletek", email.attempts.toString(), mono = true)
                                email.error?.let { Info("Hiba", it) }
                                email.orderId?.let { orderId ->
                                    Text(
                                        "Megrendelés megnyitása",
                                        style = MaterialTheme.typography.bodyLarge,
                                        modifier = Modifier
                                            .padding(top = 4.dp)
                                            .clickable { onOpenOrder(orderId) },
                                    )
                                }
                            }
                        }
                        item {
                            Card {
                                SectionTitle("Tartalom")
                                // The plain-text body, not the HTML: a phone screen is the wrong
                                // place for a rendered email, and the text is what was actually sent.
                                Text(email.bodyText, style = MaterialTheme.typography.bodyLarge)
                            }
                        }
                        if (email.attachments.isNotEmpty()) {
                            item {
                                Card {
                                    SectionTitle("Mellékletek", count = email.attachments.size)
                                    // Tap to open: the quote PDF that went out is one tap away.
                                    val uri = androidx.compose.ui.platform.LocalUriHandler.current
                                    val api = (androidx.compose.ui.platform.LocalContext.current.applicationContext as hu.autotherm.autocrm.AutoCrmApp).api
                                    val scope = androidx.compose.runtime.rememberCoroutineScope()
                                    email.attachments.forEach { attachment ->
                                        hu.autotherm.autocrm.ui.common.ListRow(
                                            title = attachment.filename ?: "#${attachment.documentId}",
                                            subtitle = attachment.byteSize?.let { b ->
                                                if (b >= 1_048_576) String.format(java.util.Locale("hu", "HU"), "%.1f MB", b / 1_048_576.0) else "${(b / 1024).coerceAtLeast(1)} kB"
                                            },
                                            leading = {
                                                androidx.compose.material3.Icon(
                                                    androidx.compose.material.icons.Icons.Filled.AttachFile,
                                                    contentDescription = null,
                                                    tint = Steel500,
                                                )
                                            },
                                            onClick = {
                                                scope.launch {
                                                    try {
                                                        uri.openUri(api.documentDownload(attachment.documentId).url)
                                                    } catch (e: Throwable) {
                                                        if (e is kotlinx.coroutines.CancellationException) throw e
                                                        hu.autotherm.autocrm.ui.common.Toasts.error(describeError(e))
                                                    }
                                                }
                                            },
                                        )
                                    }
                                }
                            }
                        }
                        if (state.error != null) {
                            item {
                                ErrorState(state.error!!) { viewModel.load(emailId) }
                            }
                        }
                    }
                }
            }
        }
    }
}
