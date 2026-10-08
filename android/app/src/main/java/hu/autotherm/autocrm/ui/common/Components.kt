package hu.autotherm.autocrm.ui.common

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.runtime.collectAsState
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.focus.onFocusEvent
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import hu.autotherm.autocrm.ui.theme.Cold
import hu.autotherm.autocrm.ui.theme.Done
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.ui.theme.Steel900
import hu.autotherm.autocrm.ui.theme.Surface

/**
 * The small set of shapes every screen is built from, mirroring the web client's
 * `globals.css` components. Kept in one file because there are six of them and a package of
 * one-composable files would be harder to read than this.
 */

enum class Tone { Steel, Signal, Cold, Done }

@Composable
fun StatusBadge(text: String, tone: Tone = Tone.Steel, modifier: Modifier = Modifier) {
    val (bg, fg) = when (tone) {
        Tone.Steel -> Steel200 to Steel900
        Tone.Signal -> Signal.copy(alpha = 0.12f) to Signal
        Tone.Cold -> Cold.copy(alpha = 0.12f) to Cold
        Tone.Done -> Done.copy(alpha = 0.12f) to Done
    }
    Box(
        modifier
            .background(bg, RoundedCornerShape(999.dp))
            .padding(horizontal = 10.dp, vertical = 2.dp),
    ) {
        Text(text, style = MaterialTheme.typography.labelMedium, color = fg)
    }
}

@Composable
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
fun Card(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    /** A press-and-hold menu (quick actions); the tap stays [onClick]. */
    onLongClick: (() -> Unit)? = null,
    content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit,
) {
    val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
    val shape = RoundedCornerShape(16.dp)
    val dark = hu.autotherm.autocrm.ui.theme.isDarkTheme
    Column(
        modifier
            .fillMaxWidth()
            .then(
                if (dark) Modifier.border(1.dp, Steel200, shape)
                else Modifier.shadow(elevation = 2.dp, shape = shape, ambientColor = Steel900.copy(alpha = 0.08f), spotColor = Steel900.copy(alpha = 0.10f)),
            )
            .clip(shape)
            .background(Surface)
            // Throttled: a double tap on a row used to open the same screen twice, and
            // Back then showed it again.
            .then(
                when {
                    onLongClick != null -> Modifier.combinedClickable(
                        onClick = { if (onClick != null && ClickGuard.allow()) onClick() },
                        onLongClick = {
                            haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                            onLongClick()
                        },
                    )
                    onClick != null -> Modifier.clickable { if (ClickGuard.allow()) onClick() }
                    else -> Modifier
                },
            )
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content,
    )
}

/** One navigation per half second, app-wide: taps that land twice count once. */
internal object ClickGuard {
    private var last = 0L

    fun allow(): Boolean {
        val now = android.os.SystemClock.uptimeMillis()
        if (now - last < 500) return false
        last = now
        return true
    }
}

/** Label above value, the web client's `<Info>`. */
@Composable
fun Info(label: String, value: String?, mono: Boolean = false, modifier: Modifier = Modifier) {
    Column(modifier) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = Steel500)
        Text(
            value?.takeIf { it.isNotBlank() } ?: "—",
            style = if (mono) MonoSmall.copy(fontSize = MaterialTheme.typography.bodyLarge.fontSize) else MaterialTheme.typography.bodyLarge,
            color = Steel900,
        )
    }
}

/**
 * Tappable contact rows: phone opens the dialer, email opens a compose. A fitter standing
 * next to a van calls the customer; copying a number digit by digit is how wrong numbers
 * get dialled.
 */
@Composable
fun PhoneInfo(label: String, value: String?, modifier: Modifier = Modifier) {
    val uriHandler = LocalUriHandler.current
    val tel = value?.trim()?.takeIf { it.isNotBlank() }
        ?.let { raw -> "tel:" + (if (raw.startsWith("+")) "+" else "") + raw.filter { it.isDigit() } }
    Column(modifier) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = Steel500)
        if (tel == null) {
            Text("—", style = MaterialTheme.typography.bodyLarge, color = Steel900)
        } else {
            Text(
                hu.autotherm.autocrm.util.formatPhone(value!!.trim()).orEmpty(),
                style = MaterialTheme.typography.bodyLarge.copy(textDecoration = TextDecoration.Underline),
                color = Cold,
                modifier = Modifier.copyOnLongPress(value.trim()) { uriHandler.openUri(tel) },
            )
        }
    }
}

