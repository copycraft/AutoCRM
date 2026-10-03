package hu.autotherm.autocrm.data.notifications

import hu.autotherm.autocrm.data.api.NotificationItem

/**
 * Decides which feed items become system notifications. Pure, so the rules are unit-tested.
 *
 *  - The first poll on a phone (no cursor) shows nothing: it only remembers where the feed
 *    is, so a fresh sign-in does not light up with weeks of old news.
 *  - After that, every unread item newer than the cursor is shown once, oldest first.
 *  - The cursor only moves forward.
 */
object NotificationPlanner {

    data class Plan(val toShow: List<NotificationItem>, val newCursor: Long)

    fun plan(cursor: Long?, feed: List<NotificationItem>): Plan {
        val newest = feed.maxOfOrNull { it.id }
        if (cursor == null) return Plan(emptyList(), newest ?: 0L)
        val fresh = feed.filter { it.id > cursor && it.readAt == null }.sortedBy { it.id }
        return Plan(fresh, maxOf(cursor, newest ?: cursor))
    }

    /**
     * Maps a server link to an in-app route, or null for one this version does not know.
     * Only `/leads/<id>` exists today.
     */
    fun routeForLink(link: String?): String? {
        val match = link?.let { Regex("^/leads/(\\d+)$").matchEntire(it) } ?: return null
        return "lead/${match.groupValues[1]}"
    }
}
