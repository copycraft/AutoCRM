package hu.autotherm.autocrm.ui.common

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.MediaRecorder
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.inspection.uploadDocument
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import java.io.File
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** A voice note runs at most this long: it is a note, not a meeting recording. */
private const val MAX_NOTE_MS = 5 * 60_000L

private fun newRecorder(context: Context): MediaRecorder =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) MediaRecorder(context) else @Suppress("DEPRECATION") MediaRecorder()

/**
 * Speak a note instead of typing it (0049): records AAC audio and files it among the
 * order's (or lead's) documents, where the office can play it on the web.
 */
@Composable
fun VoiceNoteRecorder(orderId: Long, leadId: Long? = null, onUploaded: () -> Unit = {}) {
    val context = LocalContext.current
    val app = context.applicationContext as AutoCrmApp
    val scope = rememberCoroutineScope()
    var recorder by remember { mutableStateOf<MediaRecorder?>(null) }
    var file by remember { mutableStateOf<File?>(null) }
    var startedAt by remember { mutableLongStateOf(0L) }
    var elapsed by remember { mutableLongStateOf(0L) }
    var status by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }

    fun start() {
        error = null
        status = null
        val out = File(context.cacheDir, "voice-${System.currentTimeMillis()}.m4a")
        try {
            val r = newRecorder(context).apply {
                setAudioSource(MediaRecorder.AudioSource.MIC)
                setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
                setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
                setAudioEncodingBitRate(64_000)
                setAudioSamplingRate(44_100)
                setOutputFile(out.absolutePath)
                prepare()
                start()
            }
            recorder = r
            file = out
            startedAt = System.currentTimeMillis()
            elapsed = 0
        } catch (e: Exception) {
            out.delete()
            error = "A felvétel nem indult el."
        }
    }

    fun stopAndUpload() {
        val r = recorder ?: return
        val out = file
        recorder = null
        try {
            r.stop()
        } catch (_: RuntimeException) {
            // Stopped too soon after starting: nothing usable was written.
            out?.delete()
            error = "Túl rövid felvétel."
            r.release()
            return
        }
        r.release()
        if (out == null || !out.exists() || out.length() == 0L) return
        busy = true
        status = "Feltöltés…"
        scope.launch {
            try {
                val stamp = LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyy-MM-dd_HH-mm"))
                val id = uploadDocument(
                    app,
                    orderId = orderId,
                    file = out,
                    filename = "hangjegyzet_$stamp.m4a",
                    contentType = "audio/mp4",
                    kind = "other",
                    leadId = leadId,
                )
                status = if (id != null) "Hangjegyzet feltöltve." else null
                if (id == null) error = "A feltöltés nem sikerült."
                out.delete()
                onUploaded()
            } catch (e: CancellationException) {
                throw e
            } catch (e: ApiException) {
                status = null
                error = if (e is ApiException.Network) "Nincs kapcsolat — próbáld újra." else (e.message ?: "A feltöltés nem sikerült.")
            } finally {
                busy = false
            }
        }
    }

    var granted by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED)
    }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        granted = ok
        if (ok) start() else error = "Mikrofon-engedély nélkül nem lehet hangjegyzetet felvenni."
    }

    // The clock, and the cap.
    LaunchedEffect(recorder) {
        while (recorder != null) {
            elapsed = System.currentTimeMillis() - startedAt
            if (elapsed >= MAX_NOTE_MS) stopAndUpload()
            delay(250)
        }
    }
    DisposableEffect(Unit) {
        onDispose {
            recorder?.let {
                try {
                    it.stop()
                } catch (_: RuntimeException) {
                }
                it.release()
            }
            file?.delete()
        }
    }

    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text("Hangjegyzet", style = MaterialTheme.typography.titleSmall)
                Text(
                    if (recorder != null) {
                        "Felvétel: %d:%02d".format(elapsed / 60_000, (elapsed / 1000) % 60)
                    } else {
                        status ?: "Mondd el, a dokumentumok közé kerül."
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = if (recorder != null) Signal else Steel500,
                )
            }
            if (recorder == null) {
                TextButton(
                    enabled = !busy,
                    onClick = { if (granted) start() else launcher.launch(Manifest.permission.RECORD_AUDIO) },
                ) {
                    Icon(Icons.Filled.Mic, contentDescription = null)
                    Text(" Felvétel")
                }
            } else {
                TextButton(onClick = { stopAndUpload() }) {
                    Icon(Icons.Filled.Stop, contentDescription = null)
                    Text(" Kész")
                }
            }
        }
        error?.let { Text(it, color = Signal, style = MaterialTheme.typography.labelMedium) }
    }
}
