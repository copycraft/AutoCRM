package hu.autotherm.autocrm.data.auth

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
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
        val HR_ACCESS = booleanPreferencesKey("hr_access")
        val CAPABILITIES = stringSetPreferencesKey("capabilities")
    }

    data class Account(
        val userId: Long,
        val email: String,
        val displayName: String,
        val role: String,
        val mustChangePassword: Boolean,
        val hrAccess: Boolean = false,
        val capabilities: Set<String> = emptySet(),
    ) {
        /**
         * The server's effective capabilities (role defaults plus per-user grants, see
         * `domain/role.rs`). UI-only: the server is the security boundary, and every one of
         * these is re-checked there. Hiding a button the API would refuse is a courtesy, not a
         * control. A session saved before the server sent capabilities falls back to the role.
         */
        private fun has(capability: String, vararg roles: String): Boolean =
            if (capabilities.isEmpty()) role in roles else capability in capabilities
        val canEdit: Boolean get() = has("edit_orders", "admin", "office")
        val canChangeStage: Boolean get() = has("change_stages", "admin", "office", "designer")
        val canUploadMedia: Boolean get() = has("upload_media", "admin", "office", "designer")
        val canComment: Boolean get() = has("comment", "admin", "office", "designer")
        // One flag per server capability a screen acts on, so a per-user grant (or its
        // absence) shows the same buttons the API will accept.
        val canEditLeads: Boolean get() = has("edit_leads", "admin", "office")
        val canEditPartners: Boolean get() = has("edit_partners", "admin", "office")
        val canSendEmail: Boolean get() = has("send_email", "admin", "office")
        val canManageBlockers: Boolean get() = has("manage_blockers", "admin", "office", "designer")
        val isAdmin: Boolean get() = role == "admin"
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
            hrAccess = this[Keys.HR_ACCESS] ?: false,
            capabilities = this[Keys.CAPABILITIES] ?: emptySet(),
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
            it[Keys.HR_ACCESS] = user.hrAccess
            it[Keys.CAPABILITIES] = user.capabilities.toSet()
        }
    }


    /**
     * Refreshes who the server says this is, keeping the token. An admin can grant or take
     * away HR access (or change a role) at any time; the drawer must follow without a
     * sign-in. A no-op when nobody is signed in.
     */
    suspend fun refreshUser(user: hu.autotherm.autocrm.data.api.SessionUser) {
        context.sessionDataStore.edit {
            if (it[Keys.TOKEN].isNullOrBlank()) return@edit
            it[Keys.USER_ID] = user.id
            it[Keys.EMAIL] = user.email
            it[Keys.DISPLAY_NAME] = user.displayName
            it[Keys.ROLE] = user.role
            it[Keys.MUST_CHANGE_PASSWORD] = user.mustChangePassword
            it[Keys.HR_ACCESS] = user.hrAccess
            it[Keys.CAPABILITIES] = user.capabilities.toSet()
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
