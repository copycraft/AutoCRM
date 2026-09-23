package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.Completed
import hu.autotherm.autocrm.data.api.UploadResponse
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The shapes that broke the whole upload queue: the server sends presigned
 * headers as an array of pairs (`[["Content-Type", "…"]]`), and document
 * completions carry no `image` key at all. Both decoded as something else
 * before, and every ticket died with "unparseable response".
 */
class UploadResponseTest {

    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `ticket headers decode from pairs`() {
        val body = """
            {"status":"upload","ticket":"abc","expires_at":"2026-09-23T20:00:00Z",
             "upload":{"method":"PUT","url":"https://s3/x",
              "headers":[["Content-Type","image/jpeg"],["x-amz-checksum-sha256","e3=="]]}}
        """.trimIndent()
        val response = json.decodeFromString(UploadResponse.serializer(), body)
        assertEquals("upload", response.status)
        assertEquals(
            mapOf("Content-Type" to "image/jpeg", "x-amz-checksum-sha256" to "e3=="),
            response.upload?.headers,
        )
    }

    @Test
    fun `document completion decodes without an image key`() {
        val body = """{"type":"document","document":{"id":7},"created":true}"""
        val completed = json.decodeFromString(Completed.serializer(), body)
        assertEquals("document", completed.type)
        assertEquals(7L, completed.document?.id)
        assertNull(completed.image)
    }

    @Test
    fun `image completion still decodes`() {
        val body = """
            {"type":"image","created":true,
             "image":{"id":9,"category":"inspection","thumb_url":null,"display_url":null,
              "captured_at":null,"uploaded_at":"2026-09-23T20:00:00Z","immutable":false}}
        """.trimIndent()
        val completed = json.decodeFromString(Completed.serializer(), body)
        assertEquals(9L, completed.image?.id)
        assertNotNull(completed.image)
    }
}
