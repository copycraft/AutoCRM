package hu.autotherm.autocrm.ui.spec

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import hu.autotherm.autocrm.data.api.Questionnaire
import hu.autotherm.autocrm.data.spec.Answers
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ProjectTypeField
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500

/**
 * The lead's build questionnaire, full screen: the project type first (it decides the
 * type-specific questions), then every section asked. Saving never demands completeness;
 * the badge at the top says what the conversion will still ask for.
 */
@Composable
fun LeadSpecEditDialog(
    questionnaire: Questionnaire?,
    initialProjectType: Long?,
    initialAnswers: Answers,
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onSave: (Long?, Answers) -> Unit,
) {
    var projectType by remember { mutableStateOf(initialProjectType) }
    var answers by remember { mutableStateOf(initialAnswers) }
    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        Surface(Modifier.fillMaxSize().imePadding()) {
            Column(Modifier.fillMaxSize()) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text("Műszaki kérdőív", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
                    TextButton(onClick = onDismiss) { Text("Mégse") }
                }
                Column(
                    Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    MissingBadge(questionnaire, projectType, answers)
                    ProjectTypeField(
                        selected = projectType,
                        onSelect = { projectType = it },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    if (projectType == null) {
                        Text(
                            "Válassz projekttípust: a típusra jellemző kérdések csak utána jelennek meg.",
                            style = MaterialTheme.typography.bodySmall,
                            color = Steel500,
                        )
                    }
                    if (questionnaire == null) {
                        Text("A kérdőív nem tölthető be. Ellenőrizd a kapcsolatot.", color = Signal)
                    }
                    QuestionnaireFields(
                        questionnaire = questionnaire,
                        projectTypeId = projectType,
                        answers = answers,
                        onChange = { answers = it },
                        enabled = !busy,
                    )
                    error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
                }
                PrimaryButton(
                    text = if (busy) "Mentés…" else "Kérdőív mentése",
                    enabled = !busy && questionnaire != null,
                    onClick = { onSave(projectType, answers) },
                    modifier = Modifier.fillMaxWidth().padding(16.dp),
                )
            }
        }
    }
}
