package hu.autotherm.autocrm.data.spec

import hu.autotherm.autocrm.data.api.Questionnaire
import hu.autotherm.autocrm.data.api.SpecQuestion
import hu.autotherm.autocrm.data.api.SpecSection
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull

/**
 * The build questionnaire's rules (0065), mirrored from the backend
 * (`domain/questionnaire.rs`) and the web (`lib/questionnaire.ts`): which sections a job is
 * asked, given its project type and the answers so far, and which required questions are
 * still unanswered. The server decides; this draws the form and the convert dialog.
 */
typealias Answers = Map<String, JsonElement>

/** `false` is an answer; blank text, an empty list or nothing at all is not. */
fun isAnswered(value: JsonElement?): Boolean = when (value) {
    null, JsonNull -> false
    is JsonArray -> value.isNotEmpty()
    is JsonPrimitive -> !value.isString || value.content.isNotBlank()
    else -> true
}

fun JsonElement?.asBoolean(): Boolean? = (this as? JsonPrimitive)?.takeIf { !it.isString }?.booleanOrNull

private fun findActive(qn: Questionnaire, key: String): Pair<SpecSection, SpecQuestion>? {
    for (s in qn.sections) {
        val q = s.questions.firstOrNull { it.key == key && it.isActive }
        if (q != null) return s to q
    }
    return null
}

private fun applies(section: SpecSection, projectTypeId: Long?): Boolean =
    section.isActive &&
        (section.projectTypeIds.isEmpty() || (projectTypeId != null && projectTypeId in section.projectTypeIds))

/** Whether the section is asked: its type matches, and its switch (if any) is on in a
 *  section that is itself asked. A chain that loops back is never asked. */
fun isVisible(
    qn: Questionnaire,
    section: SpecSection,
    projectTypeId: Long?,
    answers: Answers,
    seen: MutableSet<Long> = mutableSetOf(),
): Boolean {
    if (!applies(section, projectTypeId) || !seen.add(section.id)) return false
    val trigger = section.shownWhen ?: return true
    val (triggerSection, q) = findActive(qn, trigger) ?: return false
    if (q.kind != "yes_no") return false
    return answers[trigger].asBoolean() == true && isVisible(qn, triggerSection, projectTypeId, answers, seen)
}

fun visibleSections(qn: Questionnaire?, projectTypeId: Long?, answers: Answers): List<SpecSection> =
    qn?.sections?.filter { isVisible(qn, it, projectTypeId, answers) }.orEmpty()

fun visibleQuestions(qn: Questionnaire?, projectTypeId: Long?, answers: Answers): List<SpecQuestion> =
    visibleSections(qn, projectTypeId, answers).flatMap { s -> s.questions.filter { it.isActive } }

/** The required questions asked and still unanswered: what blocks the conversion. */
fun missingQuestions(qn: Questionnaire?, projectTypeId: Long?, answers: Answers): List<SpecQuestion> =
    visibleQuestions(qn, projectTypeId, answers).filter { it.required && !isAnswered(answers[it.key]) }

/** A typed number as the server reads it ("12,5" from a Hungarian keyboard), or null. */
fun parseNumber(value: JsonElement?): Double? {
    val p = value as? JsonPrimitive ?: return null
    val n = if (p.isString) p.content.trim().replace(',', '.').toDoubleOrNull() else p.doubleOrNull
    return n?.takeIf { it.isFinite() }
}

/** What is wrong with a value before it is sent, in Hungarian, or null when it is fine. */
fun answerProblem(q: SpecQuestion, value: JsonElement?): String? {
    if (q.kind != "number" || !isAnswered(value)) return null
    val n = parseNumber(value) ?: return "Csak szám adható meg."
    val min = q.minValue
    val max = q.maxValue
    if ((min != null && n < min) || (max != null && n > max)) {
        return when {
            min != null && max != null -> "Csak ${plain(min)} és ${plain(max)} között."
            min != null -> "Legalább ${plain(min)}."
            else -> "Legfeljebb ${plain(max!!)}."
        }
    }
    return null
}

private fun plain(n: Double): String =
    if (n == Math.floor(n) && !n.isInfinite()) n.toLong().toString() else n.toString().replace('.', ',')

/** An answer as one line of text, for read-only views. */
fun formatAnswer(q: SpecQuestion, value: JsonElement?): String {
    if (!isAnswered(value)) return "—"
    return when {
        q.kind == "yes_no" -> if (value.asBoolean() == true) "Igen" else "Nem"
        value is JsonArray -> value.joinToString(", ") { (it as? JsonPrimitive)?.content ?: it.toString() }
        value is JsonPrimitive -> {
            val text = if (value.isString) value.content else value.content.replace('.', ',')
            if (q.unit != null) "$text ${q.unit}" else text
        }
        else -> value.toString()
    }
}

/** The answers as a JSON object, blanks dropped. */
fun answersObject(answers: Answers): JsonObject = JsonObject(answers.filterValues { it != JsonNull })
