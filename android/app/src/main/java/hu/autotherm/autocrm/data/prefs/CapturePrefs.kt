package hu.autotherm.autocrm.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import hu.autotherm.autocrm.data.api.Lookups
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.captureDataStore by preferencesDataStore(name = "capture")

/**
 * The two pieces of state that decide how many taps the first photo costs.
 *
 * The viability review counted at least five taps from launch to attached, and named the
 * two fixes: the order pre-selected, and the category sticky. Both live here, and both are
 * per-device rather than per-session, because a fitter who spends three days on one van
 * should not re-pick it every morning.
 */
class CapturePrefs(private val context: Context) {

    private object Keys {
        val ORDER_ID = longPreferencesKey("current_order_id")
        val ORDER_NUMBER = stringPreferencesKey("current_order_number")
        val ORDER_TITLE = stringPreferencesKey("current_order_title")
        val ORDER_PLATE = stringPreferencesKey("current_order_plate")
        val CATEGORY = stringPreferencesKey("sticky_category")
    }

    data class CurrentOrder(
        val id: Long,
        val number: String,
        val title: String,
        val plate: String?,
    )

    val currentOrder: Flow<CurrentOrder?> = context.captureDataStore.data.map { prefs ->
        val id = prefs[Keys.ORDER_ID] ?: return@map null
        CurrentOrder(
            id = id,
            number = prefs[Keys.ORDER_NUMBER].orEmpty(),
            title = prefs[Keys.ORDER_TITLE].orEmpty(),
            plate = prefs[Keys.ORDER_PLATE],
        )
    }

    /**
     * Defaults to `production`, not `intake`. Intake photos are irreversible — the database
     * refuses to delete or move them (`0004_media.sql`) — so the sticky default must be the
     * harmless one. Choosing intake is always a deliberate act.
     */
    val category: Flow<String> = context.captureDataStore.data.map {
        it[Keys.CATEGORY] ?: CATEGORY_PRODUCTION
    }

    suspend fun setCurrentOrder(order: CurrentOrder?) {
        context.captureDataStore.edit { prefs ->
            if (order == null) {
                prefs.remove(Keys.ORDER_ID)
                prefs.remove(Keys.ORDER_NUMBER)
                prefs.remove(Keys.ORDER_TITLE)
                prefs.remove(Keys.ORDER_PLATE)
            } else {
                prefs[Keys.ORDER_ID] = order.id
                prefs[Keys.ORDER_NUMBER] = order.number
                prefs[Keys.ORDER_TITLE] = order.title
                order.plate?.let { prefs[Keys.ORDER_PLATE] = it } ?: prefs.remove(Keys.ORDER_PLATE)
            }
        }
    }

    suspend fun setCategory(category: String) {
        context.captureDataStore.edit { it[Keys.CATEGORY] = category }
    }

    companion object {
        const val CATEGORY_INTAKE = "intake"
        const val CATEGORY_PRODUCTION = "production"
        const val CATEGORY_COMPLETION = "completion"
        const val CATEGORY_MARKETING = "marketing"

        /** Handover-inspection walkaround photos. Never offered in the capture
         *  picker: inspection shots only come from the inspection camera loop. */
        const val CATEGORY_INSPECTION = "inspection"

        /**
         * Category labels come from the server's lookups (`GET /config/lookups`);
         * an unknown key reads as itself. The `CATEGORY_*` keys above are the
         * wire values and stay.
         */
        fun label(category: String, lookups: Lookups?): String =
            lookups?.imageCategories?.firstOrNull { it.key == category }?.labelHu
                ?.takeIf { it.isNotBlank() } ?: category

        /**
         * The categories a user may pick when attaching photos by hand. Intake
         * and handover shots are evidence with their own flows (bevétel,
         * átadás-átvétel) and must never come from the gallery or an ad-hoc
         * camera tap — per the client.
         */
        fun attachable(lookups: Lookups?): List<String> =
            lookups?.imageCategories?.filter { it.attachable }?.map { it.key }
                ?.takeIf { it.isNotEmpty() } ?: listOf(CATEGORY_PRODUCTION)

        /** Irreversible once uploaded: the database trigger refuses delete and re-filing. */
        fun isImmutable(category: String, lookups: Lookups?): Boolean =
            lookups?.imageCategories?.firstOrNull { it.key == category }?.immutable
                ?: (category == CATEGORY_INTAKE)
    }
}
