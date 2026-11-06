package hu.autotherm.autocrm.ui.photos

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.filled.CloudUpload
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import coil.compose.AsyncImage
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.api.ImageView
import hu.autotherm.autocrm.data.api.Lookups
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.ui.theme.Steel200

/**
 * The order's photos as a row of thumbnails, newest first; a tap opens them full screen
 * to swipe through. Before this the phone only showed how many photos there were — the
 * fitter could take them but never look at them.
 *
 * [revision] changes when the counts do, so a finished upload appears without a reload.
 */
@Composable
fun PhotoStrip(orderId: Long, revision: Any?, lookups: Lookups?, pendingFiles: List<String> = emptyList()) {
    val api = (LocalContext.current.applicationContext as AutoCrmApp).api
    val images by produceState(initialValue = emptyList<ImageView>(), orderId, revision) {
        value = runCatching { api.images(orderId) }.getOrDefault(value)
            .filter { it.thumbUrl != null || it.displayUrl != null }
            .sortedByDescending { it.capturedAt ?: it.uploadedAt }
    }
    var viewing by remember { mutableStateOf<Int?>(null) }
    if (images.isEmpty() && pendingFiles.isEmpty()) return

    LazyRow(
        Modifier.fillMaxWidth().padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // Just taken, still on the phone: shown straight away, marked as on its way.
        items(pendingFiles.reversed(), key = { "pending-$it" }) { path ->
            Box(Modifier.size(76.dp)) {
                AsyncImage(
                    model = java.io.File(path),
                    contentDescription = "Feltöltésre vár",
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)).background(Steel200),
                )
                Icon(
                    androidx.compose.material.icons.Icons.Filled.CloudUpload,
                    contentDescription = null,
                    tint = Color.White,
                    modifier = Modifier.align(Alignment.BottomEnd).padding(4.dp)
                        .background(Color.Black.copy(alpha = 0.5f), RoundedCornerShape(6.dp))
                        .padding(2.dp).size(16.dp),
                )
            }
        }
        itemsIndexed(images, key = { _, img -> img.id }) { index, img ->
            AsyncImage(
                model = img.thumbUrl ?: img.displayUrl,
                contentDescription = CapturePrefs.label(img.category, lookups),
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .size(76.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(Steel200)
                    .clickable { viewing = index },
            )
        }
    }

    viewing?.let { start ->
        PhotoViewer(images = images, start = start, lookups = lookups, onClose = { viewing = null })
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun PhotoViewer(images: List<ImageView>, start: Int, lookups: Lookups?, onClose: () -> Unit) {
    Dialog(
        onDismissRequest = onClose,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        val pager = rememberPagerState(initialPage = start.coerceIn(0, (images.size - 1).coerceAtLeast(0))) { images.size }
        // Double tap zooms in on a scratch or a serial plate; while zoomed, one finger pans
        // and pinching adjusts, and swiping to the next photo waits until zoomed back out.
        var zoomedPage by remember { mutableStateOf<Int?>(null) }
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            HorizontalPager(
                state = pager,
                modifier = Modifier.fillMaxSize(),
                userScrollEnabled = zoomedPage == null,
            ) { page ->
                // The list the strip was opened with can shrink underneath the viewer (a
                // pending upload finishing or an image filtered out); never index into it.
                val img = images.getOrNull(page) ?: return@HorizontalPager
                var scale by remember { mutableStateOf(1f) }
                var offset by remember { mutableStateOf(androidx.compose.ui.geometry.Offset.Zero) }
                androidx.compose.runtime.LaunchedEffect(pager.currentPage) {
                    if (pager.currentPage != page) {
                        scale = 1f
                        offset = androidx.compose.ui.geometry.Offset.Zero
                    }
                }
                AsyncImage(
                    // The display rendition: sharp on a phone, a fraction of the original.
                    model = img.displayUrl ?: img.thumbUrl,
                    contentDescription = CapturePrefs.label(img.category, lookups),
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .fillMaxSize()
                        .pointerInput(page) {
                            detectTapGestures(onDoubleTap = { tap ->
                                if (scale > 1f) {
                                    scale = 1f
                                    offset = androidx.compose.ui.geometry.Offset.Zero
                                    zoomedPage = null
                                } else {
                                    scale = 2.5f
                                    // Zoom towards the tapped point rather than the centre.
                                    val lim = panLimit(scale, size.width.toFloat(), size.height.toFloat())
                                    val wanted = (androidx.compose.ui.geometry.Offset(size.width / 2f, size.height / 2f) - tap) * 1.5f
                                    offset = androidx.compose.ui.geometry.Offset(
                                        wanted.x.coerceIn(-lim.x, lim.x),
                                        wanted.y.coerceIn(-lim.y, lim.y),
                                    )
                                    zoomedPage = page
                                }
                            })
                        }
                        // Two fingers pinch from the start (it used to take a double tap
                        // first); one finger pans only while zoomed, and otherwise is left
                        // to the pager so swiping to the next photo still works.
                        .pointerInput(page, "zoom") {
                            awaitEachGesture {
                                awaitFirstDown(requireUnconsumed = false)
                                do {
                                    val event = awaitPointerEvent()
                                    val fingers = event.changes.count { it.pressed }
                                    if (fingers >= 2 || scale > 1f) {
                                        val old = scale
                                        scale = (scale * event.calculateZoom()).coerceIn(1f, 5f)
                                        offset = if (scale <= 1f) {
                                            androidx.compose.ui.geometry.Offset.Zero
                                        } else {
                                            // Zoom about the fingers, not the centre: the spot
                                            // being pinched stays under them.
                                            val centre = androidx.compose.ui.geometry.Offset(size.width / 2f, size.height / 2f)
                                            val focus = event.calculateCentroid(useCurrent = false)
                                                .takeIf { it != androidx.compose.ui.geometry.Offset.Unspecified }
                                                ?.minus(centre) ?: androidx.compose.ui.geometry.Offset.Zero
                                            val ratio = scale / old
                                            val moved = offset * ratio + focus * (1f - ratio) + event.calculatePan()
                                            // A pan that is never clamped can drag the photo
                                            // clean off screen with nothing to grab onto.
                                            val lim = panLimit(scale, size.width.toFloat(), size.height.toFloat())
                                            androidx.compose.ui.geometry.Offset(
                                                moved.x.coerceIn(-lim.x, lim.x),
                                                moved.y.coerceIn(-lim.y, lim.y),
                                            )
                                        }
                                        zoomedPage = if (scale > 1f) page else null
                                        event.changes.forEach { if (it.positionChanged()) it.consume() }
                                    }
                                } while (event.changes.any { it.pressed })
                            }
                        }
                        .graphicsLayer {
                            scaleX = scale
                            scaleY = scale
                            translationX = offset.x
                            translationY = offset.y
                        },
                )
            }
            val current = images.getOrNull(pager.currentPage)
            Column(
                Modifier.align(Alignment.BottomStart).fillMaxWidth()
                    .background(Color.Black.copy(alpha = 0.55f))
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    "${pager.currentPage + 1} / ${images.size}" +
                        (current?.let { " · " + CapturePrefs.label(it.category, lookups) } ?: ""),
                    style = MaterialTheme.typography.titleSmall,
                    color = Color.White,
                )
                current?.let { img ->
                    hu.autotherm.autocrm.util.formatDateTime(img.capturedAt ?: img.uploadedAt)?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = Color.White.copy(alpha = 0.8f))
                    }
                }
            }
            IconButton(
                onClick = onClose,
                modifier = Modifier.align(Alignment.TopEnd).padding(8.dp)
                    .background(Color.Black.copy(alpha = 0.45f), RoundedCornerShape(50)),
            ) {
                Icon(Icons.Filled.Close, contentDescription = "Bezárás", tint = Color.White)
            }
        }
    }
}

/** How far the scaled photo may travel per axis before its edge reaches the viewport edge. */
private fun panLimit(scale: Float, width: Float, height: Float): androidx.compose.ui.geometry.Offset =
    androidx.compose.ui.geometry.Offset(width / 2f * (scale - 1f), height / 2f * (scale - 1f))
