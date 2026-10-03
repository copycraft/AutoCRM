package hu.autotherm.autocrm.data.inspection

import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.ImageCategoryEntry
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.api.LookupItem
import hu.autotherm.autocrm.data.prefs.LookupsCache

/**
 * Labels for the server's enumerations (`GET /config/lookups`).
 *
 * Nothing about which damage types, fuel marks, currencies or categories exist
 * is kept in the app: a key with no entry — offline with nothing cached, or a
 * value the office removed — reads as the key itself, spaced out.
 */
fun lookupLabel(items: List<LookupItem>, key: String): String =
    items.firstOrNull { it.key == key }?.labelHu?.takeIf { it.isNotBlank() }
        ?: humanizeZoneKey(key)

fun damageTypeLabel(type: String, lookups: Lookups?): String =
    lookups?.let { lookupLabel(it.damageTypes, type) } ?: humanizeZoneKey(type)

fun severityLabel(severity: String, lookups: Lookups?): String =
    lookups?.let { lookupLabel(it.severities, severity) } ?: humanizeZoneKey(severity)

fun verdictLabel(verdict: String, lookups: Lookups?): String =
    lookups?.let { lookupLabel(it.verdicts, verdict) } ?: humanizeZoneKey(verdict)

/**
 * The walkaround heading. Falls back to the built-in Átvétel/Kiadás when the
 * document has never downloaded (cold start, offline yard): the keys are API
 * values and stay, so the fallback cannot go stale — only a rewording waits
 * for the next fetch.
 */
fun walkaroundKindLabel(kind: String, lookups: Lookups?): String =
    lookups?.let { lookupLabel(it.walkaroundKinds, kind) } ?: inspectionKindLabel(kind)

/** The image categories a person may file a photo under by hand (production only). */
fun attachableCategories(lookups: Lookups?): List<ImageCategoryEntry> =
    lookups?.imageCategories?.filter { it.attachable }.orEmpty()

fun categoryLabel(category: String, lookups: Lookups?): String =
    lookups?.imageCategories?.firstOrNull { it.key == category }?.labelHu
        ?.takeIf { it.isNotBlank() } ?: humanizeZoneKey(category)

/** Downloads the enumerations and keeps them for offline use; null when unreachable. */
suspend fun downloadLookups(api: AutoCrmApi, cache: LookupsCache): Lookups? {
    val fetched = try {
        api.lookups()
    } catch (e: Exception) {
        null
    }
    if (fetched != null) {
        runCatching { cache.put(fetched) }
        ServerErrorTexts.update(fetched)
    }
    return fetched
}

/** The cached document, for screens that render labels without fetching. */
suspend fun cachedLookups(cache: LookupsCache): Lookups? =
    runCatching { cache.get() }.getOrNull()?.also { ServerErrorTexts.update(it) }

/**
 * Error texts from the server's lookups (`error_texts`), refreshed with every
 * download or cached read. The built-in map in ui.common stays as the
 * cold-start fallback: a login failure happens before the first fetch. A
 * reworded message or a new code reaches the phone with the next fetch —
 * no app update.
 */
object ServerErrorTexts {
    @Volatile var texts: Map<String, String> = emptyMap()

    fun update(lookups: Lookups?) {
        val fresh = lookups?.errorTexts
            ?.filter { it.code.isNotBlank() && it.textHu.isNotBlank() }
            ?.associate { it.code to it.textHu }
        if (fresh != null) texts = fresh
    }
}
