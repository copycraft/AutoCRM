package hu.autotherm.autocrm.data.inspection

import android.content.Context
import android.net.Uri
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import java.io.File
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine

/**
 * Reads the odometer off the dashboard photo, on the phone (ML Kit's bundled Latin text
 * model: nothing leaves the device, works with no signal). The result is only offered:
 * the inspector taps the right number, or types it. A dashboard shows clocks, trip
 * counters and temperatures too, so the readings are ranked, not trusted.
 */
object OdometerReader {

    /** Digit runs that could be a mileage, most likely first. Visible for tests. */
    fun candidates(text: String): List<String> {
        val runs = Regex("""\d[\d .,']{1,10}\d|\d{3,}""")
            .findAll(text)
            .map { it.value.filter(Char::isDigit) }
            .filter { it.length in 3..7 && !it.startsWith("0") }
            .distinct()
            .toList()
        // Longer runs first (a 6-digit odometer beats a 3-digit trip counter), then larger.
        return runs.sortedWith(compareByDescending<String> { it.length }.thenByDescending { it.toLong() }).take(4)
    }

    suspend fun read(context: Context, file: File): List<String> {
        val image = InputImage.fromFilePath(context, Uri.fromFile(file))
        val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        try {
            val text = suspendCancellableCoroutine { cont ->
                recognizer.process(image)
                    .addOnSuccessListener { cont.resume(it.text) }
                    .addOnFailureListener { cont.resumeWithException(it) }
            }
            return candidates(text)
        } finally {
            recognizer.close()
        }
    }
}
