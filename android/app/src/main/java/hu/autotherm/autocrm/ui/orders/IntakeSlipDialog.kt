package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.data.api.Order
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.theme.Signal
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** The five marks on a fuel gauge, matching the backend CHECK. A gauge, not a text field. */
internal val FUEL_LEVELS = listOf("E", "1/4", "1/2", "3/4", "F")

/**
 * The PATCH body for the intake slip. Mileage is always present (it is the gate);
 * every other field the user empties is sent as an explicit null so the server clears
 * it — omitted fields would be kept (ORD-L4). `valuables_declared` is always a definite
 * answer, mirroring the web: saving the slip records the answer, and null would mean
 * "never asked".
 */
internal fun intakeSlipJson(
    mileageKm: Int,
    fuelLevel: String?,
    keyCount: Int?,
    condition: String?,
    hasValuables: Boolean,
    valuables: String?,
): JsonObject = buildJsonObject {
    put("mileage_in", mileageKm)
    if (fuelLevel == null) put("fuel_level", JsonNull) else put("fuel_level", fuelLevel)
    if (keyCount == null) put("key_count", JsonNull) else put("key_count", keyCount)
    val text = condition?.trim().takeIf { !it.isNullOrEmpty() }
    if (text == null) put("intake_condition", JsonNull) else put("intake_condition", text)
    put("valuables_declared", hasValuables)
    val items = valuables?.trim().takeIf { hasValuables && !it.isNullOrEmpty() }
    if (items == null) put("valuables", JsonNull) else put("valuables", items)
}

@Composable
internal fun IntakeSlipDialog(
    order: Order,
    busy: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onSave: (JsonObject) -> Unit,
) {
    var mileage by rememberSaveable { mutableStateOf(order.mileageIn?.toString() ?: "") }
    var fuel by rememberSaveable { mutableStateOf(order.fuelLevel) }
    var keys by rememberSaveable { mutableStateOf(order.keyCount?.toString() ?: "") }
    var condition by rememberSaveable { mutableStateOf(order.intakeCondition ?: "") }
    var hasValuables by rememberSaveable { mutableStateOf(order.valuablesDeclared == true) }
    var valuables by rememberSaveable { mutableStateOf(order.valuables ?: "") }

    val mileageKm = mileage.trim().toIntOrNull()?.takeIf { it >= 0 }
    val keysCount = keys.trim().takeIf { it.isNotEmpty() }?.toIntOrNull()?.takeIf { it >= 0 }
    val keysValid = keys.trim().isEmpty() || keysCount != null
    val canSave = !busy && mileageKm != null && keysValid

    DialogShell(
        title = "Átvételi lap",
        onDismiss = onDismiss,
        actions = {
            TextButton(onClick = onDismiss) { Text("Mégse") }
            PrimaryButton(
                text = if (busy) "Mentés…" else "Mentés",
                enabled = canSave,
                onClick = {
                    onSave(
                        intakeSlipJson(
                            mileageKm = mileageKm!!,
                            fuelLevel = fuel,
                            keyCount = keysCount,
                            condition = condition,
                            hasValuables = hasValuables,
                            valuables = valuables,
                        ),
                    )
                },
            )
        },
    ) {
        AutoCrmTextField(
            value = mileage,
            onValueChange = { mileage = it.filter { c -> c.isDigit() } },
            label = "Km-óra állás *",
            modifier = Modifier.fillMaxWidth(),
        )
        Text("Üzemanyag", style = MaterialTheme.typography.labelMedium)
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            FUEL_LEVELS.forEach { level ->
                FilterChip(
                    selected = fuel == level,
                    onClick = { fuel = if (fuel == level) null else level },
                    label = { Text(level) },
                )
            }
        }
        AutoCrmTextField(
            value = keys,
            onValueChange = { keys = it.filter { c -> c.isDigit() } },
            label = "Kulcsok száma",
            modifier = Modifier.fillMaxWidth(),
        )
        AutoCrmTextField(
            value = condition,
            onValueChange = { condition = it },
            label = "Állapot",
            modifier = Modifier.fillMaxWidth(),
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            Checkbox(
                checked = hasValuables,
                onCheckedChange = { hasValuables = it },
            )
            Text("Van értéktárgy a járműben", style = MaterialTheme.typography.bodyLarge)
        }
        if (hasValuables) {
            AutoCrmTextField(
                value = valuables,
                onValueChange = { valuables = it },
                label = "Értéktárgyak",
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (!keysValid) {
            Text(
                "A kulcsok száma nem lehet negatív.",
                style = MaterialTheme.typography.bodyLarge,
                color = Signal,
            )
        }
        error?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = Signal) }
    }
}
