package hu.autotherm.autocrm.ui.update

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import hu.autotherm.autocrm.ui.common.PrimaryButton
import java.io.File

/**
 * "New version available" — one dialog, one button, the whole flow.
 *
 * The wording is deliberate: the fitter sees this on a phone in a yard, and a dialog that
 * reads like a licence agreement gets swiped away and the update never happens. So: what
 * changed, how big it is, and Telepítés.
 */
@Composable
fun UpdateGate(
    viewModel: UpdateViewModel,
    /** Open the system installer for the verified APK. */
    onInstall: (File) -> Unit,
    /** Send the user to Settings to allow installing packages. */
    onNeedPermission: () -> Unit,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val version = state.available

    // Checked when the screen appears, not on every recomposition.
    LaunchedEffect(Unit) { viewModel.check() }

    if (version == null) return

    // Re-read when the user returns from Settings: granting "unknown apps" invalidates
    // nothing observed here, so without this the dialog kept showing "Engedélyezés"
    // even after the permission was on, dead-ending the flow.
    val context = LocalContext.current
    var canInstallTick by androidx.compose.runtime.remember { androidx.compose.runtime.mutableIntStateOf(0) }
    val lifecycleOwner = androidx.compose.ui.platform.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) canInstallTick++
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    val canInstallNow = androidx.compose.runtime.remember(canInstallTick) { ApkUpdater.canInstall(context) }
    AlertDialog(
        onDismissRequest = { if (!state.mandatory) viewModel.dismiss() },
        title = {
            Text(
                if (state.mandatory) "Frissítés kötelező" else "Új verzió érhető el",
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "${version.versionName} · ${version.sizeBytes / (1024 * 1024)} MB",
                    fontFamily = FontFamily.Monospace,
                    fontSize = 13.sp,
                )
                if (version.changelogHu.isNotBlank()) {
                    Text(version.changelogHu.trim(), fontSize = 14.sp)
                }
                if (state.mandatory) {
                    Text(
                        "A szerver ezt a verziót már nem fogadja el, ezért a frissítés nélkül az app nem működik.",
                        fontSize = 14.sp,
                    )
                }
                if (!canInstallNow) {
                    Text(
                        "Először engedélyezni kell, hogy az AutoCRM telepíthessen alkalmazásokat (Beállítások).",
                        fontSize = 14.sp,
                    )
                }
                if (state.downloading) {
                    LinearProgressIndicator(
                        modifier = Modifier.fillMaxWidth(),
                        progress = { (state.progress ?: 0) / 100f },
                    )
                    Text("Letöltés… ${state.progress ?: 0}%", fontSize = 13.sp)
                }
                state.error?.let {
                    Text(it, fontSize = 13.sp)
                }
            }
        },
        confirmButton = {
            when {
                state.ready != null -> PrimaryButton(
                    text = "Telepítés",
                    onClick = { onInstall(state.ready!!) },
                )
                !canInstallNow -> PrimaryButton(
                    text = "Engedélyezés",
                    onClick = onNeedPermission,
                )
                else -> PrimaryButton(
                    text = if (state.mandatory) "Frissítés" else "Telepítés",
                    onClick = { viewModel.download { state.ready?.let(onInstall) } },
                    enabled = !state.downloading,
                )
            }
        },
        dismissButton = {
            if (!state.mandatory) {
                Row {
                    TextButton(onClick = { viewModel.dismiss() }) { Text("Később") }
                    if (state.error != null) {
                        TextButton(onClick = { viewModel.download { state.ready?.let(onInstall) } }) {
                            Text("Újra")
                        }
                    }
                }
            }
        },
    )
}