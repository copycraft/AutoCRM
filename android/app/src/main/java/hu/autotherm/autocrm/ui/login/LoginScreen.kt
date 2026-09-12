package hu.autotherm.autocrm.ui.login

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class LoginViewModel(
    private val api: AutoCrmApi,
    private val sessionStore: SessionStore,
) : ViewModel() {

    data class State(
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun signIn(email: String, password: String, onSuccess: () -> Unit) {
        if (_state.value.busy) return
        _state.value = State(busy = true)
        viewModelScope.launch {
            try {
                // The device label lands in the session list, so an admin revoking a lost
                // phone from the web app can tell which one it is.
                val label = "${Build.MANUFACTURER} ${Build.MODEL}".trim()
                val response = api.login(email.trim(), password, label)
                val token = response.token
                if (token == null) {
                    _state.value = State(error = "a szerver nem adott munkamenet-jegyet")
                    return@launch
                }
                if (response.user.mustChangePassword) {
                    // Every other endpoint would answer 422 until the password is changed,
                    // and the phone has no screen for that. Saying so beats a working login
                    // followed by an app where nothing loads.
                    _state.value = State(
                        error = "Jelszót kell változtatni. Jelentkezz be a webes felületen először.",
                    )
                    return@launch
                }
                sessionStore.save(token, response.expiresAt, response.user)
                onSuccess()
            } catch (e: ApiException) {
                _state.value = State(error = message(e))
            }
        }
    }

    private fun message(e: ApiException): String = when (e) {
        is ApiException.Unauthenticated -> "Hibás e-mail vagy jelszó."
        is ApiException.Rule -> when (e.code) {
            "account_locked" -> "A fiók zárolva. Szólj az irodának."
            else -> e.detail ?: "Nem sikerült bejelentkezni."
        }
        is ApiException.Network -> "Nincs kapcsolat a szerverrel."
        else -> e.message ?: "Nem sikerült bejelentkezni."
    }
}

@Composable
fun LoginScreen(
    viewModel: LoginViewModel,
    serverAddress: String?,
    onChangeServer: () -> Unit,
    onSignedIn: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }

    Column(
        Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("AUTOTHERM", style = MaterialTheme.typography.headlineMedium)
        Text("AutoCRM", style = MaterialTheme.typography.bodyLarge, color = Steel500)
        Spacer(Modifier.height(32.dp))

        OutlinedTextField(
            value = email,
            onValueChange = { email = it },
            label = { Text("E-mail cím") },
            singleLine = true,
            enabled = !state.busy,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Email,
                imeAction = ImeAction.Next,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(12.dp))
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text("Jelszó") },
            singleLine = true,
            enabled = !state.busy,
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Password,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(onDone = { viewModel.signIn(email, password, onSignedIn) }),
            modifier = Modifier.fillMaxWidth(),
        )

        state.error?.let {
            Spacer(Modifier.height(12.dp))
            Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal)
        }

        Spacer(Modifier.height(24.dp))
        PrimaryButton(
            text = if (state.busy) "Bejelentkezés…" else "Bejelentkezés",
            onClick = { viewModel.signIn(email, password, onSignedIn) },
            enabled = !state.busy && email.isNotBlank() && password.isNotBlank(),
            modifier = Modifier.fillMaxWidth(),
        )

        // Which server this phone is pointed at, always visible. A login failing because
        // the address is stale looks exactly like a wrong password unless the address is
        // on screen next to the error.
        Spacer(Modifier.height(24.dp))
        Text(
            serverAddress ?: "—",
            style = MonoSmall,
            color = Steel500,
        )
        TextButton(onClick = onChangeServer) { Text("Kiszolgáló módosítása") }
    }
}
