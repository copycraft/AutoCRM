package hu.autotherm.autocrm.data.prefs

import android.content.Context

/**
 * The last few partners picked on this phone, newest first. The same handful of customers
 * fill most of a week, so the picker offers them before a single letter is typed.
 * Stored as id, currency, country, name per line (the name last: it may hold anything).
 */
object RecentPartners {
    data class Entry(val id: Long, val name: String, val defaultCurrency: String?, val country: String?)

    private const val KEY = "recent_partners"
    private const val LIMIT = 6

    private fun prefs(context: Context) =
        context.getSharedPreferences("pickers", Context.MODE_PRIVATE)

    fun load(context: Context): List<Entry> =
        prefs(context).getString(KEY, null).orEmpty().lines().mapNotNull(::decode)

    fun remember(context: Context, entry: Entry) {
        val next = (listOf(entry) + load(context).filter { it.id != entry.id }).take(LIMIT)
        prefs(context).edit().putString(KEY, next.joinToString("\n", transform = ::encode)).apply()
    }

    internal fun encode(e: Entry): String =
        listOf(e.id.toString(), e.defaultCurrency.orEmpty(), e.country.orEmpty(), e.name.replace('\n', ' '))
            .joinToString("\t")

    internal fun decode(line: String): Entry? {
        val parts = line.split('\t', limit = 4)
        if (parts.size < 4) return null
        val id = parts[0].toLongOrNull() ?: return null
        return Entry(id, parts[3], parts[1].ifBlank { null }, parts[2].ifBlank { null })
    }
}
