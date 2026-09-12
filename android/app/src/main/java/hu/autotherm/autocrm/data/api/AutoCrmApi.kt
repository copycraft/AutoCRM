package hu.autotherm.autocrm.data.api

import hu.autotherm.autocrm.data.auth.SessionStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.KSerializer
import kotlinx.serialization.serializer
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.IOException
import java.util.concurrent.TimeUnit

/**
 * The whole API surface the phone uses, as plain suspend functions over OkHttp.
 *
 * No Retrofit: the client speaks about twenty endpoints, and a code-generating proxy layer
 * would be more machinery than that justifies — the same reasoning that keeps the backend
 * on plain views instead of materialized ones.
 *
 * Two rules hold everywhere here:
 *
 *  - The bearer token is attached by an interceptor, never by a call site, so no endpoint
 *    can be added that forgets it.
 *  - A non-2xx response becomes an [ApiException] with the API's own error code. Nothing
 *    returns a null that a screen has to interpret.
 */
class AutoCrmApi(
    private val baseUrl: String,
    private val sessionStore: SessionStore,
    val http: OkHttpClient = defaultClient(),
) {
    private val json = Json {
        ignoreUnknownKeys = true // the server may add fields; the phone must not break.
        explicitNulls = false // omit nulls rather than sending them: PATCH treats null as "clear".
    }
    private val jsonMedia = "application/json; charset=utf-8".toMediaType()

    companion object {
        /**
         * Timeouts sized for a van in a yard, not a desk. A 10-second connect timeout on a
         * weak signal produces a spurious failure and a photo the fitter believes is lost;
         * the queue would retry it, but the red badge is what they remember.
         */
        fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(60, TimeUnit.SECONDS)
            // Photo bodies are several megabytes over a bad uplink.
            .writeTimeout(5, TimeUnit.MINUTES)
            .retryOnConnectionFailure(true)
            .build()
    }

    private fun url(path: String): HttpUrl.Builder = (baseUrl.trimEnd('/') + "/api" + path).toHttpUrl().newBuilder()

    private suspend fun execute(request: Request.Builder): Response = withContext(Dispatchers.IO) {
        val token = sessionStore.token()
        if (token != null) request.header("Authorization", "Bearer $token")
        request.header("Accept", "application/json")
        try {
            http.newCall(request.build()).execute()
        } catch (e: IOException) {
            throw ApiException.Network(e)
        }
    }

    /** Maps the response to [T], or throws. Closes the body in every path. */
    private suspend fun <T> send(
        request: Request.Builder,
        serializer: KSerializer<T>,
    ): T = execute(request).use { response ->
        val body = response.body?.string().orEmpty()
        if (!response.isSuccessful) throw errorFor(response.code, body)
        try {
            json.decodeFromString(serializer, body)
        } catch (e: Exception) {
            // A body that does not parse is a contract break, not a user error. Reported as
            // a server fault so the queue retries rather than discarding work.
            throw ApiException.Server(response.code, "unparseable response: ${e.message}")
        }
    }

    private suspend fun sendNoContent(request: Request.Builder) {
        execute(request).use { response ->
            val body = response.body?.string().orEmpty()
            if (!response.isSuccessful) throw errorFor(response.code, body)
        }
    }

    private fun errorFor(status: Int, body: String): ApiException {
        val parsed = runCatching { json.decodeFromString(ApiErrorBody.serializer(), body) }.getOrNull()
        return when (status) {
            401 -> ApiException.Unauthenticated()
            403 -> ApiException.Forbidden()
            404 -> ApiException.NotFound(parsed?.message ?: "record")
            409, 422 -> ApiException.Rule(parsed?.error ?: "validation", parsed?.message)
            else -> ApiException.Server(status, parsed?.message ?: body.take(300))
        }
    }

    private inline fun <reified T> body(value: T): RequestBody =
        json.encodeToString(serializer<T>(), value).toRequestBody(jsonMedia)

    // ── Auth ────────────────────────────────────────────────────────────────────────

    suspend fun login(email: String, password: String, deviceLabel: String): LoginResponse =
        send(
            Request.Builder().url(url("/auth/login").build())
                .post(body(LoginBody(email = email, password = password, deviceLabel = deviceLabel))),
            LoginResponse.serializer(),
        )

    suspend fun me(): MeResponse =
        send(Request.Builder().url(url("/auth/me").build()).get(), MeResponse.serializer())

    suspend fun logout() =
        sendNoContent(Request.Builder().url(url("/auth/logout").build()).post(EMPTY))

    // ── Orders ──────────────────────────────────────────────────────────────────────

    /**
     * The compact picker: open orders only unless [all], plate-searchable, capped at 200 by
     * the server and cached for 60 seconds. This is what the capture screen opens on.
     */
    suspend fun pickerOrders(query: String? = null, all: Boolean = false): List<PickerOrder> {
        val u = url("/mobile/orders")
        if (!query.isNullOrBlank()) u.addQueryParameter("q", query)
        if (all) u.addQueryParameter("all", "true")
        return send(Request.Builder().url(u.build()).get(), Items.serializer(PickerOrder.serializer())).items
    }

    suspend fun orders(
        query: String? = null,
        stage: String? = null,
        openOnly: Boolean = false,
        limit: Int = 50,
        offset: Int = 0,
    ): List<OrderSummary> {
        val u = url("/orders")
        if (!query.isNullOrBlank()) u.addQueryParameter("q", query)
        if (!stage.isNullOrBlank()) u.addQueryParameter("stage", stage)
        if (openOnly) u.addQueryParameter("open", "true")
        u.addQueryParameter("limit", limit.toString())
        u.addQueryParameter("offset", offset.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(OrderSummary.serializer())).items
    }

    suspend fun order(id: Long): OrderDetail =
        send(Request.Builder().url(url("/orders/$id").build()).get(), OrderDetail.serializer())

    suspend fun orderStages(id: Long): List<StageEntry> =
        send(
            Request.Builder().url(url("/orders/$id/stages").build()).get(),
            Items.serializer(StageEntry.serializer()),
        ).items

    suspend fun orderTransitions(id: Long): List<TransitionOption> =
        send(
            Request.Builder().url(url("/orders/$id/transitions").build()).get(),
            Items.serializer(TransitionOption.serializer()),
        ).items

    suspend fun changeOrderStage(id: Long, stage: String, note: String?) =
        sendNoContent(
            Request.Builder().url(url("/orders/$id/stage").build())
                .post(body(StageBody(stage = stage, note = note))),
        )

    suspend fun orderNotes(id: Long): List<OrderNote> =
        send(
            Request.Builder().url(url("/orders/$id/notes").build()).get(),
            Items.serializer(OrderNote.serializer()),
        ).items

    // ── Blockers ────────────────────────────────────────────────────────────────────

    suspend fun openBlockers(): List<Blocker> =
        send(Request.Builder().url(url("/blockers").build()).get(), Items.serializer(Blocker.serializer())).items

    suspend fun orderBlockers(orderId: Long): List<Blocker> =
        send(
            Request.Builder().url(url("/orders/$orderId/blockers").build()).get(),
            Items.serializer(Blocker.serializer()),
        ).items

    // ── Media ───────────────────────────────────────────────────────────────────────

    suspend fun images(orderId: Long, category: String? = null): List<ImageView> {
        val u = url("/orders/$orderId/images")
        if (!category.isNullOrBlank()) u.addQueryParameter("category", category)
        return send(Request.Builder().url(u.build()).get(), Items.serializer(ImageView.serializer())).items
    }

    /** Step 1 of an upload: ask for a ticket and a presigned PUT. */
    suspend fun requestUpload(orderId: Long, request: UploadRequest): UploadResponse =
        send(
            Request.Builder().url(url("/orders/$orderId/uploads").build()).post(body(request)),
            UploadResponse.serializer(),
        )

    /** Step 3: hand the ticket back so the server verifies size and hash and writes the row. */
    suspend fun completeUpload(ticket: String): Completed =
        send(
            Request.Builder().url(url("/uploads/complete").build()).post(body(CompleteBody(ticket))),
            Completed.serializer(),
        )

    // ── Partners and leads ──────────────────────────────────────────────────────────

    suspend fun partners(query: String? = null, role: String? = null, limit: Int = 50): List<Partner> {
        val u = url("/partners")
        if (!query.isNullOrBlank()) u.addQueryParameter("q", query)
        if (!role.isNullOrBlank()) u.addQueryParameter("role", role)
        u.addQueryParameter("limit", limit.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(Partner.serializer())).items
    }

    suspend fun partner(id: Long): PartnerDetail =
        send(Request.Builder().url(url("/partners/$id").build()).get(), PartnerDetail.serializer())

    suspend fun leads(query: String? = null, openOnly: Boolean = false, limit: Int = 50): List<LeadSummary> {
        val u = url("/leads")
        if (!query.isNullOrBlank()) u.addQueryParameter("q", query)
        if (openOnly) u.addQueryParameter("open", "true")
        u.addQueryParameter("limit", limit.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(LeadSummary.serializer())).items
    }

    suspend fun lead(id: Long): LeadDetail =
        send(Request.Builder().url(url("/leads/$id").build()).get(), LeadDetail.serializer())

    // ── Configuration ───────────────────────────────────────────────────────────────

    suspend fun stageDefinitions(entity: String): List<StageDefinition> {
        val u = url("/stage-definitions").addQueryParameter("entity", entity)
        return send(Request.Builder().url(u.build()).get(), Items.serializer(StageDefinition.serializer())).items
    }

    suspend fun projectTypes(): List<ProjectType> =
        send(
            Request.Builder().url(url("/project-types").build()).get(),
            Items.serializer(ProjectType.serializer()),
        ).items
}

private val EMPTY: RequestBody = ByteArray(0).toRequestBody(null, 0, 0)
