package hu.autotherm.autocrm.ui.queue

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.upload.UploadQueue
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class QueueViewModel(private val queue: UploadQueue) : ViewModel() {

    val items: StateFlow<List<PendingUpload>> =
        queue.queue.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun retryAll() = viewModelScope.launch { queue.retryAll() }
    fun retry(id: Long) = viewModelScope.launch { queue.retry(id) }
    fun discard(id: Long) = viewModelScope.launch { queue.discard(id) }
}

/**
 * What is still on the phone.
 *
 * This screen exists so that "did my photos go up?" has an answer other than opening the web
 * app on a laptop. Blocked rows are shown with their server-supplied reason and two buttons:
 * try again, or throw it away. Nothing here deletes a photo without the fitter saying so —
 * the failure mode this whole subsystem is built against is a photo quietly disappearing.
 */
@Composable
fun QueueScreen(viewModel: QueueViewModel) {
    val items by viewModel.items.collectAsState()
    val blocked = items.count { it.state == PendingUpload.STATE_BLOCKED }

    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            SectionTitle("Feltöltési sor")
            if (items.isNotEmpty()) {
                OutlinedButton(onClick = viewModel::retryAll) { Text("Újrapróbálás") }
            }
        }

        if (blocked > 0) {
            Text(
                "$blocked fotó nem ment fel. Egyik sem veszett el — itt vannak a telefonon.",
                style = MaterialTheme.typography.bodyLarge,
                color = Signal,
            )
        }

        if (items.isEmpty()) {
            EmptyState("Minden fotó feltöltve.")
            return@Column
        }

        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(items, key = { it.id }) { row ->
                Card {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(
                            "${row.orderNumber} · ${CapturePrefs.label(row.category)}",
                            style = MaterialTheme.typography.titleMedium,
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

private val timeFormat = SimpleDateFormat("MM.dd. HH:mm", Locale("hu", "HU"))

private fun formatTime(epochMillis: Long): String = timeFormat.format(Date(epochMillis))
