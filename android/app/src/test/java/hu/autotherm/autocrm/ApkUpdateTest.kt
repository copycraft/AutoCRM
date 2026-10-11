package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.AppVersion
import hu.autotherm.autocrm.ui.update.ApkUpdater
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.ByteArrayInputStream
import java.io.File
import kotlin.io.path.createTempDirectory

/** The self-update path: hashing, the verified download, and the wire shape. */
class ApkUpdateTest {

    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `sha256 matches the well known digest of the bytes`() {
        val file = File(createTempDirectory("apk").toFile(), "a.bin")
        file.writeBytes("abc".toByteArray())
        // The canonical sha256("abc"), so a refactor cannot quietly change the digest.
        assertEquals(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ApkUpdater.sha256(file),
        )
    }

    @Test
    fun `the version endpoint parses, and an old server without optional fields still does`() {
        val full = json.decodeFromString(
            AppVersion.serializer(),
            """{"version_code":14,"version_name":"1.4.0","apk_url":"/api/mobile/app.apk",
               "sha256":"abc123","size_bytes":12345678,"mandatory":true,"changelog_hu":"Hűtőgép javítás."}""",
        )
        assertEquals(14L, full.versionCode)
        assertEquals("/api/mobile/app.apk", full.apkUrl)
        assertTrue(full.mandatory)
        assertEquals("Hűtőgép javítás.", full.changelogHu)

        val bare = json.decodeFromString(
            AppVersion.serializer(),
            """{"version_code":15,"version_name":"1.5","apk_url":"/api/mobile/app.apk",
               "sha256":"def","size_bytes":10}""",
        )
        assertFalse(bare.mandatory)
        assertEquals("", bare.changelogHu)
    }

    @RunWith(RobolectricTestRunner::class)
    @Config(application = android.app.Application::class)
    class Download {
        private val context: android.content.Context
            get() = androidx.test.core.app.ApplicationProvider.getApplicationContext()

        @Test
        fun `a download that matches the digest is installable`() = runTest {
            val bytes = "PK a plausible apk payload".toByteArray()
            val sha = ApkUpdater.sha256(File(createTempDirectory("apk").toFile(), "src").apply { writeBytes(bytes) })
            val percents = mutableListOf<Int>()
            val result = ApkUpdater.download(
                context = context,
                expectedSize = bytes.size.toLong(),
                expectedSha256 = sha,
                open = { ByteArrayInputStream(bytes) },
                onProgress = { percents.add(it) },
            )
            assertTrue("got $result", result is ApkUpdater.Download.Done)
            val done = result as ApkUpdater.Download.Done
            assertEquals(sha, ApkUpdater.sha256(done.file))
            // Progress is monotonic and ends at 100.
            assertEquals(percents.sorted(), percents)
            assertEquals(100, percents.last())
        }

        @Test
        fun `a download that does not match is refused and leaves nothing behind`() = runTest {
            val result = ApkUpdater.download(
                context = context,
                expectedSize = 5L,
                expectedSha256 = "0".repeat(64),
                open = { ByteArrayInputStream("hello".toByteArray()) },
                onProgress = {},
            )
            assertTrue("got $result", result is ApkUpdater.Download.Corrupt)
            // A corrupt file must never sit where the installer could pick it up.
            assertFalse(ApkUpdater.pendingApk(context).exists())
            assertFalse(File(ApkUpdater.pendingApk(context).absolutePath + ".part").exists())
        }

        @Test
        fun `a failed transfer leaves no half file either`() = runTest {
            val result = ApkUpdater.download(
                context = context,
                expectedSize = 100L,
                expectedSha256 = "0".repeat(64),
                open = { throw java.io.IOException("nincs hálózat") },
                onProgress = {},
            )
            assertTrue("got $result", result is ApkUpdater.Download.Failed)
            assertEquals("nincs hálózat", (result as ApkUpdater.Download.Failed).reason)
            assertFalse(ApkUpdater.pendingApk(context).exists())
        }
    }
}