/**
 * Tap does the obvious thing (dial, write); a long press copies the text — for pasting a
 * number into WhatsApp or an address into another app — with a confirmation.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun Modifier.copyOnLongPress(text: String, onTap: () -> Unit = {}): Modifier {
    val clipboard = androidx.compose.ui.platform.LocalClipboardManager.current
    val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
    return this.combinedClickable(
        indication = null,
        interactionSource = remember { MutableInteractionSource() },
        onLongClick = {
            clipboard.setText(androidx.compose.ui.text.AnnotatedString(text))
            haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
            // Android 13+ shows its own "copied" chip; a second message would be noise.
            if (android.os.Build.VERSION.SDK_INT < 33) Toasts.show("Vágólapra másolva: $text")
        },
        onClick = onTap,
    )
}

@Composable
fun EmailInfo(label: String, value: String?, modifier: Modifier = Modifier) {
    val uriHandler = LocalUriHandler.current
    val mailto = value?.trim()?.takeIf { it.isNotBlank() }?.let { "mailto:$it" }
    Column(modifier) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = Steel500)
        if (mailto == null) {
            Text("—", style = MaterialTheme.typography.bodyLarge, color = Steel900)
        } else {
            Text(
                value!!.trim(),
                style = MaterialTheme.typography.bodyLarge.copy(textDecoration = TextDecoration.Underline),
                color = Cold,
                modifier = Modifier.copyOnLongPress(value.trim()) { uriHandler.openUri(mailto) },
            )
        }
    }
}

@Composable
fun SectionTitle(
    text: String,
    modifier: Modifier = Modifier,
    count: Int? = null,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Row(
        modifier.fillMaxWidth().heightIn(min = 40.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier.weight(1f, fill = false),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text,
                style = MaterialTheme.typography.titleMedium,
                color = Steel900,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (count != null) {
                Box(
                    Modifier.background(Steel200, RoundedCornerShape(999.dp)).padding(horizontal = 8.dp, vertical = 1.dp),
                ) {
                    Text("$count", style = MaterialTheme.typography.labelSmall, color = Steel900)
                }
            }
        }
        if (actionLabel != null && onAction != null) {
            TextButton(onClick = onAction) { Text(actionLabel, style = MaterialTheme.typography.labelLarge, color = Cold) }
        }
    }
}

@Composable
fun LoadingState(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(color = Steel900)
    }
}

/** Sweeping highlight used by the skeleton screens below. */
@Composable
private fun SkeletonBox(modifier: Modifier = Modifier) {
    val transition = rememberInfiniteTransition(label = "skeleton")
    val sweep by transition.animateFloat(
        initialValue = -1f,
        targetValue = 2f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1400),
            repeatMode = RepeatMode.Restart,
        ),
        label = "skeletonSweep",
    )
    val base = Steel200
    // A band of lighter surface travelling across the block. Alpha-only pulsing
    // looked dead on large cards; the sweep reads as progress.
    val brush = androidx.compose.ui.graphics.Brush.linearGradient(
        colors = listOf(base, Surface, base),
        start = androidx.compose.ui.geometry.Offset(sweep * 400f - 200f, 0f),
        end = androidx.compose.ui.geometry.Offset(sweep * 400f + 200f, 200f),
    )
    Box(
        modifier
            .clip(RoundedCornerShape(8.dp))
            .background(brush),
    )
}

/**
 * First-load placeholder for lists: shaped like the content instead of a bare
 * spinner, so a slow network reads as "loading" rather than "frozen".
 *
 * Deliberately a plain Column, not a LazyColumn: five static rows need no
 * laziness, and a lazy list measures infinite inside Columns and lazy items —
 * which crashes on the device (IllegalStateException, infinite constraints).
 */
