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
import androidx.compose.ui.platform.LocalDensity
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

private const val CELL = 40
private const val GRID_W = 40
private const val GRID_H = 25

private fun snap(v: Float): Float = (Math.round(v / CELL.toFloat()) * CELL.toFloat())

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
    var draggingOrigin by remember { mutableStateOf<Offset?>(null) }
    var refreshing by remember { mutableStateOf(false) }

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

    fun spotAt(pos: Offset): LayoutItem? {
        return items.lastOrNull { item ->
            if (item.kind != "spot" && item.kind != "workshop_bay") return@lastOrNull false
            val w = if (item.rotation % 180 == 0) item.width else item.height
            val h = if (item.rotation % 180 == 0) item.height else item.width
            pos.x >= item.x && pos.x <= item.x + w && pos.y >= item.y && pos.y <= item.y + h
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

    fun createItem(kind: String, x: Float, y: Float, w: Float, h: Float) {
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
                        x = x.toInt(),
                        y = y.toInt(),
                        width = w.toInt(),
                        height = h.toInt(),
                        color = color,
                    ),
                )
                items = api.yardLayout()
            } catch (e: Exception) {
                onMessage(e.message ?: "Hiba")
            }
        }
    }

    fun moveItem(id: Long, dx: Float, dy: Float) {
        scope.launch {
            try {
                val item = items.find { it.id == id } ?: return@launch
                api.updateLayoutItem(
                    item.id,
                    LayoutItemBody(
                        x = item.x + dx.toInt(),
                        y = item.y + dy.toInt(),
                    ),
                )
                items = api.yardLayout()
            } catch (e: Exception) {
                onMessage(e.message ?: "Hiba")
            }
        }
    }

    if (loading) return ListSkeleton(modifier)
    if (error != null) return ErrorState(error!!, onRetry = { loading = true; error = null; load() }, modifier)

    val unplaced = vehicles.filter { it.locationId == null }

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
            refreshing = refreshing,
            onRefresh = { refreshing = true; load() },
        ) {
            Canvas(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(GRID_W.toFloat() / GRID_H.toFloat())
                    .pointerInput(tool, canConfigure) {
                        if (canConfigure && tool != "select") {
                            detectTapGestures { offset ->
                                val gx = snap(offset.x)
                                val gy = snap(offset.y)
                                createItem(tool, gx, gy, CELL.toFloat(), CELL.toFloat())
                            }
                        }
                    }
                    .pointerInput(canConfigure) {
                        if (canConfigure) {
                            detectDragGestures(
                                onDragStart = { offset ->
                                    val hit = spotAt(offset)
                                    if (hit != null) {
                                        selectedId = hit.id
                                        draggingItemId = hit.id
                                        draggingOrigin = Offset(hit.x.toFloat(), hit.y.toFloat())
                                        dragStart = offset
                                        dragCur = offset
                                    }
                                },
                                onDrag = { change, dragAmount ->
                                    change.consume()
                                    dragCur = change.position
                                },
                                onDragEnd = {
                                    val ds = dragStart
                                    val dc = dragCur
                                    val origin = draggingOrigin
                                    val id = draggingItemId
                                    if (ds != null && dc != null && origin != null && id != null) {
                                        moveItem(id, dc.x - ds.x, dc.y - ds.y)
                                    }
                                    dragStart = null
                                    dragCur = null
                                    draggingItemId = null
                                    draggingOrigin = null
                                },
                                onDragCancel = {
                                    dragStart = null
                                    dragCur = null
                                    draggingItemId = null
                                    draggingOrigin = null
                                },
                            )
                        }
                    }
                    .pointerInput(canMove) {
                        if (canMove) {
                            detectDragGestures(
                                onDragStart = { },
                                onDrag = { _, _ -> },
                                onDragEnd = { },
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
                        strokeWidth = 0.5f,
                    )
                }
                for (gy in 0..GRID_H) {
                    drawLine(
                        Color(0xFFE5E7EB),
                        start = Offset(0f, gy * cellPx),
                        end = Offset(w, gy * cellPx),
                        strokeWidth = 0.5f,
                    )
                }

                for (item in items) {
                    val iw = if (item.rotation % 180 == 0) item.width else item.height
                    val ih = if (item.rotation % 180 == 0) item.height else item.width
                    val c = try { Color(android.graphics.Color.parseColor(item.color)) } catch (_: Exception) { Color.Gray }
                    val isSelected = item.id == selectedId

                    val drawW = iw * cellPx
                    val drawH = ih * cellPx

                    rotate(degrees = item.rotation.toFloat(), pivot = Offset(item.x * cellPx + drawW / 2, item.y * cellPx + drawH / 2)) {
                        drawRect(
                            color = c.copy(alpha = 0.15f),
                            topLeft = Offset(item.x * cellPx, item.y * cellPx),
                            size = Size(drawW, drawH),
                        )
                        drawRect(
                            color = c,
                            topLeft = Offset(item.x * cellPx, item.y * cellPx),
                            size = Size(drawW, drawH),
                            style = Stroke(width = if (isSelected) 3f else 1.5f),
                        )
                    }

                    val label = item.number.ifEmpty { item.label }
                    if (label.isNotEmpty()) {
                        val textResult = measurer.measure(
                            label,
                            style = TextStyle(fontSize = 11.sp, color = Color(0xFF1C1917)),
                        )
                        drawText(
                            textLayoutResult = textResult,
                            topLeft = Offset(
                                item.x * cellPx + drawW / 2 - textResult.size.width / 2f,
                                item.y * cellPx + drawH / 2 - textResult.size.height / 2f,
                            ),
                        )
                    }

                    if (item.kind == "spot" || item.kind == "workshop_bay") {
                        val here = vehicles.filter { it.locationId == item.locationId }
                        here.take(3).forEachIndexed { i, v ->
                            val badgeX = item.x * cellPx + 4 + i * (cellPx * 1.5f)
                            val badgeY = item.y * cellPx + drawH - 18
                            drawRect(
                                color = Color(0xFF1C1917),
                                topLeft = Offset(badgeX, badgeY),
                                size = Size(cellPx * 1.4f, 14f),
                            )
                            val plate = v.plate ?: "#${v.vehicleId}"
                            val plateResult = measurer.measure(
                                plate,
                                style = TextStyle(fontSize = 8.sp, color = Color.White),
                            )
                            drawText(
                                textLayoutResult = plateResult,
                                topLeft = Offset(
                                    badgeX + cellPx * 0.7f - plateResult.size.width / 2f,
                                    badgeY + 7 - plateResult.size.height / 2f,
                                ),
                            )
                        }
                    }
                }

                val ds = dragStart
                val dc = dragCur
                if (ds != null && dc != null && canConfigure && tool != "select") {
                    drawRect(
                        color = Color(0xFF3B82F6).copy(alpha = 0.1f),
                        topLeft = Offset(
                            minOf(snap(ds.x), snap(dc.x)),
                            minOf(snap(ds.y), snap(dc.y)),
                        ),
                        size = Size(
                            maxOf(CELL.toFloat(), kotlin.math.abs(dc.x - ds.x)),
                            maxOf(CELL.toFloat(), kotlin.math.abs(dc.y - ds.y)),
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
                        modifier = Modifier
                            .fillMaxWidth()
                            .pointerInput(v.vehicleId) {
                                detectTapGestures(
                                    onTap = {
                                        val spot = spotAt(it)
                                    },
                                )
                            },
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
                            StatusBadge(tone = "steel") {
                                Text(v.stageLabel ?: "—", style = MaterialTheme.typography.labelSmall)
                            }
                        }
                    }
                }
            }
        }
    }
}
