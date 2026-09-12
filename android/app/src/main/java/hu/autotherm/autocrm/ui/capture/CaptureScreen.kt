package hu.autotherm.autocrm.ui.capture

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.ui.theme.Steel900
import hu.autotherm.autocrm.ui.theme.Surface
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine

/**
 * The screen the app exists for.
 *
 * Tap budget, which the viability review made the design constraint: launch (0, the app
 * opens here), shutter (1). The order is already selected because it is sticky, the
 * category is already selected because it is sticky, and neither costs a tap until the
 * fitter wants to change one. Everything else on this screen is a way of changing those two
 * things or seeing what is queued.
 *
 * The one deliberate interruption is the intake banner. An intake photo can never be
 * deleted or re-filed — `CHECK (category <> 'intake' OR immutable)` plus a trigger that
 * refuses UPDATE and DELETE — so the sticky category being `intake` is the single case
 * where silence would be dangerous.
 */
@Composable
fun CaptureScreen(
    viewModel: CaptureViewModel,
    onPickOrder: () -> Unit,
    onOpenQueue: () -> Unit,
) {
    val context = LocalContext.current
    val state by viewModel.state.collectAsState()
    var hasCamera by remember { mutableStateOf(hasCameraPermission(context)) }

    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> hasCamera = granted }

    LaunchedEffect(Unit) {
        if (!hasCamera) permissionLauncher.launch(Manifest.permission.CAMERA)
    }

    Column(Modifier.fillMaxSize().background(Steel900)) {
        CaptureHeader(
            state = state,
            onPickOrder = onPickOrder,
            onOpenQueue = onOpenQueue,
        )

        Box(Modifier.weight(1f).fillMaxWidth()) {
            when {
                !hasCamera -> PermissionNeeded { permissionLauncher.launch(Manifest.permission.CAMERA) }
                state.order == null -> NoOrderSelected(onPickOrder)
                else -> CameraPreview(viewModel)
            }
        }

        CategoryBar(
            selected = state.category,
            enabled = state.order != null && hasCamera,
            onSelect = viewModel::selectCategory,
        )

        ShutterBar(
            enabled = state.order != null && hasCamera && !state.capturing,
            shotsThisSession = state.shotsThisSession,
            onShutter = viewModel::capture,
        )
    }

    // Shown once per hour while the sticky category is intake.
    if (state.intakeWarning) {
        AlertDialog(
            onDismissRequest = viewModel::dismissIntakeWarning,
            title = { Text("Bevételi fotó") },
            text = {
                Text(
                    "A bevételi fotók véglegesek: nem törölhetők és nem sorolhatók át. " +
                        "Ezek bizonyítják a jármű átvételkori állapotát.",
                )
            },
            confirmButton = {
                TextButton(onClick = viewModel::acknowledgeIntake) { Text("Értem, bevétel") }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.selectCategory(CapturePrefs.CATEGORY_PRODUCTION) }) {
                    Text("Mégis gyártás")
                }
            },
        )
    }

    state.toast?.let { message ->
        LaunchedEffect(message) {
            kotlinx.coroutines.delay(2500)
            viewModel.clearToast()
        }
        Box(Modifier.fillMaxSize().padding(bottom = 160.dp), contentAlignment = Alignment.BottomCenter) {
            Box(
                Modifier
                    .background(Surface, RoundedCornerShape(8.dp))
                    .padding(horizontal = 16.dp, vertical = 10.dp),
            ) {
                Text(message, style = MaterialTheme.typography.bodyLarge, color = Steel900)
            }
        }
    }
}

@Composable
private fun CaptureHeader(
    state: CaptureViewModel.State,
    onPickOrder: () -> Unit,
    onOpenQueue: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .background(Steel900)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(
            Modifier
                .weight(1f)
                .clickable(onClick = onPickOrder),
        ) {
            val order = state.order
            if (order == null) {
                Text("Válassz megrendelést", style = MaterialTheme.typography.titleMedium, color = Surface)
            } else {
                Text(
                    "${order.number} · ${order.plate ?: "—"}",
                    style = MaterialTheme.typography.titleMedium,
                    color = Surface,
                )
                Text(
                    order.title,
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel200,
                    maxLines = 1,
                )
            }
        }
        // The queue badge is always visible, even at zero. A counter that only appears when
        // something is wrong is a counter nobody learns to read.
        Box(Modifier.clickable(onClick = onOpenQueue).padding(8.dp)) {
            when {
                state.blocked > 0 -> StatusBadge("${state.blocked} hibás", Tone.Signal)
                state.outstanding > 0 -> StatusBadge("${state.outstanding} várakozik", Tone.Cold)
                else -> StatusBadge("Feltöltve", Tone.Done)
            }
        }
    }
}

