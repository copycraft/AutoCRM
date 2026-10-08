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

/**
 * A deadline as a person says it: "ma", "holnap", "3 nap múlva", "2 napja"; past two
 * weeks either way, the date itself. [today] is a parameter for tests.
 */
fun relativeDay(isoDate: String?, today: LocalDate = LocalDate.now(BUDAPEST)): String? {
    val day = isoDate?.let { runCatching { LocalDate.parse(it) }.getOrNull() } ?: return formatDate(isoDate)
    val diff = java.time.temporal.ChronoUnit.DAYS.between(today, day)
    return when {
        diff == 0L -> "ma"
        diff == 1L -> "holnap"
        diff == -1L -> "tegnap"
        diff in 2..14 -> "$diff nap múlva"
        diff in -14..-2 -> "${-diff} napja"
        else -> formatDate(isoDate)
    }
}

/** RFC 3339 timestamp → local date and time in the business timezone. */
fun formatDateTime(iso: String?): String? = iso?.let {
    runCatching {
        Instant.parse(it).atZone(BUDAPEST)
            .format(DateTimeFormatter.ofPattern("yyyy. MM. dd. HH:mm", HU))
    }.getOrDefault(it)
}

/**
 * A recent moment as people say it: "most", "12 perce", "3 órája", "tegnap 14:30"; older
 * than that, the date and time. For feeds (notifications, comments, e-mails) where "how
 * long ago" matters more than the clock. [now] is a parameter for tests.
 */
fun relativeTime(iso: String?, now: Instant = Instant.now()): String? {
    val at = iso?.let { runCatching { Instant.parse(it) }.getOrNull() } ?: return formatDateTime(iso)
    val minutes = java.time.Duration.between(at, now).toMinutes()
    val local = at.atZone(BUDAPEST)
    val today = now.atZone(BUDAPEST).toLocalDate()
    val clock = local.format(DateTimeFormatter.ofPattern("HH:mm", HU))
    return when {
        minutes < 0 -> formatDateTime(iso)
        minutes < 1 -> "most"
        minutes < 60 -> "$minutes perce"
        local.toLocalDate() == today -> "${minutes / 60} órája"
        local.toLocalDate() == today.minusDays(1) -> "tegnap $clock"
        else -> formatDateTime(iso)
    }
}

/** Whole days between an RFC 3339 timestamp and now; negative values clamp to zero. */
fun daysSince(iso: String?): Long = iso?.let {
    runCatching {
        java.time.Duration.between(Instant.parse(it), Instant.now()).toDays().coerceAtLeast(0)
    }.getOrDefault(0L)
} ?: 0L

/**
 * A typed amount ("12 500", "12500,50", "1.234,5") as minor units, or null when it is not
 * a number. Spaces group thousands; a comma or a lone dot is the decimal mark.
 */
fun parseMajorToMinor(input: String): Long? {
    var s = input.replace(" ", "").replace(" ", "").trim()
    if (s.isEmpty()) return null
    // "1.234,5": dots group, the comma is decimal.
    if (s.contains(',') && s.contains('.')) s = s.replace(".", "")
    s = s.replace(',', '.')
    val value = s.toBigDecimalOrNull() ?: return null
    if (value.signum() < 0) return null
    return value.movePointRight(2).setScale(0, java.math.RoundingMode.HALF_UP).toLong()
}

/** Like [parseMajorToMinor], but a leading minus is kept: discount lines are negative. */
fun parseSignedMajorToMinor(input: String): Long? {
    val s = input.trim()
    val negative = s.startsWith('-') || s.startsWith('−')
    val minor = parseMajorToMinor(if (negative) s.drop(1) else s) ?: return null
    return if (negative) -minor else minor
}

/**
 * A typed quantity ("2", "2,5", "0.25") as the decimal string the server takes, or null
 * when the server would refuse it: it must be positive, with at most three decimals.
 */
fun parseQuantity(input: String): String? {
    val s = input.replace(" ", "").replace(',', '.').trim()
    val value = s.toBigDecimalOrNull() ?: return null
    if (value.signum() <= 0 || value.scale() > 3 || value >= java.math.BigDecimal(1_000_000_000)) return null
    return value.stripTrailingZeros().toPlainString()
}

/**
 * A Hungarian phone number grouped the way it is read out: "+36 30 123 4567",
 * "06 1 234 5678". Display only; anything that is not recognisably Hungarian (or has
 * extensions, letters, several numbers) comes back as written.
 */
fun formatPhone(raw: String?): String? {
    val text = raw?.trim()?.takeIf { it.isNotEmpty() } ?: return raw
    val digits = text.filter { it.isDigit() }
    if (text.any { it.isLetter() || it == ',' || it == ';' }) return text
    val (prefix, national) = when {
        // Mobiles have 9 national digits (30 123 4567), landlines 8 (1 234 5678, 96 123 456).
        text.startsWith("+36") && digits.length in 10..11 -> "+36" to digits.drop(2)
        text.startsWith("0036") && digits.length in 12..13 -> "+36" to digits.drop(4)
        text.startsWith("06") && digits.length in 10..11 -> "06" to digits.drop(2)
        else -> return text
    }
    // Budapest has a one-digit area code (1); everything else two (20, 30, 70, 96, ...).
    val area = if (national.startsWith("1")) 1 else 2
    val rest = national.drop(area)
    val grouped = if (rest.length in 6..7) "${rest.take(3)} ${rest.drop(3)}" else rest
    return "$prefix ${national.take(area)} $grouped"
}

/** Minor units as the amount a person types back ("12500" or "12500,5"). */
fun minorToMajorInput(minor: Long): String {
    val whole = minor / 100
    val cents = minor % 100
    return if (cents == 0L) whole.toString() else "$whole,${cents.toString().padStart(2, '0').trimEnd('0')}"
}