@Composable
fun ListSkeleton(modifier: Modifier = Modifier, count: Int = 5) {
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        repeat(count) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .background(Surface, RoundedCornerShape(12.dp))
                    .border(1.dp, Steel200, RoundedCornerShape(12.dp))
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                SkeletonBox(Modifier.fillMaxWidth(0.35f).height(16.dp))
                SkeletonBox(Modifier.fillMaxWidth(0.8f).height(20.dp))
                SkeletonBox(Modifier.fillMaxWidth(0.55f).height(14.dp))
            }
        }
    }
}

/** First-load placeholder for detail screens. */
@Composable
fun DetailSkeleton(modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SkeletonBox(Modifier.fillMaxWidth(0.4f).height(16.dp))
        SkeletonBox(Modifier.fillMaxWidth(0.85f).height(28.dp))
        SkeletonBox(Modifier.fillMaxWidth(0.6f).height(14.dp))
        repeat(3) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .background(Surface, RoundedCornerShape(12.dp))
                    .border(1.dp, Steel200, RoundedCornerShape(12.dp))
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                SkeletonBox(Modifier.fillMaxWidth(0.5f).height(18.dp))
                SkeletonBox(Modifier.fillMaxWidth().height(14.dp))
                SkeletonBox(Modifier.fillMaxWidth(0.7f).height(14.dp))
            }
        }
    }
}

/** Thin progress line shown while old rows stay on screen during a refetch. */
@Composable
fun RefreshingBar(modifier: Modifier = Modifier) {
    LinearProgressIndicator(
        modifier = modifier.fillMaxWidth().height(3.dp),
        color = Steel900,
        trackColor = Steel200,
    )
}

/**
 * The app bar every screen shares: steel title, back arrow on details, optional
 * refresh action. Previously no screen had a bar at all — details had no back
 * affordance and lists had no title or manual refresh.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScreenTopBar(
    title: String,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    /** Hamburger for root destinations inside the navigation drawer. */
    onMenu: (() -> Unit)? = null,
    refreshing: Boolean = false,
    onRefresh: (() -> Unit)? = null,
    actions: @Composable RowScope.() -> Unit = {},
) {
    TopAppBar(
        title = {
            Column {
                Text(
                    title,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.titleLarge,
                )
                if (subtitle != null) {
                    Text(
                        subtitle,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.labelMedium,
                        color = Steel500,
                    )
                }
            }
        },
        navigationIcon = {
            if (onBack != null) {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Vissza")
                }
            } else if (onMenu != null) {
                IconButton(onClick = onMenu) {
                    Icon(Icons.Filled.Menu, contentDescription = "Menü")
                }
            }
        },
        actions = {
            if (refreshing) {
                CircularProgressIndicator(
                    color = Steel900,
                    strokeWidth = 2.dp,
                    modifier = Modifier.size(24.dp).padding(2.dp),
                )
            } else if (onRefresh != null) {
                IconButton(onClick = onRefresh) {
                    Icon(Icons.Filled.Refresh, contentDescription = "Frissítés")
                }
            }
            actions()
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = Surface,
            titleContentColor = Steel900,
            navigationIconContentColor = Steel900,
            actionIconContentColor = Steel900,
        ),
    )
}

/**
 * Search field with a clear affordance and a search IME action. Previously every
 * list used a bare text field with no way to clear except deleting char by char.
 */
@Composable
fun SearchField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    onSearch: (() -> Unit)? = null,
) {
    val focus = androidx.compose.ui.platform.LocalFocusManager.current
    AutoCrmTextField(
        value = value,
        onValueChange = onValueChange,
        label = label,
        modifier = modifier,
        enabled = enabled,
        singleLine = true,
        leading = {
            Icon(Icons.Filled.Search, contentDescription = null, tint = Steel500)
        },
        trailing = {
            if (value.isNotEmpty()) {
                IconButton(onClick = { onValueChange("") }) {
                    Icon(Icons.Filled.Clear, contentDescription = "Keresés törlése", tint = Steel500)
                }
            } else {
                // Gloves on, hands dirty: say the plate instead of typing it.
                DictationButton(onText = { spoken -> onValueChange(spoken.trim()) })
            }
        },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        // The results are behind the keyboard: put it away once the search is asked.
        keyboardActions = KeyboardActions(onSearch = {
            onSearch?.invoke()
            focus.clearFocus()
        }),
    )
}

