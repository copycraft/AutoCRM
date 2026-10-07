package hu.autotherm.autocrm.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.BuildConfig
import hu.autotherm.autocrm.data.prefs.ThemePrefs
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.theme.Steel500
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
        Column(
            Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp),
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
