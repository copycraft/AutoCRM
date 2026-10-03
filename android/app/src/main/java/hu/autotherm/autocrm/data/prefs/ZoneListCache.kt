package hu.autotherm.autocrm.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import hu.autotherm.autocrm.data.api.ZoneTemplate
import kotlinx.coroutines.flow.first
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

private val Context.zoneListDataStore by preferencesDataStore(name = "zone_lists")

/**
 * The last zone list downloaded per vehicle kind and walkaround.
 *
 * The phone starts a walkaround in a yard with no signal more often than anywhere else, and
 * the list depends on the vehicle kind: a chassis with a box needs photos the built-in
 * general walk does not ask for. So every list that arrives is kept, and an offline start
 * uses the last one it saw for the same vehicle kind and walkaround.
 */
class ZoneListCache(private val context: Context) {

    suspend fun get(projectTypeId: Long?, kind: String): List<ZoneTemplate>? =
        context.zoneListDataStore.data.first()[stringPreferencesKey(key(projectTypeId, kind))]
            ?.let(::decode)

    suspend fun put(projectTypeId: Long?, kind: String, zones: List<ZoneTemplate>) {
        context.zoneListDataStore.edit { it[stringPreferencesKey(key(projectTypeId, kind))] = encode(zones) }
    }

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        /** One entry per vehicle kind and walkaround; an order with no project type uses `general`. */
        fun key(projectTypeId: Long?, kind: String): String = "${projectTypeId ?: "general"}:$kind"

        fun encode(zones: List<ZoneTemplate>): String =
            json.encodeToString(ListSerializer(ZoneTemplate.serializer()), zones)

        /** Null for anything that does not read back as a list, so a bad entry is a miss, not a crash. */
        fun decode(raw: String): List<ZoneTemplate>? = runCatching {
            json.decodeFromString(ListSerializer(ZoneTemplate.serializer()), raw)
        }.getOrNull()
    }
}
