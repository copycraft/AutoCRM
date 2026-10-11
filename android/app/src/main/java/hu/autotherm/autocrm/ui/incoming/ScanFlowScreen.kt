package hu.autotherm.autocrm.ui.incoming

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.PhotoCamera
import androidx.compose.material.icons.filled.Upload
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import coil.compose.AsyncImage
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.Toasts
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Photograph → one PDF → the incoming-invoices upload, in the order the pages were
 * shot. The system camera takes the pictures (as everywhere outside the handover
 * walkaround); this screen owns the page strip, the PDF binding ([ScanPdf]) and the
 * retry of a PDF that met a dead signal.
 *
 * A PDF that failed to upload stays under `pending/scans/` and is offered again — on
 * this screen and as a count on the list. The page JPEGs are deleted once the PDF is
 * bound: the PDF is the artifact, the pages were scaffolding.
 */
class ScanFlowViewModel(
    private val api: AutoCrmApi,
    private val context: Context,
) : ViewModel() {

    data class State(
        val pages: List<String> = emptyList(),
        val working: Boolean = false,
        val error: String? = null,
        val pendingPdfs: List<String> = emptyList(),
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    private val scansDir: File
        get() = File(File(context.filesDir, "pending"), "scans").apply { mkdirs() }

    fun loadPending() {
        viewModelScope.launch {
            val pdfs = withContext(Dispatchers.IO) {
                scansDir.listFiles { file -> file.extension.lowercase() == "pdf" }
                    ?.sortedByDescending { it.lastModified() }
                    ?.map { it.absolutePath }
                    .orEmpty()
            }
            _state.value = _state.value.copy(pendingPdfs = pdfs)
        }
    }

    /** A file the system camera writes the next page into, with its content URI. */
    fun newPageTarget(): Pair<File, Uri> {
        val file = File(
            File(context.filesDir, "pending").apply { mkdirs() },
            "scan-${System.currentTimeMillis()}-p${_state.value.pages.size + 1}.jpg",
        )
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        return file to uri
    }

    fun cameraResult(saved: Boolean, file: File) {
        if (!saved || file.length() == 0L) {
            file.delete()
            return
        }
        _state.value = _state.value.copy(pages = _state.value.pages + file.absolutePath)
    }

    fun cameraUnavailable() {
        _state.value = _state.value.copy(error = "Nincs elérhető kameraalkalmazás ezen a telefonon.")
    }

    fun removePage(path: String) {
        viewModelScope.launch(Dispatchers.IO) {
            File(path).delete()
        }
        _state.value = _state.value.copy(pages = _state.value.pages - path)
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }

    fun buildAndUpload(onUploaded: (Long) -> Unit) {
        val pages = _state.value.pages.map(::File)
        if (pages.isEmpty() || _state.value.working) return
        viewModelScope.launch {
            _state.value = _state.value.copy(working = true, error = null)
            try {
                val pdf = withContext(Dispatchers.IO) {
                    val small = pages.mapIndexed { index, raw ->
                        val dst = File.createTempFile("oldal-$index-", ".jpg", context.cacheDir)
                        ScanPdf.downscaleForPdf(raw, dst)
                    }
                    try {
                        val out = File(scansDir, ScanPdf.scanFileName())
                        ScanPdf.buildPdfFromImages(small, out)
                        out
                    } finally {
                        small.forEach { it.delete() }
                    }
                }
                pages.forEach { it.delete() }
                _state.value = _state.value.copy(pages = emptyList())
                uploadPdf(pdf, onUploaded)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(working = false, error = describeError(e))
            }
        }
    }

    fun retryPending(path: String, onUploaded: (Long) -> Unit) {
        val pdf = File(path)
        if (!pdf.exists()) {
            loadPending()
            return
        }
        viewModelScope.launch {
            _state.value = _state.value.copy(working = true, error = null)
            try {
                uploadPdf(pdf, onUploaded)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(working = false, error = describeError(e))
            }
        }
    }

    fun deletePending(path: String) {
        File(path).delete()
        loadPending()
    }

    private suspend fun uploadPdf(pdf: File, onUploaded: (Long) -> Unit) {
        try {
            val bytes = withContext(Dispatchers.IO) { pdf.readBytes() }
            val invoice = api.uploadIncomingInvoice(bytes, pdf.name, "application/pdf")
            withContext(Dispatchers.IO) { pdf.delete() }
            _state.value = _state.value.copy(working = false)
            loadPending()
            Toasts.show("Feltöltve: ${invoice.supplierName}.")
            onUploaded(invoice.id)
        } catch (e: Throwable) {
            // The PDF stays for a later retry; the row was never created, so uploading
            // twice cannot duplicate it (the server dedupes by content hash anyway).
            _state.value = _state.value.copy(working = false, error = describeError(e))
            loadPending()
        }
    }
}

@Composable
fun ScanFlowScreen(
    viewModel: ScanFlowViewModel,
    onBack: () -> Unit,
    onUploaded: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    // Held across the launcher round trip: the result only says whether the camera app
    // saved, not where (same pattern as the order photo section).
    var cameraPath by rememberSaveable { mutableStateOf<String?>(null) }

    val cameraLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture(),
    ) { saved ->
        val path = cameraPath
        if (path != null) viewModel.cameraResult(saved, File(path))
        cameraPath = null
    }

    LaunchedEffect(Unit) { viewModel.loadPending() }

    fun shoot() {
        try {
            val (file, uri) = viewModel.newPageTarget()
            cameraPath = file.absolutePath
            cameraLauncher.launch(uri)
        } catch (e: Exception) {
            viewModel.cameraUnavailable()
        }
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "Szkennelés",
                subtitle = if (state.pages.isEmpty()) "Fotózd le az oldalakat sorban" else "${state.pages.size}. oldal",
                onBack = onBack,
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = ::shoot,
                icon = { Icon(Icons.Filled.PhotoCamera, contentDescription = null) },
                text = { Text(if (state.pages.isEmpty()) "Első oldal" else "Következő oldal") },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (state.error != null) {
                ErrorState(state.error!!) { viewModel.clearError() }
            }
            if (state.pages.isEmpty()) {
                EmptyState(
                    "Tartsd a számlát sík felületen, töltsd ki vele a képet.",
                    actionLabel = "Oldal fotózása",
                    onAction = ::shoot,
                )
            } else {
                SectionTitle(text = "Oldalak (${state.pages.size})")
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.pages, key = { it }) { path ->
                        Box {
                            AsyncImage(
                                model = File(path),
                                contentDescription = "Beszkennelt oldal",
                                contentScale = ContentScale.Crop,
                                modifier = Modifier.size(120.dp, 160.dp).clip(RoundedCornerShape(12.dp)),
                            )
                            IconButton(
                                onClick = { viewModel.removePage(path) },
                                modifier = Modifier.align(Alignment.TopEnd),
                            ) {
                                Icon(Icons.Filled.Close, contentDescription = "Oldal törlése")
                            }
                        }
                    }
                }
                PrimaryButton(
                    text = if (state.working) "Készül…" else "PDF és feltöltés (${state.pages.size} oldal)",
                    onClick = { viewModel.buildAndUpload(onUploaded) },
                    enabled = !state.working,
                )
                Text(
                    "Feltöltés után az oldalak fotói törlődnek; a PDF a számla adatlapjáról nyitható.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Steel500,
                )
            }
            if (state.pendingPdfs.isNotEmpty()) {
                SectionTitle(text = "Feltöltésre vár (${state.pendingPdfs.size})")
                state.pendingPdfs.forEach { path ->
                    val file = File(path)
                    Card {
                        Row(
                            Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(file.name, style = MaterialTheme.typography.titleSmall)
                                Text(
                                    "${file.length() / 1024} KB",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = Steel500,
                                )
                            }
                            IconButton(onClick = { viewModel.retryPending(path, onUploaded) }) {
                                Icon(Icons.Filled.Upload, contentDescription = "Újra feltöltés")
                            }
                            IconButton(onClick = { viewModel.deletePending(path) }) {
                                Icon(Icons.Filled.Close, contentDescription = "Törlés")
                            }
                        }
                    }
                }
            }
        }
    }
}
