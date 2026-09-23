package hu.autotherm.autocrm.ui.queue

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
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.upload.UploadQueue
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatTime
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class QueueViewModel(
    private val queue: UploadQueue,
    private val api: AutoCrmApi,
    private val sessionStore: SessionStore,
) : ViewModel() {

    val items: StateFlow<List<PendingUpload>> =
        queue.queue.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun retryAll() = viewModelScope.launch { queue.retryAll() }
    fun retry(id: Long) = viewModelScope.launch { queue.retry(id) }
    fun discard(id: Long) = viewModelScope.launch { queue.discard(id) }

    /**
     * Signs out. The server is told best-effort; the local session is cleared regardless —
     * a dead token must never brick the app into an ErrorState loop with no way back to
     * the login screen. The queue survives: photos belong to the job, not the session.
     */
    fun logout(onDone: () -> Unit = {}) = viewModelScope.launch {
        runCatching { api.logout() }
        sessionStore.clear()
        onDone()
    }
}

/**
 * What is still on the phone.
 *
 * This screen exists so that "did my photos go up?" has an answer other than opening the web
 * app on a laptop. Blocked rows are shown with their server-supplied reason and two buttons:
 * try again, or throw it away. Nothing here deletes a photo without the fitter saying so —
 * the failure mode this whole subsystem is built against is a photo quietly disappearing.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QueueScreen(viewModel: QueueViewModel, onMenu: () -> Unit) {
    val items by viewModel.items.collectAsState()
    val blocked = items.count { it.state == PendingUpload.STATE_BLOCKED }
    val waiting = items.size - blocked

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Feltöltési sor",
                subtitle = when {
                    items.isEmpty() -> null
                    blocked > 0 -> "$blocked hiba · $waiting várakozik"
                    else -> "$waiting várakozik"
                },
                onMenu = onMenu,
                actions = {
                    if (items.isNotEmpty()) {
                        TextButton(onClick = viewModel::retryAll) { Text("Újra") }
                    }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (blocked > 0) {
                Card {
                    Text(
                        "$blocked fotó nem ment fel. Egyik sem veszett el — itt vannak a telefonon.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = Signal,
                    )
                }
            }

            if (items.isEmpty()) {
                EmptyState("Minden fotó feltöltve.")
            } else {
                LazyColumn(
                    modifier = Modifier.weight(1f, fill = false),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(items, key = { it.id }) { row ->
                        Card(Modifier.animateItem()) {
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                Text(
                                    "${row.orderNumber} · ${CapturePrefs.label(row.category)}",
                                    style = MaterialTheme.typography.titleMedium,
                                    modifier = Modifier.weight(1f, fill = false),
                                )
                                when (row.state) {
                                    PendingUpload.STATE_BLOCKED -> StatusBadge("Hiba", Tone.Signal)
                                    PendingUpload.STATE_UPLOADING -> StatusBadge("Feltöltés", Tone.Cold)
                                    PendingUpload.STATE_DONE -> StatusBadge("Kész", Tone.Done)
                                    else -> StatusBadge("Várakozik", Tone.Steel)
                                }
                            }
                            Text(
                                "${formatTime(row.createdAt)} · ${row.byteSize / 1024} kB · ${row.attempts}. próbálkozás",
                                style = MonoSmall,
                                color = Steel500,
                            )
                            row.lastError?.let {
                                Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal)
                            }
                            if (row.state == PendingUpload.STATE_BLOCKED) {
                                if (row.category == "inspection") {
                                    // Owned by the inspection sync, not by this screen:
                                    // discarding it would orphan an inspection draft's photo.
                                    Text(
                                        "Átvételi fotó – az átvételnél kezelendő.",
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Steel500,
                                    )
                                } else {
                                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                        TextButton(onClick = { viewModel.retry(row.id) }) { Text("Újra") }
                                        TextButton(onClick = { viewModel.discard(row.id) }) { Text("Eldobás") }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // The only sign-out in the app. Two taps: a fitter must not lose a session to a
            // pocket tap, and an expired token must never leave them with no way back to login.
            var confirmLogout by rememberSaveable { mutableStateOf(false) }
            OutlinedButton(
                onClick = { if (confirmLogout) viewModel.logout() else confirmLogout = true },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (confirmLogout) "Biztos? Koppints újra a kijelentkezéshez" else "Kijelentkezés")
            }
        }
    }
}
