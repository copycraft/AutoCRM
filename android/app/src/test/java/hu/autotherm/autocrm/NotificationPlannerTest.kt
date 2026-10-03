package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.Absence
import hu.autotherm.autocrm.data.api.NotificationFeed
import hu.autotherm.autocrm.data.api.NotificationItem
import hu.autotherm.autocrm.data.notifications.NotificationPlanner
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** When the phone shows a system notification, and what a tap opens. */
class NotificationPlannerTest {

    private fun item(id: Long, read: Boolean = false) = NotificationItem(
        id = id, kind = "lead", title = "Új érdeklődés", createdAt = "2026-10-03T10:00:00Z",
        readAt = if (read) "2026-10-03T11:00:00Z" else null,
    )

    @Test
    fun `the first poll only remembers where the feed is`() {
        val plan = NotificationPlanner.plan(null, listOf(item(9), item(8)))
        assertTrue(plan.toShow.isEmpty())
        assertEquals(9L, plan.newCursor)
    }

    @Test
    fun `an empty feed on the first poll starts the cursor at zero so the next lead is shown`() {
        val plan = NotificationPlanner.plan(null, emptyList())
        assertEquals(0L, plan.newCursor)
        val next = NotificationPlanner.plan(plan.newCursor, listOf(item(1)))
        assertEquals(listOf(1L), next.toShow.map { it.id })
    }

    @Test
    fun `only unread items newer than the cursor are shown, oldest first`() {
        val plan = NotificationPlanner.plan(
            5L,
            listOf(item(8), item(7, read = true), item(6), item(5), item(4)),
        )
        assertEquals(listOf(6L, 8L), plan.toShow.map { it.id })
        assertEquals(8L, plan.newCursor)
    }

    @Test
    fun `the cursor never moves backwards`() {
        assertEquals(10L, NotificationPlanner.plan(10L, emptyList()).newCursor)
        assertEquals(10L, NotificationPlanner.plan(10L, listOf(item(3))).newCursor)
    }

    @Test
    fun `a lead link opens the lead and anything else opens nothing`() {
        assertEquals("lead/42", NotificationPlanner.routeForLink("/leads/42"))
        assertNull(NotificationPlanner.routeForLink("/leads/42/edit"))
        assertNull(NotificationPlanner.routeForLink("/orders/1"))
        assertNull(NotificationPlanner.routeForLink(null))
    }

    @Test
    fun `the feed and an absence parse from the server shapes`() {
        val json = Json { ignoreUnknownKeys = true }
        val feed = json.decodeFromString(
            NotificationFeed.serializer(),
            """{"items":[{"id":3,"kind":"lead","title":"T","body":null,"link":"/leads/5","created_at":"2026-10-03T10:00:00Z","read_at":null}],"unread":1}""",
        )
        assertEquals(1L, feed.unread)
        assertEquals("/leads/5", feed.items.single().link)

        val absence = json.decodeFromString(
            Absence.serializer(),
            """{"id":1,"employee_id":7,"employee_name":"Kiss Péter","kind":"annual","start_date":"2026-06-08","end_date":"2026-06-12","working_days":5,"note":null,"created_at":"2026-01-01T00:00:00Z"}""",
        )
        assertEquals(5, absence.workingDays)
    }
}
