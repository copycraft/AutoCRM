package hu.autotherm.autocrm.ui.spec

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.data.api.Questionnaire
import hu.autotherm.autocrm.data.api.SpecQuestion
import hu.autotherm.autocrm.data.spec.Answers
import hu.autotherm.autocrm.data.spec.answerProblem
import hu.autotherm.autocrm.data.spec.asBoolean
import hu.autotherm.autocrm.data.spec.formatAnswer
import hu.autotherm.autocrm.data.spec.isAnswered
import hu.autotherm.autocrm.data.spec.missingQuestions
import hu.autotherm.autocrm.data.spec.visibleSections
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.FieldLabel
import hu.autotherm.autocrm.ui.common.Info
import hu.autotherm.autocrm.ui.common.SegmentedChoice
import hu.autotherm.autocrm.ui.common.SelectField
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.app
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive

// The build questionnaire (0065) on the phone: the sections asked for this project type and
// these answers, each question drawn by its kind. Answering "Hűteni kell?" igen opens the
// cooling questions as you go. The configuration comes from settings on the server.

const val REQUIRED_HINT = "Kötelező a megrendeléshez"

/** The questionnaire from the server; null while loading or offline. */
@Composable
fun rememberQuestionnaire(): Questionnaire? {
    val api = app().api
    val qn by produceState<Questionnaire?>(initialValue = null) {
        value = runCatching { api.questionnaire() }.getOrNull()
    }
    return qn
}

/** One question as a field. Numbers stay as typed ("12,5") until the server reads them. */
@Composable
fun QuestionField(
    question: SpecQuestion,
    value: JsonElement?,
    onChange: (JsonElement?) -> Unit,
    highlight: Boolean = false,
    enabled: Boolean = true,
) {
    val q = question
    val label = q.label + if (q.required) " *" else ""
    val flagged = highlight && !isAnswered(value)
    val problem = answerProblem(q, value)
    val note = problem ?: if (flagged) REQUIRED_HINT else q.help
    when (q.kind) {
        "yes_no" -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            FieldLabel(label)
            SegmentedChoice(
                options = listOf<Pair<Boolean?, String>>(true to "Igen", false to "Nem"),
                selected = value.asBoolean(),
                onSelect = { onChange(it?.let(::JsonPrimitive)) },
                enabled = enabled,
            )
            note?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = if (flagged) Signal else Steel500) }
        }
        "choice" -> {
            val current = (value as? JsonPrimitive)?.takeIf { it.isString }?.content
            val options = q.options + listOfNotNull(current?.takeIf { it !in q.options })
            SelectField(
                label = label,
                options = options.map { it to it },
                selected = current,
                onSelect = { onChange(it?.let(::JsonPrimitive)) },
                noneLabel = "—",
                enabled = enabled,
                supporting = note,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        "multi_choice" -> Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            FieldLabel(label)
            val picked = (value as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.content }.orEmpty()
            (q.options + picked.filter { it !in q.options }).forEach { option ->
                val checked = option in picked
                val toggle = {
                    val next = if (checked) picked - option else picked + option
                    onChange(if (next.isEmpty()) null else JsonArray(next.map(::JsonPrimitive)))
                }
                Row(
                    Modifier.fillMaxWidth().clickable(enabled = enabled, onClick = toggle),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = checked, onCheckedChange = { toggle() }, enabled = enabled)
                    Text(option, style = MaterialTheme.typography.bodyLarge)
                }
            }
            note?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = if (flagged) Signal else Steel500) }
        }
        else -> {
            val text = (value as? JsonPrimitive)?.content.orEmpty()
            val number = q.kind == "number"
            AutoCrmTextField(
                value = text,
                onValueChange = { typed -> onChange(if (typed.isEmpty()) null else JsonPrimitive(typed)) },
                label = if (number && q.unit != null) "$label (${q.unit})" else label,
                singleLine = q.kind != "long_text",
                enabled = enabled,
                isError = flagged || problem != null,
                supporting = note,
                keyboardOptions = if (number) KeyboardOptions(keyboardType = KeyboardType.Decimal) else KeyboardOptions.Default,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/** Every section asked, as fields. [exclude] leaves out questions drawn elsewhere. */
@Composable
fun QuestionnaireFields(
    questionnaire: Questionnaire?,
    projectTypeId: Long?,
    answers: Answers,
    onChange: (Answers) -> Unit,
    highlight: Set<String> = emptySet(),
    exclude: Set<String> = emptySet(),
    enabled: Boolean = true,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        visibleSections(questionnaire, projectTypeId, answers).forEach { section ->
            val questions = section.questions.filter { it.isActive && it.key !in exclude }
            if (questions.isEmpty()) return@forEach
            Text(section.title, style = MaterialTheme.typography.titleMedium)
            questions.forEach { q ->
                QuestionField(
                    question = q,
                    value = answers[q.key],
                    onChange = { v -> onChange(if (v == null) answers - q.key else answers + (q.key to v)) },
                    highlight = q.key in highlight,
                    enabled = enabled,
                )
            }
        }
    }
}

/** How many required answers the conversion still needs, as a badge. */
@Composable
fun MissingBadge(questionnaire: Questionnaire?, projectTypeId: Long?, answers: Answers) {
    if (questionnaire == null) return
    val missing = missingQuestions(questionnaire, projectTypeId, answers).size
    if (missing > 0) {
        StatusBadge("$missing kötelező kérdés hiányzik", Tone.Signal)
    } else {
        StatusBadge("Megrendelésre kész", Tone.Done)
    }
}

/**
 * The answers, read-only, grouped by the sections asked. [custom] draws one answer
 * differently (the cooling unit's serial, which the phone files) — return true when it did.
 */
@Composable
fun QuestionnaireView(
    questionnaire: Questionnaire?,
    projectTypeId: Long?,
    answers: Answers,
    custom: @Composable (SpecQuestion, JsonElement?) -> Boolean = { _, _ -> false },
) {
    val sections = visibleSections(questionnaire, projectTypeId, answers)
    if (questionnaire == null) {
        Text("A kérdőív nem tölthető be.", style = MaterialTheme.typography.bodyMedium, color = Steel500)
        return
    }
    if (sections.isEmpty()) {
        Text("Még nincs kitöltve.", style = MaterialTheme.typography.bodyMedium, color = Steel500)
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        sections.forEach { section ->
            val questions = section.questions.filter { it.isActive }
            if (questions.isEmpty()) return@forEach
            Text(section.title, style = MaterialTheme.typography.titleSmall)
            questions.forEach { q ->
                if (!custom(q, answers[q.key])) {
                    Info(q.label, formatAnswer(q, answers[q.key]), mono = q.kind == "number")
                }
            }
        }
    }
}
