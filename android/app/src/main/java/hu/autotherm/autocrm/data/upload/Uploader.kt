package hu.autotherm.autocrm.data.upload

import android.util.Base64
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.UploadRequest
import hu.autotherm.autocrm.data.api.UploadTarget
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.db.PendingUploadDao
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.Request
import okhttp3.RequestBody.Companion.asRequestBody
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import java.io.File
import java.io.IOException
import java.security.MessageDigest

/**
 * The three-step upload, as the API defines it:
 *
 *   1. `POST /orders/{id}/uploads` → a ticket and a presigned PUT, or "already uploaded".
 *   2. `PUT` the bytes straight to object storage. They never pass through the API server,
 *      which is what makes a 120-photo batch from a phone survivable.
 *   3. `POST /uploads/complete` with the ticket. The server HEADs the object, checks size
 *      and hash, and only then writes the row.
 *
 * Every step is safe to repeat. Step 1 answers `already_uploaded` for bytes it has seen,
 * step 2 is a PUT to a content-addressed key, and step 3 is idempotent on
 * `(order_id, content_hash)`. That is what lets this retry for a week without duplicating a
 * single photo — and it is why the queue never has to decide whether something "probably"
 * got through.
 */
class Uploader(
    private val api: AutoCrmApi,
    private val dao: PendingUploadDao,
) {
    /** What happened to one photo, for the worker to decide whether to keep going. */
    sealed class Outcome {
        data object Uploaded : Outcome()
        /** Will not succeed on its own; the fitter has to look at it. */
        data class Blocked(val reason: String) : Outcome()
        /** Worth another go later. */
        data class Retry(val reason: String) : Outcome()
    }

    suspend fun upload(row: PendingUpload): Outcome {
        val file = File(row.filePath)
        if (!file.exists()) {
            // The bytes are gone — cleared storage, a manual wipe. There is nothing left to
            // send, and pretending otherwise would keep a dead row retrying forever.
            return Outcome.Blocked("a fájl már nincs meg a telefonon")
        }

        return try {
            dao.setState(row.id, PendingUpload.STATE_UPLOADING)

            // Reuse a ticket that is still comfortably valid rather than asking for another:
            // a resumed upload should not re-sign bytes that are already in the store.
            val ticket = if (row.isTicketUsable) {
                row.ticket!!
            } else {
                val response = api.requestUpload(
                    orderId = row.orderId,
                    request = UploadRequest(
                        target = UploadTarget(type = "image", category = row.category),
                        filename = row.filename,
                        contentType = row.contentType,
                        byteSize = row.byteSize,
                        sha256 = row.sha256,
                    ),
                )
                when (response.status) {
                    "already_uploaded" -> {
                        // The server already has these exact bytes on this order. This is
                        // the normal answer when a batch is retried after a partial success.
                        dao.markDone(row.id, response.imageId)
                        return Outcome.Uploaded
                    }
                    "upload" -> {
                        val presigned = response.upload
                            ?: return Outcome.Retry("a szerver nem adott feltöltési címet")
                        putBytes(presigned.url, presigned.headers, file, row.contentType)
                        val expiry = response.expiresAt?.let(::parseInstantMillis)
                        val issued = response.ticket
                            ?: return Outcome.Retry("a szerver nem adott jegyet")
                        dao.setTicket(row.id, issued, expiry)
                        issued
                    }
                    else -> return Outcome.Retry("ismeretlen válasz: ${response.status}")
                }
            }

            val completed = api.completeUpload(ticket)
            dao.markDone(row.id, completed.image?.id)
            Outcome.Uploaded
        } catch (e: ApiException) {
            if (e.isRetryable) Outcome.Retry(e.message ?: "hálózati hiba")
            else Outcome.Blocked(describe(e))
        } catch (e: IOException) {
            Outcome.Retry(e.message ?: "hálózati hiba")
        }
    }

    /**
     * The PUT goes to object storage, not to the API, and every header in [headers] is part
     * of the signature — including the sha256 checksum and, for intake photos, the object
     * lock. Adding or dropping one turns the request into a 403.
     */
    private suspend fun putBytes(
        url: String,
        headers: Map<String, String>,
        file: File,
        contentType: String,
    ) = withContext(Dispatchers.IO) {
        val builder = Request.Builder().url(url)
        headers.forEach { (name, value) -> builder.header(name, value) }
        val response = try {
            api.http.newCall(builder.put(file.asRequestBody(contentType.toMediaTypeOrNull())).build()).execute()
        } catch (e: IOException) {
            throw ApiException.Network(e)
        }
        response.use {
            if (!it.isSuccessful) {
                val detail = it.body?.string()?.take(300).orEmpty()
                // A presigned URL that has expired answers 403. That is retryable: the next
                // attempt asks for a fresh ticket, because markRetryable clears this one.
                throw if (it.code in 500..599 || it.code == 403) {
                    ApiException.Server(it.code, detail)
                } else {
                    ApiException.Rule("upload_rejected", "tárhely: ${it.code} $detail")
                }
            }
        }
    }

    private fun describe(e: ApiException): String = when (e) {
        is ApiException.Unauthenticated -> "a munkamenet lejárt, jelentkezz be újra"
        is ApiException.Forbidden -> "nincs jogosultság a feltöltéshez"
        is ApiException.NotFound -> "a megrendelés már nem létezik"
        is ApiException.Rule -> e.detail ?: e.code
        else -> e.message ?: "ismeretlen hiba"
    }

    companion object {
        /** RFC 3339 from the API. Unparseable means "treat the ticket as already expired". */
        fun parseInstantMillis(value: String): Long? = runCatching {
            java.time.Instant.parse(value).toEpochMilli()
        }.getOrNull()
    }
}

/**
 * sha256 of a file, lowercase hex, computed in 64 KB blocks.
 *
 * The hash is taken once at capture and never recomputed: it identifies the photo to the
 * server, binds the upload ticket, and makes a repeated batch idempotent. Streaming rather
 * than reading the file into memory matters — a 12 MP JPEG is several megabytes and the
 * phones on this shop floor are not new.
 */
fun File.sha256Hex(): String {
    val digest = MessageDigest.getInstance("SHA-256")
    inputStream().use { stream ->
        val buffer = ByteArray(64 * 1024)
        while (true) {
            val read = stream.read(buffer)
            if (read <= 0) break
            digest.update(buffer, 0, read)
        }
    }
    return digest.digest().joinToString("") { "%02x".format(it) }
}

/** The same digest base64-encoded, the form S3 expects in `x-amz-checksum-sha256`. */
fun String.hexToBase64(): String =
    Base64.encodeToString(chunked(2).map { it.toInt(16).toByte() }.toByteArray(), Base64.NO_WRAP)
