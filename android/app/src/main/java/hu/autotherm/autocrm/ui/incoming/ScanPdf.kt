package hu.autotherm.autocrm.ui.incoming

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.min

/**
 * The supplier-invoice scanner's document end, written against the file formats
 * themselves rather than any imaging library: the system camera takes the pages, this
 * object binds them into one PDF, and the API's `POST /incoming-invoices/upload`
 * takes the raw bytes (it sniffs `%PDF` itself).
 *
 * Deliberately no document-detection model, no OpenCV, no PDF library: every one of
 * those is a native dependency that bloats the APK and breaks the single-ABI sideload
 * build. A supplier invoice on a desk does not need perspective correction to be
 * readable — it needs to be quick, offline, and to survive a dead signal.
 *
 * Everything here is pure Kotlin over bytes except [downscaleForPdf] (which needs the
 * framework to decode pixels). The JPEG parsing, the rotation math and the PDF writer
 * run on the plain JVM, which is why `ScanPdfTest` covers them without Robolectric.
 *
 * Flow: photograph each page (portrait, invoice filling the frame) → review the page
 * strip (retake, drop) → one PDF → upload now, or keep the PDF under `scans/` for a
 * later retry. Nothing here needs the network.
 */
object ScanPdf {

    /** Long edge a page image is downscaled to before embedding. A4 at ~170 dpi. */
    const val MAX_EDGE_PX = 2000

    /** JPEG quality of the embedded pages: readable small print, small file. */
    const val JPEG_QUALITY = 85

    /** A4 in points, the PDF's page size. */
    const val PAGE_W_PT = 595
    const val PAGE_H_PT = 842

