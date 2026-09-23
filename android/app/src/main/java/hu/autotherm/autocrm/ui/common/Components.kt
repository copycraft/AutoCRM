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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Clear
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
fun Card(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit,
) {
    val clickableMod = if (onClick != null) {
        Modifier.clickable(onClick = onClick)
    } else Modifier;
    Column(
        modifier
            .fillMaxWidth()
            .then(clickableMod)
            .background(Surface, RoundedCornerShape(12.dp))
            .border(1.dp, Steel200, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content,
    )
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
                value!!.trim(),
                style = MaterialTheme.typography.bodyLarge.copy(textDecoration = TextDecoration.Underline),
                color = Cold,
                modifier = Modifier.clickable(
                    indication = null,
                    interactionSource = remember { MutableInteractionSource() },
                ) { uriHandler.openUri(tel) },
            )
        }
    }
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
                modifier = Modifier.clickable(
                    indication = null,
                    interactionSource = remember { MutableInteractionSource() },
                ) { uriHandler.openUri(mailto) },
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
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text + (if (count != null) " ($count)" else ""),
            style = MaterialTheme.typography.titleLarge,
            color = Steel900,
            modifier = Modifier.weight(1f, fill = false),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (actionLabel != null && onAction != null) {
            TextButton(onClick = onAction) { Text(actionLabel) }
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
            }
        },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSearch?.invoke() }),
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
            OutlinedButton(onClick = onRetry) { Text("Újra") }
        }
    }
}

@Composable
fun EmptyState(message: String, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
        Text(message, style = MaterialTheme.typography.bodyLarge, color = Steel500, textAlign = TextAlign.Center)
    }
}

@Composable
fun PrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier,
        shape = RoundedCornerShape(8.dp),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp),
    ) {
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
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = placeholder,
        leadingIcon = leading,
        trailingIcon = trailing,
        textStyle = textStyle,
        modifier = modifier,
        singleLine = singleLine,
        enabled = enabled,
        readOnly = readOnly,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        visualTransformation = visualTransformation,
        shape = RoundedCornerShape(8.dp),
        colors = OutlinedTextFieldDefaults.colors(
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
                shape = RoundedCornerShape(12.dp),
                color = Surface,
                border = androidx.compose.foundation.BorderStroke(1.dp, Steel200),
            ) {
            Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(title, style = MaterialTheme.typography.titleLarge, color = Steel900)
                // Dialogs size to content: cap the body and scroll inside, or the option
                // list plus note field overflow small phones and the keyboard traps them.
                Column(
                    Modifier.heightIn(max = 400.dp).verticalScroll(rememberScrollState()),
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
) {
    var open by remember { mutableStateOf(false) }

    AutoCrmTextField(
        value = value.takeIf { it.isNotBlank() }?.let { hu.autotherm.autocrm.util.formatDate(it) } ?: "",
        onValueChange = {},
        label = label,
        modifier = modifier.clickable(enabled = enabled) { open = true },
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
