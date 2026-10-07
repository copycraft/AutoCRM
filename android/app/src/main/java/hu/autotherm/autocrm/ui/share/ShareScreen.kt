package hu.autotherm.autocrm.ui.share

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
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
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.inspection.uploadDocument
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import java.io.File
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** One file another app handed over ("Megosztás → AutoCRM"). */
data class SharedFile(val uri: Uri, val mime: String?)

/** Where it goes. */
data class ShareTarget(val kind: String, val id: Long, val title: String, val subtitle: String?)

/**
 * Files shared from another app (0049): a customer's photo from a chat, a PDF from the
 * mail app. They are filed as documents of the chosen order or lead, never as evidence
 * photos — the photo evidence chain only takes pictures from the app's own camera.
 */
class ShareViewModel(private val app: AutoCrmApp) : ViewModel() {

    data class State(
        val kind: String = "order",
        val query: String = "",
        val results: List<ShareTarget> = emptyList(),
        val searching: Boolean = false,
        val uploading: Boolean = false,
        val done: Int = 0,
        val failed: Int = 0,
        val finished: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()
    private var searchJob: Job? = null

    fun setKind(kind: String) {
        _state.value = _state.value.copy(kind = kind, results = emptyList())
        search(_state.value.query)
    }

    fun search(query: String) {
        _state.value = _state.value.copy(query = query, searching = true, error = null)
        searchJob?.cancel()
        searchJob = viewModelScope.launch {
            delay(250)
            try {
                val q = query.trim().ifBlank { null }
                val results = if (_state.value.kind == "order") {
                    app.api.pickerOrders(q).map {
                        ShareTarget("order", it.id, "${it.number} · ${it.title}", listOfNotNull(it.partnerName, it.vehiclePlate).joinToString(" · "))
                    }
                } else {
                    app.api.leads(q, openOnly = q == null).map {
                        ShareTarget("lead", it.id, it.title, listOfNotNull(it.partnerName ?: it.contactName, it.stageLabel).joinToString(" · "))
                    }
                }
                _state.value = _state.value.copy(results = results, searching = false)
            } catch (e: CancellationException) {
                throw e
            } catch (e: ApiException) {
                _state.value = _state.value.copy(searching = false, error = if (e is ApiException.Network) "Nincs kapcsolat." else e.message)
            }
        }
    }

    fun upload(context: Context, files: List<SharedFile>, target: ShareTarget) {
        if (_state.value.uploading) return
        _state.value = _state.value.copy(uploading = true, done = 0, failed = 0, error = null)
        viewModelScope.launch {
            var done = 0
            var failed = 0
            for (f in files) {
                try {
                    val (copy, name) = withContext(Dispatchers.IO) { copyToCache(context, f.uri) }
                    try {
                        val id = uploadDocument(
                            app,
                            orderId = if (target.kind == "order") target.id else 0,
                            file = copy,
                            filename = name,
                            contentType = f.mime?.takeIf { it.contains('/') && it != "*/*" } ?: guessMime(name),
                            kind = "other",
                            leadId = if (target.kind == "lead") target.id else null,
                        )
                        if (id != null) done++ else failed++
                    } finally {
                        copy.delete()
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    failed++
                }
                _state.value = _state.value.copy(done = done, failed = failed)
            }
            _state.value = _state.value.copy(uploading = false, finished = true)
        }
    }
}

private fun guessMime(name: String): String = when (name.substringAfterLast('.', "").lowercase()) {
    "jpg", "jpeg" -> "image/jpeg"
    "png" -> "image/png"
    "webp" -> "image/webp"
    "heic", "heif" -> "image/heic"
    "pdf" -> "application/pdf"
    "mp4" -> "video/mp4"
    "m4a" -> "audio/mp4"
    "dwg" -> "image/vnd.dwg"
    "dxf" -> "image/vnd.dxf"
    else -> "application/octet-stream"
}

/** The shared content as a temporary file, with the name the sending app gave it. */
private fun copyToCache(context: Context, uri: Uri): Pair<File, String> {
    var name = "megosztott_fajl"
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
        if (c.moveToFirst()) c.getString(0)?.takeIf { it.isNotBlank() }?.let { name = it }
    }
    val safe = name.replace(Regex("[\\\\/:*?\"<>|]"), "_").take(150)
    val out = File(context.cacheDir, "share-${System.nanoTime()}-$safe")
    context.contentResolver.openInputStream(uri)?.use { input ->
        out.outputStream().use { input.copyTo(it) }
    } ?: throw IllegalStateException("unreadable share")
    return out to safe
}

@Composable
fun ShareScreen(
    files: List<SharedFile>,
    viewModel: ShareViewModel,
    onDone: (ShareTarget?) -> Unit,
) {
    val context = LocalContext.current
    val state by viewModel.state.collectAsState()
    var chosen by remember { mutableStateOf<ShareTarget?>(null) }
    LaunchedEffect(Unit) { viewModel.search("") }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Megosztás az AutoCRM-be",
                subtitle = "${files.size} fájl",
                onBack = { onDone(null) },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (state.finished) {
                Card {
                    Text(
                        "${state.done} fájl feltöltve" + if (state.failed > 0) ", ${state.failed} sikertelen." else ".",
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(
                        "A dokumentumok között találod.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Steel500,
                    )
                }
                PrimaryButton(text = "Megnyitás", onClick = { onDone(chosen) }, modifier = Modifier.fillMaxWidth())
                return@Column
            }
            Text(
                "Dokumentumként kerül a kiválasztott munkához vagy leadhez (nem bizonyítékfotó).",
                style = MaterialTheme.typography.bodyMedium,
                color = Steel500,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(selected = state.kind == "order", onClick = { viewModel.setKind("order") }, label = { Text("Munka") })
                FilterChip(selected = state.kind == "lead", onClick = { viewModel.setKind("lead") }, label = { Text("Lead") })
            }
            AutoCrmTextField(
                value = state.query,
                onValueChange = viewModel::search,
                label = if (state.kind == "order") "Munkaszám, rendszám, partner…" else "Lead neve, partner…",
                modifier = Modifier.fillMaxWidth(),
            )
            state.error?.let { Text(it, color = Signal) }
            if (state.uploading) {
                Text("Feltöltés: ${state.done + state.failed} / ${files.size}", style = MaterialTheme.typography.bodyLarge)
            }
            if (!state.searching && state.results.isEmpty()) {
                EmptyState("Nincs találat.")
            }
            LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(state.results, key = { "${it.kind}-${it.id}" }) { target ->
                    Card(
                        onClick = {
                            if (!state.uploading) {
                                chosen = target
                                viewModel.upload(context, files, target)
                            }
                        },
                    ) {
                        Text(target.title, style = MaterialTheme.typography.bodyLarge)
                        target.subtitle?.takeIf { it.isNotBlank() }?.let {
                            Text(it, style = MaterialTheme.typography.labelMedium, color = Steel500)
                        }
                    }
                }
            }
        }
    }
}