/**
 * An error the user can act on. Always paired with a retry: a dead end with no button is
 * how an app teaches people to force-quit it.
 */
@Composable
fun ErrorState(message: String, modifier: Modifier = Modifier, onRetry: (() -> Unit)? = null) {
    Column(
        modifier
            .fillMaxWidth()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            message,
            style = MaterialTheme.typography.bodyLarge,
            color = Steel900,
            textAlign = TextAlign.Center,
        )
        if (onRetry != null) {
            // In a dead spot the fitter should not have to keep tapping "Újra": the screen
            // tries again by itself the moment the phone has a network.
            val online by hu.autotherm.autocrm.data.net.NetworkState.online.collectAsState()
            val retry by androidx.compose.runtime.rememberUpdatedState(onRetry)
            var waited by remember { mutableStateOf(!online) }
            androidx.compose.runtime.LaunchedEffect(online) {
                if (!online) {
                    waited = true
                } else if (waited) {
                    waited = false
                    retry()
                }
            }
            if (!online) {
                Text(
                    "Nincs hálózat. Amint lesz, magától újrapróbálja.",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                    textAlign = TextAlign.Center,
                )
            }
            OutlinedButton(onClick = onRetry) { Text("Újra") }
        }
    }
}

@Composable
fun EmptyState(
    message: String,
    modifier: Modifier = Modifier,
    icon: androidx.compose.ui.graphics.vector.ImageVector = Icons.Filled.Search,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Column(
        modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 40.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            Modifier.size(56.dp).background(Steel200.copy(alpha = 0.6f), RoundedCornerShape(18.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = Steel500, modifier = Modifier.size(28.dp))
        }
        Text(message, style = MaterialTheme.typography.bodyLarge, color = Steel500, textAlign = TextAlign.Center)
        if (actionLabel != null && onAction != null) {
            SecondaryButton(text = actionLabel, onClick = onAction)
        }
    }
}

@Composable
fun PrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    /** Working: a spinner beside the label, taps ignored. Every busy label in the app
     *  already ends in an ellipsis ("Mentés…"), so that is the default signal. */
    busy: Boolean = text.endsWith("…"),
) {
    Button(
        onClick = { if (!busy) onClick() },
        enabled = enabled,
        modifier = modifier.heightIn(min = 52.dp),
        shape = RoundedCornerShape(14.dp),
        contentPadding = PaddingValues(horizontal = 24.dp, vertical = 14.dp),
    ) {
        if (busy) {
            CircularProgressIndicator(
                modifier = Modifier.size(18.dp),
                strokeWidth = 2.dp,
                color = LocalContentColor.current,
            )
            Spacer(Modifier.width(10.dp))
        }
        Text(text, style = MaterialTheme.typography.labelLarge)
    }
}

/** A label/value row for dense read-only lists. */
@Composable
fun KeyValueRow(label: String, value: String?, valueStyle: TextStyle? = null) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = Steel500)
        Text(
            value?.takeIf { it.isNotBlank() } ?: "—",
            style = valueStyle ?: MaterialTheme.typography.bodyLarge,
            color = Steel900,
        )
    }
}

