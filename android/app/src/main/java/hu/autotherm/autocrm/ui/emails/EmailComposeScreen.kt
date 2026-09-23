package hu.autotherm.autocrm.ui.emails

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.ComposeBody
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Signal
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * A short manual letter from the shop floor: who, what about, the text. Templates,
 * heroes and attachments stay on the desktop — this covers "the paint shop needs
 * the plate number today".
 */
class EmailComposeViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val to: String = "",
        val subject: String = "",
        val body: String = "",
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun prefill(orderId: Long?, leadId: Long?) {
        viewModelScope.launch {
            try {
                when {
                    orderId != null -> {
                        val orderDeferred = async { api.order(orderId) }
                        val detail = orderDeferred.await()
                        val email = runCatching { api.partner(detail.order.partnerId).partner.email }
                            .getOrNull().orEmpty()
                        _state.value = _state.value.copy(
                            to = email,
                            subject = "#${detail.order.number} · ${detail.order.title}",
                        )
                    }
                    leadId != null -> {
                        val lead = api.lead(leadId).lead
                        _state.value = _state.value.copy(
                            to = lead.contactEmail.orEmpty(),
                            subject = lead.title,
                        )
                    }
                }
            } catch (e: Throwable) {
                // Prefill is a courtesy; a failure here must not block composing.
            }
        }
    }

    fun set(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    fun send(orderId: Long?, leadId: Long?, onSent: (Long) -> Unit) {
        val s = _state.value
        if (s.busy || s.to.isBlank()) return
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            try {
                val sent = api.sendEmail(
                    ComposeBody(
                        orderId = orderId,
                        leadId = leadId,
                        to = s.to.trim(),
                        subject = s.subject.trim().takeIf { it.isNotBlank() },
                        body = s.body.trim().takeIf { it.isNotBlank() },
                    ),
                )
                _state.value = _state.value.copy(busy = false)
                onSent(sent.id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EmailComposeScreen(
    orderId: Long?,
    leadId: Long?,
    viewModel: EmailComposeViewModel,
    onBack: () -> Unit,
    onSent: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(orderId, leadId) { viewModel.prefill(orderId, leadId) }

    Scaffold(
        topBar = {
            ScreenTopBar(title = "Új e-mail", onBack = onBack)
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            AutoCrmTextField(
                value = state.to,
                onValueChange = { v -> viewModel.set { it.copy(to = v) } },
                label = "Címzett *",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.subject,
                onValueChange = { v -> viewModel.set { it.copy(subject = v) } },
                label = "Tárgy",
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.body,
                onValueChange = { v -> viewModel.set { it.copy(body = v) } },
                label = "Szöveg",
                singleLine = false,
                modifier = Modifier.fillMaxWidth(),
            )
            state.error?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal)
            }
            PrimaryButton(
                text = if (state.busy) "Küldés…" else "Küldés",
                onClick = { viewModel.send(orderId, leadId, onSent) },
                enabled = !state.busy && state.to.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
