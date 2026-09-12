package hu.autotherm.autocrm.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
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
        /** The last session in which the intake warning was acknowledged. */
        val INTAKE_ACK_AT = longPreferencesKey("intake_acknowledged_at")
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

    /**
     * True when the intake banner still has to be shown. Acknowledgement lasts one hour:
     * long enough not to nag through a single van's intake set, short enough that tomorrow's
     * first photo asks again. A photo filed as intake by mistake can never be deleted or
     * moved, so "confirm once, forever" is the wrong trade.
     */
    suspend fun needsIntakeConfirmation(): Boolean {
        val acknowledged = context.captureDataStore.data.first()[Keys.INTAKE_ACK_AT] ?: 0
        return System.currentTimeMillis() - acknowledged > 60 * 60 * 1000
    }

    suspend fun acknowledgeIntake() {
        context.captureDataStore.edit { it[Keys.INTAKE_ACK_AT] = System.currentTimeMillis() }
    }

    companion object {
        const val CATEGORY_INTAKE = "intake"
        const val CATEGORY_PRODUCTION = "production"
        const val CATEGORY_COMPLETION = "completion"
        const val CATEGORY_MARKETING = "marketing"

        /** Order matches `image_category` in 0002_partners_leads.sql. */
        val ALL = listOf(CATEGORY_INTAKE, CATEGORY_PRODUCTION, CATEGORY_COMPLETION, CATEGORY_MARKETING)

        fun label(category: String): String = when (category) {
            CATEGORY_INTAKE -> "Bevétel"
            CATEGORY_PRODUCTION -> "Gyártás"
            CATEGORY_COMPLETION -> "Átadás/MEO"
            CATEGORY_MARKETING -> "Referencia"
            else -> category
        }

        /** Irreversible once uploaded: the database trigger refuses delete and re-filing. */
        fun isImmutable(category: String): Boolean = category == CATEGORY_INTAKE
    }
}
