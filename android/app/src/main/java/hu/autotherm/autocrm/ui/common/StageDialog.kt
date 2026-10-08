package hu.autotherm.autocrm.ui.common

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.data.api.TransitionOption
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500

/**
 * The transition list comes from the server with `allowed` and `reason` already decided.
 * A disallowed option is shown greyed with its reason rather than hidden: "why can't I move
 * this" is the question, and the reason is the answer.
 */
@Composable
fun StageDialog(
    options: List<TransitionOption>,
    error: String?,
    busy: Boolean,
    onDismiss: () -> Unit,
    /** Leads: the reasons offered when the target is `lost` (the web asks the same). */
    lostReasons: List<hu.autotherm.autocrm.data.api.LostReason> = emptyList(),
    /** Where it stands now, so the choice reads as "from X to …". */
    currentLabel: String? = null,
    onConfirm: (stage: String, note: String?, lostReasonId: Long?) -> Unit,
) {
    // Stage keys and notes are plain strings: saveable across rotation, unlike the
    // TransitionOption objects themselves.
    var selectedKey by rememberSaveable { mutableStateOf<String?>(null) }
    var note by rememberSaveable { mutableStateOf("") }
    var lostReason by rememberSaveable { mutableStateOf<Long?>(null) }
    val selected = options.find { it.stageKey == selectedKey }
    val asksReason = selected?.stageKey == "lost" && lostReasons.isNotEmpty()

    DialogShell(
        title = "Fázisváltás",
        onDismiss = onDismiss,
        actions = {
            val choice = selected
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Váltás",
                enabled = choice != null && !busy && (choice.requiresNote.not() || note.isNotBlank()),
                onClick = { choice?.let { onConfirm(it.stageKey, note, lostReason.takeIf { asksReason }) } },
            )
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            currentLabel?.let {
                Text("Jelenleg: $it", style = MaterialTheme.typography.labelLarge, color = Steel500)
            }
            options.forEach { option ->
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clickable(enabled = option.allowed) { selectedKey = option.stageKey }
                        .padding(vertical = 6.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(
                            option.labelHu,
                            style = MaterialTheme.typography.bodyLarge,
                            color = if (option.allowed) MaterialTheme.colorScheme.onSurface else Steel500,
                        )
                        if (selectedKey == option.stageKey) StatusBadge("Kiválasztva", Tone.Cold)
                    }
                    val reason = option.reason
                    if (!option.allowed && reason != null) {
                        Text(reason, style = MaterialTheme.typography.labelMedium, color = Signal)
                    } else if (option.allowed && option.requiresNote) {
                        // Said before the tap, not discovered after it.
                        Text("Visszalépés: indoklás kell", style = MaterialTheme.typography.labelMedium, color = Steel500)
                    }
                }
            }
            if (asksReason) {
                SelectField(
                    label = "Miért veszett el? (nem kötelező)",
                    options = lostReasons.map { it.id to it.label },
                    selected = lostReason,
                    onSelect = { lostReason = it },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (selected?.requiresNote == true) {
                // The reason is the next thing to do: the keyboard opens on it.
                val noteFocus = androidx.compose.runtime.remember { androidx.compose.ui.focus.FocusRequester() }
                androidx.compose.runtime.LaunchedEffect(selectedKey) { runCatching { noteFocus.requestFocus() } }
                AutoCrmTextField(
                    value = note,
                    onValueChange = { note = it },
                    label = "Indoklás (kötelező)",
                    singleLine = false,
                    modifier = Modifier.fillMaxWidth().focusRequester(noteFocus),
                )
            }
            error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
        }
    }
}
