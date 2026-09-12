package hu.autotherm.autocrm

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.db.AutoCrmDatabase
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.upload.Uploader
import hu.autotherm.autocrm.data.upload.sha256Hex
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * The queue is the part of this app that can lose a photo, so it is the part with tests.
 *
 * These run on the JVM under Robolectric against an in-memory Room database and a
 * MockWebServer standing in for the API — no device, no emulator, no network. The three
 * behaviours proven here are the three the viability review said were unknown: a photo
 * survives a failure, a retried batch does not duplicate, and nothing is ever deleted
 * without either the server confirming it or the fitter saying so.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class UploadQueueTest {

    private lateinit var db: AutoCrmDatabase
    private lateinit var server: MockWebServer
    private lateinit var api: AutoCrmApi
    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        db = Room.inMemoryDatabaseBuilder(context, AutoCrmDatabase::class.java)
            .allowMainThreadQueries()
            .build()
        server = MockWebServer()
        server.start()
        // A session store with no token: the API attaches no Authorization header, which is
        // fine because MockWebServer does not check one.
        api = AutoCrmApi(server.url("/").toString().trimEnd('/'), SessionStore(context))
    }

    @After
    fun tearDown() {
        server.shutdown()
        db.close()
    }

    private fun photo(name: String = "photo.jpg", content: String = "fake-jpeg-bytes"): File =
        File(context.cacheDir, name).apply { writeText(content) }

    private suspend fun enqueue(file: File, orderId: Long = 7): PendingUpload {
        val dao = db.pendingUploads()
        val sha = file.sha256Hex()
        val row = PendingUpload(
            orderId = orderId,
            orderNumber = "2026-0007",
            category = "completion",
            filePath = file.absolutePath,
            contentType = "image/jpeg",
            byteSize = file.length(),
            sha256 = sha,
            filename = "x.jpg",
        )
        val id = dao.insert(row)
        return dao.byId(if (id == -1L) dao.byContent(orderId, sha)!!.id else id)!!
    }

    @Test
    fun `a photo survives a network failure and stays queued`() = runTest {
        val dao = db.pendingUploads()
        val row = enqueue(photo("survives.jpg"))
        // No queued responses: MockWebServer closes the connection, which surfaces as IO.
        server.shutdown()

        val outcome = Uploader(api, dao).upload(row)

        assertTrue("a network failure must be retryable", outcome is Uploader.Outcome.Retry)
        val after = dao.byId(row.id)
        assertNotNull("the row must still exist", after)
        assertTrue(
            "the file must still be on disk",
            File(after!!.filePath).exists(),
        )
    }

    @Test
    fun `a 422 blocks the photo instead of retrying forever`() = runTest {
        val dao = db.pendingUploads()
        val row = enqueue(photo("blocked.jpg"))
        server.enqueue(
            MockResponse()
                .setResponseCode(422)
                .setBody("""{"error":"validation","message":"unsupported image type"}"""),
        )

        val outcome = Uploader(api, dao).upload(row)

        assertTrue(outcome is Uploader.Outcome.Blocked)
        // Blocked, not deleted: the fitter decides, never the app.
        dao.markBlocked(row.id, (outcome as Uploader.Outcome.Blocked).reason)
        val after = dao.byId(row.id)!!
        assertEquals(PendingUpload.STATE_BLOCKED, after.state)
        assertTrue(File(after.filePath).exists())
    }

    @Test
    fun `already_uploaded completes without a second PUT`() = runTest {
        val dao = db.pendingUploads()
        val row = enqueue(photo("dedup.jpg"))
        // This is the server's answer when a batch is retried after a partial success. It
        // is what makes retrying the whole batch free rather than duplicating photos.
        server.enqueue(
            MockResponse().setBody("""{"status":"already_uploaded","image_id":99}"""),
        )

        val outcome = Uploader(api, dao).upload(row)

        assertTrue(outcome is Uploader.Outcome.Uploaded)
        assertEquals(1, server.requestCount)
        val after = dao.byId(row.id)!!
        assertEquals(PendingUpload.STATE_DONE, after.state)
        assertEquals(99L, after.uploadedImageId)
    }

    @Test
    fun `the full three step upload marks the row done`() = runTest {
        val dao = db.pendingUploads()
        val row = enqueue(photo("full.jpg"))
        val storage = server.url("/bucket/object").toString()
        server.enqueue(
            MockResponse().setBody(
                """{"status":"upload","ticket":"t-1","expires_at":"2999-01-01T00:00:00Z",
                    "upload":{"method":"PUT","url":"$storage","headers":{"x-amz-checksum-sha256":"abc"}}}""",
            ),
        )
        server.enqueue(MockResponse().setResponseCode(200)) // the PUT to object storage
        server.enqueue(
            MockResponse().setBody("""{"type":"image","created":true,"image":{"id":41,"category":"completion","uploaded_at":"2026-01-01T00:00:00Z","immutable":false}}"""),
        )

        val outcome = Uploader(api, dao).upload(row)

        assertTrue("expected Uploaded, got $outcome", outcome is Uploader.Outcome.Uploaded)
        assertEquals("ticket, PUT, complete", 3, server.requestCount)
        val after = dao.byId(row.id)!!
        assertEquals(PendingUpload.STATE_DONE, after.state)
        assertEquals(41L, after.uploadedImageId)
    }

    @Test
    fun `the same photo twice on one order is one row`() = runTest {
        val dao = db.pendingUploads()
        val file = photo("double-tap.jpg", content = "identical")
        val first = enqueue(file)

        val second = File(context.cacheDir, "double-tap-copy.jpg").apply { writeText("identical") }
        val duplicate = PendingUpload(
            orderId = 7,
            orderNumber = "2026-0007",
            category = "completion",
            filePath = second.absolutePath,
            contentType = "image/jpeg",
            byteSize = second.length(),
            sha256 = second.sha256Hex(),
            filename = "y.jpg",
        )
        val inserted = dao.insert(duplicate)

        assertEquals("a duplicate insert is ignored", -1L, inserted)
        assertEquals(1, dao.watchQueue().first().size)
        assertEquals(first.sha256, second.sha256Hex())
    }

    @Test
    fun `a missing file blocks rather than retrying forever`() = runTest {
        val dao = db.pendingUploads()
        val file = photo("vanished.jpg")
        val row = enqueue(file)
        file.delete()

        val outcome = Uploader(api, dao).upload(row)

        assertTrue(outcome is Uploader.Outcome.Blocked)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun `sweeping deletes confirmed rows and their files, and nothing else`() = runTest {
        val dao = db.pendingUploads()
        val done = enqueue(photo("done.jpg"), orderId = 1)
        val waiting = enqueue(photo("waiting.jpg", content = "other"), orderId = 2)
        dao.markDone(done.id, 5)

        dao.sweepCompleted { path -> File(path).delete() }

        assertNull("a confirmed row is gone", dao.byId(done.id))
        assertTrue("its file is gone too", !File(done.filePath).exists())
        assertNotNull("a waiting row is untouched", dao.byId(waiting.id))
        assertTrue("its file is untouched", File(waiting.filePath).exists())
    }

    @Test
    fun `backoff grows and then caps`() {
        assertEquals(0L, PendingUpload.backoffFor(0))
        assertEquals(10_000L, PendingUpload.backoffFor(1))
        assertTrue(PendingUpload.backoffFor(3) > PendingUpload.backoffFor(2))
        // Far past the end of the table: capped, not out of bounds.
        assertEquals(PendingUpload.backoffFor(5), PendingUpload.backoffFor(50))
    }

    @Test
    fun `errors are classified as retryable or not`() {
        assertTrue(ApiException.Network(java.io.IOException("no route")).isRetryable)
        assertTrue(ApiException.Server(503, "restarting").isRetryable)
        assertTrue(!ApiException.Rule("validation", "bad").isRetryable)
        assertTrue(!ApiException.Forbidden().isRetryable)
        assertTrue(!ApiException.Unauthenticated().isRetryable)
    }
}
