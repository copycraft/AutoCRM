package hu.autotherm.autocrm.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import hu.autotherm.autocrm.data.api.Lookups
import kotlinx.coroutines.flow.first
import kotlinx.serialization.json.Json

private val Context.lookupsDataStore by preferencesDataStore(name = "lookups")

/**
 * The last enumerations document downloaded from `GET /config/lookups`.
 *
 * Selects, chips and labels render from this when the server cannot be
 * reached; unknown keys read as themselves (see `lookupLabel`), so a stale
 * cache never renders blank. One entry: the document is versioned by deploy,
 * not by vehicle kind.
 */
class LookupsCache(private val context: Context) {

    suspend fun get(): Lookups? =
        context.lookupsDataStore.data.first()[stringPreferencesKey(KEY)]?.let(::decode)

    suspend fun put(lookups: Lookups) {
        context.lookupsDataStore.edit { it[stringPreferencesKey(KEY)] = encode(lookups) }
    }

    companion object {
        private const val KEY = "current"
        private val json = Json { ignoreUnknownKeys = true }

        fun encode(lookups: Lookups): String =
            json.encodeToString(Lookups.serializer(), lookups)

        /**
         * Null for anything that does not read back, so a bad entry is a miss,
         * not a crash. An empty document is a miss too: with no entries every
         * label would read humanized, so refetch instead of keeping it.
         */
        fun decode(raw: String): Lookups? = runCatching {
            json.decodeFromString(Lookups.serializer(), raw)
        }.getOrNull()?.takeIf { lookups ->
            lookups.damageTypes.isNotEmpty() || lookups.severities.isNotEmpty() ||
                lookups.verdicts.isNotEmpty() || lookups.fuelLevels.isNotEmpty() ||
                lookups.currencies.isNotEmpty() || lookups.imageCategories.isNotEmpty()
        }
    }
}
