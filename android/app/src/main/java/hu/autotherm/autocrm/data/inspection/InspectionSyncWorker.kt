package hu.autotherm.autocrm.data.inspection

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.data.db.InspectionDraft
import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json

/**
 * Pushes handover-inspection drafts to the server, oldest first.
 *
 * Network-constrained like the photo queue: car parks have poor signal, and every
 * step is idempotent, so a dead battery mid-sync resumes cleanly. Failures that
 * need a human (validation, conflicts) are recorded on the draft and shown in
 * the inspection list; anything else comes back as a retry.
 */
class InspectionSyncWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val app = applicationContext as AutoCrmApp
        app.sessionStore.token() ?: return Result.success()

        val dao = app.database.inspectionDrafts()
        var retryLater = false
        for (draft in dao.pendingSync()) {
            when (syncDraft(app, draft.localUuid)) {
                is SyncResult.Done -> Unit
                is SyncResult.Retry -> retryLater = true
                is SyncResult.Failed -> Unit
            }
            if (isStopped) break
        }
        // Drafts record their own errors; only connectivity brings us back.
        return if (retryLater) Result.retry() else Result.success()
    }

    companion object {
        private const val UNIQUE_WORK = "autocrm-inspection-sync"

        /** After every draft mutation, at app start, and from the "sync now" button. */
        fun enqueue(context: Context) {
            val request = OneTimeWorkRequestBuilder<InspectionSyncWorker>()
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .build(),
                )
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build()
            WorkManager.getInstance(context)
                .enqueueUniqueWork(UNIQUE_WORK, ExistingWorkPolicy.KEEP, request)
        }

        fun enqueueNow(context: Context) {
            val request = OneTimeWorkRequestBuilder<InspectionSyncWorker>()
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .build(),
                )
                .build()
            WorkManager.getInstance(context)
                .enqueueUniqueWork(UNIQUE_WORK, ExistingWorkPolicy.REPLACE, request)
        }

        /** Drafts that will never sync: remove with their photo files and queue rows. */
        suspend fun discard(app: AutoCrmApp, uuid: String) {
            val dao = app.database.inspectionDrafts()
            val draft = dao.byUuid(uuid)
            dao.delete(uuid)
            if (draft != null) {
                val payload = runCatching {
                    Json.decodeFromString(
                        DraftPayload.serializer(),
                        draft.payloadJson,
                    )
                }.getOrNull()
                val uploads = app.database.pendingUploads()
                for (photo in payload?.photos.orEmpty()) {
                    uploads.byContent(draft.orderId, photo.sha256)?.let { row ->
                        uploads.delete(row.id)
                        java.io.File(row.filePath).delete()
                    }
                }
            }
            inspectionDir(app, uuid).deleteRecursively()
        }
    }
}
