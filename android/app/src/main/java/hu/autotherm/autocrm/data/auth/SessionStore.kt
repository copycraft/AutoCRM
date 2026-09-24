package hu.autotherm.autocrm.data.auth

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

private val Context.sessionDataStore by preferencesDataStore(name = "session")

/**
 * The bearer token and who it belongs to.
 *
 * Stored in app-private DataStore rather than EncryptedSharedPreferences. The reasoning,
 * so it can be argued with rather than rediscovered: app-private storage is already
 * unreadable by other apps on a non-rooted device, the token is a session token with a
 * server-side expiry that an admin can revoke from `/auth/sessions`, and the
 * `androidx.security-crypto` library that would encrypt it is itself deprecated. On a
 * rooted or unlocked device an attacker with the file also has the keystore-backed
 * decryption path; the encryption would buy the appearance of protection, not protection.
 *
 * What would change this: storing anything that is not revocable — a password, an API key
 * with no expiry. Then it belongs in the keystore.
 */
class SessionStore(private val context: Context) {

    private object Keys {
        val TOKEN = stringPreferencesKey("token")
        val EXPIRES_AT = stringPreferencesKey("expires_at")
        val USER_ID = longPreferencesKey("user_id")
        val DISPLAY_NAME = stringPreferencesKey("display_name")
        val EMAIL = stringPreferencesKey("email")
        val ROLE = stringPreferencesKey("role")
        val MUST_CHANGE_PASSWORD = booleanPreferencesKey("must_change_password")
    }

    data class Account(
        val userId: Long,
        val email: String,
        val displayName: String,
        val role: String,
        val mustChangePassword: Boolean,
    ) {
        /**
         * Mirrors `domain/role.rs`. UI-only: the server is the security boundary, and every
         * one of these is re-checked there. Hiding a button the API would refuse is a
         * courtesy, not a control.
         */
        val canEdit: Boolean get() = role == "admin" || role == "office"
        val canChangeStage: Boolean get() = canEdit || role == "designer"
        val canUploadMedia: Boolean get() = canChangeStage
    }

    val account: Flow<Account?> = context.sessionDataStore.data.map { it.toAccount() }

    private fun Preferences.toAccount(): Account? {
        val id = this[Keys.USER_ID] ?: return null
        if (this[Keys.TOKEN].isNullOrBlank()) return null
        return Account(
            userId = id,
            email = this[Keys.EMAIL].orEmpty(),
            displayName = this[Keys.DISPLAY_NAME].orEmpty(),
            role = this[Keys.ROLE].orEmpty(),
            mustChangePassword = this[Keys.MUST_CHANGE_PASSWORD] ?: false,
        )
    }

    suspend fun token(): String? = context.sessionDataStore.data.first()[Keys.TOKEN]

    suspend fun currentAccount(): Account? = context.sessionDataStore.data.first().toAccount()

    suspend fun save(token: String, expiresAt: String?, user: hu.autotherm.autocrm.data.api.SessionUser) {
        context.sessionDataStore.edit {
            it[Keys.TOKEN] = token
            if (expiresAt != null) it[Keys.EXPIRES_AT] = expiresAt
            it[Keys.USER_ID] = user.id
            it[Keys.EMAIL] = user.email
            it[Keys.DISPLAY_NAME] = user.displayName
            it[Keys.ROLE] = user.role
            it[Keys.MUST_CHANGE_PASSWORD] = user.mustChangePassword
        }
    }

    /**
     * Forgets the session. Deliberately does **not** touch the upload queue: photos taken
     * on this phone belong to the job, not to the session, and a fitter whose token expired
     * overnight must find their morning's work still queued when they sign back in.
     */
    suspend fun clear() {
        context.sessionDataStore.edit { it.clear() }
    }

    /**
     * Forgets the session only if it is still the one that [token] belongs to. The server
     * answered 401 for that token (expired, revoked, account disabled or re-roled); a newer
     * sign-in that happened while the request was in flight must survive it.
     */
    suspend fun clearIfToken(token: String) {
        context.sessionDataStore.edit { if (it[Keys.TOKEN] == token) it.clear() }
    }
}
