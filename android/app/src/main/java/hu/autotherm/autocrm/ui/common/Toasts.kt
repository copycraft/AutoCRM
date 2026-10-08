package hu.autotherm.autocrm.ui.common

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.composed
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel900
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/**
 * One line at the bottom of the screen, for anything that happened after the screen was
 * already showing: "Mentve", "Tétel törölve · Visszavonás", or why a tap did not work.
 * Callable from a ViewModel (no Compose needed); the scaffold's [AppSnackbarHost] shows
 * them one after the other.
 */
object Toasts {
    data class Message(
        val text: String,
        val actionLabel: String? = null,
        val isError: Boolean = false,
        val onAction: (() -> Unit)? = null,
    )

    private val _messages = MutableSharedFlow<Message>(extraBufferCapacity = 8)
    val messages: SharedFlow<Message> = _messages.asSharedFlow()

    /** A confirmation; with [actionLabel] it stays longer and offers the action (Undo). */
    fun show(text: String, actionLabel: String? = null, onAction: (() -> Unit)? = null) {
        _messages.tryEmit(Message(text, actionLabel, isError = false, onAction = onAction))
    }

    /**
     * A refusal or failure of something the person just did. With [retry], the line offers
     * "Újra" — a dropped connection costs one tap, not finding the button again.
     */
    fun error(text: String, retry: (() -> Unit)? = null) {
        _messages.tryEmit(Message(text, actionLabel = retry?.let { "Újra" }, isError = true, onAction = retry))
    }
}

@Composable
fun AppSnackbarHost(modifier: Modifier = Modifier) {
    val host = remember { SnackbarHostState() }
    var current by remember { mutableStateOf<Toasts.Message?>(null) }
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(Unit) {
        // Sequential on purpose: showSnackbar suspends until the line is gone, so a burst
        // of messages queues instead of each one cutting the last off.
        Toasts.messages.collect { message ->
            current = message
            // A firm buzz for a refusal, a light tick for "done".
            haptics.performHapticFeedback(
                if (message.isError) HapticFeedbackType.LongPress else HapticFeedbackType.TextHandleMove,
            )
            val result = host.showSnackbar(
                message = message.text,
                actionLabel = message.actionLabel,
                withDismissAction = message.isError,
                duration = when {
                    message.actionLabel != null -> SnackbarDuration.Long
                    message.isError -> SnackbarDuration.Long
                    else -> SnackbarDuration.Short
                },
            )
            if (result == SnackbarResult.ActionPerformed) message.onAction?.invoke()
        }
    }
    SnackbarHost(host, modifier) { data ->
        Snackbar(
            snackbarData = data,
            shape = RoundedCornerShape(14.dp),
            // Ink-coloured bar with the page colour as text: dark on light, light on dark.
            // (White text on the ink colour was unreadable in dark mode, where ink is light.)
            containerColor = if (current?.isError == true) Signal else Steel900,
            contentColor = if (current?.isError == true) androidx.compose.ui.graphics.Color.White else hu.autotherm.autocrm.ui.theme.Surface,
            actionColor = if (hu.autotherm.autocrm.ui.theme.isDarkTheme) androidx.compose.ui.graphics.Color(0xFF0F5C7A) else androidx.compose.ui.graphics.Color(0xFFFFD27A),
            dismissActionContentColor = if (current?.isError == true) androidx.compose.ui.graphics.Color.White else hu.autotherm.autocrm.ui.theme.Surface,
        )
    }
}

/**
 * Tapping empty space on a form puts the keyboard away, as every other app does. Taps on
 * fields and buttons are theirs (they consume them first); only the gaps reach this.
 */
fun Modifier.clearFocusOnTap(): Modifier = composed {
    val focus = LocalFocusManager.current
    pointerInput(focus) { detectTapGestures(onTap = { focus.clearFocus() }) }
}

/**
 * Leaving a form with unsaved typing asks first — system back and the top bar's arrow
 * alike. Returns the guarded back action for the top bar.
 */
