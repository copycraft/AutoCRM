package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.ApiErrorBody
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The server's error envelope is `{"error": {"code", "message"}}` (backend/src/error.rs).
 * The phone once modelled `error` as a plain string, so every decode failed and all
 * 409/422s collapsed to `Rule("validation", null)` — the real code and message lost.
 * This test pins the envelope shape: it fails if either side moves.
 */
class ErrorEnvelopeTest {

    private val json = Json { ignoreUnknownKeys = true }

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
}
