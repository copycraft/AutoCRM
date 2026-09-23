package hu.autotherm.autocrm.ui.inspection

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import hu.autotherm.autocrm.data.api.InspectionDetail
import hu.autotherm.autocrm.data.inspection.damageTypeLabel
import hu.autotherm.autocrm.data.inspection.severityLabel
import hu.autotherm.autocrm.data.inspection.zoneTitle
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DetailSkeleton
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.theme.Steel500
import java.io.File

/**
 * Check-in review: every check-in damage against the check-out record, zone by
 * zone. Same-zone + same-type pairs suggest "pre-existing"; the inspector
 * confirms, re-links, or dismisses each flag (dirt is not a scratch).
 */
@Composable
fun ComparisonStep(viewModel: WalkaroundViewModel, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    val checkout = viewModel.comparisonCheckout.collectAsState().value

    LaunchedEffect(Unit) {
        if (checkout == null && !state.comparisonLoading) viewModel.loadComparison()
    }

    if (checkout == null) {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (state.comparisonLoading) {
                DetailSkeleton(Modifier.fillMaxWidth())
            } else {
                Card {
                    Text("Összehasonlítás", style = MaterialTheme.typography.titleLarge)
                    Text(
                        "Az átadás adataihoz jel kell.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = Steel500,
                    )
                }
                PrimaryButton(
                    text = "Újrapróbálás",
                    onClick = viewModel::loadComparison,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedButton(
                    onClick = viewModel::gotoSummary,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Kihagyás (aláíráskor úgyis kell)") }
            }
            state.error?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.error)
            }
        }
        return
    }

    val checkoutById = checkout.damages.associateBy { it.id }
    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Card {
                Text("Összehasonlítás az átadással", style = MaterialTheme.typography.titleLarge)
                Text(
                    "Elöl a visszavétel fotója, mellette az átadásé ugyanabból a zónából.",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
            }
        }
        val damages = payload.damages.filter { it.damageType.isNotBlank() }
        if (damages.isEmpty()) {
            item {
                Card {
                    Text("Nincs rögzített sérülés a visszavételen.")
                }
            }
        }
        items(damages, key = { it.localId }) { damage ->
            val verdict = payload.verdicts.firstOrNull { v -> v.damageLocalId == damage.localId }
            // Same-zone + same-type check-out damage, if any.
            val match = checkout.damages.firstOrNull {
                it.zoneKey == damage.zoneKey && it.damageType == damage.damageType
            }
            val checkoutPhotos = checkout.photos.filter {
                it.zoneKey == damage.zoneKey && it.purpose == "overview"
            }.mapNotNull { it.displayUrl }
            val checkinFiles = payload.photos.filter {
                it.damageLocalId == damage.localId && it.purpose == "closeup"
            }
            Card {
                Text(
                    "${zoneTitle(damage.zoneKey)} · ${damageTypeLabel(damage.damageType)} · " +
                        severityLabel(damage.severity),
                    style = MaterialTheme.typography.titleMedium,
                )
                if (match != null) {
                    StatusBadge("Átadáskor is megvolt", Tone.Cold)
                } else {
                    StatusBadge("Újnak tűnik", Tone.Signal)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Column(Modifier.weight(1f)) {
                        Text("Most", style = MaterialTheme.typography.labelMedium, color = Steel500)
                        checkinFiles.firstOrNull()?.let { photo ->
                            val context = LocalContext.current
                            AsyncImage(
                                model = File(
                                    context.filesDir,
                                    "inspections/${state.uuid}/${photo.fileName}",
                                ),
                                contentDescription = null,
                                contentScale = ContentScale.Crop,
                                modifier = Modifier.fillMaxWidth().size(140.dp)
                                    .clip(RoundedCornerShape(8.dp)),
                            )
                        }
                    }
                    Column(Modifier.weight(1f)) {
                        Text("Átadáskor", style = MaterialTheme.typography.labelMedium, color = Steel500)
                        checkoutPhotos.firstOrNull()?.let { url ->
                            AsyncImage(
                                model = url,
                                contentDescription = null,
                                contentScale = ContentScale.Crop,
                                modifier = Modifier.fillMaxWidth().size(140.dp)
                                    .clip(RoundedCornerShape(8.dp)),
                            )
                        }
                    }
                }
                val current = verdict?.verdict
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    VerdictChip("Megvolt", current == "preexisting") {
                        viewModel.setVerdict(damage.localId, match?.id, "preexisting", null)
                    }
                    VerdictChip("Új", current == "new") {
                        viewModel.setVerdict(damage.localId, null, "new", null)
                    }
                    VerdictChip("Nem sérülés", current == "dismissed") {
                        viewModel.setVerdict(damage.localId, null, "dismissed", null)
                    }
                }
                if (match != null && current == null) {
                    TextButton(
                        onClick = {
                            viewModel.setVerdict(damage.localId, match.id, "preexisting", null)
                        },
                    ) { Text("Javaslat elfogadása") }
                }
            }
        }
        item {
            val pending = damages.count { d ->
                payload.verdicts.none { it.damageLocalId == d.localId }
            }
            PrimaryButton(
                text = if (pending > 0) "Tovább ($pending döntés hátra)" else "Tovább az összesítőre",
                onClick = viewModel::gotoSummary,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun VerdictChip(label: String, selected: Boolean, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label) },
    )
}

