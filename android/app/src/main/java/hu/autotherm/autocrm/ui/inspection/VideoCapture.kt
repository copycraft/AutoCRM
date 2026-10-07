package hu.autotherm.autocrm.ui.inspection

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.video.FileOutputOptions
import androidx.camera.video.Quality
import androidx.camera.video.QualitySelector
import androidx.camera.video.Recorder
import androidx.camera.video.Recording
import androidx.camera.video.VideoCapture
import androidx.camera.video.VideoRecordEvent
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
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
import androidx.compose.material.icons.filled.FiberManualRecord
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
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
import kotlinx.coroutines.delay

/** Clips stay short: they are evidence of a zone, not a film, and they upload over mobile data. */
const val MAX_CLIP_MS = 30_000L

/**
 * The walkaround's clip camera: live view, one record button, stops by itself after
 * [MAX_CLIP_MS]. No sound is recorded, so no microphone permission is asked for. 720p keeps
 * a clip around 15–20 MB.
 */
@Composable
fun VideoCaptureScreen(
    request: VideoRequest,
    onRecorded: (durationMs: Long) -> Unit,
    onCancel: () -> Unit,
) {
    val context = LocalContext.current
    var hasPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED,
        )
    }
    val permissionLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        hasPermission = it
    }
    LaunchedEffect(Unit) { if (!hasPermission) permissionLauncher.launch(Manifest.permission.CAMERA) }
    if (!hasPermission) {
        Column(Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.Center) {
            Text("Kameraengedély nélkül nem készülhet videó.", style = MaterialTheme.typography.titleLarge)
            Button(onClick = onCancel, modifier = Modifier.padding(top = 16.dp)) { Text("Vissza") }
        }
        return
    }

    val lifecycleOwner = LocalLifecycleOwner.current
    val preview = remember { Preview.Builder().build() }
    var videoCapture by remember { mutableStateOf<VideoCapture<Recorder>?>(null) }
    var recording by remember { mutableStateOf<Recording?>(null) }
    var startedAt by remember { mutableLongStateOf(0L) }
    var elapsed by remember { mutableLongStateOf(0L) }
    var error by remember { mutableStateOf<String?>(null) }

    DisposableEffect(lifecycleOwner) {
        val future = ProcessCameraProvider.getInstance(context)
        var provider: ProcessCameraProvider? = null
        future.addListener({
            provider = runCatching { future.get() }.getOrNull()
            val recorder = Recorder.Builder()
                .setQualitySelector(QualitySelector.from(Quality.HD))
                .build()
            val capture = VideoCapture.withOutput(recorder)
            videoCapture = capture
            try {
                provider?.unbindAll()
                provider?.bindToLifecycle(lifecycleOwner, CameraSelector.DEFAULT_BACK_CAMERA, preview, capture)
            } catch (e: Exception) {
                error = "a kamera nem indult el"
            }
        }, ContextCompat.getMainExecutor(context))
        onDispose {
            recording?.stop()
            runCatching { provider?.unbindAll() }
        }
    }

    fun stop() {
        recording?.stop()
        recording = null
    }

    LaunchedEffect(recording) {
        while (recording != null) {
            elapsed = System.currentTimeMillis() - startedAt
            if (elapsed >= MAX_CLIP_MS) stop()
            delay(200)
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView(
            factory = { ctx ->
                PreviewView(ctx).apply {
                    scaleType = PreviewView.ScaleType.FILL_CENTER
                    preview.setSurfaceProvider(surfaceProvider)
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        Text(
            request.instruction + if (recording != null) "  ·  ${elapsed / 1000} / ${MAX_CLIP_MS / 1000} s" else "",
            style = MaterialTheme.typography.titleLarge,
            color = Color.White,
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(16.dp)
                .background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(8.dp))
                .padding(horizontal = 12.dp, vertical = 8.dp),
        )
        error?.let {
            Text(it, color = Color.White, modifier = Modifier.align(Alignment.Center))
        }
        Row(
            Modifier.fillMaxWidth().align(Alignment.BottomCenter).padding(24.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = {
                stop()
                onCancel()
            }) {
                Icon(Icons.Filled.Close, contentDescription = "Mégse", tint = Color.White)
            }
            IconButton(
                onClick = {
                    if (recording != null) {
                        stop()
                        return@IconButton
                    }
                    val capture = videoCapture ?: return@IconButton
                    startedAt = System.currentTimeMillis()
                    elapsed = 0
                    recording = capture.output
                        .prepareRecording(context, FileOutputOptions.Builder(request.file).build())
                        .start(ContextCompat.getMainExecutor(context)) { event ->
                            if (event is VideoRecordEvent.Finalize) {
                                if (event.hasError() && !request.file.exists()) {
                                    error = "a felvétel nem sikerült"
                                } else {
                                    onRecorded(System.currentTimeMillis() - startedAt)
                                }
                            }
                        }
                },
                modifier = Modifier.size(76.dp).background(Color.White, CircleShape),
            ) {
                Icon(
                    if (recording != null) Icons.Filled.Stop else Icons.Filled.FiberManualRecord,
                    contentDescription = if (recording != null) "Megállítás" else "Felvétel",
                    tint = Color(0xFFE11D48),
                    modifier = Modifier.size(40.dp),
                )
            }
            Box(Modifier.size(48.dp))
        }
    }
}