/**
 * The single text field every form uses: 8dp radius, steel border, steel focus ring.
 * A wrapper rather than a theme because M3 reads component defaults, not the colour scheme,
 * for field chrome — a bare OutlinedTextField renders the M3 purple focus.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun AutoCrmTextField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    singleLine: Boolean = true,
    enabled: Boolean = true,
    placeholder: (@Composable () -> Unit)? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    readOnly: Boolean = false,
    textStyle: TextStyle = MaterialTheme.typography.bodyLarge,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    isError: Boolean = false,
    /** A line under the field: a hint, or (with [isError]) what is wrong. */
    supporting: String? = null,
) {
    // When the keyboard opens it covers the lower half of a form. Once it has slid in
    // (the scrolling parent has shrunk by then), the focused field scrolls itself into view.
    val bringIntoView = remember { BringIntoViewRequester() }
    val scope = rememberCoroutineScope()
    // The keyboard's action key walks the form: "Next" on a one-line field moves on to the
    // following field, and on the last one puts the keyboard away. A caller's own choice
    // (Search, Done, multi-line Enter) is left alone.
    val focus = androidx.compose.ui.platform.LocalFocusManager.current
    val chained = singleLine &&
        (keyboardOptions.imeAction == ImeAction.Default || keyboardOptions.imeAction == ImeAction.Unspecified)
    // Plain text starts with a capital, as Hungarian writing does: names, notes, titles.
    // Typed fields (e-mail, phone, numbers, passwords, web addresses) and any explicit
    // choice by the caller are left alone.
    val autoCaps = keyboardOptions.capitalization == androidx.compose.ui.text.input.KeyboardCapitalization.None &&
        (
            keyboardOptions.keyboardType == androidx.compose.ui.text.input.KeyboardType.Text ||
                keyboardOptions.keyboardType == androidx.compose.ui.text.input.KeyboardType.Unspecified
            ) &&
        visualTransformation == VisualTransformation.None
    var effectiveOptions = keyboardOptions
    if (autoCaps) {
        effectiveOptions = effectiveOptions.copy(
            capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Sentences,
        )
    }
    if (chained) effectiveOptions = effectiveOptions.copy(imeAction = ImeAction.Next)
    val effectiveActions = if (chained && keyboardActions == KeyboardActions.Default) {
        KeyboardActions(
            onNext = {
                if (!focus.moveFocus(androidx.compose.ui.focus.FocusDirection.Down)) focus.clearFocus()
            },
        )
    } else {
        keyboardActions
    }
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = placeholder,
        leadingIcon = leading,
        trailingIcon = trailing,
        textStyle = textStyle,
        isError = isError,
        supportingText = supporting?.let { text -> { Text(text, style = MaterialTheme.typography.bodySmall) } },
        modifier = modifier
            .bringIntoViewRequester(bringIntoView)
            .onFocusEvent { focus ->
                if (focus.isFocused) {
                    scope.launch {
                        delay(320)
                        bringIntoView.bringIntoView()
                    }
                }
            },
        singleLine = singleLine,
        enabled = enabled,
        readOnly = readOnly,
        keyboardOptions = effectiveOptions,
        keyboardActions = effectiveActions,
        visualTransformation = visualTransformation,
        shape = RoundedCornerShape(12.dp),
        colors = OutlinedTextFieldDefaults.colors(
            focusedContainerColor = Surface,
            unfocusedContainerColor = Surface,
            disabledContainerColor = Surface,
            focusedBorderColor = Steel900,
            unfocusedBorderColor = Steel200,
            disabledBorderColor = Steel200,
            focusedLabelColor = Steel900,
            unfocusedLabelColor = Steel500,
            cursorColor = Steel900,
            focusedTextColor = Steel900,
            unfocusedTextColor = Steel900,
            disabledTextColor = Steel500,
        ),
    )
}

/**
 * Dialog container matching the web client's dialog: surface card, 12dp radius, steel border,
 * section-type title. Content and actions are the caller's; the shell owns the chrome so a
 * dialog never renders the raw M3 alert.
 */
@Composable
fun DialogShell(
    title: String,
    onDismiss: () -> Unit,
    actions: @Composable RowScope.() -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    Dialog(onDismissRequest = onDismiss) {
        // Dialogs pop in instead of blinking on: scale + fade on first show.
        var shown by remember { mutableStateOf(false) }
        androidx.compose.runtime.LaunchedEffect(Unit) { shown = true }
        AnimatedVisibility(
            visible = shown,
            enter = fadeIn(tween(150)) + scaleIn(tween(180), initialScale = 0.94f),
        ) {
            Surface(
                shape = RoundedCornerShape(24.dp),
                color = Surface,
                tonalElevation = 0.dp,
                shadowElevation = 8.dp,
            ) {
            Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Text(title, style = MaterialTheme.typography.titleLarge, color = Steel900)
                // Dialogs size to content: cap the body and scroll inside, or the option
                // list plus note field overflow small phones and the keyboard traps them.
                // With the keyboard up the cap shrinks to what is left above it, so the
                // buttons stay on screen and the focused field scrolls into view inside.
                val density = androidx.compose.ui.platform.LocalDensity.current
                val keyboard = with(density) {
                    WindowInsets.ime.getBottom(density).toDp()
                }
                val screen = androidx.compose.ui.platform.LocalConfiguration.current.screenHeightDp.dp
                val bodyMax = (screen - keyboard - 230.dp).coerceIn(140.dp, 400.dp)
                Column(
                    Modifier.heightIn(max = bodyMax).verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    content()
                }
                Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                    verticalAlignment = Alignment.CenterVertically,
                    content = actions,
                )
            }
        }
    }
}
}

