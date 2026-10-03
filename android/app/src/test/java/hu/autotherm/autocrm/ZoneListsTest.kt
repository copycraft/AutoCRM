package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.InspectionDetail
import hu.autotherm.autocrm.data.api.ZoneTemplate
import hu.autotherm.autocrm.data.inspection.DraftPayload
import hu.autotherm.autocrm.data.inspection.WALKAROUND_KINDS
import hu.autotherm.autocrm.data.inspection.ZONE_LIST_UNAVAILABLE
import hu.autotherm.autocrm.data.inspection.ZoneListSource
import hu.autotherm.autocrm.data.inspection.displayTitle
import hu.autotherm.autocrm.data.inspection.humanizeZoneKey
import hu.autotherm.autocrm.data.inspection.inspectionKindLabel
import hu.autotherm.autocrm.data.inspection.resolveZones
import hu.autotherm.autocrm.data.inspection.zoneListNotice
import hu.autotherm.autocrm.data.inspection.zoneTitle
import hu.autotherm.autocrm.data.prefs.ZoneListCache
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Which photos a vehicle needs is decided on the server, per vehicle kind and walkaround,
 * and the phone has no list of its own: it walks what the server sent, or what it last
 * downloaded, or nothing. The phone also starts walkarounds with no signal, so what it does
 * without the server, and how it names zones it was only told about, is what these pin down.
 */
class ZoneListsTest {

    private fun zone(key: String, title: String = "", position: Long = 1) = ZoneTemplate(
        id = position,
        setKey = "refrigerated_body:checkin",
        zoneKey = key,
        position = position,
        instruction = "Fotó: $key",
        optional = false,
        required = true,
        title = title,
        projectTypeId = 4,
        kind = "checkin",
    )

    // ── Which list a walkaround starts with ──

    @Test
    fun `the list the server just sent wins, even when an older one is cached`() {
        val fetched = listOf(zone("type_plate"))
        val resolved = resolveZones(fetched, cached = listOf(zone("old")))!!
        assertSame(fetched, resolved.zones)
        assertEquals(ZoneListSource.SERVER, resolved.source)
        assertNull(zoneListNotice(resolved.source))
    }

    @Test
    fun `offline, the last list downloaded for that vehicle kind is used and the screen says so`() {
        val cached = listOf(zone("type_plate"), zone("vin_windshield", position = 2))
        val resolved = resolveZones(fetched = null, cached = cached)!!
        assertSame(cached, resolved.zones)
        assertEquals(ZoneListSource.CACHE, resolved.source)
        assertNotNull(zoneListNotice(resolved.source))
    }

    @Test
    fun `offline with nothing downloaded starts nothing, rather than guessing which photos are needed`() {
        for (nothing in listOf<List<ZoneTemplate>?>(null, emptyList())) {
            assertNull(resolveZones(fetched = nothing, cached = nothing))
        }
        // And the inspector is told what to do about it.
        assertTrue(ZONE_LIST_UNAVAILABLE, "Átvétel-átadás" in ZONE_LIST_UNAVAILABLE)
    }

    @Test
    fun `an empty answer from the server is not a list`() {
        // A list with no zones would let a walkaround be signed with no photos at all.
        val cached = listOf(zone("type_plate"))
        assertSame(cached, resolveZones(fetched = emptyList(), cached = cached)!!.zones)
    }

    @Test
    fun `both walkarounds are downloaded ahead of time, so either can start offline`() {
        assertEquals(listOf("checkout", "checkin"), WALKAROUND_KINDS)
    }

    // ── What a zone is called ──

    @Test
    fun `a zone is called by the title the server gave it, or by its key spaced out`() {
        assertEquals("Gyári adattábla", zone("type_plate", title = "Gyári adattábla").displayTitle())
        // A draft saved before titles existed has none, and the app keeps no names of its own.
        assertEquals("Front left", zone("front_left", title = "").displayTitle())
        assertEquals("Some new zone", zone("some_new_zone", title = "  ").displayTitle())
        assertEquals("Roof", humanizeZoneKey("roof"))
    }

    @Test
    fun `a zone is named from the draft's frozen list, or from the titles the server sent with an inspection`() {
        val frozen = listOf(zone("box_side_door", title = "Doboz oldalajtaja"))
        assertEquals("Doboz oldalajtaja", zoneTitle("box_side_door", frozen))
        assertEquals("a key not in the list reads as itself", "Front", zoneTitle("front", frozen))

        val fromServer = mapOf("box_side_door" to "Doboz oldalajtaja", "blank" to " ")
        assertEquals("Doboz oldalajtaja", zoneTitle("box_side_door", fromServer))
        assertEquals("Front", zoneTitle("front", fromServer))
        assertEquals("a blank title is no title", "Blank", zoneTitle("blank", fromServer))
    }

    @Test
    fun `the two walkarounds are átvétel and kiadás, not the rental-car words`() {
        assertEquals("Átvétel", inspectionKindLabel("checkout"))
        assertEquals("Kiadás", inspectionKindLabel("checkin"))
    }

    // ── What is stored and what comes over the wire ──

    @Test
    fun `an entry is kept per vehicle kind and walkaround, and an order with no type has its own`() {
        assertEquals("4:checkin", ZoneListCache.key(4, "checkin"))
        assertEquals("4:checkout", ZoneListCache.key(4, "checkout"))
        assertEquals("general:checkout", ZoneListCache.key(null, "checkout"))
    }

    @Test
    fun `a cached list reads back as it was written`() {
        val zones = listOf(zone("type_plate", "Gyári adattábla", 1), zone("rear", "Hátul", 2))
        assertEquals(zones, ZoneListCache.decode(ZoneListCache.encode(zones)))
    }

    @Test
    fun `a damaged cache entry is a miss, not a crash`() {
        assertNull(ZoneListCache.decode("not json"))
        assertNull(ZoneListCache.decode("""{"a":1}"""))
    }

    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `a draft saved before titles existed still opens`() {
        // The zone as an older build froze it into the draft: no title, kind or project type.
        val old = """
            {"vehicle_plate":"ABC-123","templates":[{"id":1,"set_key":"default","zone_key":"front",
            "position":1,"instruction":"Elölről","optional":false,"required":true}]}
        """.trimIndent()
        val zone = json.decodeFromString(DraftPayload.serializer(), old).templates.single()
        assertEquals("", zone.title)
        assertNull(zone.projectTypeId)
        assertEquals("Front", zone.displayTitle())
    }

    @Test
    fun `an inspection from an older server, without zone titles, still parses`() {
        val detail = json.decodeFromString(
            InspectionDetail.serializer(),
            """
            {"inspection":{"id":1,"order_id":1,"kind":"checkout","status":"draft","vehicle_plate":"A",
            "inspector_name":"S","created_by":1,"created_at":"2026-09-30T10:00:00Z",
            "updated_at":"2026-09-30T10:00:00Z"}}
            """.trimIndent(),
        )
        assertTrue(detail.zoneTitles.isEmpty())
    }
}
