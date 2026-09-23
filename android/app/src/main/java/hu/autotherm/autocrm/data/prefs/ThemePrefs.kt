package hu.autotherm.autocrm.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.themeDataStore by preferencesDataStore(name = "appearance")

/**
 * How the app looks. Device-wide and survives sign-out like the server address:
 * the phone belongs to the shop, and the night-shift fitter gets the dark theme
 * without asking the office.
 */
class ThemePrefs(private val context: Context) {

    companion object {
        const val MODE_SYSTEM = "system"
        const val MODE_LIGHT = "light"
        const val MODE_DARK = "dark"

        val MODES = listOf(MODE_SYSTEM, MODE_LIGHT, MODE_DARK)
    }

    private object Keys {
        val MODE = stringPreferencesKey("theme_mode")
        val AMOLED = booleanPreferencesKey("amoled_black")
    }

    /** `system` | `light` | `dark`. Anything else reads as `system`. */
    val mode: Flow<String> = context.themeDataStore.data.map {
        it[Keys.MODE]?.takeIf { m -> m in MODES } ?: MODE_SYSTEM
    }

    /** True black backgrounds under a dark theme. Off on its own does nothing. */
    val amoled: Flow<Boolean> = context.themeDataStore.data.map { it[Keys.AMOLED] ?: false }

    suspend fun setMode(mode: String) {
        require(mode in MODES)
        context.themeDataStore.edit { it[Keys.MODE] = mode }
    }

    suspend fun setAmoled(enabled: Boolean) {
        context.themeDataStore.edit { it[Keys.AMOLED] = enabled }
    }
}
