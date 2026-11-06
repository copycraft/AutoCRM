package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.ModulePatch
import hu.autotherm.autocrm.data.api.OrderModule
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.SectionTitle
import hu.autotherm.autocrm.ui.common.Toasts
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.util.relativeTime
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** `#rrggbb` → a Compose colour; grey if the server ever sends something else. */
fun departmentColor(hex: String): Color =
    runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrDefault(Steel500)

fun moduleStatusLabel(m: OrderModule): String = when {
    m.changedSinceRelease -> "Módosult a kiadás óta"
    m.status == "released" -> "Kiadva"
    m.status == "done" -> "Kész"
    else -> "Tervezet"
}

class ModulesViewModel(private val api: AutoCrmApi) : ViewModel() {
    data class State(
        val loading: Boolean = true,
        val modules: List<OrderModule> = emptyList(),
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun load(orderId: Long) {
        viewModelScope.launch {
            try {
                val modules = api.orderModules(orderId).items
                _state.value = _state.value.copy(loading = false, modules = modules, error = null)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, error = describeError(e))
            }
        }
    }

    private fun act(orderId: Long, done: String, call: suspend () -> Unit, onDone: () -> Unit = {}) {
        viewModelScope.launch {
            _state.value = _state.value.copy(busy = true, error = null)
            try {
                call()
                Toasts.show(done)
                onDone()
                _state.value = _state.value.copy(busy = false)
                load(orderId)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(e))
            }
        }
    }

    fun save(orderId: Long, module: OrderModule, content: String, onSaved: () -> Unit) =
        act(orderId, "Mentve", { api.updateModule(module.id, ModulePatch(content = content)) }, onSaved)

    fun release(orderId: Long, module: OrderModule) =
        act(orderId, "Kiadva: ${module.departmentName}", { api.releaseModule(module.id) })

    fun setDone(orderId: Long, module: OrderModule, done: Boolean) =
        act(
            orderId,
            if (done) "Késznek jelölve" else "Újranyitva",
            { api.updateModule(module.id, ModulePatch(status = if (done) "done" else "draft")) },
        )
}

/**
 * Műszaki tartalom: the order's technical content, one part per department, each in its
 * department's colour. Everyone reads every part; the server says per part whether this
 * person may write it (the office, or a member of that department). Handing a part over
 * tells the department's heads; saving alone tells nobody.
 */
@Composable
fun ModulesCard(orderId: Long) {
    val context = LocalContext.current
    val vm: ModulesViewModel = viewModel(key = "modules-$orderId") {
        ModulesViewModel((context.applicationContext as AutoCrmApp).api)
    }
    val state by vm.state.collectAsState()
    var editing by rememberSaveable(orderId) { mutableStateOf<Long?>(null) }
    LaunchedEffect(orderId) { vm.load(orderId) }

    if (!state.loading && state.modules.isEmpty() && state.error == null) return

    Card {
        SectionTitle("Műszaki tartalom", count = state.modules.size.takeIf { it > 0 })
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        state.modules.forEach { m ->
            val color = departmentColor(m.departmentColor)
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp)
                    .height(IntrinsicSize.Min)
                    .background(color.copy(alpha = 0.08f), RoundedCornerShape(12.dp)),
            ) {
                // The department's colour down the left edge, as on the web and the sheet.
                Box(
                    Modifier
                        .width(5.dp)
                        .fillMaxHeight()
                        .background(color, RoundedCornerShape(topStart = 12.dp, bottomStart = 12.dp)),
                )
                Column(Modifier.padding(12.dp).weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Box(Modifier.size(10.dp).background(color, CircleShape))
                        Text(m.departmentName, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold, color = color)
                        Text(
                            moduleStatusLabel(m),
                            style = MaterialTheme.typography.labelMedium,
                            color = if (m.changedSinceRelease) Signal else Steel500,
                        )
                    }
                    Text(
                        m.content.ifBlank { "Még üres." },
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (m.content.isBlank()) Steel500 else MaterialTheme.colorScheme.onSurface,
                    )
                    if (m.files.isNotEmpty()) {
                        Text(
                            "Csatolva: " + m.files.joinToString(", ") { it.filename ?: "fotó" },
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                    m.updatedByName?.let {
                        Text(
                            "$it · ${relativeTime(m.contentUpdatedAt).orEmpty()}",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                    if (m.canEdit) {
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            TextButton(onClick = { editing = m.id }, enabled = !state.busy) { Text("Szerkesztés") }
                            if (m.status == "draft" || m.changedSinceRelease) {
                                TextButton(
                                    onClick = { vm.release(orderId, m) },
                                    enabled = !state.busy && m.content.isNotBlank(),
                                ) { Text(if (m.changedSinceRelease) "Módosítás kiküldése" else "Kiadás") }
                            }
                            if (m.status == "released" && !m.changedSinceRelease) {
                                TextButton(onClick = { vm.setDone(orderId, m, true) }, enabled = !state.busy) { Text("Kész") }
                            }
                            if (m.status == "done") {
                                TextButton(onClick = { vm.setDone(orderId, m, false) }, enabled = !state.busy) { Text("Újranyitás") }
                            }
                        }
                    }
                }
            }
        }
    }

    state.modules.firstOrNull { it.id == editing }?.let { m ->
        var text by rememberSaveable(m.id) { mutableStateOf(m.content) }
        AlertDialog(
            onDismissRequest = { if (!state.busy) editing = null },
            title = { Text(m.departmentName) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    AutoCrmTextField(
                        value = text,
                        onValueChange = { text = it },
                        label = "Műszaki tartalom",
                        singleLine = false,
                        modifier = Modifier.heightIn(min = 160.dp),
                    )
                    if (m.status != "draft") {
                        Text(
                            "Mentés után a részleg nem kap értesítést: a „Módosítás kiküldése” gombbal szóljon nekik.",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { vm.save(orderId, m, text) { editing = null } }, enabled = !state.busy) {
                    Text("Mentés")
                }
            },
            dismissButton = {
                TextButton(onClick = { editing = null }, enabled = !state.busy) { Text("Mégse") }
            },
        )
    }
}
