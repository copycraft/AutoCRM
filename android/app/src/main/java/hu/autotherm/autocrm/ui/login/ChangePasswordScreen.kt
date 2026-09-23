package hu.autotherm.autocrm.ui.login

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ChangePasswordViewModel(
    private val api: AutoCrmApi,
    private val sessionStore: SessionStore,
) : ViewModel() {

    data class State(
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun changePassword(current: String, next: String, onDone: () -> Unit) {
        if (_state.value.busy) return
        if (next.length < 12) {
            _state.value = State(error = "Az új jelszó legalább 12 karakter legyen.")
            return
        }
        if (current == next) {
            _state.value = State(error = "Az új jelszó nem egyezhet a régivel.")
            return
        }
        _state.value = State(busy = true)
        viewModelScope.launch {
            try {
                api.changePassword(current, next)
                // Re-read me: the flag flips server-side, and the account flow
                // re-composes MainActivity into the app. Without this the gate
                // still sees must_change_password and loops back here.
                val me = api.me()
                val token = sessionStore.token()
                if (token == null) {
                    _state.value = State(error = "A munkamenet lejárt. Jelentkezz be újra.")
                    return@launch
                }
                sessionStore.save(token, null, me.user)
                onDone()
            } catch (e: ApiException) {
                _state.value = State(error = message(e))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.value = State(error = "Nem sikerült jelszót változtatni.")
            }
        }
    }

    fun signOut(onDone: () -> Unit) {
        viewModelScope.launch {
            runCatching { api.logout() }
            sessionStore.clear()
            onDone()
        }
    }

    private fun message(e: ApiException): String = when (e) {
        is ApiException.Unauthenticated -> "Hibás e-mail vagy jelszó."
        is ApiException.Rule -> when (e.code) {
            "wrong_password" -> "Hibás a jelenlegi jelszó."
            "account_locked" -> "A fiók zárolva. Szólj az irodának."
            else -> e.detail ?: "Nem sikerült jelszót változtatni."
        }
        is ApiException.Network -> "Nincs kapcsolat a szerverrel."
        else -> e.message ?: "Nem sikerült jelszót változtatni."
    }
}

@Composable
fun ChangePasswordScreen(
    viewModel: ChangePasswordViewModel,
    onChanged: () -> Unit,
    onSignedOut: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    var current by rememberSaveable { mutableStateOf("") }
    var next by rememberSaveable { mutableStateOf("") }
    var showPasswords by rememberSaveable { mutableStateOf(false) }
    val transformation = if (showPasswords) VisualTransformation.None else PasswordVisualTransformation()

    Column(
        Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("Jelszócsere szükséges", style = MaterialTheme.typography.headlineMedium)
        Text(
            "Első belépéskor új jelszót kell megadnod.",
            style = MaterialTheme.typography.bodyLarge,
            color = Steel500,
        )
        Spacer(Modifier.height(32.dp))

        AutoCrmTextField(
            value = current,
            onValueChange = { current = it },
            label = "Jelenlegi jelszó",
            enabled = !state.busy,
            visualTransformation = transformation,
            trailing = {
                IconButton(onClick = { showPasswords = !showPasswords }) {
                    Icon(
                        if (showPasswords) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                        contentDescription = if (showPasswords) "Jelszó elrejtése" else "Jelszó mutatása",
                    )
                }
            },
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Password,
                imeAction = ImeAction.Next,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(12.dp))
        AutoCrmTextField(
            value = next,
            onValueChange = { next = it },
            label = "Új jelszó (min. 12 karakter)",
            enabled = !state.busy,
            visualTransformation = transformation,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Password,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(
                onDone = { viewModel.changePassword(current, next, onChanged) },
            ),
            modifier = Modifier.fillMaxWidth(),
        )

        state.error?.let {
            Spacer(Modifier.height(12.dp))
            Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal)
        }

        Spacer(Modifier.height(24.dp))
        PrimaryButton(
            text = if (state.busy) "Mentés…" else "Jelszó módosítása",
            onClick = { viewModel.changePassword(current, next, onChanged) },
            enabled = !state.busy && current.isNotBlank() && next.isNotBlank(),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(8.dp))
        TextButton(
            onClick = { viewModel.signOut(onSignedOut) },
            enabled = !state.busy,
        ) { Text("Kijelentkezés") }
    }
}