/**
 * A due date as an actual calendar. Read-only field showing the Hungarian date;
 * tapping the calendar icon (or the field) opens the Material date picker, and
 * the X clears it. No more typing `ÉÉÉÉ-HH-NN` by hand.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DateField(
    /** `YYYY-MM-DD` or blank. */
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    /** One-tap dates under the field: label to days from today ("1 hét" to 7). */
    quickPicks: List<Pair<String, Long>> = emptyList(),
) {
    var open by remember { mutableStateOf(false) }

    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
    AutoCrmTextField(
        value = value.takeIf { it.isNotBlank() }?.let { hu.autotherm.autocrm.util.formatDate(it) } ?: "",
        onValueChange = {},
        label = label,
        modifier = Modifier.fillMaxWidth().clickable(enabled = enabled) { open = true },
        enabled = enabled,
        readOnly = true,
        trailing = {
            Row {
                if (value.isNotBlank()) {
                    IconButton(onClick = { onValueChange("") }, enabled = enabled) {
                        Icon(Icons.Filled.Clear, contentDescription = "Dátum törlése", tint = Steel500)
                    }
                }
                IconButton(onClick = { open = true }, enabled = enabled) {
                    Icon(Icons.Filled.DateRange, contentDescription = "Naptár", tint = Steel500)
                }
            }
        },
    )
    if (quickPicks.isNotEmpty() && enabled) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val today = java.time.LocalDate.now()
            quickPicks.forEach { (pickLabel, days) ->
                val day = today.plusDays(days).toString()
                androidx.compose.material3.FilterChip(
                    selected = value == day,
                    onClick = { onValueChange(if (value == day) "" else day) },
                    label = { Text(pickLabel) },
                )
            }
        }
    }
    }

    if (open) {
        val initial = runCatching {
            java.time.LocalDate.parse(value)
                .atStartOfDay(java.time.ZoneId.systemDefault())
                .toInstant().toEpochMilli()
        }.getOrNull() ?: System.currentTimeMillis()
        val pickerState = androidx.compose.material3.rememberDatePickerState(
            initialSelectedDateMillis = initial,
        )
        androidx.compose.material3.DatePickerDialog(
            onDismissRequest = { open = false },
            confirmButton = {
                TextButton(
                    onClick = {
                        pickerState.selectedDateMillis?.let { millis ->
                            val day = java.time.Instant.ofEpochMilli(millis)
                                .atZone(java.time.ZoneId.systemDefault())
                                .toLocalDate()
                            onValueChange(day.toString())
                        }
                        open = false
                    },
                ) { Text("Kész") }
            },
            dismissButton = {
                TextButton(onClick = { open = false }) { Text("Mégse") }
            },
        ) {
            androidx.compose.material3.DatePicker(state = pickerState)
        }
    }
}

/** The quieter partner of [PrimaryButton]: outlined, same height and radius. */
@Composable
fun SecondaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    icon: androidx.compose.ui.graphics.vector.ImageVector? = null,
) {
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.heightIn(min = 48.dp),
        shape = RoundedCornerShape(14.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, Steel200),
        contentPadding = PaddingValues(horizontal = 18.dp, vertical = 12.dp),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp), tint = Steel900)
            Spacer(Modifier.width(8.dp))
        }
        Text(text, style = MaterialTheme.typography.labelLarge, color = Steel900)
    }
}

