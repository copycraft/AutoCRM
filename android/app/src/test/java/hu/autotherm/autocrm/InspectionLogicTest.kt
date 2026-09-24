package hu.autotherm.autocrm

import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import hu.autotherm.autocrm.data.db.InspectionDraft
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.inspection.DraftDamage
import hu.autotherm.autocrm.data.inspection.DraftPayload
import hu.autotherm.autocrm.data.inspection.InspectionSyncWorker
import hu.autotherm.autocrm.data.inspection.SyncResult
import hu.autotherm.autocrm.data.inspection.syncDraft
import hu.autotherm.autocrm.data.upload.Uploader
import hu.autotherm.autocrm.data.upload.sha256Hex
import hu.autotherm.autocrm.ui.photos.OrderPhotoViewModel
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.serialization.json.Json
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
 * Logic audit, inspection & media journey. Each test is named after the expected
 * behaviour from scratchpad/spec/inspection.md and failed on the code before its fix.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = AutoCrmApp::class)
class InspectionLogicTest {

    private lateinit var app: AutoCrmApp
    private lateinit var server: MockWebServer
    private val json = Json { ignoreUnknownKeys = true }

    /** The database is a process-wide singleton; each test gets a fresh one. */
    private fun resetDatabaseSingleton() {
        hu.autotherm.autocrm.data.db.AutoCrmDatabase::class.java
            .getDeclaredField("instance")
            .apply { isAccessible = true }
            .set(null, null)
    }

    @Before
    fun setUp() {
        resetDatabaseSingleton()
        app = ApplicationProvider.getApplicationContext()
        app.deleteDatabase("autocrm.db")
        server = MockWebServer()
        server.start()
        runBlocking { app.serverStore.save(server.url("/").toString()) }
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
        server.shutdown()
        runBlocking { app.serverStore.clear() }
        app.database.close()
        resetDatabaseSingleton()
    }

    private fun inspectionJson(id: Long, status: String = "draft") =
        """{"id":$id,"order_id":7,"kind":"checkout","status":"$status","vehicle_plate":"ABC-123",
            "inspector_name":"Sanyi","created_by":1,"created_at":"2026-09-10T09:00:00Z",
            "updated_at":"2026-09-10T09:00:00Z"}"""

    private fun draft(payload: DraftPayload, serverId: Long? = null, uuid: String = "u-1") =
        runBlocking {
            app.database.inspectionDrafts().upsert(
                InspectionDraft(
                    localUuid = uuid,
                    orderId = 7,
                    orderNumber = "2026-0007",
                    kind = "checkout",
                    serverId = serverId,
                    payloadJson = json.encodeToString(DraftPayload.serializer(), payload),
                ),
            )
            uuid
        }

    private fun queueRow(category: String, createdAt: Long, content: String): PendingUpload {
        val file = File(app.cacheDir, "$content.jpg").apply { writeText(content) }
        return PendingUpload(
            orderId = 7,
            orderNumber = "2026-0007",
            category = category,
            filePath = file.absolutePath,
            contentType = "image/jpeg",
            byteSize = file.length(),
            sha256 = file.sha256Hex(),
            filename = file.name,
            createdAt = createdAt,
        )
    }

    // MEDIA-10: the general worker must see its own rows even when the oldest
    // due rows all belong to the inspection sync.
    @Test
    fun `production photos are due even behind twenty older inspection photos`() = runBlocking {
        val dao = app.database.pendingUploads()
        repeat(20) { i -> dao.insert(queueRow("inspection", createdAt = 1_000L + i, content = "insp-$i")) }
        dao.insert(queueRow("production", createdAt = 5_000L, content = "prod"))

        val due = dao.dueForUpload(System.currentTimeMillis())

        assertTrue(
            "the production photo must be in the worker's batch, got ${due.map { it.category }}",
            due.any { it.category == "production" },
        )
        assertTrue("inspection rows belong to the inspection sync", due.none { it.category == "inspection" })
    }

    // MEDIA-15: a ticket whose bytes are already in storage is reused on the next
    // attempt, so a failed complete does not upload the photo a second time.
    @Test
    fun `a retry after a failed complete reuses the ticket instead of uploading again`() = runBlocking {
        val dao = app.database.pendingUploads()
        val id = dao.insert(queueRow("production", createdAt = 1L, content = "ticket-reuse"))
        val storage = server.url("/bucket/object").toString()
        server.enqueue(
            MockResponse().setBody(
                """{"status":"upload","ticket":"t-1","expires_at":"2999-01-01T00:00:00Z",
                    "upload":{"method":"PUT","url":"$storage","headers":[]}}""",
            ),
        )
        server.enqueue(MockResponse().setResponseCode(200)) // PUT
        server.enqueue(MockResponse().setResponseCode(503)) // complete fails
        val first = Uploader(app.api, dao).upload(dao.byId(id)!!)
        assertTrue(first is Uploader.Outcome.Retry)
        // What UploadWorker does with a retry.
        dao.markRetryable(id, "503", nextAttemptAt = 0)

        server.enqueue(
            MockResponse().setBody("""{"type":"image","created":true,"image":{"id":41,"category":"production","uploaded_at":"2026-01-01T00:00:00Z","immutable":false}}"""),
        )
        val second = Uploader(app.api, dao).upload(dao.byId(id)!!)

        assertTrue("expected Uploaded, got $second", second is Uploader.Outcome.Uploaded)
        assertEquals("ticket, PUT, complete, then only complete again", 4, server.requestCount)
    }

