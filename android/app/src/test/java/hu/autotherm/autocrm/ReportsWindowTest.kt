package hu.autotherm.autocrm

import hu.autotherm.autocrm.ui.reports.workloadWindow
import java.time.Instant
import java.time.LocalDate
import java.util.TimeZone
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/**
 * The reports screen asks for "the last 30 days" ending on the business day (Budapest),
 * like the backend's business_today and the web's budapestIsoPlus — not the phone's zone.
 */
class ReportsWindowTest {
    private lateinit var saved: TimeZone

    @Before fun pinDeviceZoneToUtc() {
        saved = TimeZone.getDefault()
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"))
    }

    @After fun restore() {
        TimeZone.setDefault(saved)
    }

    @Test fun lastThirtyDaysEndOnTheBudapestCalendarDayNotTheDeviceDay() {
        // 22:30 UTC on 22 Sep = 00:30 on 23 Sep in Budapest (CEST).
        val (from, to) = workloadWindow(Instant.parse("2026-09-22T22:30:00Z"))
        assertEquals(LocalDate.parse("2026-09-23"), to)
        assertEquals(LocalDate.parse("2026-08-25"), from)
    }
}
