package hu.autotherm.autocrm.ui.inspection

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.ui.theme.Signal
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel900

/**
 * Tap-to-mark outline, top view. Coordinates are normalised 0..1 so they survive
 * any screen size; the server stores them as-is and the web draws the same dots.
 * One view only (`top`): interior and close-up positions are described by zone +
 * note instead, which is what the inspector reaches for first anyway.
 */
@Composable
fun CarOutline(
    marks: List<Pair<Float, Float>>,
    onTap: ((Float, Float) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    Canvas(
        modifier
            .fillMaxWidth()
            .height(220.dp)
            .pointerInput(onTap) {
                if (onTap != null) {
                    detectTapGestures { offset ->
                        onTap(
                            (offset.x / size.width).coerceIn(0f, 1f),
                            (offset.y / size.height).coerceIn(0f, 1f),
                        )
                    }
                }
            },
    ) {
        val w = size.width
        val h = size.height
        val bodyW = w * 0.52f
        val bodyH = h * 0.86f
        val left = (w - bodyW) / 2
        val top = (h - bodyH) / 2

        // Body.
        drawRoundRect(
            color = Steel900,
            topLeft = Offset(left, top),
            size = Size(bodyW, bodyH),
            cornerRadius = CornerRadius(bodyW * 0.28f, bodyW * 0.28f),
            style = Stroke(width = 5f),
        )
        // Windscreen + rear window.
        val glassW = bodyW * 0.72f
        val glassLeft = (w - glassW) / 2
        drawRoundRect(
            color = Steel200,
            topLeft = Offset(glassLeft, top + bodyH * 0.18f),
            size = Size(glassW, bodyH * 0.10f),
            cornerRadius = CornerRadius(8f, 8f),
            style = Stroke(width = 4f),
        )
        drawRoundRect(
            color = Steel200,
            topLeft = Offset(glassLeft, top + bodyH * 0.72f),
            size = Size(glassW, bodyH * 0.08f),
            cornerRadius = CornerRadius(8f, 8f),
            style = Stroke(width = 4f),
        )
        // Wheels.
        val wheelR = bodyW * 0.10f
        val wheelOffsets = listOf(
            Offset(left - wheelR * 0.5f, top + bodyH * 0.22f),
            Offset(left + bodyW - wheelR * 0.5f, top + bodyH * 0.22f),
            Offset(left - wheelR * 0.5f, top + bodyH * 0.70f),
            Offset(left + bodyW - wheelR * 0.5f, top + bodyH * 0.70f),
        )
        for (o in wheelOffsets) {
            drawCircle(Color.White, radius = wheelR, center = o)
            drawCircle(Steel900, radius = wheelR, center = o, style = Stroke(width = 4f))
        }
        // Front marker.
        drawCircle(Steel200, radius = 8f, center = Offset(w / 2, top - 22f))

        // Damage marks.
        for ((x, y) in marks) {
            val c = Offset(x * w, y * h)
            drawCircle(Signal, radius = 16f, center = c)
            drawCircle(Color.White, radius = 16f, center = c, style = Stroke(width = 4f))
        }
    }
}
