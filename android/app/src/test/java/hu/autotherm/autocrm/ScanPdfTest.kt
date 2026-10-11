package hu.autotherm.autocrm

import hu.autotherm.autocrm.ui.incoming.ScanPdf
import java.io.File
import kotlin.io.path.createTempDirectory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The scanner's document end, without a device: page geometry, JPEG parsing and the
 * PDF writer are pure Kotlin over bytes. If the binding breaks, scans silently
 * produce unreadable files — these are the tests that would catch it.
 */
class ScanPdfTest {

    @Test
    fun `a portrait photo fills the a4 width`() {
        // 3:4 portrait → width-bound.
        val rect = ScanPdf.fitRect(1200, 1600)
        assertEquals(0f, rect.left, 0.01f)
        assertEquals(ScanPdf.PAGE_W_PT.toFloat(), rect.right, 0.01f)
        assertEquals(ScanPdf.PAGE_W_PT * 1600f / 1200f, rect.height(), 0.5f)
        // Centered vertically.
        assertEquals((ScanPdf.PAGE_H_PT - rect.height()) / 2f, rect.top, 0.5f)
    }

    @Test
    fun `a landscape photo fills the a4 width, centered vertically`() {
        // 4:3 is wider than the A4 ratio, so it binds on the width.
        val rect = ScanPdf.fitRect(1600, 1200)
        assertEquals(0f, rect.left, 0.01f)
        assertEquals(ScanPdf.PAGE_W_PT.toFloat(), rect.right, 0.01f)
        assertEquals(446.25f, rect.height(), 0.5f)
        assertEquals((ScanPdf.PAGE_H_PT - rect.height()) / 2f, rect.top, 0.5f)
    }

    @Test
    fun `scan file names are timestamped pdfs`() {
        val name = ScanPdf.scanFileName()
        assertTrue("got $name", name.startsWith("szallitoi-szamla-") && name.endsWith(".pdf"))
    }

    // ── Minimal JPEG fixtures, built byte by byte ─────────────────────────────

    /** SOI + optional segments + SOF0(W x H, [components]) + EOI. */
    private fun jpeg(width: Int, height: Int, components: Int = 3, extra: ByteArray = byteArrayOf()): ByteArray {
        val out = mutableListOf<Byte>(0xFF.toByte(), 0xD8.toByte())
        out += extra.toList()
        val compBytes = (0 until components).flatMap { listOf(1.toByte(), 0x22.toByte(), 0.toByte()) }
        val body = mutableListOf<Byte>(
            8.toByte(),
            (height shr 8).toByte(), height.toByte(),
            (width shr 8).toByte(), width.toByte(),
            components.toByte(),
        ) + compBytes
        val length = body.size + 2
        out += listOf(0xFF.toByte(), 0xC0.toByte(), (length shr 8).toByte(), length.toByte())
        out += body
        out += listOf(0xFF.toByte(), 0xD9.toByte())
        return out.toByteArray()
    }

    /** An APP1 EXIF segment declaring [orientation], little-endian TIFF. */
    private fun exifSegment(orientation: Int): ByteArray {
        val tiff = mutableListOf<Byte>() +
            "Exif\u0000\u0000II".toByteArray(Charsets.US_ASCII).toList() +
            listOf(42.toByte(), 0.toByte()) + // magic
            listOf(8.toByte(), 0.toByte(), 0.toByte(), 0.toByte()) + // IFD offset
            listOf(1.toByte(), 0.toByte()) + // one entry
            listOf(0x12.toByte(), 0x01.toByte()) + // tag 0x0112
            listOf(3.toByte(), 0.toByte()) + // SHORT
            listOf(1.toByte(), 0.toByte(), 0.toByte(), 0.toByte()) + // count 1
            listOf(orientation.toByte(), 0.toByte(), 0.toByte(), 0.toByte()) + // value
            listOf(0.toByte(), 0.toByte(), 0.toByte(), 0.toByte()) // next IFD
        val length = tiff.size + 2
        return byteArrayOf(0xFF.toByte(), 0xE1.toByte(), (length shr 8).toByte(), length.toByte()) +
            tiff.toByteArray()
    }

    @Test
    fun `dimensions and components come from the sof marker`() {
        val info = ScanPdf.readJpegInfo(jpeg(30, 10))
        assertEquals(30, info.width)
        assertEquals(10, info.height)
        assertEquals(3, info.components)
        assertEquals(1, info.orientation)
        assertEquals(30 to 10, info.effectiveSize())
    }

    @Test
    fun `a rotated photo reports its displayed size`() {
        val bytes = jpeg(30, 10, extra = exifSegment(6))
        val info = ScanPdf.readJpegInfo(bytes)
        assertEquals(6, info.orientation)
        assertEquals(10 to 30, info.effectiveSize())
    }

    @Test
    fun `garbage is refused, not an empty pdf`() {
        var failed = 0
        try {
            ScanPdf.readJpegInfo("nem jpeg".toByteArray())
        } catch (e: IllegalArgumentException) {
            failed++
        }
        try {
            ScanPdf.buildPdf(emptyList(), File("sehova.pdf"))
        } catch (e: IllegalArgumentException) {
            failed++
        }
        assertEquals(2, failed)
    }

    @Test
    fun `two pages bind into one pdf with a sound cross reference table`() {
        val gray = jpeg(10, 20, components = 1)
        val rotated = jpeg(30, 10, extra = exifSegment(6))
        val dir = createTempDirectory("scan").toFile()
        try {
            val pdf = File(dir, "szamla.pdf")
            ScanPdf.buildPdf(listOf(gray, rotated), pdf)
            assertTrue(pdf.exists() && pdf.length() > 0)
            val raw = pdf.readBytes()
            val text = raw.toString(Charsets.US_ASCII)
            assertTrue(text.startsWith("%PDF-"))
            assertTrue(text.contains("/Count 2"))
            assertTrue(text.contains("/DeviceGray"))
            assertTrue(text.contains("/DeviceRGB"))
            // Each page embeds exactly the bytes it was given.
            assertTrue(text.contains("/Length ${gray.size}"))
            assertTrue(text.contains("/Length ${rotated.size}"))
            // The rotated page draws through a rotation matrix, not axis-aligned.
            assertTrue(text.contains("/Im1 Do"))
            val matrix = text.substringBefore(" /Im1 Do").substringAfterLast("stream\n")
            assertTrue("got $matrix", matrix.contains("-9.36") && matrix.contains("84.20"))
            // startxref points at the xref table it names.
            val xrefAt = text.substringAfterLast("startxref\n").substringBefore("\n").trim().toInt()
            assertEquals("xref", text.substring(xrefAt, xrefAt + 4))
            assertTrue(text.trimEnd().endsWith("%%EOF"))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test
    fun `file backed binding writes the same bytes as the memory path`() {
        val dir = createTempDirectory("scan").toFile()
        try {
            val first = File(dir, "a.jpg").apply { writeBytes(jpeg(10, 20, components = 1)) }
            val second = File(dir, "b.jpg").apply { writeBytes(jpeg(30, 10)) }
            val pdf = File(dir, "fuzet.pdf")
            ScanPdf.buildPdfFromImages(listOf(first, second), pdf)
            assertTrue(pdf.readBytes().toString(Charsets.US_ASCII).startsWith("%PDF-"))
            assertTrue(pdf.readBytes().toString(Charsets.US_ASCII).contains("/Count 2"))
        } finally {
            dir.deleteRecursively()
        }
    }
}