/** Initials in a tinted circle: who or what a row is about, at a glance. */
@Composable
fun InitialsAvatar(name: String, modifier: Modifier = Modifier, size: androidx.compose.ui.unit.Dp = 40.dp) {
    val initials = name.split(' ', '-', '.')
        .filter { it.isNotBlank() && it.first().isLetterOrDigit() }
        .take(2)
        .joinToString("") { it.first().uppercase() }
        .ifBlank { "?" }
    // A stable tint per name, from the brand's cool and warm accents.
    val tints = listOf(Cold, Done, Signal, Steel500)
    val tint = tints[(name.hashCode() and 0x7fffffff) % tints.size]
    Box(
        modifier.size(size).background(tint.copy(alpha = 0.14f), androidx.compose.foundation.shape.CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(initials, style = if (size < 32.dp) MaterialTheme.typography.labelSmall else MaterialTheme.typography.titleSmall, color = tint)
    }
}

/**
 * One tappable row of a list: avatar or icon, title, subtitle, and whatever sits on the right
 * (a badge, an amount). The list screens' common shape.
 */
@Composable
fun ListRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    overline: String? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    Card(modifier = modifier, onClick = onClick) {
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            leading?.invoke()
            Column(Modifier.weight(1f)) {
                if (overline != null) {
                    Text(overline, style = MonoSmall, color = Steel500, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                Text(title, style = MaterialTheme.typography.titleMedium, color = Steel900, maxLines = 2, overflow = TextOverflow.Ellipsis)
                if (subtitle != null) {
                    Text(subtitle, style = MaterialTheme.typography.bodySmall, color = Steel500, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
            }
            trailing?.invoke()
        }
    }
}

/** A titled group of form fields, so a long form reads as a few short ones. */
@Composable
fun FormSection(
    title: String,
    modifier: Modifier = Modifier,
    hint: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(modifier) {
        Text(title, style = MaterialTheme.typography.titleSmall, color = Steel500)
        if (hint != null) {
            Text(hint, style = MaterialTheme.typography.bodySmall, color = Steel500)
        }
        Column(verticalArrangement = Arrangement.spacedBy(12.dp), content = content)
    }
}

/** A red line under a form: what the server (or the form) refused, in words. */
@Composable
fun FormError(message: String?) {
    if (message == null) return
    Row(
        Modifier.fillMaxWidth().background(Signal.copy(alpha = 0.10f), RoundedCornerShape(12.dp)).padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(message, style = MaterialTheme.typography.bodyMedium, color = Signal)
    }
}

/** Two or three mutually exclusive choices as one segmented row (currency, kind). */
@Composable
fun <T> SegmentedChoice(
    options: List<Pair<T, String>>,
    selected: T,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    Row(
        modifier.fillMaxWidth().background(Steel200.copy(alpha = 0.5f), RoundedCornerShape(12.dp)).padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        options.forEach { (value, label) ->
            val on = value == selected
            Box(
                Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(9.dp))
                    .background(if (on) Surface else androidx.compose.ui.graphics.Color.Transparent)
                    .clickable(enabled = enabled) { onSelect(value) }
                    .padding(vertical = 10.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    label,
                    style = MaterialTheme.typography.labelLarge,
                    color = if (on) Steel900 else Steel500,
                )
            }
        }
    }
}

/** A number plate as it looks on the van: black on white, framed, monospaced. */
@Composable
fun PlateBadge(plate: String, modifier: Modifier = Modifier) {
    Box(
        modifier
            .border(1.5.dp, androidx.compose.ui.graphics.Color(0xFF1B2327), RoundedCornerShape(6.dp))
            .background(androidx.compose.ui.graphics.Color.White, RoundedCornerShape(6.dp))
            .padding(horizontal = 8.dp, vertical = 2.dp),
    ) {
        Text(
            plate.uppercase(),
            style = MonoSmall.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold),
            color = androidx.compose.ui.graphics.Color(0xFF1B2327),
        )
    }
}

/** The labelled "new" button lists float: says what it makes, not just "+". */
@Composable
fun NewFab(text: String, onClick: () -> Unit) {
    androidx.compose.material3.ExtendedFloatingActionButton(
        onClick = onClick,
        icon = { Icon(Icons.Filled.Add, contentDescription = null) },
        text = { Text(text, style = MaterialTheme.typography.labelLarge) },
        containerColor = Steel900,
        contentColor = Surface,
        shape = RoundedCornerShape(16.dp),
    )
}