@Composable
private fun CameraPreview(viewModel: CaptureViewModel) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val previewView = remember { PreviewView(context).apply { scaleType = PreviewView.ScaleType.FILL_CENTER } }

    DisposableEffect(lifecycleOwner) {
        val providerFuture = ProcessCameraProvider.getInstance(context)
        providerFuture.addListener({
            val provider = providerFuture.get()
            val preview = Preview.Builder().build().also {
                it.setSurfaceProvider(previewView.surfaceProvider)
            }
            // MINIMIZE_LATENCY over MAXIMIZE_QUALITY: a fitter takes twenty photos in a row
            // and a shutter that lags is a shutter that gets double-tapped.
            val capture = ImageCapture.Builder()
                .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
                .build()
            provider.unbindAll()
            provider.bindToLifecycle(
                lifecycleOwner,
                androidx.camera.core.CameraSelector.DEFAULT_BACK_CAMERA,
                preview,
                capture,
            )
            viewModel.attachCamera(capture)
        }, ContextCompat.getMainExecutor(context))

        onDispose {
            viewModel.attachCamera(null)
            runCatching { ProcessCameraProvider.getInstance(context).get().unbindAll() }
        }
    }

    AndroidView(factory = { previewView }, modifier = Modifier.fillMaxSize())
}

@Composable
private fun PermissionNeeded(onRequest: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            "A fotózáshoz kamera-hozzáférés kell.",
            style = MaterialTheme.typography.bodyLarge,
            color = Surface,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(16.dp))
        PrimaryButton("Engedélyezés", onRequest)
    }
}

@Composable
private fun NoOrderSelected(onPickOrder: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            "Melyik járművön dolgozol?",
            style = MaterialTheme.typography.titleLarge,
            color = Surface,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(16.dp))
        PrimaryButton("Megrendelés választása", onPickOrder)
    }
}

@Composable
private fun CategoryBar(selected: String, enabled: Boolean, onSelect: (String) -> Unit) {
    LazyRow(
        Modifier
            .fillMaxWidth()
            .background(Steel900)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        items(CapturePrefs.ALL.size) { index ->
            val category = CapturePrefs.ALL[index]
            val active = category == selected
            val immutable = CapturePrefs.isImmutable(category)
            Box(
                Modifier
                    .background(
                        when {
                            active && immutable -> Signal
                            active -> Surface
                            else -> Color.Transparent
                        },
                        RoundedCornerShape(999.dp),
                    )
                    .border(1.dp, if (active) Color.Transparent else Steel500, RoundedCornerShape(999.dp))
                    .clickable(enabled = enabled) { onSelect(category) }
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            ) {
                Text(
                    CapturePrefs.label(category),
                    style = MaterialTheme.typography.labelLarge,
                    color = when {
                        active && immutable -> Surface
                        active -> Steel900
                        else -> Steel200
                    },
                )
            }
        }
    }
}

@Composable
private fun ShutterBar(enabled: Boolean, shotsThisSession: Int, onShutter: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .background(Steel900)
            .padding(vertical = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center,
    ) {
        Box(Modifier.width(72.dp), contentAlignment = Alignment.Center) {
            if (shotsThisSession > 0) {
                Text("$shotsThisSession", style = MaterialTheme.typography.titleLarge, color = Surface)
            }
        }
        Box(
            Modifier
                .size(76.dp)
                .background(if (enabled) Surface else Steel500, CircleShape)
                .clickable(enabled = enabled, onClick = onShutter),
        )
        Spacer(Modifier.width(72.dp))
    }
}

private fun hasCameraPermission(context: Context): Boolean =
    ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED

/** CameraX's callback API as a suspend function, so the ViewModel reads top to bottom. */
suspend fun ImageCapture.takePictureTo(
    file: java.io.File,
    executor: java.util.concurrent.Executor,
): Result<Unit> = suspendCoroutine { continuation ->
    val options = ImageCapture.OutputFileOptions.Builder(file).build()
    takePicture(
        options,
        executor,
        object : ImageCapture.OnImageSavedCallback {
            override fun onImageSaved(output: ImageCapture.OutputFileResults) {
                continuation.resume(Result.success(Unit))
            }

            override fun onError(exception: ImageCaptureException) {
                continuation.resume(Result.failure(exception))
            }
        },
    )
}
