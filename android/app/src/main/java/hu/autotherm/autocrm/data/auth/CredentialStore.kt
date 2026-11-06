package hu.autotherm.autocrm.data.auth

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The remembered e-mail + password for quick sign-in ("Bejelentkezés mentett fiókkal"),
 * after a session expiry or a fresh install on the phone the fitter uses alone.
 *
 * Deliberately unlike [SessionStore]'s plaintext token, because a password is not
 * revocable: it is a long-lived secret (and usually the office staff's AD password).
 * The blob is AES/GCM ciphertext under an AndroidKeyStore key the other apps never
 * see; app-private storage holds only the ciphertext and the IV. On a rooted device
 * (see SessionStore's reasoning) this is not bulletproof either — the key file is
 * beside every other system secret — but it is the strongest practical form for a
 * shop-floor phone, and the explicit "remember" checkbox keeps it opt-in.
 *
 * One device one credential: the phone's last (verified) sign-in overwrites the blob.
 * Signing out clears it entirely — the next user of this shared phone must not be able
 * to tap themselves in as the last one (MainActivity does that next to session clear).
 */
object CredentialStore {

    data class Saved(val email: String, val password: String)

    private const val PREFS = "secure_credentials"
    private const val KEY_ALIAS = "autocrm_credentials"
    private const val BLOB = "blob"
    private const val TAG = "saved:"
    private const val SEPARATOR = "|"

    private fun masterKey(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val generator = KeyGenerator.getInstance("AES", "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    fun save(context: Context, email: String, password: String) {
        try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, masterKey())
            // A random IV per cipher, stored beside the ciphertext: GCM must never reuse one.
            val payload = cipher.iv + cipher.doFinal((email + SEPARATOR + password).toByteArray(Charsets.UTF_8))
            prefs(context).edit().putString(BLOB, TAG + Base64.encodeToString(payload, Base64.NO_WRAP)).apply()
        } catch (_: Exception) {
            // The phone's keystore is broken or degraded: forget it, never fail the login
            // because remembering was not possible.
            clear(context)
        }
    }

    /** Null when nothing is saved or the blob no longer decrypts (stale key, new device copy). */
    fun load(context: Context): Saved? {
        val stored = prefs(context).getString(BLOB, null)?.takeIf { it.startsWith(TAG) } ?: return null
        return try {
            val payload = Base64.decode(stored.substring(TAG.length), Base64.NO_WRAP)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, masterKey(), GCMParameterSpec(128, payload, 0, 12))
            val plain = String(cipher.doFinal(payload, 12, payload.size - 12), Charsets.UTF_8)
            val split = plain.indexOf(SEPARATOR)
            if (split <= 0) return null
            Saved(plain.substring(0, split), plain.substring(split + SEPARATOR.length))
        } catch (_: Exception) {
            clear(context)
            null
        }
    }

    fun clear(context: Context) {
        prefs(context).edit().remove(BLOB).apply()
    }

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, android.content.Context.MODE_PRIVATE)
}