@Composable
fun SummaryStep(
    viewModel: WalkaroundViewModel,
    onSigning: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Card {
                Text("Összesítő", style = MaterialTheme.typography.titleLarge)
                Text(
                    "${payload.vehiclePlate} · ${payload.photos.size} fotó · " +
                        "${payload.damages.count { it.damageType.isNotBlank() }} sérülés",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
                Text(
                    "Óraállás: ${payload.odometer.ifBlank { "—" }} km · " +
                        "Üzemanyag: ${payload.fuelLevel ?: "—"}",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
            }
        }
        val byZone = payload.photos.filter { it.purpose == "overview" }.groupBy { it.zoneKey }
        items(payload.templates, key = { it.zoneKey }) { zone ->
            val photos = byZone[zone.zoneKey].orEmpty()
            val damages = payload.damages.filter {
                it.zoneKey == zone.zoneKey && it.damageType.isNotBlank()
            }
            Card {
                Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(zoneTitle(zone.zoneKey), style = MaterialTheme.typography.titleMedium)
                    if (damages.isNotEmpty()) StatusBadge("${damages.size} sérülés", Tone.Signal)
                }
                if (photos.isNotEmpty()) {
                    LocalPhotoRow(photos.map { it.fileName }, state.uuid)
                } else {
                    Text("Nincs fotó.", style = MaterialTheme.typography.labelMedium, color = Steel500)
                }
                damages.forEach { damage ->
                    Text(
                        "${damageTypeLabel(damage.damageType)} · ${severityLabel(damage.severity)}" +
                            (damage.note?.takeIf { it.isNotBlank() }?.let { " · $it" } ?: ""),
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
            }
        }
        if (state.kind == "checkin") {
            item {
                val newCount = payload.verdicts.count { it.verdict == "new" }
                val preCount = payload.verdicts.count { it.verdict == "preexisting" }
                Card {
                    Text("Új sérülések: $newCount · Korábbiak: $preCount")
                }
            }
        }
        item {
            AutoCrmTextField(
                value = payload.customerComment,
                onValueChange = { v -> viewModel.setReadings { it.copy(customerComment = v) } },
                label = "Ügyfél megjegyzése / vita (opcionális)",
                singleLine = false,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        item {
            PrimaryButton(
                text = "Tovább az aláíráshoz",
                onClick = onSigning,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
fun SigningStep(viewModel: WalkaroundViewModel, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsState()
    val payload = state.payload ?: return
    val context = LocalContext.current
    val draftDir = File(context.filesDir, "inspections/${state.uuid}").apply { mkdirs() }
    val inspector = payload.signatures.first { it.role == "inspector" }
    val customer = payload.signatures.first { it.role == "customer" }
    val inspectorPad = rememberSignatureState()
    val customerPad = rememberSignatureState()

    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Card {
                Text("Aláírás lezáráshoz", style = MaterialTheme.typography.titleLarge)
                Text(
                    "Aláírás után az átvétel zárolva van: csak külön, időbélyegzett " +
                        "megjegyzés fűzhető hozzá.",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
            }
        }
        item {
            SignatureBlock(
                title = "Átadó",
                name = inspector.name,
                onName = { v -> viewModel.setSignatureName("inspector", v) },
                pad = inspectorPad,
                savedFile = inspector.fileName,
                draftDir = draftDir,
                fileName = "sig_inspector.png",
                onSaved = { file -> viewModel.saveSignatureFile("inspector", file) },
            )
        }
        item {
            SignatureBlock(
                title = "Ügyfél / sofőr",
                name = customer.name,
                onName = { v -> viewModel.setSignatureName("customer", v) },
                pad = customerPad,
                savedFile = customer.fileName,
                draftDir = draftDir,
                fileName = "sig_customer.png",
                onSaved = { file -> viewModel.saveSignatureFile("customer", file) },
            )
        }
        item {
            state.error?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.error)
            }
            Button(
                onClick = viewModel::signNow,
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Aláírás és lezárás") }
        }
    }
}

/** Signature pad with save/clear around it; the PNG lands in the draft dir. */
@Composable
private fun SignatureBlock(
    title: String,
    name: String,
    onName: (String) -> Unit,
    pad: SignatureState,
    savedFile: String?,
    draftDir: File,
    fileName: String,
    onSaved: (File) -> Unit,
) {
    Card {
        Text(title, style = MaterialTheme.typography.titleMedium)
        AutoCrmTextField(
            value = name,
            onValueChange = onName,
            label = "Nyomtatott név *",
            modifier = Modifier.fillMaxWidth(),
        )
        SignaturePad(state = pad, modifier = Modifier.fillMaxWidth())
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { pad.clear() }, modifier = Modifier.weight(1f)) {
                Text("Törlés")
            }
            Button(
                onClick = {
                    val file = File(draftDir, fileName)
                    pad.savePng(file)?.let { onSaved(it) }
                },
                enabled = pad.hasContent,
                modifier = Modifier.weight(1f),
            ) { Text(if (savedFile == null) "Mentés" else "Újramentés") }
        }
        if (savedFile != null) {
            Text("Aláírás mentve.", style = MaterialTheme.typography.labelMedium, color = Steel500)
        }
    }
}

@Composable
fun DoneStep(onExit: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Card {
            Text("Átvétel lezárva", style = MaterialTheme.typography.titleLarge)
            Text(
                "Az adatok a telefonon vannak, jellel feltöltődnek. " +
                    "A fotók a Sorban követhetők.",
                style = MaterialTheme.typography.bodyLarge,
            )
        }
        PrimaryButton(text = "Vissza a munkához", onClick = onExit, modifier = Modifier.fillMaxWidth())
    }
}

/** Server-side read-only detail (history): photos, damages, signatures, notes. */
@Composable
fun ServerInspectionDetail(detail: InspectionDetail, modifier: Modifier = Modifier) {
    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Card {
                Text(
                    "${detail.inspection.vehiclePlate} · " +
                        if (detail.inspection.kind == "checkin") "Visszavétel" else "Kiadás",
                    style = MaterialTheme.typography.titleLarge,
                )
                Text(
                    "${detail.inspection.inspectorName} · ${detail.inspection.signedAt ?: detail.inspection.createdAt}",
                    style = MaterialTheme.typography.labelMedium,
                    color = Steel500,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatusBadge(
                        if (detail.inspection.status == "signed") "Lezárva" else "Piszkozat",
                        if (detail.inspection.status == "signed") Tone.Done else Tone.Steel,
                    )
                }
            }
        }
        val byZone = detail.photos.filter { it.purpose == "overview" }.groupBy { it.zoneKey }
        items(byZone.entries.toList(), key = { it.key }) { (zone, photos) ->
            Card {
                Text(zoneTitle(zone), style = MaterialTheme.typography.titleMedium)
                photos.forEach { photo ->
                    photo.displayUrl?.let { url ->
                        AsyncImage(
                            model = url,
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxWidth().size(180.dp)
                                .clip(RoundedCornerShape(8.dp)),
                        )
                    }
                }
                detail.damages.filter { it.zoneKey == zone }.forEach { damage ->
                    Text(
                        "${damageTypeLabel(damage.damageType)} · ${severityLabel(damage.severity)}" +
                            (damage.note?.takeIf { it.isNotBlank() }?.let { " · $it" } ?: ""),
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
            }
        }
        if (detail.notes.isNotEmpty()) {
            item { Text("Megjegyzések", style = MaterialTheme.typography.titleMedium) }
            items(detail.notes, key = { it.id }) { note ->
                Card { Text(note.body, style = MaterialTheme.typography.bodyLarge) }
            }
        }
    }
}
