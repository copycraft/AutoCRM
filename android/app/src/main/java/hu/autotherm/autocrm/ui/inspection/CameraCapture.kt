package hu.autotherm.autocrm.ui.inspection

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.FlashOff
import androidx.compose.material.icons.filled.FlashOn
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.io.File
import java.util.concurrent.Executor

/**
 * The inspection camera: live view only, no gallery, no file picker, no way to
 * attach an image that was not taken here, now, by this camera. Every photo in
 * an inspection passes through this screen.
 *
 * Minimal chrome by design: instruction overlay, a faint framing guide, shutter,
 * flash toggle, cancel. Rear camera, full resolution.
 */
@Composable
fun CameraCapture(
    /** Short instruction for the shot, e.g. "Jobb oldal – az egész autó a képben". */
    instruction: String,
    /** File the JPEG is written to. Created by the caller inside the draft dir. */
    outputFile: File,
    onCaptured: (File) -> Unit,
    onCancel: () -> Unit,
) {
    val context = LocalContext.current
    var hasPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED,
        )
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> hasPermission = granted }

    LaunchedEffect(Unit) {
        if (!hasPermission) permissionLauncher.launch(Manifest.permission.CAMERA)
    }

    if (!hasPermission) {
        CameraDenied(onRetry = { permissionLauncher.launch(Manifest.permission.CAMERA) })
        return
    }

    val lifecycleOwner = LocalLifecycleOwner.current
    val executor: Executor = remember { ContextCompat.getMainExecutor(context) }
    var imageCapture: ImageCapture? by remember { mutableStateOf(null) }
    var flashOn by remember { mutableStateOf(false) }
    var taking by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val preview: Preview = remember { Preview.Builder().build() }

    DisposableEffect(lifecycleOwner) {
        var provider: ProcessCameraProvider? = null
        var bound = false
        val future = try {
            ProcessCameraProvider.getInstance(context)
        } catch (e: Exception) {
            error = "a kamera nem indult el"
            null
        }
        val listener = Runnable {
            if (bound) return@Runnable
            provider = try {
                future?.get()
            } catch (e: Exception) {
                error = "a kamera nem indult el"
                return@Runnable
            }
            val capture = ImageCapture.Builder()
                .setCaptureMode(ImageCapture.CAPTURE_MODE_MAXIMIZE_QUALITY)
                .build()
            imageCapture = capture
            try {
                provider?.unbindAll()
                provider?.bindToLifecycle(
                    lifecycleOwner,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    capture,
                )
                bound = true
            } catch (e: Exception) {
                // No back camera, or another app holds it: explain, don't die.
                error = "a kamera nem indult el"
            }
        }
        future?.addListener(listener, executor)
        onDispose { runCatching { provider?.unbindAll() } }
    }

    LaunchedEffect(flashOn) {
        imageCapture?.flashMode =
            if (flashOn) ImageCapture.FLASH_MODE_ON else ImageCapture.FLASH_MODE_OFF
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView(
            factory = { ctx ->
                PreviewView(ctx).apply {
                    scaleType = PreviewView.ScaleType.FILL_CENTER
                    implementationMode = PreviewView.ImplementationMode.COMPATIBLE
                    preview.setSurfaceProvider(surfaceProvider)
                }
            },
            modifier = Modifier.fillMaxSize(),
        )

        // Framing guide: a faint rounded frame. Not a mask — the whole sensor
        // frame is the evidence, the guide only steadies the hand.
        Box(
            Modifier
                .fillMaxSize()
                .padding(32.dp)
                .border(2.dp, Color.White.copy(alpha = 0.5f), RoundedCornerShape(16.dp)),
        )

        Column(
            Modifier.fillMaxWidth().align(Alignment.TopCenter).padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                instruction,
                style = MaterialTheme.typography.titleLarge,
                color = Color.White,
                modifier = Modifier
                    .background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(8.dp))
                    .padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }

        error?.let {
            Text(
                it,
                color = Color.White,
                modifier = Modifier
                    .align(Alignment.Center)
                    .background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(8.dp))
                    .padding(12.dp),
            )
        }

        Row(
            Modifier.fillMaxWidth().align(Alignment.BottomCenter).padding(24.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onCancel) {
                Icon(Icons.Filled.Close, contentDescription = "Mégse", tint = Color.White)
            }
            IconButton(
                onClick = {
                    if (taking) return@IconButton
                    val capture = imageCapture ?: return@IconButton
                    taking = true
                    val options = ImageCapture.OutputFileOptions.Builder(outputFile).build()
                    capture.takePicture(
                        options,
                        executor,
                        object : ImageCapture.OnImageSavedCallback {
                            override fun onImageSaved(output: ImageCapture.OutputFileResults) {
                                taking = false
                                onCaptured(outputFile)
                            }

                            override fun onError(exception: ImageCaptureException) {
                                taking = false
                                error = "a felvétel nem sikerült"
                            }
                        },
                    )
                },
                modifier = Modifier
                    .size(76.dp)
                    .background(Color.White, CircleShape)
                    .border(4.dp, Color.White.copy(alpha = 0.6f), CircleShape),
            ) {
                if (taking) {
                    CircularProgressIndicator(color = Color.Black, modifier = Modifier.size(32.dp))
                }
            }
            IconButton(onClick = { flashOn = !flashOn }) {
                Icon(
                    if (flashOn) Icons.Filled.FlashOn else Icons.Filled.FlashOff,
                    contentDescription = "Vaku",
                    tint = Color.White,
                )
            }
        }
    }
}

@Composable
private fun CameraDenied(onRetry: () -> Unit) {
    val context = LocalContext.current
    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            "Kameraengedély nélkül az átvétel nem folytatható.",
            style = MaterialTheme.typography.titleLarge,
        )
        Text(
            "A Beállításokban, az AutoCRM engedélyei között kapcsold be a Kamerát, " +
                "aztán gyere vissza.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 8.dp),
        )
        Row(
            Modifier.padding(top = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Button(onClick = onRetry) { Text("Újra") }
            androidx.compose.material3.OutlinedButton(
                onClick = {
                    context.startActivity(
                        Intent(
                            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                            Uri.fromParts("package", context.packageName, null),
                        ),
                    )
                },
            ) { Text("Beállítások") }
        }
    }
}
