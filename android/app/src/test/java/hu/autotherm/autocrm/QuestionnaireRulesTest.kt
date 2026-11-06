package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.Questionnaire
import hu.autotherm.autocrm.data.api.SpecQuestion
import hu.autotherm.autocrm.data.api.SpecSection
import hu.autotherm.autocrm.data.spec.Answers
import hu.autotherm.autocrm.data.spec.answerProblem
import hu.autotherm.autocrm.data.spec.formatAnswer
import hu.autotherm.autocrm.data.spec.missingQuestions
import hu.autotherm.autocrm.data.spec.visibleSections
import hu.autotherm.autocrm.ui.leads.convertBody
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The phone's copy of the questionnaire rules (0065) must agree with the server's. */
class QuestionnaireRulesTest {
    private val box = 1L
    private val van = 2L

    private fun q(id: Long, section: Long, key: String, kind: String, required: Boolean = false, min: Double? = null, max: Double? = null, unit: String? = null) =
        SpecQuestion(
            id = id, sectionId = section, key = key, label = key, kind = kind, required = required,
            minValue = min, maxValue = max, unit = unit,
        )

    private val qn = Questionnaire(
        listOf(
            SpecSection(1, "general", "Alapok", questions = listOf(q(10, 1, "heats", "yes_no", true), q(11, 1, "cools", "yes_no", true))),
            SpecSection(2, "box", "Doboz", projectTypeIds = listOf(box), questions = listOf(q(20, 2, "box_length_mm", "number", true))),
            SpecSection(3, "cooling", "Hűtés", shownWhen = "cools", questions = listOf(
                q(30, 3, "cool_temp_c", "number", true, -40.0, 30.0, "°C"),
                q(31, 3, "freezes", "yes_no", true),
            )),
            SpecSection(4, "freezer", "Fagyasztás", shownWhen = "freezes", questions = listOf(q(40, 4, "freeze_temp_c", "number", true))),
            SpecSection(5, "heating", "Fűtés", shownWhen = "heats", questions = listOf(q(50, 5, "heat_temp_c", "number", true))),
        ),
    )

    private fun answers(json: String): Answers = Json.parseToJsonElement(json).jsonObject

    @Test
    fun `switches open their sections, both at once, and the freezer only behind cooling`() {
        assertEquals(listOf("general", "box"), visibleSections(qn, box, emptyMap()).map { it.key })
        assertEquals(
            listOf("general", "box", "cooling", "heating"),
            visibleSections(qn, box, answers("""{"heats": true, "cools": true}""")).map { it.key },
        )
        assertEquals(
            listOf("general", "cooling", "freezer"),
            visibleSections(qn, van, answers("""{"cools": true, "freezes": true}""")).map { it.key },
        )
        assertEquals(
            listOf("general"),
            visibleSections(qn, van, answers("""{"cools": false, "freezes": true}""")).map { it.key },
        )
        assertEquals(listOf("general"), visibleSections(qn, null, emptyMap()).map { it.key })
    }

    @Test
    fun `no is an answer, blank is not, and opened sections add their questions`() {
        assertEquals(listOf("heats", "cools", "box_length_mm"), missingQuestions(qn, box, emptyMap()).map { it.key })
        assertEquals(
            listOf("box_length_mm"),
            missingQuestions(qn, box, answers("""{"heats": false, "cools": false, "box_length_mm": " "}""")).map { it.key },
        )
        assertEquals(
            listOf("cool_temp_c", "freeze_temp_c"),
            missingQuestions(qn, box, answers("""{"heats": false, "cools": true, "box_length_mm": 4200, "freezes": true}""")).map { it.key },
        )
    }

    @Test
    fun `numbers are checked against their range and shown with their unit`() {
        val temp = qn.sections[2].questions[0]
        assertNull(answerProblem(temp, JsonPrimitive("-18,5")))
        assertEquals("Csak -40 és 30 között.", answerProblem(temp, JsonPrimitive("-50")))
        assertEquals("Csak szám adható meg.", answerProblem(temp, JsonPrimitive("hideg")))
        assertEquals("-18,5 °C", formatAnswer(temp, JsonPrimitive(-18.5)))
        assertEquals("Nem", formatAnswer(qn.sections[0].questions[0], JsonPrimitive(false)))
    }

    @Test
    fun `the conversion carries the answers`() {
        val body = convertBody(
            "Doboz", "HUF", projectTypeId = box,
            specAnswers = buildJsonObject { put("heats", false); put("cools", false) },
        )
        assertEquals(box, body.projectTypeId)
        assertEquals(JsonPrimitive(false), body.specAnswers?.get("cools"))
    }
}
