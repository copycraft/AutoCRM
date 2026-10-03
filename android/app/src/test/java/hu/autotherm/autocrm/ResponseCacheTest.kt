package hu.autotherm.autocrm

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.SessionUser
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.cache.ResponseCache
import hu.autotherm.autocrm.data.cache.ResponseCacheDatabase
import hu.autotherm.autocrm.data.prefs.ServerStore
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Screens keep working with no signal: a read the server cannot answer is served from the
 * last good copy, and nothing else is. These tests pin the rules that keep that safe.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class ResponseCacheTest {

    private lateinit var context: Context
    private lateinit var db: ResponseCacheDatabase
    private lateinit var cache: ResponseCache
    private lateinit var server: MockWebServer
    private lateinit var session: SessionStore
    private var online = true
    private lateinit var api: AutoCrmApi

    private val partners =
        """{"items":[{"id":1,"kind":"business","name":"Kovács Kft.","country":"HU"}]}"""

    private fun user(id: Long) = SessionUser(
        id = id, email = "u$id@autotherm.test", displayName = "U$id", role = "office", mustChangePassword = false,
    )

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        db = Room.inMemoryDatabaseBuilder(context, ResponseCacheDatabase::class.java)
            .allowMainThreadQueries().build()
        cache = ResponseCache(db.responses())
        server = MockWebServer().also { it.start() }
        val serverStore = ServerStore(context)
        runBlocking { serverStore.save(server.url("/").toString()) }
        session = SessionStore(context)
        runBlocking { session.save("token-1", null, user(1)) }
        api = AutoCrmApi(serverStore, session, cache = cache, isOnline = { online })
    }

    @After
    fun tearDown() {
        runCatching { server.shutdown() }
        db.close()
        runBlocking {
            ServerStore(context).clear()
            session.clear()
        }
    }

    @Test
    fun `a read the server cannot answer comes from the last good copy`() = runBlocking {
        server.enqueue(MockResponse().setBody(partners))
        assertEquals("Kovács Kft.", api.partners().single().name)
        assertNull(api.offlineSince.value)

        server.shutdown() // the signal is gone
        val offline = api.partners()
        assertEquals("Kovács Kft.", offline.single().name)
        assertNotNull("the screen is told this is old data", api.offlineSince.value)
    }

    @Test
    fun `coming back online clears the offline flag`() = runBlocking {
        server.enqueue(MockResponse().setBody(partners))
        api.partners()
        // The signal goes: the old copy is shown, and the flag says so.
        val port = server.port
        server.shutdown()
        api.partners()
        assertNotNull(api.offlineSince.value)
        // The server is back on the same address and answers again.
        server = MockWebServer().also { it.start(port) }
        server.enqueue(MockResponse().setBody(partners))
        api.partners()
        assertNull(api.offlineSince.value)
    }

    @Test
    fun `a phone that says it is offline still tries, and a wrong hint costs nothing`() = runBlocking {
        server.enqueue(MockResponse().setBody(partners))
        api.partners()
        online = false
        // The hint is wrong (the server is reachable): the fresh answer is used, not the copy.
        server.enqueue(MockResponse().setBody(partners))
        assertEquals(1, api.partners().size)
        assertNull("the server answered, so this is not offline", api.offlineSince.value)
    }

    @Test
    fun `a read that was never fetched has nothing to fall back to`() = runBlocking {
        online = false
        val failure = runCatching { api.partners() }.exceptionOrNull()
        assertTrue(failure is ApiException.Network)
        assertNull(api.offlineSince.value)
    }

    @Test
    fun `a write is never faked while offline`() = runBlocking {
        online = false
        val failure = runCatching { api.deleteItem(5) }.exceptionOrNull()
        assertTrue("the change must fail loudly, not appear to succeed", failure is ApiException.Network)
    }

    @Test
    fun `staff data and user lists are never kept on the phone`() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"items":[{"id":1,"full_name":"Kiss Péter"}]}"""))
        api.employees()
        server.enqueue(MockResponse().setBody("""{"items":[]}"""))
        api.users()
        online = false
        assertTrue(runCatching { api.employees() }.exceptionOrNull() is ApiException.Network)
        assertTrue(runCatching { api.users() }.exceptionOrNull() is ApiException.Network)
    }

    @Test
    fun `one user never reads another users copy`() = runBlocking {
        server.enqueue(MockResponse().setBody(partners))
        api.partners()
        session.clear()
        session.save("token-2", null, user(2))
        online = false
        assertTrue(runCatching { api.partners() }.exceptionOrNull() is ApiException.Network)
    }

    @Test
    fun `a server error is an error, not a reason to show the old copy`() = runBlocking {
        server.enqueue(MockResponse().setBody(partners))
        api.partners()
        server.enqueue(MockResponse().setResponseCode(500).setBody("""{"error":{"code":"internal","message":"x"}}"""))
        assertTrue(runCatching { api.partners() }.exceptionOrNull() is ApiException.Server)
    }

    @Test
    fun `being signed out by the server empties the cache`() = runBlocking {
        server.enqueue(MockResponse().setBody(partners))
        api.partners()
        server.enqueue(MockResponse().setResponseCode(401).setBody("""{"error":{"code":"unauthenticated","message":"x"}}"""))
        assertTrue(runCatching { api.partners() }.exceptionOrNull() is ApiException.Unauthenticated)
        assertNull(cache.get(server.url("/api/partners?limit=50").toString(), 1))
        assertNull(db.responses().find(server.url("/api/partners?limit=50").toString(), 1))
    }

    @Test
    fun `which paths are worth keeping`() {
        for (path in listOf("/api/orders", "/api/orders/7", "/api/partners", "/api/leads/3", "/api/config/lookups", "/api/reports/stalled", "/api/emails")) {
            assertTrue(path, ResponseCache.isCacheable(path))
        }
        for (path in listOf(
            "/api/auth/me", "/api/hr/employees", "/api/hr/absences", "/api/users", "/api/notifications",
            "/api/search", "/api/orders/7/images", "/api/orders/7/documents", "/api/documents/9/url", "/health",
        )) {
            assertFalse(path, ResponseCache.isCacheable(path))
        }
    }

    @Test
    fun `old and surplus rows are dropped as new ones arrive`() = runBlocking {
        var clock = 1_000_000_000_000L
        val small = ResponseCache(db.responses()) { clock }
        // 25 writes trigger housekeeping; the first is far older than the age limit.
        small.put("old", 1, "{}")
        clock += ResponseCache.MAX_AGE_MS + 1
        repeat(24) { small.put("k$it", 1, "{}") }
        assertNull(small.get("old", 1))
        assertNotNull(small.get("k0", 1))
    }
}
