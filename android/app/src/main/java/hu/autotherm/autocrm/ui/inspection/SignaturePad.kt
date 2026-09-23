package hu.autotherm.autocrm.ui.inspection

import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.drag
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel900
import hu.autotherm.autocrm.ui.theme.Surface
import java.io.File
import java.io.FileOutputStream

/**
 * Finger-drawn signature. Strokes render to a PNG file on demand; an empty pad
 * refuses to save, so a signature row always carries a real drawing.
 */
class SignatureState {
    private val _strokes = mutableListOf<MutableList<Offset>>()
    val strokes: List<List<Offset>> get() = _strokes

    var revision by mutableStateOf(0)
        private set

    val hasContent: Boolean get() = _strokes.any { it.size > 1 }

    fun startStroke(at: Offset) {
        _strokes.add(mutableListOf(at))
        revision++
    }

    fun extendStroke(to: Offset) {
        _strokes.lastOrNull()?.add(to)
        revision++
    }

    fun clear() {
        _strokes.clear()
        revision++
    }

    /** Renders the drawing to a PNG file. Null when the pad is empty. */
    fun savePng(file: File, widthPx: Int = 900, heightPx: Int = 300): File? {
        if (!hasContent) return null
        val bitmap = Bitmap.createBitmap(widthPx, heightPx, Bitmap.Config.ARGB_8888)
        val canvas = android.graphics.Canvas(bitmap)
        canvas.drawColor(android.graphics.Color.WHITE)
        val paint = android.graphics.Paint().apply {
            color = android.graphics.Color.BLACK
            strokeWidth = 6f
            style = android.graphics.Paint.Style.STROKE
            strokeCap = android.graphics.Paint.Cap.ROUND
            strokeJoin = android.graphics.Paint.Join.ROUND
            isAntiAlias = true
        }
        // Scale the compose-space strokes into the bitmap.
        val all = _strokes.flatten()
        val minX = all.minOf { it.x }
        val minY = all.minOf { it.y }
        val maxX = all.maxOf { it.x }
        val maxY = all.maxOf { it.y }
        val spanX = (maxX - minX).takeIf { it > 1f } ?: 1f
        val spanY = (maxY - minY).takeIf { it > 1f } ?: 1f
        val scale = minOf((widthPx - 40) / spanX, (heightPx - 40) / spanY)
        val dx = 20 - minX * scale
        val dy = 20 - minY * scale
        for (stroke in _strokes) {
            if (stroke.size < 2) continue
            val path = android.graphics.Path()
            path.moveTo(stroke[0].x * scale + dx, stroke[0].y * scale + dy)
            for (p in stroke.drop(1)) path.lineTo(p.x * scale + dx, p.y * scale + dy)
            canvas.drawPath(path, paint)
        }
        FileOutputStream(file).use { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
        }
        bitmap.recycle()
        return file.takeIf { it.length() > 0 }
    }
}

@Composable
fun rememberSignatureState(): SignatureState = remember { SignatureState() }

@Composable
fun SignaturePad(
    state: SignatureState,
    modifier: Modifier = Modifier,
) {
    // Read revision so every stroke recomposes the canvas.
    state.revision
    val paths = remember(state.revision) {
        state.strokes.map { points ->
            Path().apply {
                if (points.isNotEmpty()) {
                    moveTo(points[0].x, points[0].y)
                    for (p in points.drop(1)) lineTo(p.x, p.y)
                }
            }
        }
    }
    Canvas(
        modifier
            .fillMaxWidth()
            .height(180.dp)
            .background(Surface)
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    state.startStroke(down.position)
                    drag(down.id) { change ->
                        state.extendStroke(change.position)
                        change.consume()
                    }
                }
            },
    ) {
        // Lined writing area, like the paper slip it replaces.
        val lineY = size.height - 24.dp.toPx()
        drawLine(
            color = Steel200,
            start = Offset(16.dp.toPx(), lineY),
            end = Offset(size.width - 16.dp.toPx(), lineY),
            strokeWidth = 2.dp.toPx(),
        )
        for (path in paths) {
            drawPath(
                path = path,
                color = Steel900,
                style = Stroke(width = 6f, cap = StrokeCap.Round, join = StrokeJoin.Round),
            )
        }
    }
}
