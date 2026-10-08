package hu.autotherm.autocrm.data.api

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

/**
 * A PATCH body that says "clear" out loud. The shared encoder omits nulls, and the server
 * reads an omitted field as "leave it" — so a phone number deleted in a form came back on
 * the next load. Here a null value is written as JSON `null`, which the server's
 * `patch_field` reads as "clear". Name only the fields the form shows: anything listed is
 * overwritten, anything left out is kept.
 */
fun patchOf(vararg fields: Pair<String, Any?>): JsonObject = buildJsonObject {
    for ((key, value) in fields) {
        put(
            key,
            when (value) {
                null -> JsonNull
                is JsonElement -> value
                is String -> JsonPrimitive(value)
                is Number -> JsonPrimitive(value)
                is Boolean -> JsonPrimitive(value)
                else -> throw IllegalArgumentException("patchOf: unsupported value for $key")
            },
        )
    }
}
