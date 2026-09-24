package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.ui.common.ERROR_TEXT
import hu.autotherm.autocrm.ui.common.describeError
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * `ui/common/Errors.kt` renders refusals from a table copied out of the backend's error
 * catalog (`ErrorCode` / `x-error-catalog` in `openapi/openapi.json`). This fails when the
 * backend adds a code the phone has no words for, or rewords one the phone still copies.
 * Regenerate the table with the snippet in `docs/error-codes.md`.
 */
class ErrorCatalogTest {

    private val catalog: Map<String, String> by lazy {
        val file = File("../../openapi/openapi.json")
        assertTrue("openapi/openapi.json not found at ${file.absolutePath}", file.exists())
        Json.parseToJsonElement(file.readText()).jsonObject["components"]!!.jsonObject["schemas"]!!
            .jsonObject["ErrorCode"]!!.jsonObject["x-error-catalog"]!!.jsonArray
            .associate {
                val o = it.jsonObject
                o["code"]!!.jsonPrimitive.content to o["hu"]!!.jsonPrimitive.content
            }
    }

    @Test
    fun `every backend code has the catalog's text on the phone`() {
        assertTrue("the catalog is empty", catalog.isNotEmpty())
        val missing = catalog.keys - ERROR_TEXT.keys
        assertTrue("codes with no phone text: $missing", missing.isEmpty())
        val drifted = catalog.filter { (code, hu) -> ERROR_TEXT[code] != hu }.keys
        assertTrue("phone text differs from the catalog for: $drifted", drifted.isEmpty())
        val stale = ERROR_TEXT.keys - catalog.keys
        assertTrue("phone text for codes the backend no longer sends: $stale", stale.isEmpty())
    }

    @Test
    fun `a rule refusal renders from the catalog, validation from its detail`() {
        assertEquals(
            catalog["currency_locked"],
            describeError(ApiException.Rule("currency_locked", "currency cannot change")),
        )
        assertEquals("title is required", describeError(ApiException.Rule("validation", "title is required")))
        // A code from a newer server still says something rather than nothing.
        assertEquals("brand new", describeError(ApiException.Rule("brand_new_code", "brand new")))
    }
}