@Composable
fun rememberDiscardGuard(dirty: Boolean, onLeave: () -> Unit): () -> Unit {
    var asking by remember { mutableStateOf(false) }
    androidx.activity.compose.BackHandler(enabled = dirty) { asking = true }
    if (asking) {
        DialogShell(
            title = "Elveted a módosításokat?",
            onDismiss = { asking = false },
            actions = {
                androidx.compose.material3.TextButton(onClick = { asking = false }) {
                    androidx.compose.material3.Text("Maradok")
                }
                PrimaryButton(
                    text = "Elvetés",
                    onClick = {
                        asking = false
                        onLeave()
                    },
                )
            },
        ) {
            androidx.compose.material3.Text(
                "A beírt adatok nincsenek elmentve.",
                style = androidx.compose.material3.MaterialTheme.typography.bodyLarge,
            )
        }
    }
    return { if (dirty) asking = true else onLeave() }
}

/**
 * Tapping the bottom-bar tab you are already on scrolls its list back to the top, as in
 * every other app. The bar emits the tab's route; the list with that route listens.
 */
object ScrollToTop {
    private val _requests = MutableSharedFlow<String>(extraBufferCapacity = 1)
    val requests: SharedFlow<String> = _requests.asSharedFlow()

    fun request(route: String) {
        _requests.tryEmit(route)
    }
}

/** A list state that jumps to the top when [route]'s tab is tapped again. */
@Composable
fun rememberTopScrollableListState(route: String): androidx.compose.foundation.lazy.LazyListState {
    val list = androidx.compose.foundation.lazy.rememberLazyListState()
    LaunchedEffect(route) {
        ScrollToTop.requests.collect { requested ->
            if (requested == route) list.animateScrollToItem(0)
        }
    }
    return list
}

/**
 * Runs [onReturn] each time the screen comes back into view after having been left — back
 * from an edit, an e-mail, the camera, or the app returning from the background — but not
 * on first arrival (the screen loads itself then). Detail screens reload in place with it,
 * so what they show is never the version from before the edit.
 */
@Composable
fun RefreshOnReturn(onReturn: () -> Unit) {
    val owner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    val callback by androidx.compose.runtime.rememberUpdatedState(onReturn)
    androidx.compose.runtime.DisposableEffect(owner) {
        var arrived = false
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                if (arrived) callback() else arrived = true
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose { owner.lifecycle.removeObserver(observer) }
    }
}

/**
 * A short centred form (sign-in, server address, new password) that stays usable with the
 * keyboard up: centred while it fits, scrollable once the keys take half the screen.
 */
@Composable
fun CenteredScrollColumn(content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit) {
    androidx.compose.foundation.layout.BoxWithConstraints(
        Modifier.fillMaxSize().imePadding(),
    ) {
        val visible = maxHeight
        androidx.compose.foundation.layout.Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(androidx.compose.foundation.rememberScrollState())
                .heightIn(min = visible)
                .padding(24.dp),
            verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
            horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally,
            content = content,
        )
    }
}

/**
 * Pull-to-refresh with a light tick when the pull lets go: the hand knows the refresh
 * started before the eye finds the spinner. Every list in the app goes through this.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun AppPullToRefresh(
    isRefreshing: Boolean,
    onRefresh: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable androidx.compose.foundation.layout.BoxScope.() -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    androidx.compose.material3.pulltorefresh.PullToRefreshBox(
        isRefreshing = isRefreshing,
        onRefresh = {
            haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
            onRefresh()
        },
        modifier = modifier,
        content = content,
    )
}

/**
 * The usual answers as one-tap chips under a text field: picking one fills the field
 * (replacing what was there), and it can still be edited after.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun QuickPicks(options: List<String>, current: String, onPick: (String) -> Unit) {
    androidx.compose.foundation.layout.FlowRow(
        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp),
    ) {
        options.forEach { option ->
            androidx.compose.material3.FilterChip(
                selected = current.trim().equals(option, ignoreCase = true),
                onClick = { onPick(option) },
                label = { androidx.compose.material3.Text(option) },
            )
        }
    }
}
