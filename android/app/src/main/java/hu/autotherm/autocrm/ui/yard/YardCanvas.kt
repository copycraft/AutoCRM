package hu.autotherm.autocrm.ui.yard

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import hu.autotherm.autocrm.data.api.*
import hu.autotherm.autocrm.ui.common.AppPullToRefresh
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.StatusBadge
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin

// The layout is stored on the server in grid cells: an integer (x, y, width, height)
// on a GRID_W x GRID_H board. Everything here converts pixel gestures into cells once,
// at the gesture boundary, so no draw code ever mixes the two.
// One grid cell is one parking place for a spot; buildings and bays span several.
private const val GRID_W = 40
private const val GRID_H = 25

/** Snaps a pixel-space coordinate to the grid: whole cells in cell space. */
private fun snapCell(v: Float): Int = Math.round(v)

@Composable
fun YardCanvas(
    api: AutoCrmApi,
    canMove: Boolean,
    canConfigure: Boolean,
    onMessage: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<LayoutItem>>(emptyList()) }
    var vehicles by remember { mutableStateOf<List<YardVehicle>>(emptyList()) }
    var locations by remember { mutableStateOf<List<YardLocation>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var tool by remember { mutableStateOf("select") }
    var selectedId by remember { mutableStateOf<Long?>(null) }
    var dragStart by remember { mutableStateOf<Offset?>(null) }
    var dragCur by remember { mutableStateOf<Offset?>(null) }
    var draggingItemId by remember { mutableStateOf<Long?>(null) }
    var refreshing by remember { mutableStateOf(false) }

    // Pixel size of the canvas, kept as state so the gesture handlers below can convert
    // finger positions into grid cells (the lambda reads it fresh on every event).
    var canvasWidthPx by remember { mutableStateOf(0f) }

    val measurer = rememberTextMeasurer()

    fun load() {
        scope.launch {
            try {
                error = null
                val layout = api.yardLayout()
                val board = api.yardBoard()
                items = layout
                vehicles = board.vehicles
                locations = board.locations
            } catch (e: Exception) {
                error = e.message ?: "Hiba"
            } finally {
                loading = false
                refreshing = false
            }
        }
    }

    LaunchedEffect(Unit) { load() }

    /** The last spot or bay under a *grid-space* point, hit-testing the rotated bounds. */
    fun spotAtCell(g: Offset): LayoutItem? {
        return items.lastOrNull { item ->
            if (item.kind != "spot" && item.kind != "workshop_bay") return@lastOrNull false
            hitTest(item, g)
        }
    }

    fun moveVehicle(vehicleId: Long, locationId: Long?) {
        scope.launch {
            try {
                val v = vehicles.find { it.vehicleId == vehicleId }
                api.moveVehicle(MoveBody(vehicleId, locationId, v?.orderId))
                val layout = api.yardLayout()
                val board = api.yardBoard()
                items = layout
                vehicles = board.vehicles
            } catch (e: Exception) {
                onMessage(e.message ?: "Hiba")
            }
        }
    }

    fun createItem(kind: String, x: Int, y: Int) {
        scope.launch {
            try {
                val locId = if (kind == "spot") locations.firstOrNull()?.id else null
                val parentId = if (kind == "workshop_bay") selectedId else null
                val color = when (kind) {
                    "spot" -> "#3b82f6"
                    "building" -> "#6b7280"
                    "workshop_bay" -> "#f59e0b"
                    else -> "#6b7280"
                }
                api.saveLayoutItem(
                    LayoutItemBody(
                        kind = kind,
                        locationId = locId,
                        parentId = parentId,
                        x = x,
                        y = y,
                        width = 1,
                        height = 1,
                        color = color,
                    ),
                )
                items = api.yardLayout()
            } catch (e: Exception) {
                onMessage(e.message ?: "Hiba")
            }
        }
    }

    fun moveItem(id: Long, x: Int, y: Int) {
        scope.launch {
            try {
                val item = items.find { it.id == id } ?: return@launch
                api.updateLayoutItem(
                    item.id,
                    LayoutItemBody(
                        x = x.coerceIn(0, GRID_W - item.width),
                        y = y.coerceIn(0, GRID_H - item.height),
                    ),
                )
                items = api.yardLayout()
            } catch (e: Exception) {
                onMessage(e.message ?: "Hiba")
            }
        }
    }

    if (loading) return ListSkeleton(modifier)
    if (error != null) return ErrorState(error!!, modifier, onRetry = { loading = true; error = null; load() })

    val unplaced = vehicles.filter { it.locationId == null }

    // Text measured once when the data changes, not on every frame of a drag.
    val itemLabels = remember(measurer, items, canvasWidthPx) {
        items.mapNotNull { item ->
            val label = item.number.ifEmpty { item.label }
            if (label.isEmpty()) {
                null
            } else {
                item.id to measurer.measure(
                    label,
                    style = TextStyle(fontSize = 11.sp, color = Color(0xFF1C1917)),
                    maxLines = 1,
                )
            }
        }.toMap()
    }
    val plates = remember(measurer, vehicles) {
        vehicles.associate { v ->
            v.vehicleId to measurer.measure(
                v.plate ?: "#${v.vehicleId}",
                style = TextStyle(fontSize = 8.sp, color = Color.White),
                maxLines = 1,
            )
        }
    }

    Column(modifier = modifier.fillMaxWidth()) {
        if (canConfigure) {
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                listOf("select", "spot", "building", "workshop_bay").forEach { tl ->
                    FilterChip(
                        selected = tool == tl,
                        onClick = { tool = tl },
                        label = { Text(when (tl) {
                            "select" -> "Kiválasztás"
                            "spot" -> "Parkoló"
                            "building" -> "Épület"
                            "workshop_bay" -> "Állás"
                            else -> tl
                        }) },
                    )
                }
            }
        }

        AppPullToRefresh(
            isRefreshing = refreshing,
            onRefresh = { refreshing = true; load() },
        ) {
            val gridPointHandler: (Offset) -> Unit = { offset ->
                val cellPx = canvasWidthPx / GRID_W
                if (cellPx > 0f) {
                    val gx = snapCell(offset.x / cellPx)
                    val gy = snapCell(offset.y / cellPx).coerceIn(0, GRID_H - 1)
                    createItem(tool, gx, gy)
                }
            }
            Canvas(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(GRID_W.toFloat() / GRID_H.toFloat())
                    .onSizeChanged { canvasWidthPx = it.width.toFloat() }
                    .pointerInput(tool, canConfigure, canvasWidthPx) {
                        if (canConfigure && tool != "select") {
                            detectTapGestures { offset -> gridPointHandler(offset) }
                        }
                    }
                    .pointerInput(canConfigure, canvasWidthPx) {
                        if (canConfigure) {
                            detectDragGestures(
                                onDragStart = { offset ->
                                    val cellPx = canvasWidthPx / GRID_W
                                    if (cellPx > 0f) {
                                        val hit = spotAtCell(Offset(offset.x / cellPx, offset.y / cellPx))
                                        if (hit != null) {
                                            selectedId = hit.id
                                            draggingItemId = hit.id
                                            dragStart = offset
                                            dragCur = offset
                                        }
                                    }
                                },
                                onDrag = { change, _ ->
                                    change.consume()
                                    dragCur = change.position
                                },
                                onDragEnd = {
                                    val ds = dragStart
                                    val dc = dragCur
                                    val id = draggingItemId
                                    val cellPx = canvasWidthPx / GRID_W
                                    if (ds != null && dc != null && id != null && cellPx > 0f) {
                                        val item = items.find { it.id == id }
                                        if (item != null) {
                                            moveItem(
                                                id,
                                                item.x + Math.round((dc.x - ds.x) / cellPx),
                                                item.y + Math.round((dc.y - ds.y) / cellPx),
                                            )
                                        }
                                    }
                                    dragStart = null
                                    dragCur = null
                                    draggingItemId = null
                                },
                                onDragCancel = {
                                    dragStart = null
                                    dragCur = null
                                    draggingItemId = null
                                },
                            )
                        }
                    },
            ) {
                val w = size.width
                val h = size.height
                val cellPx = w / GRID_W

                drawRect(Color.White, topLeft = Offset.Zero, size = Size(w, h))

                for (gx in 0..GRID_W) {
                    drawLine(
                        Color(0xFFE5E7EB),
                        start = Offset(gx * cellPx, 0f),
                        end = Offset(gx * cellPx, h),
                        strokeWidth = 0.5.dp.toPx(),
                    )
                }
                for (gy in 0..GRID_H) {
                    drawLine(
                        Color(0xFFE5E7EB),
                        start = Offset(0f, gy * cellPx),
                        end = Offset(w, gy * cellPx),
                        strokeWidth = 0.5.dp.toPx(),
                    )
                }

                for (item in items) {
                    val c = try { Color(android.graphics.Color.parseColor(item.color)) } catch (_: Exception) { Color.Gray }
                    val isSelected = item.id == selectedId

                    val pw = item.width * cellPx
                    val ph = item.height * cellPx
                    val cx = item.x * cellPx + pw / 2f
                    val cy = item.y * cellPx + ph / 2f

                    // The rect is drawn at its stored (unrotated) position and rotated about
                    // its own centre — the same geometry spotAtCell hit-tests against.
                    rotate(degrees = item.rotation.toFloat(), pivot = Offset(cx, cy)) {
                        drawRect(
                            color = c.copy(alpha = 0.15f),
                            topLeft = Offset(item.x * cellPx, item.y * cellPx),
                            size = Size(pw, ph),
                        )
                        drawRect(
                            color = c,
                            topLeft = Offset(item.x * cellPx, item.y * cellPx),
                            size = Size(pw, ph),
                            style = Stroke(width = if (isSelected) 3.dp.toPx() else 1.5.dp.toPx()),
                        )
                    }

                    itemLabels[item.id]?.let { labelResult ->
                        drawText(
                            textLayoutResult = labelResult,
                            topLeft = Offset(
                                cx - labelResult.size.width / 2f,
                                cy - labelResult.size.height / 2f,
                            ),
                        )
                    }

                    if (item.kind == "spot" || item.kind == "workshop_bay") {
                        val here = vehicles.filter { it.locationId == item.locationId }
                        // Plates stack inward from the right edge of the spot so they never
                        // spill onto the neighbours, whatever the spot's size.
                        val badgeW = cellPx * 1.4f
                        val badgeH = 14f
                        val pitch = badgeW + 2f
                        val shown = here.take((pw / pitch).toInt().coerceIn(1, 3))
                        shown.forEachIndexed { i, v ->
                            val badgeX = item.x * cellPx + pw - (i + 1) * pitch
                            val badgeY = item.y * cellPx + ph - badgeH - 3f
                            drawRect(
                                color = Color(0xFF1C1917),
                                topLeft = Offset(badgeX, badgeY),
                                size = Size(badgeW, badgeH),
                            )
                            plates[v.vehicleId]?.let { plateResult ->
                                drawText(
                                    textLayoutResult = plateResult,
                                    topLeft = Offset(
                                        badgeX + badgeW / 2f - plateResult.size.width / 2f,
                                        badgeY + badgeH / 2f - plateResult.size.height / 2f,
                                    ),
                                )
                            }
                        }
                    }
                }

                val ds = dragStart
                val dc = dragCur
                if (ds != null && dc != null && canConfigure && tool != "select") {
                    drawRect(
                        color = Color(0xFF3B82F6).copy(alpha = 0.1f),
                        topLeft = Offset(
                            minOf(ds.x, dc.x),
                            minOf(ds.y, dc.y),
                        ),
                        size = Size(
                            maxOf(1.dp.toPx(), abs(dc.x - ds.x)),
                            maxOf(1.dp.toPx(), abs(dc.y - ds.y)),
                        ),
                    )
                }
            }
        }

        if (unplaced.isNotEmpty() && canMove) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Nincs elhelyezve:", style = MaterialTheme.typography.labelMedium)
                unplaced.forEach { v ->
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(
                                v.plate ?: v.vin ?: "#${v.vehicleId}",
                                style = MaterialTheme.typography.titleSmall,
                            )
                            StatusBadge(v.stageLabel ?: "—")
                        }
                    }
                }
            }
        }
    }
}

/**
 * Whether a grid-space point falls inside an item — the inverse rotation of the point
 * about the rect's centre, then an axis-aligned test on the stored rect. This matches
 * exactly how the item is drawn (rect at its stored corner, rotated about its centre).
 */
private fun hitTest(item: LayoutItem, g: Offset): Boolean {
    val cx = item.x + item.width / 2.0
    val cy = item.y + item.height / 2.0
    val a = Math.toRadians(item.rotation.toDouble())
    val cos = cos(-a)
    val sin = sin(-a)
    val dx = g.x - cx
    val dy = g.y - cy
    val rx = cx + dx * cos - dy * sin
    val ry = cy + dx * sin + dy * cos
    return rx >= item.x && rx <= item.x + item.width && ry >= item.y && ry <= item.y + item.height
}
