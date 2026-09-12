package hu.autotherm.autocrm.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
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
    content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit,
) {
    Column(
        modifier
            .fillMaxWidth()
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

@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier) {
    Text(text, style = MaterialTheme.typography.titleLarge, color = Steel900, modifier = modifier)
}

@Composable
fun LoadingState(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(color = Steel900)
    }
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
