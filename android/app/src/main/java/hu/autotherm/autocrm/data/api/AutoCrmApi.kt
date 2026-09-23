package hu.autotherm.autocrm.data.api

import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.prefs.ServerStore
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
    private val serverStore: ServerStore,
    private val sessionStore: SessionStore,
    val http: OkHttpClient = defaultClient(),
) {
    private val json = Json {
        ignoreUnknownKeys = true // the server may add fields; the phone must not break.
        explicitNulls = false // omit nulls rather than sending them: PATCH treats null as "clear".
        // Belt and braces after the login bug: a value the code states must reach the wire,
        // even when it happens to equal a Kotlin default.
        encodeDefaults = true
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

    /**
     * Suspend because the server address is a stored setting rather than a build constant:
     * the same APK follows the phone from the workshop Wi-Fi to the office. DataStore keeps
     * the value in memory after the first read, so this is not a disk hit per request.
     */
    private suspend fun url(path: String): HttpUrl.Builder =
        (serverStore.require() + "/api" + path).toHttpUrl().newBuilder()

    /**
     * Is anything answering at [candidate]? Used by the setup screen, before any address is
     * saved, so it takes the URL rather than reading the store. `/health` sits outside
     * `/api` and needs no session, which makes it the one call that can prove reachability
     * without a login.
     */
    suspend fun probe(candidate: String): Boolean = withContext(Dispatchers.IO) {
        try {
            http.newCall(Request.Builder().url("$candidate/health").get().build())
                .execute()
                .use { it.isSuccessful }
        } catch (e: IOException) {
            false
        }
    }

    /** A finished response: status and body, with the connection already closed. */
    private data class Raw(val code: Int, val body: String)

    /**
     * Performs the call **and reads the body**, both on [Dispatchers.IO].
     *
     * Reading the body is what makes that one function rather than two. `Response.body.string()`
     * streams from the socket, so it is network I/O as much as the call is; returning an
     * open Response from a withContext block and reading it at the call site put that read
     * back on whatever dispatcher the caller was using — the main thread, by way of
     * viewModelScope — and every screen died with NetworkOnMainThreadException.
     */
    private suspend fun execute(request: Request.Builder): Raw = withContext(Dispatchers.IO) {
        val token = sessionStore.token()
        if (token != null) request.header("Authorization", "Bearer $token")
        request.header("Accept", "application/json")
        try {
            http.newCall(request.build()).execute().use { response ->
                Raw(response.code, response.body?.string().orEmpty())
            }
        } catch (e: IOException) {
            throw ApiException.Network(e)
        }
    }

    /** Maps the response to [T], or throws. */
    private suspend fun <T> send(
        request: Request.Builder,
        serializer: KSerializer<T>,
    ): T {
        val raw = execute(request)
        if (raw.code !in 200..299) throw errorFor(raw.code, raw.body)
        return try {
            json.decodeFromString(serializer, raw.body)
        } catch (e: Exception) {
            // A body that does not parse is a contract break, not a user error. Reported as
            // a server fault so the queue retries rather than discarding work.
            throw ApiException.Server(raw.code, "unparseable response: ${e.message}")
        }
    }

    private suspend fun sendNoContent(request: Request.Builder) {
        val raw = execute(request)
        if (raw.code !in 200..299) throw errorFor(raw.code, raw.body)
    }

    private fun errorFor(status: Int, body: String): ApiException {
        val parsed = runCatching { json.decodeFromString(ApiErrorBody.serializer(), body) }.getOrNull()
        val code = parsed?.error?.code ?: "validation"
        val message = parsed?.error?.message
        return when (status) {
            401 -> ApiException.Unauthenticated()
            403 -> ApiException.Forbidden()
            404 -> ApiException.NotFound(message ?: "record")
            // 400 carries the same envelope as 409/422 (backend/src/error.rs):
            // a validation refusal, not a server fault. Treating it as Server
            // hid field errors behind "server error" retries.
            400, 409, 422 -> ApiException.Rule(code, message)
            else -> ApiException.Server(status, message ?: body.take(300))
        }
    }

    private inline fun <reified T> body(value: T): RequestBody =
        json.encodeToString(serializer<T>(), value).toRequestBody(jsonMedia)

    // ── Auth ────────────────────────────────────────────────────────────────────────

    suspend fun login(email: String, password: String, deviceLabel: String): LoginResponse =
        send(
            Request.Builder().url(url("/auth/login").build())
                .post(
                    body(
                        LoginBody(
                            email = email,
                            password = password,
                            client = "mobile",
                            deviceLabel = deviceLabel,
                        ),
                    ),
                ),
            LoginResponse.serializer(),
        )

    suspend fun me(): MeResponse =
        send(Request.Builder().url(url("/auth/me").build()).get(), MeResponse.serializer())

    suspend fun changePassword(currentPassword: String, newPassword: String) =
        sendNoContent(
            Request.Builder().url(url("/auth/password").build())
                .post(body(ChangePasswordBody(currentPassword, newPassword))),
        )

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

    suspend fun createOrder(body: OrderBody): Order =
        send(
            Request.Builder().url(url("/orders").build()).post(body(body)),
            Order.serializer(),
        )

    suspend fun patchOrder(id: Long, body: OrderBody): Order =
        send(
            Request.Builder().url(url("/orders/$id").build()).patch(body(body)),
            Order.serializer(),
        )

    suspend fun orderItems(id: Long): List<ItemView> =
        send(
            Request.Builder().url(url("/orders/$id/items").build()).get(),
            Items.serializer(ItemView.serializer()),
        ).items

    suspend fun addItem(orderId: Long, body: AddItemBody): ItemView =
        send(
            Request.Builder().url(url("/orders/$orderId/items").build()).post(body(body)),
            ItemView.serializer(),
        )

    suspend fun deleteItem(id: Long) =
        sendNoContent(Request.Builder().url(url("/order-items/$id").build()).delete())

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

    suspend fun createBlocker(orderId: Long, body: BlockerBody): Blocker =
        send(
            Request.Builder().url(url("/orders/$orderId/blockers").build()).post(body(body)),
            Blocker.serializer(),
        )

    suspend fun resolveBlocker(id: Long, note: String?): Blocker =
        send(
            Request.Builder().url(url("/blockers/$id/resolve").build())
                .post(body(ResolveBody(note))),
            Blocker.serializer(),
        )

    suspend fun reopenBlocker(id: Long): Blocker =
        send(
            Request.Builder().url(url("/blockers/$id/reopen").build()).post(EMPTY),
            Blocker.serializer(),
        )

    // ── Tasks ───────────────────────────────────────────────────────────────────────

    suspend fun tasksMine(): List<Task> =
        send(
            Request.Builder().url(url("/tasks").build()).get(),
            Items.serializer(Task.serializer()),
        ).items

    suspend fun tasksFor(entity: String, id: Long): List<Task> =
        send(
            Request.Builder().url(url("/tasks/for/$entity/$id").build()).get(),
            Items.serializer(Task.serializer()),
        ).items

    suspend fun createTask(body: TaskBody): Task =
        send(
            Request.Builder().url(url("/tasks").build()).post(body(body)),
            Task.serializer(),
        )

    suspend fun setTaskDone(id: Long, done: Boolean): Task =
        send(
            Request.Builder().url(url("/tasks/$id/done").build())
                .post(body(DoneBody(done))),
            Task.serializer(),
        )

    suspend fun deleteTask(id: Long) =
        sendNoContent(Request.Builder().url(url("/tasks/$id").build()).delete())

    // ── Reports ─────────────────────────────────────────────────────────────────────

    suspend fun workload(from: String, to: String): WorkloadReport {
        val u = url("/reports/workload")
            .addQueryParameter("from", from)
            .addQueryParameter("to", to)
        return send(Request.Builder().url(u.build()).get(), WorkloadReport.serializer())
    }

    suspend fun stalled(): List<StalledOrder> =
        send(
            Request.Builder().url(url("/reports/stalled").build()).get(),
            Items.serializer(StalledOrder.serializer()),
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

    // ── Email ───────────────────────────────────────────────────────────────────────

    suspend fun emails(limit: Int = 50): List<EmailSummary> {
        val u = url("/emails").addQueryParameter("limit", limit.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(EmailSummary.serializer())).items
    }

    suspend fun email(id: Long): EmailMessage =
        send(Request.Builder().url(url("/emails/$id").build()).get(), EmailMessage.serializer())

    suspend fun sendEmail(body: ComposeBody): EmailMessage =
        send(
            Request.Builder().url(url("/emails").build()).post(body(body)),
            EmailMessage.serializer(),
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

    suspend fun createPartner(body: PartnerBody): Partner =
        send(
            Request.Builder().url(url("/partners").build()).post(body(body)),
            Partner.serializer(),
        )

    suspend fun patchPartner(id: Long, body: PartnerBody): Partner =
        send(
            Request.Builder().url(url("/partners/$id").build()).patch(body(body)),
            Partner.serializer(),
        )

    suspend fun createContact(partnerId: Long, body: ContactBody): Contact =
        send(
            Request.Builder().url(url("/partners/$partnerId/contacts").build()).post(body(body)),
            Contact.serializer(),
        )

    suspend fun patchContact(id: Long, body: ContactBody): Contact =
        send(
            Request.Builder().url(url("/contacts/$id").build()).patch(body(body)),
            Contact.serializer(),
        )

    suspend fun leads(query: String? = null, openOnly: Boolean = false, limit: Int = 50): List<LeadSummary> {
        val u = url("/leads")
        if (!query.isNullOrBlank()) u.addQueryParameter("q", query)
        if (openOnly) u.addQueryParameter("open", "true")
        u.addQueryParameter("limit", limit.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(LeadSummary.serializer())).items
    }

    suspend fun lead(id: Long): LeadDetail =
        send(Request.Builder().url(url("/leads/$id").build()).get(), LeadDetail.serializer())

    suspend fun createLead(body: LeadBody): Lead =
        send(
            Request.Builder().url(url("/leads").build()).post(body(body)),
            Lead.serializer(),
        )

    suspend fun patchLead(id: Long, body: LeadBody): Lead =
        send(
            Request.Builder().url(url("/leads/$id").build()).patch(body(body)),
            Lead.serializer(),
        )

    suspend fun leadTransitions(id: Long): List<TransitionOption> =
        send(
            Request.Builder().url(url("/leads/$id/transitions").build()).get(),
            Items.serializer(TransitionOption.serializer()),
        ).items

    suspend fun changeLeadStage(id: Long, stage: String, note: String?) =
        sendNoContent(
            Request.Builder().url(url("/leads/$id/stage").build())
                .post(body(StageBody(stage = stage, note = note))),
        )

    suspend fun convertLead(id: Long, body: OrderBody): Order =
        send(
            Request.Builder().url(url("/leads/$id/convert").build()).post(body(body)),
            Order.serializer(),
        )

    // ── Handover inspections ────────────────────────────────────────────────────

    suspend fun inspections(orderId: Long): List<Inspection> {
        val u = url("/inspections").addQueryParameter("order_id", orderId.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(Inspection.serializer())).items
    }

    suspend fun createInspection(body: InspectionBody): Inspection =
        send(
            Request.Builder().url(url("/inspections").build()).post(body(body)),
            Inspection.serializer(),
        )

    suspend fun inspection(id: Long): InspectionDetail =
        send(Request.Builder().url(url("/inspections/$id").build()).get(), InspectionDetail.serializer())

    suspend fun patchInspection(id: Long, body: InspectionBody): Inspection =
        send(
            Request.Builder().url(url("/inspections/$id").build()).patch(body(body)),
            Inspection.serializer(),
        )

    suspend fun deleteInspection(id: Long) =
        sendNoContent(Request.Builder().url(url("/inspections/$id").build()).delete())

    suspend fun attachInspectionPhoto(id: Long, body: AttachPhotoBody): InspectionPhoto =
        send(
            Request.Builder().url(url("/inspections/$id/photos").build()).post(body(body)),
            InspectionPhoto.serializer(),
        )

    suspend fun addInspectionDamage(id: Long, body: DamageBody): InspectionDamage =
        send(
            Request.Builder().url(url("/inspections/$id/damages").build()).post(body(body)),
            InspectionDamage.serializer(),
        )

    suspend fun deleteInspectionDamage(id: Long, damageId: Long) =
        sendNoContent(Request.Builder().url(url("/inspections/$id/damages/$damageId").build()).delete())

    suspend fun addInspectionSignature(id: Long, body: SignatureBody): InspectionSignature =
        send(
            Request.Builder().url(url("/inspections/$id/signatures").build()).post(body(body)),
            InspectionSignature.serializer(),
        )

    suspend fun signInspection(id: Long, customerComment: String?): Inspection =
        send(
            Request.Builder().url(url("/inspections/$id/sign").build())
                .post(body(SignBody(customerComment))),
            Inspection.serializer(),
        )

    suspend fun addInspectionNote(id: Long, body: String): InspectionNote =
        send(
            Request.Builder().url(url("/inspections/$id/notes").build())
                .post(body(InspectionNoteBody(body))),
            InspectionNote.serializer(),
        )

    suspend fun inspectionComparison(id: Long): Comparison =
        send(
            Request.Builder().url(url("/inspections/$id/comparison").build()).get(),
            Comparison.serializer(),
        )

    suspend fun setInspectionVerdict(id: Long, body: VerdictBody): InspectionVerdict =
        send(
            Request.Builder().url(url("/inspections/$id/verdicts").build()).post(body(body)),
            InspectionVerdict.serializer(),
        )

    suspend fun zoneTemplates(projectTypeId: Long?): List<ZoneTemplate> {
        val u = url("/inspections/templates")
        if (projectTypeId != null) u.addQueryParameter("project_type_id", projectTypeId.toString())
        return send(Request.Builder().url(u.build()).get(), Items.serializer(ZoneTemplate.serializer())).items
    }

    /** Step 1 of a signature upload: a finger-drawn PNG travels as an `other` document. */
    suspend fun requestDocumentUpload(
        orderId: Long,
        filename: String,
        contentType: String,
        byteSize: Long,
        sha256: String,
    ): UploadResponse =
        send(
            Request.Builder().url(url("/orders/$orderId/uploads").build()).post(
                body(
                    UploadRequest(
                        target = UploadTarget(type = "document", kind = "other"),
                        filename = filename,
                        contentType = contentType,
                        byteSize = byteSize,
                        sha256 = sha256,
                    ),
                ),
            ),
            UploadResponse.serializer(),
        )

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
