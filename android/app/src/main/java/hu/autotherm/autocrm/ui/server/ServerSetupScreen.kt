package hu.autotherm.autocrm.ui.server

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.BuildConfig
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.prefs.ServerStore
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.theme.Done
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ServerSetupViewModel(
    private val serverStore: ServerStore,
    private val api: AutoCrmApi,
    private val sessionStore: SessionStore,
) : ViewModel() {

    data class State(
        val address: String = "",
        val checking: Boolean = false,
        /** Null before a check, true/false after one. */
        val reachable: Boolean? = null,
        val error: String? = null,
        /** Cleartext to something that is not on a local network. */
        val insecureWarning: Boolean = false,
        val saved: Boolean = false,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** Prefills the stored address if there is one, else the build's default. Never
     * overwrites in-progress edits: this re-runs on rotation and the ViewModel survives it. */
    fun prefill() {
        if (_state.value.address.isNotBlank()) return
        viewModelScope.launch {
            val existing = serverStore.current()
            if (_state.value.address.isBlank()) {
                _state.value = _state.value.copy(address = existing ?: BuildConfig.API_BASE_URL)
            }
        }
    }

    fun setAddress(value: String) {
        // Any edit invalidates the previous verdict: a green tick next to an address that
        // has since been retyped is worse than no tick.
        _state.value = _state.value.copy(
            address = value,
            reachable = null,
            error = null,
            insecureWarning = ServerStore.normalize(value)
                ?.let(ServerStore::isUnencryptedAndRemote) == true,
        )
    }

    /**
     * Checks before saving. Typing an IP is exactly the moment someone gets a digit wrong,
     * and finding out at the login screen — where the error looks like a bad password —
     * is how an afternoon gets lost.
     */
    fun check() {
        val normalized = ServerStore.normalize(_state.value.address)
        if (normalized == null) {
            _state.value = _state.value.copy(error = "Ez nem használható cím.", reachable = null)
            return
        }
        _state.value = _state.value.copy(checking = true, error = null, reachable = null)
        viewModelScope.launch {
            val ok = api.probe(normalized)
            _state.value = _state.value.copy(
                checking = false,
                reachable = ok,
                address = normalized,
                error = if (ok) null else "Nem válaszol. Fut a szerver, és ugyanazon a hálózaton vagy?",
            )
        }
    }

    /** Saving without a successful check is allowed: the server may be starting later. */
    fun save(onSaved: () -> Unit) {
        val normalized = ServerStore.normalize(_state.value.address)
        if (normalized == null) {
            _state.value = _state.value.copy(error = "Ez nem használható cím.")
            return
        }
        viewModelScope.launch {
            serverStore.save(normalized)
            // A bearer token belongs to one server: pointing the phone elsewhere must drop
            // it, or the old token is sent to the new host and every call 401s.
            sessionStore.clear()
            _state.value = _state.value.copy(saved = true)
            onSaved()
        }
    }
}

/**
 * The first screen, and the only one that appears before a login.
 *
 * The address is a phone-level setting, not a build constant, so this exists before there
 * is an account to sign into. It is also reachable from the login screen, because the
 * commonest time to discover the address is wrong is when a login will not go through.
 */
@Composable
fun ServerSetupScreen(
    viewModel: ServerSetupViewModel,
    onSaved: () -> Unit,
    onCancel: (() -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.prefill() }

    hu.autotherm.autocrm.ui.common.CenteredScrollColumn {
        Text("AUTOTHERM", style = MaterialTheme.typography.headlineMedium)
        Text("Kiszolgáló beállítása", style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(8.dp))
        Text(
            "Add meg az AutoCRM szerver címét. Ez a telefonon marad, kijelentkezés után is.",
            style = MaterialTheme.typography.bodyLarge,
            color = Steel500,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(24.dp))

        AutoCrmTextField(
            value = state.address,
            onValueChange = viewModel::setAddress,
            label = "Cím",
            placeholder = { Text("192.168.1.10:8080") },
            enabled = !state.checking,
            textStyle = MonoSmall.copy(fontSize = MaterialTheme.typography.bodyLarge.fontSize),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Uri,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(onDone = { viewModel.check() }),
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            "Séma nélkül: helyi címhez http://, minden máshoz https:// kerül.",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
            modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
        )

        when {
            state.checking -> StatusLine("Ellenőrzés…", Steel500)
            state.reachable == true -> StatusLine("A szerver válaszol.", Done)
            state.error != null -> StatusLine(state.error!!, Signal)
        }

        // Helyi hálózaton a titkosítatlan kapcsolat vállalható; az interneten nem.
        if (state.insecureWarning) {
            StatusLine(
                "Ez a cím nem helyi hálózati és titkosítatlan: a jelszó olvashatóan utazik. " +
                    "Éles szerverhez használj https:// címet.",
                Signal,
            )
        }

        Spacer(Modifier.height(24.dp))
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedButton(
                onClick = viewModel::check,
                enabled = !state.checking && state.address.isNotBlank(),
                modifier = Modifier.weight(1f),
            ) { Text("Ellenőrzés") }
            PrimaryButton(
                text = "Mentés",
                onClick = { viewModel.save(onSaved) },
                enabled = !state.checking && state.address.isNotBlank(),
                modifier = Modifier.weight(1f),
            )
        }

        if (onCancel != null) {
            Spacer(Modifier.height(8.dp))
            TextButton(onClick = onCancel) { Text("Mégse") }
        }
    }
}

@Composable
private fun StatusLine(text: String, color: androidx.compose.ui.graphics.Color) {
    Spacer(Modifier.height(12.dp))
    Text(text, style = MaterialTheme.typography.bodyLarge, color = color, textAlign = TextAlign.Center)
}
