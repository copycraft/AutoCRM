package hu.autotherm.autocrm.ui.common

import android.content.Context
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500

/**
 * App lock (0049): with it on, the app asks for the fingerprint, face or the phone's own
 * PIN/pattern when it comes back after a few minutes away. The session itself stays; this
 * only keeps a phone left on a workbench from showing customers' details to whoever picks
 * it up.
 */
object AppLock {
    private const val PREFS = "app_lock"
    private const val KEY_ENABLED = "enabled"
    /** Away longer than this, and the lock comes back. */
    const val TIMEOUT_MS = 2 * 60_000L

    private var backgroundedAt: Long = 0L

    /** Whether the gate is showing. Starts locked: a cold start asks too. */
    var locked by mutableStateOf(true)

    fun isEnabled(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(KEY_ENABLED, false)

    fun setEnabled(context: Context, enabled: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putBoolean(KEY_ENABLED, enabled).apply()
        if (!enabled) locked = false
    }

    /** The phone can confirm the owner at all (a screen lock or a biometric is set up). */
    fun available(context: Context): Boolean =
        BiometricManager.from(context).canAuthenticate(BIOMETRIC_WEAK or DEVICE_CREDENTIAL) ==
            BiometricManager.BIOMETRIC_SUCCESS

    fun onBackground() {
        backgroundedAt = System.currentTimeMillis()
    }

    fun onForeground(context: Context) {
        if (!isEnabled(context) || !available(context)) {
            locked = false
            return
        }
        if (backgroundedAt != 0L && System.currentTimeMillis() - backgroundedAt > TIMEOUT_MS) locked = true
    }

    fun prompt(activity: FragmentActivity, onError: (String) -> Unit) {
        val prompt = BiometricPrompt(
            activity,
            ContextCompat.getMainExecutor(activity),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    locked = false
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    if (errorCode != BiometricPrompt.ERROR_USER_CANCELED &&
                        errorCode != BiometricPrompt.ERROR_CANCELED
                    ) {
                        onError(errString.toString())
                    }
                }
            },
        )
        prompt.authenticate(
            BiometricPrompt.PromptInfo.Builder()
                .setTitle("AutoCRM feloldása")
                .setSubtitle("Ujjlenyomat, arc vagy a telefon PIN-kódja")
                .setAllowedAuthenticators(BIOMETRIC_WEAK or DEVICE_CREDENTIAL)
                .build(),
        )
    }
}

/** Shows [content], or the lock screen while [AppLock.locked]. */
@Composable
fun AppLockGate(content: @Composable () -> Unit) {
    val context = LocalContext.current
    val enabled = remember { AppLock.isEnabled(context) && AppLock.available(context) }
    if (!enabled) {
        content()
        return
    }
    if (!AppLock.locked) {
        content()
        return
    }
    var error by remember { mutableStateOf<String?>(null) }
    val activity = context as? FragmentActivity
    LaunchedEffect(Unit) { activity?.let { AppLock.prompt(it) { e -> error = e } } }
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(Icons.Filled.Lock, contentDescription = null, tint = Steel500)
        Spacer(Modifier.height(16.dp))
        Text("Az AutoCRM zárolva", style = MaterialTheme.typography.titleLarge)
        Text(
            "Oldd fel ujjlenyomattal, arccal vagy a telefon PIN-kódjával.",
            style = MaterialTheme.typography.bodyMedium,
            color = Steel500,
        )
        error?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, color = Signal)
        }
        Spacer(Modifier.height(24.dp))
        PrimaryButton(
            text = "Feloldás",
            onClick = { activity?.let { AppLock.prompt(it) { e -> error = e } } },
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** The switch on the settings screen. */
@Composable
fun AppLockSetting() {
    val context = LocalContext.current
    val available = remember { AppLock.available(context) }
    var enabled by remember { mutableStateOf(AppLock.isEnabled(context)) }
    Card {
        Text("Alkalmazászár", style = MaterialTheme.typography.titleLarge)
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f, fill = false)) {
                Text("Feloldás ujjlenyomattal / PIN-nel", style = MaterialTheme.typography.bodyLarge)
                Text(
                    if (available) {
                        "Ha 2 percnél tovább volt háttérben, az alkalmazás azonosítást kér."
                    } else {
                        "Ehhez a telefonon képernyőzárat vagy ujjlenyomatot kell beállítani."
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
            }
            Switch(
                checked = enabled,
                enabled = available,
                onCheckedChange = {
                    enabled = it
                    AppLock.setEnabled(context, it)
                    // Turning it on does not lock straight away; leaving the app does.
                    if (it) AppLock.locked = false
                },
            )
        }
    }
}
