package hu.autotherm.autocrm.ui.emails

import androidx.compose.foundation.layout.Arrangement
import hu.autotherm.autocrm.ui.common.clearFocusOnTap
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.ComposeBody
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.FieldLabel
import hu.autotherm.autocrm.ui.common.FormError
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Panel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Close enough to what the server's normalize_address accepts to catch typos early. */
private val EMAIL = Regex("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$")

/**
 * A short manual letter from the shop floor: who, what about, the text. Templates,
 * heroes and attachments stay on the desktop — this covers "the paint shop needs
 * the plate number today".
 */
class EmailComposeViewModel(private val api: AutoCrmApi) : ViewModel() {

    /** An address the record already knows, offered as a one-tap recipient. */
    data class Known(val label: String, val email: String)

    data class State(
        val to: String = "",
        val subject: String = "",
        val body: String = "",
        val known: List<Known> = emptyList(),
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun prefill(orderId: Long?, leadId: Long?, partnerId: Long? = null) {
        viewModelScope.launch {
            try {
                when {
                    orderId != null -> {
                        val detail = api.order(orderId)
                        val partner = runCatching { api.partner(detail.order.partnerId) }.getOrNull()
                        val known = buildList {
                            partner?.partner?.email?.let { add(Known(partner.partner.name, it)) }
                            partner?.contacts.orEmpty().forEach { c -> c.email?.let { add(Known(c.name, it)) } }
                        }.distinctBy { it.email.lowercase() }
                        // The order's own contact first: that is who the job is with.
                        val contactEmail = partner?.contacts?.firstOrNull { it.id == detail.order.contactId }?.email
                        _state.value = _state.value.copy(
                            to = _state.value.to.ifBlank { contactEmail ?: known.firstOrNull()?.email.orEmpty() },
                            subject = _state.value.subject.ifBlank { "#${detail.order.number} · ${detail.order.title}" },
                            known = known,
                        )
                    }
                    partnerId != null -> {
                        val detail = api.partner(partnerId)
                        val known = buildList {
                            detail.partner.email?.let { add(Known(detail.partner.name, it)) }
                            detail.contacts.forEach { c -> c.email?.let { add(Known(c.name, it)) } }
                        }.distinctBy { it.email.lowercase() }
                        _state.value = _state.value.copy(
                            to = _state.value.to.ifBlank { known.firstOrNull()?.email.orEmpty() },
                            known = known,
                        )
                    }
                    leadId != null -> {
                        val lead = api.lead(leadId).lead
                        val known = listOfNotNull(
                            lead.contactEmail?.let { Known(lead.contactName ?: it, it) },
                        )
                        _state.value = _state.value.copy(
                            to = _state.value.to.ifBlank { lead.contactEmail.orEmpty() },
                            subject = _state.value.subject.ifBlank { lead.title },
                            known = known,
                        )
                    }
                }
            } catch (e: Throwable) {
                // Prefill is a courtesy; a failure here must not block composing.
            }
        }
    }

    /** Recipient and subject handed over by the screen that opened this one. */
    fun seed(to: String?, subject: String?) {
        val s = _state.value
        _state.value = s.copy(
            to = s.to.ifBlank { to.orEmpty() },
            subject = s.subject.ifBlank { subject.orEmpty() },
        )
    }

    fun set(next: (State) -> State) {
        _state.value = next(_state.value).copy(error = null)
    }

    /** What the server would refuse (`recipient address is invalid`, `subject/body is required`). */
    private fun problem(s: State): String? = when {
        s.to.isBlank() -> "Add meg a címzettet."
        !EMAIL.matches(s.to.trim()) -> "A címzett e-mail címe nem érvényes."
        s.subject.isBlank() -> "A tárgy kötelező."
        s.body.isBlank() -> "Az üzenet szövege kötelező."
        else -> null
    }

    fun send(orderId: Long?, leadId: Long?, onSent: (Long) -> Unit, partnerId: Long? = null) {
        val s = _state.value
        if (s.busy) return
        problem(s)?.let {
            _state.value = s.copy(error = it)
            return
        }
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            try {
                val sent = api.sendEmail(
                    ComposeBody(
                        orderId = orderId,
                        leadId = leadId,
                        // The server takes one subject: the job or enquiry wins over the partner.
                        partnerId = partnerId.takeIf { orderId == null && leadId == null },
                        to = s.to.trim(),
                        subject = s.subject.trim(),
                        body = s.body.trim(),
                    ),
                )
                _state.value = _state.value.copy(busy = false)
                hu.autotherm.autocrm.ui.common.Toasts.show("Levél elküldve: " + s.to.trim())
                onSent(sent.id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun EmailComposeScreen(
    orderId: Long?,
    leadId: Long?,
    viewModel: EmailComposeViewModel,
    onBack: () -> Unit,
    onSent: (Long) -> Unit,
    partnerId: Long? = null,
    initialTo: String? = null,
    initialSubject: String? = null,
) {
    val state by viewModel.state.collectAsState()
    // A typed message is work; leaving asks first.
    val guardedBack = hu.autotherm.autocrm.ui.common.rememberDiscardGuard(state.body.isNotBlank() && !state.busy, onBack)
    LaunchedEffect(orderId, leadId, partnerId) {
        // Seeded first: the record's prefill only fills what is still empty.
        viewModel.seed(initialTo, initialSubject)
        viewModel.prefill(orderId, leadId, partnerId)
    }

    Scaffold(
        containerColor = Panel,
        topBar = {
            ScreenTopBar(title = "Új e-mail", onBack = guardedBack)
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().clearFocusOnTap()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            AutoCrmTextField(
                value = state.to,
                onValueChange = { v -> viewModel.set { it.copy(to = v.trim()) } },
                label = "Címzett *",
                isError = state.to.isNotBlank() && !EMAIL.matches(state.to.trim()),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                modifier = Modifier.fillMaxWidth(),
            )
            if (state.known.size > 1 || (state.known.size == 1 && state.known[0].email != state.to.trim())) {
                FieldLabel("Ismert címek")
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    state.known.forEach { k ->
                        FilterChip(
                            selected = state.to.trim().equals(k.email, ignoreCase = true),
                            onClick = { viewModel.set { it.copy(to = k.email) } },
                            label = { Text("${k.label} · ${k.email}", maxLines = 1) },
                        )
                    }
                }
            }
            AutoCrmTextField(
                value = state.subject,
                onValueChange = { v -> viewModel.set { it.copy(subject = v) } },
                label = "Tárgy *",
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.body,
                onValueChange = { v -> viewModel.set { it.copy(body = v) } },
                label = "Üzenet *",
                singleLine = false,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                modifier = Modifier.fillMaxWidth().heightIn(min = 160.dp),
            )
            FormError(state.error)
            PrimaryButton(
                text = if (state.busy) "Küldés…" else "Küldés",
                onClick = { viewModel.send(orderId, leadId, onSent, partnerId) },
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(24.dp))
        }
    }
}
