package hu.autotherm.autocrm.ui.common

import hu.autotherm.autocrm.ui.common.copyOnLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Comment
import hu.autotherm.autocrm.data.api.CommentBody
import hu.autotherm.autocrm.data.api.Mentionable
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.formatDateTime
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** The ids of the people whose `@Name` is in the text. */
fun mentionedIds(body: String, people: List<Mentionable>): List<Long> =
    people.filter { body.contains("@${it.displayName}") }.map { it.id }

class CommentsViewModel(private val api: AutoCrmApi) : ViewModel() {
    data class State(
        val loading: Boolean = true,
        val comments: List<Comment> = emptyList(),
        val people: List<Mentionable> = emptyList(),
        val sending: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(entity: String, id: Long) {
        viewModelScope.launch {
            try {
                val comments = api.comments(entity, id)
                val people = runCatching { api.mentionable() }.getOrDefault(_state.value.people)
                _state.value = _state.value.copy(loading = false, comments = comments, people = people, error = null)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    fun send(entity: String, id: Long, body: String, onSent: () -> Unit) {
        val text = body.trim()
        if (text.isEmpty()) return
        viewModelScope.launch {
            _state.value = _state.value.copy(sending = true, error = null)
            try {
                api.createComment(CommentBody(entity, id, text, mentionedIds(text, _state.value.people)))
                onSent()
                _state.value = _state.value.copy(sending = false)
                Toasts.show("Megjegyzés elküldve")
                load(entity, id)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(sending = false, error = describeError(e))
            }
        }
    }
}

/**
 * Staff comments on an order or a lead, with mentions: the @ button lists the people who
 * can be named, and whoever is named gets a notification. Writing needs a connection.
 */
@Composable
fun CommentsSection(entity: String, id: Long, canComment: Boolean) {
    val context = LocalContext.current
    val vm: CommentsViewModel = viewModel(key = "comments-$entity-$id") {
        CommentsViewModel((context.applicationContext as AutoCrmApp).api)
    }
    val state by vm.state.collectAsState()
    // Saveable: the section lives in a scrolling list, and a plain remember dropped a
    // half-typed comment as soon as it scrolled off screen (or the phone rotated).
    var text by androidx.compose.runtime.saveable.rememberSaveable(entity, id) { mutableStateOf("") }
    var picking by remember { mutableStateOf(false) }
    LaunchedEffect(entity, id) { vm.load(entity, id) }

    Card {
        SectionTitle("Megjegyzések", count = state.comments.size.takeIf { it > 0 })
        if (!state.loading && state.comments.isEmpty()) {
            Text("Még nincs megjegyzés.", style = MaterialTheme.typography.bodyLarge, color = Steel500)
        }
        val me by (context.applicationContext as AutoCrmApp).sessionStore.account.collectAsState(initial = null)
        state.comments.forEach { c ->
            Text(
                // "Te" for your own: the thread reads like a conversation.
                "${if (c.createdBy == me?.userId) "Te" else c.authorName} · ${hu.autotherm.autocrm.util.relativeTime(c.createdAt).orEmpty()}" + if (c.editedAt != null) " (szerkesztve)" else "",
                style = MaterialTheme.typography.labelMedium,
                color = Steel500,
                modifier = Modifier.padding(top = 4.dp),
            )
            // Press and hold to copy: a part number or an address out of a comment.
            Text(c.body, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.copyOnLongPress(c.body))
        }
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        if (canComment) {
            AutoCrmTextField(
                value = text,
                onValueChange = { v ->
                    // Typing "@" at the start of a word opens the people list, as in any chat.
                    val typedAt = v.length == text.length + 1 && v.endsWith("@") &&
                        (v.length == 1 || v[v.length - 2].isWhitespace())
                    if (typedAt && state.people.isNotEmpty()) {
                        text = v.dropLast(1)
                        picking = true
                    } else {
                        text = v
                    }
                },
                label = "Új megjegyzés (@ megemlítés)",
                singleLine = false,
                modifier = Modifier.fillMaxWidth(),
                trailing = { DictationButton(onText = { text = appendDictated(text, it) }) },
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                TextButton(onClick = { picking = true }, enabled = state.people.isNotEmpty()) { Text("@ Megemlítés") }
                TextButton(
                    onClick = { vm.send(entity, id, text) { text = "" } },
                    enabled = text.isNotBlank() && !state.sending,
                ) { Text(if (state.sending) "Küldés…" else "Küldés") }
            }
        }
    }

    if (picking) {
        DialogShell(
            title = "Kit említ meg?",
            onDismiss = { picking = false },
            actions = { TextButton(onClick = { picking = false }) { Text("Mégse") } },
        ) {
            state.people.forEach { p ->
                TextButton(
                    onClick = {
                        text = (if (text.isBlank() || text.endsWith(" ")) text else "$text ") + "@${p.displayName} "
                        picking = false
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(p.displayName) }
            }
        }
    }
}
