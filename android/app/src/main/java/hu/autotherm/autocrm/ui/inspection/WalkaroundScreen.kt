@file:OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)

package hu.autotherm.autocrm.ui.inspection

import androidx.compose.material.icons.filled.PhotoCamera
import androidx.compose.foundation.background
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import hu.autotherm.autocrm.data.inspection.DraftDamage
import hu.autotherm.autocrm.data.inspection.damageTypeLabel
import hu.autotherm.autocrm.data.inspection.severityLabel
import hu.autotherm.autocrm.data.inspection.displayTitle
import hu.autotherm.autocrm.data.inspection.walkaroundKindLabel
import hu.autotherm.autocrm.data.inspection.zoneTitle
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.theme.Steel500
import java.io.File

/**
 * The guided walkaround: readings → zone loop (camera → review → damage?) →
 * comparison (check-in) → summary → signatures. One hand, big buttons, minimal
 * typing. The camera opens automatically for every shot; review is one tap.
 */
@Composable
fun WalkaroundScreen(
    uuid: String?,
    orderId: Long,
    kind: String,
    viewModel: WalkaroundViewModel,
    onExit: () -> Unit,
) {
    val state by viewModel.state.collectAsState()

    LaunchedEffect(uuid, orderId, kind) {
        if (state.payload == null && state.error == null) {
            viewModel.open(uuid, orderId, kind)
        }
    }
    // Back steps back through the walkaround (camera, damage, zone, summary) instead of
    // dropping the fitter out of the whole inspection.
    androidx.activity.compose.BackHandler {
        if (!viewModel.back()) onExit()
    }
    // The fitter walks round the van with the phone in hand: the screen must not dim and
    // lock between zones (and ask for the PIN with gloves on).
    val view = androidx.compose.ui.platform.LocalView.current
    androidx.compose.runtime.DisposableEffect(view) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }

    // Full-screen camera or review take over everything while active.
    val review = state.review
    if (review != null) {
        PhotoReview(
            file = review.second,
            onRetake = viewModel::retakePhoto,
            onKeep = viewModel::keepPhoto,
        )
        return
    }
    state.videoCapture?.let { request ->
        VideoCaptureScreen(
            request = request,
            onRecorded = { durationMs -> viewModel.onVideoRecorded(request, durationMs) },
            onCancel = viewModel::cancelVideo,
        )
        return
    }
    state.capture?.let { request ->
        CameraCapture(
            instruction = request.instruction,
            outputFile = request.file,
            onCaptured = { viewModel.onCaptured(request) },
            onCancel = viewModel::cancelCapture,
        )
        return
    }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = walkaroundKindLabel(if (kind == "checkin" || state.kind == "checkin") "checkin" else "checkout", state.lookups),
                subtitle = if (state.orderNumber.isNotBlank()) {
                    "${state.orderNumber} · ${state.payload?.vehiclePlate.orEmpty()}"
                } else null,
                onBack = onExit,
            )
        },
    ) { padding ->
        when (val phase = state.phase) {
            is Phase.Loading -> if (state.payload == null && state.error != null) {
                // The order (or templates) failed to load: a bare skeleton here would
                // spin forever with no way out. Retry, or leave the walkaround.
                ErrorState(
                    state.error!!,
                    Modifier.fillMaxSize().padding(padding),
                    onRetry = viewModel::retryOpen,
                )
            } else {
                DetailSkeleton(
                    Modifier.fillMaxSize().padding(padding).padding(16.dp),
                )
            }
            is Phase.Readings -> ReadingsStep(
                viewModel = viewModel,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            is Phase.Zone -> ZoneStep(
                viewModel = viewModel,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            is Phase.Damage -> DamageStep(
                viewModel = viewModel,
                damageLocalId = phase.damageLocalId,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            is Phase.Comparison -> ComparisonStep(
                viewModel = viewModel,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            is Phase.Summary -> SummaryStep(
                viewModel = viewModel,
                onSigning = viewModel::gotoSigning,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            is Phase.Signing -> SigningStep(
                viewModel = viewModel,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
            is Phase.Done -> DoneStep(
                onExit = onExit,
                modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            )
        }
        state.error?.let {
            // Transient banner under the top bar content is handled per step; a
            // global fallback keeps unexpected errors visible anywhere.
        }
    }
}

@Composable
private fun ReadingsStep(viewModel: WalkaroundViewModel, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        state.notice?.let { notice ->
            item {
                Card {
                    Text(notice, style = MaterialTheme.typography.bodyLarge)
                }
            }
        }
        item {
            Card {
                Text("${walkaroundKindLabel(state.kind, state.lookups)} adatai", style = MaterialTheme.typography.titleLarge)
                AutoCrmTextField(
                    value = payload.inspectorName,
                    onValueChange = { v -> viewModel.setReadings { it.copy(inspectorName = v) } },
                    label = "Átadó / felvevő neve *",
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Words),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = payload.driverName,
                    onValueChange = { v -> viewModel.setReadings { it.copy(driverName = v) } },
                    label = "Sofőr / ügyfél neve",
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Words),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = payload.location,
                    onValueChange = { v -> viewModel.setReadings { it.copy(location = v) } },
                    label = "Helyszín",
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
        item {
            Card {
                Text("Óraállás, üzemanyag, figyelmeztetések", style = MaterialTheme.typography.titleLarge)
                // Which photos are taken is the zone list's business, not this screen's: a
                // dashboard shot is a zone like any other, asked for (or not) by the server.
                Text(
                    "Olvasd le és írd be az értékeket.",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
                AutoCrmTextField(
                    value = payload.odometer,
                    onValueChange = { v ->
                        viewModel.setReadings { it.copy(odometer = v.filter { c -> c.isDigit() }) }
                    },
                    label = "Óraállás (km)",
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                        keyboardType = androidx.compose.ui.text.input.KeyboardType.Number,
                    ),
                    visualTransformation = hu.autotherm.autocrm.util.GroupedNumberTransformation,
                    modifier = Modifier.fillMaxWidth(),
                    trailing = {
                        androidx.compose.material3.IconButton(onClick = viewModel::requestOdometerPhoto) {
                            androidx.compose.material3.Icon(
                                androidx.compose.material.icons.Icons.Filled.PhotoCamera,
                                contentDescription = "Óraállás fotóról",
                            )
                        }
                    },
                )
                if (state.readingOdometer) {
                    Text("Számok felismerése a fotón…", style = MaterialTheme.typography.labelMedium, color = Steel500)
                }
                if (state.odometerCandidates.isNotEmpty()) {
                    Text("Felismert értékek – koppintson a helyesre:", style = MaterialTheme.typography.labelMedium)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        state.odometerCandidates.forEach { value ->
                            FilterChip(
                                selected = payload.odometer == value,
                                onClick = { viewModel.useOdometer(value) },
                                label = { Text("$value km") },
                            )
                        }
                    }
                }
                Text("Üzemanyagszint", style = MaterialTheme.typography.labelMedium)
                // FlowRow, not LazyRow: lazy rows measure infinite inside lazy items
                // and crash on the device. The marks come from the server's lookups.
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    (state.lookups?.fuelLevels.orEmpty()).forEach { entry ->
                        FilterChip(
                            selected = payload.fuelLevel == entry.key,
                            onClick = {
                                viewModel.setReadings {
                                    it.copy(fuelLevel = if (it.fuelLevel == entry.key) null else entry.key)
                                }
                            },
                            label = { Text(entry.labelHu.ifBlank { entry.key }) },
                        )
                    }
                }
                AutoCrmTextField(
                    value = payload.batteryPct,
                    onValueChange = { v ->
                        viewModel.setReadings { it.copy(batteryPct = v.filter { c -> c.isDigit() }.take(3)) }
                    },
                    label = "Akkumulátor (%) – elektromosnál",
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                        keyboardType = androidx.compose.ui.text.input.KeyboardType.Number,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                AutoCrmTextField(
                    value = payload.warningLights,
                    onValueChange = { v -> viewModel.setReadings { it.copy(warningLights = v) } },
                    label = "Égő figyelmeztető lámpák",
                    singleLine = false,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
        item { TyresCard(viewModel) }
        item {
            state.error?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.error)
            }
            PrimaryButton(
                text = "Körbejárás indítása",
                onClick = {
                    if (payload.inspectorName.isBlank()) {
                        viewModel.setError("az átadó neve kötelező")
                    } else {
                        viewModel.readingsDone()
                    }
                },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun ZoneStep(viewModel: WalkaroundViewModel, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    val templates = payload.templates
    if (payload.zoneIndex >= templates.size) {
        // Walk finished from another path; move on.
        LaunchedEffect(Unit) { viewModel.gotoSummary() }
        return
    }
    val zone = templates[payload.zoneIndex]
    val overviews = payload.photos.filter { it.zoneKey == zone.zoneKey && it.purpose == "overview" }
    val damages = payload.damages.filter { it.zoneKey == zone.zoneKey && it.damageType.isNotBlank() }

    // The camera opens automatically once per zone without an overview yet. Once
    // only: cancelling must land back on the zone step, not reopen the camera in
    // a loop the user cannot escape (which reads as a crash).
    var autoOpened by rememberSaveable(zone.zoneKey) { mutableStateOf(false) }
    LaunchedEffect(zone.zoneKey, overviews.isEmpty()) {
        if (overviews.isEmpty() && !autoOpened) {
            autoOpened = true
            viewModel.requestCapture("overview", zone.zoneKey, zone.instruction)
        }
    }

    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            LinearProgressIndicator(
                progress = { (payload.zoneIndex).toFloat() / templates.size.toFloat() },
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                "${payload.zoneIndex + 1} / ${templates.size} · ${zone.displayTitle()}",
                style = MaterialTheme.typography.titleLarge,
            )
            Text(zone.instruction, style = MaterialTheme.typography.bodyLarge, color = Steel500)
        }
        if (overviews.isNotEmpty()) {
            item {
                Card {
                    LocalPhotoRow(overviews.map { it.fileName }, state.uuid)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(
                            onClick = {
                                viewModel.requestCapture("overview", zone.zoneKey, zone.instruction)
                            },
                            modifier = Modifier.weight(1f),
                        ) { Text("Újrafotózás") }
                        OutlinedButton(
                            onClick = { viewModel.requestVideo(zone.zoneKey, zone.displayTitle()) },
                            modifier = Modifier.weight(1f),
                        ) { Text("Videó") }
                    }
                    val clips = payload.videos.filter { it.zoneKey == zone.zoneKey }
                    if (clips.isNotEmpty()) {
                        Text(
                            "${clips.size} videó ehhez a zónához",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                }
            }
            item {
                Card {
                    Text("Van sérülés ebben a zónában?", style = MaterialTheme.typography.titleLarge)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = {
                                val id = viewModel.startDamage(zone.zoneKey)
                                viewModel.requestCapture(
                                    "closeup",
                                    zone.zoneKey,
                                    "Közeli kép a sérülésről",
                                    damageLocalId = id,
                                )
                            },
                            modifier = Modifier.weight(1f),
                        ) { Text("Igen") }
                        OutlinedButton(
                            onClick = viewModel::nextZone,
                            modifier = Modifier.weight(1f),
                        ) { Text("Nem") }
                    }
                }
            }
            if (damages.isNotEmpty()) {
                item {
                    Text("Rögzített sérülések", style = MaterialTheme.typography.titleMedium)
                }
                items(damages, key = { it.localId }) { damage ->
                    DamageCard(damage = damage, lookups = state.lookups)
                }
            }
        } else {
            item {
                PrimaryButton(
                    text = "Fotó készítése",
                    onClick = {
                        viewModel.requestCapture("overview", zone.zoneKey, zone.instruction)
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
                // An optional zone ("Tető (ha elérhető)") may be skipped: sign-off only
                // requires overviews of the non-optional zones.
                if (zone.optional) {
                    OutlinedButton(
                        onClick = viewModel::nextZone,
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text("Kihagyás") }
                }
            }
        }
        state.error?.let {
            item {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.error)
            }
        }
    }
}

@Composable
private fun DamageStep(
    viewModel: WalkaroundViewModel,
    damageLocalId: String,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    val damage = payload.damages.firstOrNull { it.localId == damageLocalId } ?: run {
        LaunchedEffect(Unit) { viewModel.damageDone() }
        return
    }
    val closeups = payload.photos.filter {
        it.damageLocalId == damageLocalId && it.purpose == "closeup"
    }
    var subStep by rememberSaveable(damageLocalId) { mutableStateOf(0) } // 0 photo → 1 more? → 2 form → 3 another?

    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Text("Sérülés dokumentálása", style = MaterialTheme.typography.titleLarge)
            Text(
                "${zoneTitle(damage.zoneKey, payload.templates)} · ${closeups.size} közeli fotó",
                style = MaterialTheme.typography.labelMedium,
                color = Steel500,
            )
        }
        when (subStep) {
            0 -> {
                item {
                    PrimaryButton(
                        text = if (closeups.isEmpty()) "Közeli fotó készítése" else "További közeli fotó",
                        onClick = {
                            viewModel.requestCapture(
                                "closeup",
                                damage.zoneKey,
                                "Közeli kép a sérülésről",
                                damageLocalId = damageLocalId,
                            )
                        },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (closeups.isNotEmpty()) {
                    item { Card { LocalPhotoRow(closeups.map { it.fileName }, state.uuid) } }
                    item {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedButton(
                                onClick = {
                                    viewModel.requestCapture(
                                        "closeup",
                                        damage.zoneKey,
                                        "További közeli kép a sérülésről",
                                        damageLocalId = damageLocalId,
                                    )
                                },
                                modifier = Modifier.weight(1f),
                            ) { Text("Még egy közeli") }
                            Button(
                                onClick = { subStep = 1 },
                                modifier = Modifier.weight(1f),
                            ) { Text("Leírás") }
                        }
                    }
                }
            }
            1 -> {
                item {
                    Card {
                        Text("Helye a karosszérián", style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Koppints a sérülés helyére.",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                        CarOutline(
                            marks = damage.x?.let { x ->
                                damage.y?.let { y -> listOf(x.toFloat() to y.toFloat()) }
                            }.orEmpty(),
                            onTap = { x, y ->
                                viewModel.updateDamage(damageLocalId) {
                                    it.copy(x = x.toDouble(), y = y.toDouble())
                                }
                            },
                        )
                    }
                }
                item {
                    Card {
                        Text("Sérülés típusa", style = MaterialTheme.typography.titleMedium)
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            // The server's damage types; an empty cached document
                            // means no chips until the list downloads.
                            (state.lookups?.damageTypes.orEmpty()).forEach { entry ->
                                FilterChip(
                                    selected = damage.damageType == entry.key,
                                    onClick = {
                                        viewModel.updateDamage(damageLocalId) { it.copy(damageType = entry.key) }
                                    },
                                    label = { Text(entry.labelHu.ifBlank { entry.key }) },
                                )
                            }
                        }
                    }
                }
                item {
                    Card {
                        Text("Súlyosság", style = MaterialTheme.typography.titleMedium)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            (state.lookups?.severities.orEmpty()).forEach { entry ->
                                FilterChip(
                                    selected = damage.severity == entry.key,
                                    onClick = {
                                        viewModel.updateDamage(damageLocalId) { it.copy(severity = entry.key) }
                                    },
                                    label = { Text(entry.labelHu.ifBlank { entry.key }) },
                                )
                            }
                        }
                        AutoCrmTextField(
                            value = damage.note.orEmpty(),
                            onValueChange = { v ->
                                viewModel.updateDamage(damageLocalId) { it.copy(note = v) }
                            },
                            label = "Megjegyzés (opcionális)",
                            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                            singleLine = false,
                            trailing = {
                                hu.autotherm.autocrm.ui.common.DictationButton(onText = { spoken ->
                                    viewModel.updateDamage(damageLocalId) {
                                        it.copy(note = hu.autotherm.autocrm.ui.common.appendDictated(it.note.orEmpty(), spoken))
                                    }
                                })
                            },
                        )
                    }
                }
                item {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        TextButton(onClick = { viewModel.removeDamage(damageLocalId) }) {
                            Text("Eldobás")
                        }
                        PrimaryButton(
                            text = "Kész",
                            onClick = {
                                if (damage.damageType.isBlank() || damage.severity.isBlank()) {
                                    viewModel.setError("válassz típust és súlyosságot")
                                } else {
                                    viewModel.setError(null)
                                    subStep = 2
                                }
                            },
                            modifier = Modifier.weight(1f),
                        )
                    }
                    state.error?.let {
                        Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.error)
                    }
                }
            }
            else -> {
                item {
                    Card {
                        Text("Van másik sérülés ebben a zónában?", style = MaterialTheme.typography.titleLarge)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(
                                onClick = {
                                    val id = viewModel.startDamage(damage.zoneKey)
                                    viewModel.requestCapture(
                                        "closeup",
                                        damage.zoneKey,
                                        "Közeli kép a sérülésről",
                                        damageLocalId = id,
                                    )
                                },
                                modifier = Modifier.weight(1f),
                            ) { Text("Igen") }
                            OutlinedButton(
                                onClick = viewModel::damageDone,
                                modifier = Modifier.weight(1f),
                            ) { Text("Nincs, tovább") }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DamageCard(damage: DraftDamage, lookups: hu.autotherm.autocrm.data.api.Lookups?) {
    Card {
        Text(
            "${damageTypeLabel(damage.damageType, lookups)} · ${severityLabel(damage.severity, lookups)}",
            style = MaterialTheme.typography.titleMedium,
        )
        damage.note?.takeIf { it.isNotBlank() }?.let {
            Text(it, style = MaterialTheme.typography.bodyLarge)
        }
    }
}

@Composable
fun LocalPhotoRow(fileNames: List<String>, uuid: String) {
    val context = LocalContext.current
    // Fixed height: a lazy row inside lazy items/columns measures infinite
    // otherwise, which crashes on the device (same class as ListSkeleton did).
    LazyRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.height(120.dp),
    ) {
        items(fileNames, key = { it }) { name ->
            AsyncImage(
                model = File(context.filesDir, "inspections/$uuid/$name"),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.size(120.dp).clip(RoundedCornerShape(8.dp)),
            )
        }
    }
}

@Composable
private fun PhotoReview(
    file: File,
    onRetake: () -> Unit,
    onKeep: () -> Unit,
) {
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface)) {
        AsyncImage(
            model = file,
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = Modifier.fillMaxWidth().weight(1f),
        )
        Row(
            Modifier.fillMaxWidth().padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedButton(onClick = onRetake, modifier = Modifier.weight(1f)) {
                Text("Újra")
            }
            Button(onClick = onKeep, modifier = Modifier.weight(1f)) {
                Text("Megtartom")
            }
        }
    }
}

/** Tread and condition per tyre (0048). Only the tyres touched are recorded. */
@Composable
private fun TyresCard(viewModel: WalkaroundViewModel) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    val positions = state.lookups?.tyrePositions.orEmpty()
    val conditions = state.lookups?.tyreConditions.orEmpty()
    if (positions.isEmpty()) return
    Card {
        Text("Gumik", style = MaterialTheme.typography.titleLarge)
        Text(
            "Profilm\u00e9lys\u00e9g mm-ben \u00e9s \u00e1llapot. Ami nincs kit\u00f6ltve, nem ker\u00fcl r\u00f6gz\u00edt\u00e9sre.",
            style = MaterialTheme.typography.labelMedium,
            color = Steel500,
        )
        positions.forEach { pos ->
            val tyre = payload.tyres.firstOrNull { it.position == pos.key }
            Text(pos.labelHu.ifBlank { pos.key }, style = MaterialTheme.typography.titleMedium)
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                conditions.forEach { c ->
                    FilterChip(
                        selected = tyre?.condition == c.key,
                        onClick = {
                            viewModel.setTyre(pos.key) { cur ->
                                if (cur?.condition == c.key) null
                                else (cur ?: hu.autotherm.autocrm.data.inspection.DraftTyre(position = pos.key)).copy(condition = c.key)
                            }
                        },
                        label = { Text(c.labelHu.ifBlank { c.key }) },
                    )
                }
            }
            if (tyre != null) {
                AutoCrmTextField(
                    value = tyre.treadMm,
                    onValueChange = { v ->
                        viewModel.setTyre(pos.key) { cur ->
                            cur?.copy(treadMm = v.filter { ch -> ch.isDigit() || ch == ',' || ch == '.' }.take(4))
                        }
                    },
                    label = "Profilm\u00e9lys\u00e9g (mm)",
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                        keyboardType = androidx.compose.ui.text.input.KeyboardType.Decimal,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}
