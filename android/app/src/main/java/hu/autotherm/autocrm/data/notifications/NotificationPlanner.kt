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
     * `/leads/<id>`, and `/orders/<id>` (a department told about its technical content,
     * 0059); a query string such as `?tab=modules` is the web's and is dropped here.
     */
    fun routeForLink(link: String?): String? {
        val path = link?.substringBefore('?') ?: return null
        Regex("^/leads/(\\d+)$").matchEntire(path)?.let { return "lead/${it.groupValues[1]}" }
        Regex("^/orders/(\\d+)$").matchEntire(path)?.let { return "order/${it.groupValues[1]}" }
        return null
    }
}
