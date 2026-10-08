package hu.autotherm.autocrm.util

import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation

/**
 * Shows "1250000,5" as "1 250 000,5" while it is typed, so a missing or extra zero is seen
 * before it is saved. Display only: the field's value keeps no spaces (callers drop them on
 * input), and [parseMajorToMinor] reads either form. The cursor maps across the inserted
 * gaps, so typing in the middle of a number lands where the finger put it.
 */
object GroupedNumberTransformation : VisualTransformation {
    /** Original offsets before which a gap is shown. */
    internal fun gaps(raw: String): List<Int> {
        val start = if (raw.startsWith("-") || raw.startsWith("−")) 1 else 0
        val end = raw.indexOfFirst { it == ',' || it == '.' }.let { if (it < 0) raw.length else it }
        if (end <= start) return emptyList()
        val whole = raw.substring(start, end)
        if (whole.length <= 3 || !whole.all { it.isDigit() }) return emptyList()
        return (1 until whole.length).filter { (whole.length - it) % 3 == 0 }.map { start + it }
    }

    internal fun format(raw: String): String {
        val gaps = gaps(raw)
        if (gaps.isEmpty()) return raw
        val out = StringBuilder(raw.length + gaps.size)
        raw.forEachIndexed { i, c ->
            if (i in gaps) out.append(' ')
            out.append(c)
        }
        return out.toString()
    }

    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val gaps = gaps(raw)
        if (gaps.isEmpty()) return TransformedText(text, OffsetMapping.Identity)
        val mapping = object : OffsetMapping {
            override fun originalToTransformed(offset: Int): Int = offset + gaps.count { it < offset }
            override fun transformedToOriginal(offset: Int): Int =
                (0..raw.length).last { it + gaps.count { g -> g < it } <= offset }
        }
        return TransformedText(AnnotatedString(format(raw)), mapping)
    }
}

/**
 * A Hungarian tax number typed as eleven digits, shown as "12345678-1-23": the dashes
 * appear by themselves, so nobody hunts for the minus key on a number pad. The value
 * stays digits only; the server reads either form.
 */
object HuTaxNumberTransformation : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        if (!raw.all { it.isDigit() }) return TransformedText(text, OffsetMapping.Identity)
        val out = buildString {
            raw.forEachIndexed { i, c ->
                if (i == 8 || i == 9) append('-')
                append(c)
            }
        }
        val mapping = object : OffsetMapping {
            override fun originalToTransformed(offset: Int): Int = offset + (if (offset > 8) 1 else 0) + (if (offset > 9) 1 else 0)
            override fun transformedToOriginal(offset: Int): Int =
                (offset - (if (offset > 8) 1 else 0) - (if (offset > 10) 1 else 0)).coerceIn(0, raw.length)
        }
        return TransformedText(AnnotatedString(out), mapping)
    }
}
