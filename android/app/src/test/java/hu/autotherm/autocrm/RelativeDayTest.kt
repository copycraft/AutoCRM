package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.Task
import hu.autotherm.autocrm.ui.tasks.taskGroups
import hu.autotherm.autocrm.util.relativeDay
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate

/** Deadlines read the way people say them, and tasks line up by what is due first. */
class RelativeDayTest {
    private val today = LocalDate.of(2026, 10, 8)

    @Test
    fun nearDaysInWordsFarDaysAsDates() {
        assertEquals("ma", relativeDay("2026-10-08", today))
        assertEquals("holnap", relativeDay("2026-10-09", today))
        assertEquals("tegnap", relativeDay("2026-10-07", today))
        assertEquals("3 nap múlva", relativeDay("2026-10-11", today))
        assertEquals("5 napja", relativeDay("2026-10-03", today))
        assertEquals("2026. 12. 24.", relativeDay("2026-12-24", today))
        assertNull(relativeDay(null, today))
        assertEquals("nem dátum", relativeDay("nem dátum", today))
    }

    private fun task(id: Long, due: String?) = Task(id, "order", 1, "t$id", dueDate = due)

    @Test
    fun tasksGroupOverdueFirstAndDropEmptyGroups() {
        val groups = taskGroups(
            listOf(task(1, "2026-10-20"), task(2, null), task(3, "2026-10-01"), task(4, "2026-10-08"), task(5, "2026-10-10")),
            today,
        )
        assertEquals(listOf("Lejárt", "Ma", "Később", "Nincs határidő"), groups.map { it.first })
        assertEquals(listOf(5L, 1L), groups.first { it.first == "Később" }.second.map { it.id })
    }
}
