package hu.autotherm.autocrm

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.prefs.ServerStore
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * What the login request actually puts on the wire.
 *
 * This exists because of a bug that no amount of reading the Kotlin would have shown:
 * `client` was declared with a default of `"mobile"`, and kotlinx.serialization omits
 * default values unless `encodeDefaults` is set. The field never left the phone, the server
 * applied its own default of `web`, sent back a cookie the app cannot read, and the app
 * reported "the server gave no session ticket" — an error that points at the server.
 *
 * Asserting the serialised body, rather than the Kotlin object, is the only way that class
 * of mistake is visible.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class LoginRequestTest {

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
    fun `the login body says client mobile, or the server answers with a cookie`() = runTest {
        server.enqueue(
            MockResponse().setBody(
                """{"user":{"id":1,"email":"a@b.hu","display_name":"A","role":"office",
                    "must_change_password":false},"token":"t","expires_at":"2999-01-01T00:00:00Z"}""",
            ),
        )

        api.login("a@b.hu", "secret", "Pixel 6")

        val sent = server.takeRequest()
        val body = Json.parseToJsonElement(sent.body.readUtf8()).jsonObject

        assertEquals("mobile", body["client"]?.jsonPrimitive?.content)
        assertEquals("a@b.hu", body["email"]?.jsonPrimitive?.content)
        assertEquals("secret", body["password"]?.jsonPrimitive?.content)
        // The device label reaches the session list, so an admin revoking a lost phone from
        // the web app can tell which one it is.
        assertEquals("Pixel 6", body["device_label"]?.jsonPrimitive?.content)
        assertEquals("/api/auth/login", sent.path)
    }

    @Test
    fun `the bearer token is attached to later requests`() = runTest {
        server.enqueue(
            MockResponse().setBody(
                """{"user":{"id":1,"email":"a@b.hu","display_name":"A","role":"office",
                    "must_change_password":false},"token":"tok-123","expires_at":"2999-01-01T00:00:00Z"}""",
            ),
        )
        val response = api.login("a@b.hu", "secret", "Pixel 6")
        SessionStore(context).save(response.token!!, response.expiresAt, response.user)
        server.takeRequest()

        server.enqueue(MockResponse().setBody("""{"items":[]}"""))
        api.pickerOrders()

        val sent = server.takeRequest()
        assertEquals("Bearer tok-123", sent.getHeader("Authorization"))
    }
}