    /**
     * What the upload calls the file. Timestamped so two scans never collide, prefixed
     * so the office list reads sensibly.
     */
    fun scanFileName(now: Date = Date()): String {
        val stamp = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.ROOT).format(now)
        return "szallitoi-szamla-$stamp.pdf"
    }

    /**
     * Which rectangle of an A4 page ([PAGE_W_PT] x [PAGE_H_PT]) an [imageW] x [imageH]
     * photo fills, preserving aspect ratio and centered, in top-left-origin points.
     * A plain data class rather than `android.graphics.RectF`: this geometry runs on
     * the JVM in tests, where framework graphics are stubs that throw.
     */
    data class PageRect(val left: Float, val top: Float, val right: Float, val bottom: Float) {
        fun width(): Float = right - left
        fun height(): Float = bottom - top
    }

    fun fitRect(imageW: Int, imageH: Int): PageRect {
        require(imageW > 0 && imageH > 0) { "empty image" }
        val scale = min(PAGE_W_PT.toFloat() / imageW, PAGE_H_PT.toFloat() / imageH)
        val w = imageW * scale
        val h = imageH * scale
        val left = (PAGE_W_PT - w) / 2f
        val top = (PAGE_H_PT - h) / 2f
        return PageRect(left, top, left + w, top + h)
    }

    /** A JPEG's stored size, component count and EXIF orientation, parsed from bytes. */
    data class JpegInfo(
        /** Stored pixel width (before EXIF rotation is applied). */
        val width: Int,
        /** Stored pixel height. */
        val height: Int,
        /** 1 for greyscale, 3 for color. */
        val components: Int,
        /** EXIF orientation 1..8; 1 when absent or unreadable. */
        val orientation: Int,
    ) {
        /** Displayed size once the orientation is honored (5..8 swap the axes). */
        fun effectiveSize(): Pair<Int, Int> =
            if (orientation in 5..8) height to width else width to height
    }

    /**
     * Reads dimensions from the SOF marker and orientation from the APP1 EXIF block.
     * Throws [IllegalArgumentException] when the bytes are not a JPEG with a size.
     */
    fun readJpegInfo(bytes: ByteArray): JpegInfo {
        if (bytes.size < 4 || bytes[0] != 0xFF.toByte() || bytes[1] != 0xD8.toByte()) {
            throw IllegalArgumentException("not a JPEG")
        }
        var orientation = 1
        var pos = 2
        while (pos + 4 <= bytes.size) {
            if (bytes[pos] != 0xFF.toByte()) {
                pos++
                continue
            }
            val marker = bytes[pos + 1].toInt() and 0xFF
            // Standalone markers carry no length.
            if (marker == 0xD8 || marker == 0xD9 || marker == 0x01 || marker in 0xD0..0xD7) {
                pos += 2
                continue
            }
            if (pos + 4 > bytes.size) break
            val length = ((bytes[pos + 2].toInt() and 0xFF) shl 8) or (bytes[pos + 3].toInt() and 0xFF)
            if (length < 2 || pos + length > bytes.size) break
            val body = pos + 2
            when {
                // The APP1 content starts after the marker and its length field.
                marker == 0xE1 -> readExifOrientation(bytes, pos + 4, pos + 2 + length)?.let { orientation = it }
                marker in SOF_MARKERS && length >= 8 -> {
                    val height = ((bytes[body + 3].toInt() and 0xFF) shl 8) or (bytes[body + 4].toInt() and 0xFF)
                    val width = ((bytes[body + 5].toInt() and 0xFF) shl 8) or (bytes[body + 6].toInt() and 0xFF)
                    val components = bytes[body + 7].toInt() and 0xFF
                    if (width <= 0 || height <= 0 || components !in 1..4) {
                        throw IllegalArgumentException("bogus JPEG size")
                    }
                    return JpegInfo(width, height, components, orientation)
                }
            }
            pos += length
        }
        throw IllegalArgumentException("no JPEG size found")
    }

    private val SOF_MARKERS = setOf(
        0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
    )

    /**
     * The EXIF orientation from an APP1 segment body, or null. Bounds-checked throughout:
     * a corrupt camera header must not crash a scan.
     */
    internal fun readExifOrientation(bytes: ByteArray, from: Int, to: Int): Int? {
        // "Exif\0\0" then the TIFF header.
        if (to - from < 14) return null
        if (!(bytes[from] == 'E'.code.toByte() && bytes[from + 1] == 'x'.code.toByte() &&
                bytes[from + 2] == 'i'.code.toByte() && bytes[from + 3] == 'f'.code.toByte() &&
                bytes[from + 4] == 0.toByte() && bytes[from + 5] == 0.toByte())
        ) {
            return null
        }
        val tiff = from + 6
        if (to - tiff < 8) return null
        val little = bytes[tiff] == 'I'.code.toByte() && bytes[tiff + 1] == 'I'.code.toByte()
        val big = bytes[tiff] == 'M'.code.toByte() && bytes[tiff + 1] == 'M'.code.toByte()
        if (!little && !big) return null
        fun u16(at: Int): Int {
            if (at + 2 > to) return -1
            val b0 = bytes[at].toInt() and 0xFF
            val b1 = bytes[at + 1].toInt() and 0xFF
            return if (little) b0 or (b1 shl 8) else (b0 shl 8) or b1
        }
        fun u32(at: Int): Long {
            if (at + 4 > to) return -1
            val b = (0..3).map { bytes[at + it].toInt() and 0xFF }
            return if (little) {
                b[0].toLong() or (b[1].toLong() shl 8) or (b[2].toLong() shl 16) or (b[3].toLong() shl 24)
            } else {
                (b[0].toLong() shl 24) or (b[1].toLong() shl 16) or (b[2].toLong() shl 8) or b[3].toLong()
            }
        }
        if (u16(tiff + 2) != 42) return null
        val ifd = tiff + u32(tiff + 4).toInt()
        if (ifd < tiff || ifd + 2 > to) return null
        val count = u16(ifd)
        if (count < 0 || count > 64) return null
        for (i in 0 until count) {
            val entry = ifd + 2 + i * 12
            if (entry + 12 > to) break
            if (u16(entry) != 0x0112) continue
            if (u16(entry + 2) != 3) return null // SHORT expected
            if (u32(entry + 4) != 1L) return null // count 1 expected
            val value = u16(entry + 8)
            return if (value in 1..8) value else null
        }
        return null
    }

    /**
     * The content-stream matrix drawing a stored [info] photo into the [fitRect] of its
     * effective size. Mirrored orientations (2, 4, 5, 7) keep their rotation and drop
     * the flip — phone cameras do not produce them, and a flipped invoice reads fine.
     * Coordinates are PDF points, origin bottom-left.
     */
    internal fun drawMatrix(info: JpegInfo, rect: PageRect): String {
        val w = rect.width()
        val h = rect.height()
        val x = rect.left
        // RectF counts from the top; PDF counts from the bottom.
        val y = PAGE_H_PT - rect.bottom
        fun num(v: Float): String = "%.2f".format(Locale.US, v)
        // PDF image space is the unit square (spec 8.9.7.2): the coefficients carry the
        // rect's w and h as-is. The old division by stored sizes squeezed a 90°-rotated
        // scan far outside the A4 page; a rotation only swaps which axis gets which.
        return when (info.orientation) {
            3 -> "q ${num(-w)} 0 0 ${num(-h)} ${num(x + w)} ${num(y + h)} cm"
            // 6 = stored row 0 is the visual right side, column 0 the visual top.
            6 -> "q 0 ${num(-h)} ${num(w)} 0 ${num(x)} ${num(y + h)} cm"
            // 8 = stored row 0 is the visual left side, column 0 the visual bottom.
            8 -> "q 0 ${num(h)} ${num(-w)} 0 ${num(x + w)} ${num(y)} cm"
            else -> "q ${num(w)} 0 0 ${num(h)} ${num(x)} ${num(y)} cm"
        }
    }

    /**
     * Binds one JPEG per page into [out]. The bytes are embedded as-is (DCTDecode), so
     * nothing is re-encoded and nothing is lost: what the camera wrote is what the
     * office opens.
     */
    fun buildPdf(pages: List<ByteArray>, out: File) {
        require(pages.isNotEmpty()) { "nincs oldal a PDF-hez" }
        val infos = pages.map(::readJpegInfo)
        val buf = ByteArrayOutputStream()
        fun ascii(s: String) = buf.write(s.toByteArray(Charsets.US_ASCII))
        val offsets = mutableListOf<Int>()
        fun obj(number: Int, body: ByteArray) {
            offsets.add(buf.size())
            ascii("$number 0 obj\n")
            buf.write(body)
            ascii("\nendobj\n")
        }
        fun dict(s: String) = s.toByteArray(Charsets.US_ASCII)

        val pageCount = pages.size
        // Object numbering: 1 catalog, 2 pages, then per page: page, image, content.
        ascii("%PDF-1.4\n")
        val kidRefs = (0 until pageCount).map { i -> "${3 + i * 3} 0 R" }
        obj(1, dict("<< /Type /Catalog /Pages 2 0 R >>"))
        obj(2, dict("<< /Type /Pages /Kids [${kidRefs.joinToString(" ")}] /Count $pageCount >>"))
        for (i in pages.indices) {
            val info = infos[i]
            val (ew, eh) = info.effectiveSize()
            val rect = fitRect(ew, eh)
            val pageObj = 3 + i * 3
            val imageObj = pageObj + 1
            val contentObj = pageObj + 2
            val colorSpace = if (info.components == 1) "/DeviceGray" else "/DeviceRGB"
            obj(
                pageObj,
                dict(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 $PAGE_W_PT $PAGE_H_PT] " +
                        "/Resources << /XObject << /Im$i $imageObj 0 R >> >> " +
                        "/Contents $contentObj 0 R >>",
                ),
            )
            val imageHeader = (
                "<< /Type /XObject /Subtype /Image /Width ${info.width} /Height ${info.height} " +
                    "/ColorSpace $colorSpace /BitsPerComponent 8 /Filter /DCTDecode " +
                    "/Length ${pages[i].size} >>\nstream\n"
                ).toByteArray(Charsets.US_ASCII)
            offsets.add(buf.size())
            ascii("$imageObj 0 obj\n")
            buf.write(imageHeader)
            buf.write(pages[i])
            ascii("\nendstream\nendobj\n")
            val content = "${drawMatrix(info, rect)} /Im$i Do Q"
            val contentBytes = content.toByteArray(Charsets.US_ASCII)
            val streamHeader = "<< /Length ${contentBytes.size} >>\nstream\n".toByteArray(Charsets.US_ASCII)
            offsets.add(buf.size())
            ascii("$contentObj 0 obj\n")
            buf.write(streamHeader)
            buf.write(contentBytes)
            ascii("\nendstream\nendobj\n")
        }
        val xrefAt = buf.size()
        ascii("xref\n0 ${3 + pageCount * 3}\n")
        ascii("0000000000 65535 f \n")
        for (offset in offsets) {
            ascii("%010d 00000 n \n".format(Locale.US, offset))
        }
        ascii("trailer\n<< /Size ${3 + pageCount * 3} /Root 1 0 R >>\n")
        ascii("startxref\n$xrefAt\n%%EOF")
        FileOutputStream(out).use { buf.writeTo(it) }
    }

    /** File-backed [buildPdf]: each entry becomes one page, in order. */
    fun buildPdfFromImages(images: List<File>, out: File) {
        require(images.isNotEmpty()) { "nincs oldal a PDF-hez" }
        buildPdf(images.map { it.readBytes() }, out)
    }

    /**
     * Downscales [src] (a full-resolution camera JPEG) into [dst]. Needs the framework
     * to decode pixels; the EXIF rotation is applied here, because the re-encoded JPEG
     * carries no EXIF and [buildPdf] embeds the bytes as-is — without this a portrait
     * scan lies on its side in the PDF. Returns [dst].
     */
    fun downscaleForPdf(src: File, dst: File): File {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(src.absolutePath, bounds)
        val longEdge = maxOf(bounds.outWidth, bounds.outHeight).coerceAtLeast(1)
        var sample = 1
        while (longEdge / sample > MAX_EDGE_PX) sample *= 2
        val opts = BitmapFactory.Options().apply { inSampleSize = sample }
        var bitmap = BitmapFactory.decodeFile(src.absolutePath, opts)
            ?: throw IllegalArgumentException("nem olvasható kép: ${src.name}")
        try {
            val orientation = runCatching { readJpegInfo(src.readBytes()).orientation }.getOrDefault(1)
            if (orientation == 3 || orientation == 6 || orientation == 8) {
                val matrix = android.graphics.Matrix().apply {
                    postRotate(
                        when (orientation) {
                            6 -> 90f
                            8 -> -90f
                            else -> 180f
                        },
                    )
                }
                val rotated = Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
                if (rotated != bitmap) {
                    bitmap.recycle()
                    bitmap = rotated
                }
            }
            FileOutputStream(dst).use { out ->
                if (!bitmap.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, out)) {
                    throw IllegalStateException("a JPEG tömörítés nem sikerült")
                }
            }
        } finally {
            if (!bitmap.isRecycled) bitmap.recycle()
        }
        return dst
    }
}
