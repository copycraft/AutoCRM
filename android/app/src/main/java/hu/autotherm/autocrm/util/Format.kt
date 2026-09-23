package hu.autotherm.autocrm.util

import java.text.NumberFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

private val HU = Locale("hu", "HU")
private val BUDAPEST: ZoneId = ZoneId.of("Europe/Budapest")

/**
 * Money as the web client shows it (frontend/src/lib/utils/format.ts): integer minor units
 * in, grouped whole units out, fraction appended as a string. Integer arithmetic only —
 * never divide into floating point (HUF 4 850 000 stored as 485000000 must not wobble).
 * HUF fillér render when nonzero; whole-forint amounts render without decimals.
 */
fun formatMoney(minor: Long, currency: String): String {
    val negative = minor < 0
    val abs = if (negative) -minor else minor
    val frac = (abs % 100).toInt()
    val whole = abs / 100
    val grouped = NumberFormat.getIntegerInstance(HU).format(whole)
    val sign = if (negative) "-" else ""
    val cents = frac.toString().padStart(2, '0')
    // Compared case-insensitively: a lowercase "huf" from anywhere must not fall through
    // to the generic branch and render "1 000,00 huf".
    return when (currency.uppercase()) {
        "HUF" -> if (frac == 0) "$sign$grouped Ft" else "$sign$grouped,$cents Ft"
        "EUR" -> "$sign$grouped,$cents €"
        else -> "$sign$grouped,$cents $currency"
    }
}

/** Queue timestamps (epoch millis) in the business timezone, like formatDateTime. */
fun formatTime(epochMillis: Long): String =
    Instant.ofEpochMilli(epochMillis).atZone(BUDAPEST)
        .format(DateTimeFormatter.ofPattern("MM.dd. HH:mm", HU))

/** `2026-09-12` → `2026. 09. 12.`, the Hungarian date form the web client uses. */
fun formatDate(isoDate: String?): String? = isoDate?.let {
    runCatching { LocalDate.parse(it).format(DateTimeFormatter.ofPattern("yyyy. MM. dd.", HU)) }
        .getOrDefault(it)
}

/** RFC 3339 timestamp → local date and time in the business timezone. */
fun formatDateTime(iso: String?): String? = iso?.let {
    runCatching {
        Instant.parse(it).atZone(BUDAPEST)
            .format(DateTimeFormatter.ofPattern("yyyy. MM. dd. HH:mm", HU))
    }.getOrDefault(it)
}

/** Whole days between an RFC 3339 timestamp and now; negative values clamp to zero. */
fun daysSince(iso: String?): Long = iso?.let {
    runCatching {
        java.time.Duration.between(Instant.parse(it), Instant.now()).toDays().coerceAtLeast(0)
    }.getOrDefault(0L)
} ?: 0L
