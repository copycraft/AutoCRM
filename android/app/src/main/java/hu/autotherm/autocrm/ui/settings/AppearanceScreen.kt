package hu.autotherm.autocrm.ui.settings

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.BuildConfig
import hu.autotherm.autocrm.data.prefs.DisplayScale
import hu.autotherm.autocrm.data.prefs.ThemePrefs
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.PlateBadge
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SecondaryButton
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.MonoSmall
import hu.autotherm.autocrm.ui.theme.MonoStyle
import hu.autotherm.autocrm.ui.theme.Panel
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class AppearanceViewModel(private val prefs: ThemePrefs) : ViewModel() {

    data class State(val mode: String = ThemePrefs.MODE_SYSTEM, val amoled: Boolean = false)

    val state: StateFlow<State> = combine(prefs.mode, prefs.amoled, ::State)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), State())

    fun setMode(mode: String) = viewModelScope.launch { prefs.setMode(mode) }
    fun setAmoled(enabled: Boolean) = viewModelScope.launch { prefs.setAmoled(enabled) }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppearanceScreen(
    viewModel: AppearanceViewModel,
    onMenu: () -> Unit,
) {
    val state by viewModel.state.collectAsState()

    Scaffold(
        topBar = {
            ScreenTopBar(title = "Beállítások", onMenu = onMenu)
        },
    ) { padding ->
        // Scrolls: at the largest size the cards no longer fit a phone screen.
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Card {
                Text("Megjelenés", style = MaterialTheme.typography.titleLarge)
                ModeRow(
                    selected = state.mode == ThemePrefs.MODE_SYSTEM,
                    title = "Rendszer",
                    body = "Követi a telefon sötét módját.",
                    onClick = { viewModel.setMode(ThemePrefs.MODE_SYSTEM) },
                )
                ModeRow(
                    selected = state.mode == ThemePrefs.MODE_LIGHT,
                    title = "Világos",
                    body = "Mindig világos, műhelyfényhez.",
                    onClick = { viewModel.setMode(ThemePrefs.MODE_LIGHT) },
                )
                ModeRow(
                    selected = state.mode == ThemePrefs.MODE_DARK,
                    title = "Sötét",
                    body = "Kíméli az akkut és a szemet éjszaka.",
                    onClick = { viewModel.setMode(ThemePrefs.MODE_DARK) },
                )
                Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f, fill = false)) {
                        Text("Tiszta fekete", style = MaterialTheme.typography.bodyLarge)
                        Text(
                            "Sötét módban valódi fekete háttér (AMOLED).",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                    Switch(
                        checked = state.amoled,
                        enabled = state.mode == ThemePrefs.MODE_DARK,
                        onCheckedChange = viewModel::setAmoled,
                    )
                }
            }
            SizeCard()
            hu.autotherm.autocrm.ui.common.AppLockSetting()
            Card {
                Text("Névjegy", style = MaterialTheme.typography.titleLarge)
                Text(
                    "AutoCRM ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})",
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    "A megjelenés a telefonon marad, kijelentkezés után is.",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
            }
        }
    }
}

/**
 * Display size and text size (DisplayScale). The sliders preview on a sample job card at
 * once; "Alkalmazás" stores the choice and restarts the screen, because dialogs and menus
 * only pick up a new size with a fresh activity.
 */
@Composable
private fun SizeCard() {
    val context = LocalContext.current
    val applied = remember { DisplayScale.read(context) }
    var ui by rememberSaveable { mutableFloatStateOf(applied.ui) }
    var text by rememberSaveable { mutableFloatStateOf(applied.text) }
    val pending = DisplayScale.Scale(ui, text)
    val apply: (DisplayScale.Scale) -> Unit = { scale ->
        DisplayScale.save(context, scale)
        context.findActivity()?.recreate()
    }

    Card {
        Text("Méret", style = MaterialTheme.typography.titleLarge)
        Text(
            "Mekkora legyen minden ezen a telefonon. A telefon saját kijelző- és " +
                "betűméretére szorzódik.",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
        )
        StepSlider(
            title = "Felület",
            body = "Minden együtt: gombok, ikonok, szöveg.",
            value = ui,
            steps = DisplayScale.UI_STEPS,
            onChange = { ui = it },
        )
        StepSlider(
            title = "Betűméret",
            body = "Csak a szöveg.",
            value = text,
            steps = DisplayScale.TEXT_STEPS,
            onChange = { text = it },
        )
        Text("Előnézet", style = MaterialTheme.typography.labelMedium, color = Steel500)
        // The sample is drawn at the pending size relative to the one in force, so it
        // shows exactly what "Alkalmazás" will give.
        val density = LocalDensity.current
        Box(
            Modifier.fillMaxWidth()
                .clip(RoundedCornerShape(14.dp))
                .background(Panel)
                .border(1.dp, Steel200, RoundedCornerShape(14.dp))
                .padding(12.dp),
        ) {
            CompositionLocalProvider(
                LocalDensity provides Density(
                    density.density * ui / applied.ui,
                    density.fontScale * text / applied.text,
                ),
            ) {
                SamplePreview()
            }
        }
        // Halves, not natural widths: at the largest sizes two labels side by side no
        // longer fit the card.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Straight back to normal in one tap: someone who shrank it too far to read
            // should not have to find the slider again.
            SecondaryButton(
                text = "Alaphelyzet",
                enabled = !applied.isDefault || !pending.isDefault,
                onClick = {
                    ui = 1f
                    text = 1f
                    if (!applied.isDefault) apply(DisplayScale.Scale())
                },
                modifier = Modifier.weight(1f),
            )
            PrimaryButton(
                text = "Alkalmazás",
                enabled = pending != applied,
                onClick = { apply(pending) },
                modifier = Modifier.weight(1f),
            )
        }
    }
}

/** One scale as a stepped slider, with −/+ for exact taps (gloves) and the value in %. */
@Composable
private fun StepSlider(
    title: String,
    body: String,
    value: Float,
    steps: List<Float>,
    onChange: (Float) -> Unit,
) {
    val index = steps.indexOf(value).coerceAtLeast(0)
    Column {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.bodyLarge)
                Text(body, style = MaterialTheme.typography.labelMedium, color = Steel500)
            }
            Text("${(value * 100).roundToInt()}%", style = MonoStyle)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { onChange(steps[index - 1]) }, enabled = index > 0) {
                Icon(Icons.Filled.Remove, contentDescription = "$title kisebb")
            }
            Slider(
                value = index.toFloat(),
                onValueChange = { onChange(steps[it.roundToInt().coerceIn(0, steps.lastIndex)]) },
                valueRange = 0f..steps.lastIndex.toFloat(),
                steps = steps.size - 2,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = { onChange(steps[index + 1]) }, enabled = index < steps.lastIndex) {
                Icon(Icons.Filled.Add, contentDescription = "$title nagyobb")
            }
        }
    }
}

/** A job card as the list shows it: the thing people will actually be reading. */
@Composable
private fun SamplePreview() {
    Card {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            PlateBadge("ABC-123")
            Text("2024-0042", style = MonoSmall, color = Steel500)
        }
        Text("Klíma töltés és szivárgáskeresés", style = MaterialTheme.typography.titleMedium)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            StatusBadge("Folyamatban")
            StatusBadge("Határidő: holnap", Tone.Cold)
        }
    }
}

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}

@Composable
private fun ModeRow(
    selected: Boolean,
    title: String,
    body: String,
    onClick: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        RadioButton(selected = selected, onClick = onClick)
        Column(Modifier.weight(1f, fill = false)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(body, style = MaterialTheme.typography.labelMedium, color = Steel500)
        }
    }
}
