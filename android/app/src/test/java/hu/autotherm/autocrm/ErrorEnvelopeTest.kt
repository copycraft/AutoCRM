package hu.autotherm.autocrm

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import hu.autotherm.autocrm.data.api.ApiErrorBody
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.prefs.ServerStore
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The server's error envelope is `{"error": {"code", "message"}}` (backend/src/error.rs).
 * The phone once modelled `error` as a plain string, so every decode failed and all
 * 409/422s collapsed to `Rule("validation", null)` — the real code and message lost.
 * This test pins the envelope shape: it fails if either side moves.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class ErrorEnvelopeTest {

    private val json = Json { ignoreUnknownKeys = true }

    private lateinit var server: MockWebServer
    private lateinit var api: AutoCrmApi
    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        server = MockWebServer()
        server.start()
        val serverStore = ServerStore(context)
        runBlocking { serverStore.save(server.url("/").toString()) }
        api = AutoCrmApi(serverStore, SessionStore(context))
    }

    @After
    fun tearDown() {
        server.shutdown()
        runBlocking { ServerStore(context).clear() }
    }

    @Test
    fun `stage gate refusal decodes with its code and message`() {
        val body = json.decodeFromString<ApiErrorBody>(
            ApiErrorBody.serializer(),
            """{"error":{"code":"stage_gate","message":"Hiányzik a kötelező fotó."}}""",
        )
        assertEquals("stage_gate", body.error.code)
        assertEquals("Hiányzik a kötelező fotó.", body.error.message)
    }

    @Test
    fun `validation error without message decodes with null message`() {
        val body = json.decodeFromString<ApiErrorBody>(
            ApiErrorBody.serializer(),
            """{"error":{"code":"validation"}}""",
        )
        assertEquals("validation", body.error.code)
        assertEquals(null, body.error.message)
    }

    @Test
    fun `a 400 validation refusal surfaces as Rule, not a server fault`() = runTest {
        // 400 carries the same envelope as 409/422. Mapping it to Server hid field
        // errors behind "server error" lines and wrongly marked queue rows retryable.
        server.enqueue(
            MockResponse().setResponseCode(400)
                .setBody("""{"error":{"code":"validation","message":"title is required"}}"""),
        )
        try {
            api.me()
            fail("expected ApiException.Rule")
        } catch (e: ApiException.Rule) {
            assertEquals("validation", e.code)
            assertEquals("title is required", e.detail)
        }
    }

    @Test
    fun `a 500 still surfaces as a retryable server fault`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(500)
                .setBody("""{"error":{"code":"internal","message":"boom"}}"""),
        )
        try {
            api.me()
            fail("expected ApiException.Server")
        } catch (e: ApiException.Server) {
            assertEquals(500, e.status)
            assertTrue(e.isRetryable)
        }
    }
}
