package hu.autotherm.autocrm.data.notifications

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first

private val Context.notificationDataStore by preferencesDataStore(name = "notification_cursor")

/**
 * The id of the newest feed item this phone has already considered, per signed-in user, so a
 * second account on the same phone never inherits the first one's place in the feed.
 */
class NotificationCursor(private val context: Context) {

    private val lastId = longPreferencesKey("last_id")
    private val owner = longPreferencesKey("owner_user_id")

    suspend fun get(userId: Long): Long? {
        val prefs = context.notificationDataStore.data.first()
        return if (prefs[owner] == userId) prefs[lastId] else null
    }

    suspend fun set(userId: Long, id: Long) {
        context.notificationDataStore.edit {
            it[owner] = userId
            it[lastId] = id
        }
    }
}