    // INSP-39 / INSP-40: the server sign happens only after the local sign-off
    // (overview photos, both signatures, verdicts) has passed on the phone.
    @Test
    fun `an unsigned draft is not pushed or signed by the sync`() = runBlocking {
        val uuid = draft(DraftPayload(vehiclePlate = "ABC-123", inspectorName = "Sanyi", signed = false))

        val result = syncDraft(app, uuid)

        assertEquals("no request may reach the server for a walk in progress", 0, server.requestCount)
        assertFalse(result is SyncResult.Failed)
        val row = app.database.inspectionDrafts().byUuid(uuid)
        assertNotNull("the draft stays on the phone", row)
        assertNull(row!!.serverId)
        assertNull(row.error)
    }

    // INSP-L10: the create carries the draft's stable local UUID as the
    // idempotency key, so a retry after a lost response replays to the same
    // server row instead of failing forever on `checkout_open`.
    @Test
    fun `the inspection create carries the draft uuid as its idempotency key`() = runBlocking {
        val uuid = draft(
            DraftPayload(vehiclePlate = "ABC-123", inspectorName = "Sanyi", signed = true),
            uuid = "draft-uuid-7",
        )
        server.enqueue(MockResponse().setResponseCode(201).setBody(inspectionJson(42)))
        server.enqueue(MockResponse().setBody(inspectionJson(42, "signed")))

        val result = syncDraft(app, uuid)

        assertEquals(SyncResult.Done, result)
        val create = server.takeRequest()
        assertEquals("/api/inspections", create.path)
        val body = create.body.readUtf8()
        assertTrue(
            "create must send client_key=$uuid for idempotent retry, got: $body",
            body.contains("\"client_key\":\"$uuid\""),
        )
    }

    // INSP-19 / INSP-20: a damage without type and severity was never finished
    // ("válassz típust és súlyosságot"), so it is not part of the record.
    @Test
    fun `an unfinished damage without type is not sent to the server`() = runBlocking {
        val uuid = draft(
            DraftPayload(
                vehiclePlate = "ABC-123",
                inspectorName = "Sanyi",
                damages = listOf(DraftDamage(zoneKey = "front", damageType = "", severity = "")),
                signed = true,
            ),
        )
        server.enqueue(MockResponse().setResponseCode(201).setBody(inspectionJson(42)))
        server.enqueue(MockResponse().setBody(inspectionJson(42, "signed")))

        val result = syncDraft(app, uuid)

        server.takeRequest() // create
        val next = server.takeRequest()
        assertEquals("/api/inspections/42/sign", next.path)
        assertEquals(SyncResult.Done, result)
    }

    // INSP-46: every sync step is idempotent. A sync that died after the server
    // sign but before the local delete resumes as done, not as a permanent error.
    @Test
    fun `a draft already signed on the server finishes as done`() = runBlocking {
        val uuid = draft(
            DraftPayload(vehiclePlate = "ABC-123", inspectorName = "Sanyi", signed = true),
            serverId = 42,
        )
        server.enqueue(
            MockResponse().setResponseCode(422)
                .setBody("""{"error":{"code":"locked","message":"a signed inspection cannot be changed; add a follow-up note instead"}}"""),
        )
        server.enqueue(MockResponse().setBody(
            """{"inspection":${inspectionJson(42, "signed")},"photos":[],"damages":[],"verdicts":[],"signatures":[],"notes":[]}""",
        ))

        val result = syncDraft(app, uuid)

        assertEquals(SyncResult.Done, result)
        assertNull(app.database.inspectionDrafts().byUuid(uuid))
    }

    // INSP-14 / INSP-08: discarding a draft that already reached the server as a
    // draft also discards the server draft, or the order is stuck on `checkout_open`.
    @Test
    fun `discarding a synced draft discards the server draft too`() = runBlocking {
        val uuid = draft(
            DraftPayload(vehiclePlate = "ABC-123", inspectorName = "Sanyi", signed = true),
            serverId = 42,
        )
        server.enqueue(MockResponse().setResponseCode(204))

        InspectionSyncWorker.discard(app, uuid)

        assertEquals(1, server.requestCount)
        val request = server.takeRequest()
        assertEquals("DELETE", request.method)
        assertEquals("/api/inspections/42", request.path)
        assertNull(app.database.inspectionDrafts().byUuid(uuid))
    }

    // MEDIA-04: the camera writes straight into the queue's directory and the file is
    // moved into the queue, so no second copy is left behind.
    @Test
    fun `a camera photo leaves no orphan copy in the queue directory`() = runBlocking {
        val vm = OrderPhotoViewModel(app.uploadQueue, app.capturePrefs)
        val pendingDir = File(app.filesDir, "pending").apply { mkdirs() }
        val target = File(pendingDir, "camera-1.jpg").apply { writeText("camera-bytes") }

        val sha = File(app.cacheDir, "x").let { it.writeText("camera-bytes"); it.sha256Hex() }

        vm.cameraResult(true, target, Uri.fromFile(target), 7, "2026-0007")

        // The queue works on the IO dispatcher: wait for the row.
        var rows: PendingUpload? = null
        val deadline = System.currentTimeMillis() + 5_000
        while (rows == null && System.currentTimeMillis() < deadline) {
            rows = app.database.pendingUploads().byContent(7, sha)
            if (rows == null) Thread.sleep(50)
        }
        assertNotNull("the photo is queued", rows)
        assertFalse("the camera target must not stay behind", target.exists())
        assertEquals(
            listOf(File(rows!!.filePath).name),
            pendingDir.listFiles()!!.map { it.name },
        )
    }
}
