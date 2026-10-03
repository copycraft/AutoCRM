package hu.autotherm.autocrm.data.inspection

import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.ZoneTemplate
import hu.autotherm.autocrm.data.prefs.ZoneListCache

/** The two walkarounds: `checkout` is the first (átvétel), `checkin` the second (kiadás). */
val WALKAROUND_KINDS = listOf("checkout", "checkin")

/** Where the zone list a walkaround starts with came from. */
enum class ZoneListSource {
    /** Just downloaded: the vehicle kind's own list, or the general one. */
    SERVER,

    /** The last list downloaded for this vehicle kind and walkaround, used because the server could not be reached. */
    CACHE,
}

class ResolvedZones(val zones: List<ZoneTemplate>, val source: ZoneListSource)

/**
 * The zone list a fresh walkaround starts with: what the server just said, else the list
 * last downloaded for this vehicle kind and walkaround, else nothing.
 *
 * There is deliberately no built-in list to fall back to. Which photos a vehicle needs is
 * decided on the server, so a list kept in the app would be the one thing an app update was
 * needed to change. [prefetchZoneLists] keeps the phone supplied instead.
 */
fun resolveZones(fetched: List<ZoneTemplate>?, cached: List<ZoneTemplate>?): ResolvedZones? = when {
    !fetched.isNullOrEmpty() -> ResolvedZones(fetched, ZoneListSource.SERVER)
    !cached.isNullOrEmpty() -> ResolvedZones(cached, ZoneListSource.CACHE)
    else -> null
}

/** What to tell the inspector about where the list came from; null when it is the server's own. */
fun zoneListNotice(source: ZoneListSource): String? = when (source) {
    ZoneListSource.SERVER -> null
    ZoneListSource.CACHE ->
        "Nincs kapcsolat: a telefon az utoljára letöltött fotólistát használja ehhez a járműtípushoz. " +
            "Ha időközben módosították, az új lista csak kapcsolattal induló körbejárásnál érvényes."
}

/** Shown instead of starting, when the phone has never downloaded this list and cannot reach the server. */
const val ZONE_LIST_UNAVAILABLE =
    "Nincs kapcsolat, és a telefonon még nincs letöltött fotólista ehhez a járműtípushoz. " +
        "Kapcsolattal nyisd meg egyszer az Átvétel-átadás képernyőt, utána offline is indíthatod."

/** Downloads the list and keeps it for offline starts; null when the server cannot be reached. */
suspend fun downloadZoneList(
    api: AutoCrmApi,
    cache: ZoneListCache,
    projectTypeId: Long?,
    kind: String,
): List<ZoneTemplate>? {
    val fetched = try {
        api.zoneTemplates(projectTypeId, kind).takeIf { it.isNotEmpty() }
    } catch (e: Exception) {
        null
    }
    if (fetched != null) runCatching { cache.put(projectTypeId, kind, fetched) }
    return fetched
}

/**
 * Downloads both walkarounds' lists for a vehicle kind, quietly. Called when the
 * Átvétel-átadás screen opens, which is online more often than the yard where the
 * walkaround starts, so the phone has the current lists before it needs them.
 */
suspend fun prefetchZoneLists(api: AutoCrmApi, cache: ZoneListCache, projectTypeId: Long?) {
    for (kind in WALKAROUND_KINDS) downloadZoneList(api, cache, projectTypeId, kind)
}
