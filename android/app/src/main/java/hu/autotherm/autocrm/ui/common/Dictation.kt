package hu.autotherm.autocrm.ui.common

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.Composable

/**
 * Dictation: the phone's own speech recognizer turns speech into text for a note or a
 * damage description. Uses the system's recognizer screen, so the app needs no microphone
 * permission for it, and nothing is recorded or kept.
 */
@Composable
fun DictationButton(
    onText: (String) -> Unit,
    // A phone without a speech recogniser says so, rather than the mic doing nothing.
    onUnavailable: () -> Unit = { Toasts.show("Ezen a telefonon nincs beszédfelismerés.") },
) {
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            result.data
                ?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)
                ?.firstOrNull()
                ?.takeIf { it.isNotBlank() }
                ?.let(onText)
        }
    }
    IconButton(onClick = {
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, "hu-HU")
            putExtra(RecognizerIntent.EXTRA_PROMPT, "Mondja…")
        }
        try {
            launcher.launch(intent)
        } catch (e: ActivityNotFoundException) {
            onUnavailable()
        }
    }) {
        Icon(Icons.Filled.Mic, contentDescription = "Diktálás")
    }
}

/** Appends dictated text to what is already typed, with a space between. */
fun appendDictated(current: String, spoken: String): String =
    if (current.isBlank()) spoken else "${current.trimEnd()} $spoken"
