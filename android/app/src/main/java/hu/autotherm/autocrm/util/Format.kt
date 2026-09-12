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
 * Money as the web client shows it: minor units in, grouped major units out, currency
 * symbol after the number. HUF has no fractional part in practice — the API still stores
 * fillér, so the division is by 100 either way and HUF simply never shows the remainder.
 */
fun formatMoney(minor: Long, currency: String): String {
    val amount = minor / 100.0
    val format = NumberFormat.getNumberInstance(HU).apply {
        minimumFractionDigits = if (currency == "HUF") 0 else 2
        maximumFractionDigits = if (currency == "HUF") 0 else 2
    }
    val symbol = when (currency) {
        "HUF" -> "Ft"
        "EUR" -> "€"
        else -> currency
    }
    return "${format.format(amount)} $symbol"
}

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
