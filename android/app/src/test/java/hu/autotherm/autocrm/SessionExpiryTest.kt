package hu.autotherm.autocrm

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.SessionUser
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.prefs.ServerStore
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A 401 means "the session is gone; the only cure is signing in again" (ApiError.kt). The
 * app only leaves the signed-in screens when the stored account disappears (MainActivity),
 * so a 401 on a request that carried the token must forget that token.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class SessionExpiryTest {

    private lateinit var server: MockWebServer
    private lateinit var api: AutoCrmApi
    private lateinit var sessions: SessionStore
    private lateinit var context: Context

    private val user = SessionUser(
        id = 1,
        email = "a@b.hu",
        displayName = "A",
        role = "office",
        mustChangePassword = false,
    )

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        server = MockWebServer()
        server.start()
        val serverStore = ServerStore(context)
        runBlocking { serverStore.save(server.url("/").toString()) }
        sessions = SessionStore(context)
        api = AutoCrmApi(serverStore, sessions)
    }

    @After
    fun tearDown() {
        server.shutdown()
        runBlocking {
            ServerStore(context).clear()
            sessions.clear()
        }
    }

    private fun unauthenticated() = MockResponse().setResponseCode(401)
        .setBody("""{"error":{"code":"unauthenticated","message":"authentication required"}}""")

    @Test
    fun `a 401 on a signed-in request signs the phone out`() = runTest {
        sessions.save("dead-token", null, user)
        server.enqueue(unauthenticated())

        try {
            api.pickerOrders()
            fail("expected Unauthenticated")
        } catch (e: ApiException.Unauthenticated) {
            // expected
        }

        assertNull(sessions.currentAccount())
        assertNull(sessions.token())
    }

    @Test
    fun `a 401 for an old token does not sign out a newer session`() = runTest {
        sessions.save("old-token", null, user)
        // The request goes out with the old token; before its answer arrives, the user
        // signs in again and a new token is stored.
        server.enqueue(unauthenticated().setBodyDelay(300, TimeUnit.MILLISECONDS))
        // On IO, not the test dispatcher: takeRequest() below blocks this thread.
        val call = async(Dispatchers.IO) { runCatching { api.pickerOrders() } }
        assertNotNull(server.takeRequest(5, TimeUnit.SECONDS))
        sessions.save("new-token", null, user)
        call.await()

        assertEquals("new-token", sessions.token())
        assertNotNull(sessions.currentAccount())
    }

    @Test
    fun `a 403 keeps the session`() = runTest {
        sessions.save("live-token", null, user)
        server.enqueue(
            MockResponse().setResponseCode(403)
                .setBody("""{"error":{"code":"forbidden","message":"no"}}"""),
        )

        runCatching { api.pickerOrders() }

        assertEquals("live-token", sessions.token())
    }
}
