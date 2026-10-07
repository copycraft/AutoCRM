package hu.autotherm.autocrm.ui.orders

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.OptIn
import androidx.camera.core.CameraSelector
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import java.util.concurrent.Executors
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * The cooling unit's serial number (0049): read off the barcode on its plate with the
 * camera, or typed. Saved straight to the order's spec.
 */
@Composable
fun CoolingSerialRow(orderId: Long, current: String?, canEdit: Boolean, onSaved: () -> Unit) {
    val context = LocalContext.current
    val app = context.applicationContext as AutoCrmApp
    val scope = rememberCoroutineScope()
    var editing by remember { mutableStateOf(false) }
    var scanning by remember { mutableStateOf(false) }
    var draft by remember(current) { mutableStateOf(current.orEmpty()) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    fun save(value: String) {
        busy = true
        error = null
        scope.launch {
            try {
                app.api.setCoolingSerial(orderId, value.trim().ifBlank { null })
                editing = false
                onSaved()
            } catch (e: CancellationException) {
                throw e
            } catch (e: ApiException) {
                error = when (e) {
                    is ApiException.Network -> "Nincs kapcsolat."
                    is ApiException.Rule -> e.detail ?: "Érvénytelen."
                    else -> e.message ?: "Nem sikerült menteni."
                }
            } finally {
                busy = false
            }
        }
    }

    Column(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text("Hűtőgép gyári száma", style = MaterialTheme.typography.labelMedium, color = Steel500)
                Text(current ?: "—", style = MonoSmall)
            }
            if (canEdit) {
                TextButton(onClick = { scanning = true }, enabled = !busy) {
                    Icon(Icons.Filled.QrCodeScanner, contentDescription = null)
                    Text(" Beolvasás")
                }
                TextButton(onClick = { editing = true }, enabled = !busy) { Text("Kézzel") }
            }
        }
        error?.let { Text(it, color = Signal, style = MaterialTheme.typography.labelMedium) }
    }

    if (editing) {
        AlertDialog(
            onDismissRequest = { if (!busy) editing = false },
            title = { Text("Hűtőgép gyári száma") },
            text = {
                OutlinedTextField(
                    value = draft,
                    onValueChange = { draft = it.take(100) },
                    singleLine = true,
                    label = { Text("Gyári szám") },
                )
            },
            confirmButton = { TextButton(enabled = !busy, onClick = { save(draft) }) { Text("Mentés") } },
            dismissButton = { TextButton(enabled = !busy, onClick = { editing = false }) { Text("Mégse") } },
        )
    }
    if (scanning) {
        BarcodeScanDialog(
            onFound = { value ->
                scanning = false
                draft = value
                editing = true // confirm before saving: a plate can carry several codes
            },
            onDismiss = { scanning = false },
        )
    }
}

/** Live camera that closes on the first barcode it reads (any format ML Kit knows). */
@Composable
fun BarcodeScanDialog(onFound: (String) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    var granted by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED,
        )
    }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it }
    LaunchedEffect(Unit) { if (!granted) launcher.launch(Manifest.permission.CAMERA) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Vonalkód beolvasása") },
        text = {
            Column {
                Text(
                    "Tartsd a kamerát a hűtőgép típustábláján lévő vonalkódra.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Steel500,
                )
                Box(Modifier.fillMaxWidth().height(260.dp).padding(top = 8.dp)) {
                    if (granted) {
                        BarcodeCamera(onFound)
                    } else {
                        Text("Kamera-engedély nélkül nem megy; írd be kézzel.", color = Signal)
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Mégse") } },
    )
}

@OptIn(ExperimentalGetImage::class)
@Composable
private fun BarcodeCamera(onFound: (String) -> Unit) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val executor = remember { Executors.newSingleThreadExecutor() }
    val scanner = remember { BarcodeScanning.getClient() }
    var done by remember { mutableStateOf(false) }
    val previewView = remember { PreviewView(context) }

    DisposableEffect(lifecycleOwner) {
        val providerFuture = ProcessCameraProvider.getInstance(context)
        var provider: ProcessCameraProvider? = null
        providerFuture.addListener({
            val p = providerFuture.get()
            provider = p
            val preview = Preview.Builder().build().also { it.setSurfaceProvider(previewView.surfaceProvider) }
            val analysis = ImageAnalysis.Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
            analysis.setAnalyzer(executor) { proxy ->
                val media = proxy.image
                if (media == null || done) {
                    proxy.close()
                    return@setAnalyzer
                }
                val input = InputImage.fromMediaImage(media, proxy.imageInfo.rotationDegrees)
                scanner.process(input)
                    .addOnSuccessListener { codes ->
                        val value = codes.firstNotNullOfOrNull { it.rawValue?.trim()?.takeIf(String::isNotEmpty) }
                        if (value != null && !done) {
                            done = true
                            ContextCompat.getMainExecutor(context).execute { onFound(value) }
                        }
                    }
                    .addOnCompleteListener { proxy.close() }
            }
            try {
                p.unbindAll()
                p.bindToLifecycle(lifecycleOwner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
            } catch (_: Exception) {
                // No back camera, or it is busy: the manual field remains.
            }
        }, ContextCompat.getMainExecutor(context))
        onDispose {
            provider?.unbindAll()
            scanner.close()
            executor.shutdown()
        }
    }
    AndroidView(factory = { previewView }, modifier = Modifier.fillMaxWidth().height(250.dp))
}
