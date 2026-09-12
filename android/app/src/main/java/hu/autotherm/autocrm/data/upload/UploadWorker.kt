package hu.autotherm.autocrm.data.upload

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
import hu.autotherm.autocrm.data.db.PendingUpload
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Drains the queue whenever there is a network.
 *
 * WorkManager rather than a service or a coroutine tied to a screen: the fitter photographs
 * a van, locks the phone and walks away, and the upload has to survive that, plus a reboot,
 * plus Doze. A coroutine in a ViewModel dies with the screen; this does not.
 *
 * The worker is deliberately dumb about ordering and cleverness — it takes the oldest due
 * rows and tries them one at a time. Parallel uploads from a phone on a workshop Wi-Fi make
 * every one of them slower and the failures harder to read.
 */
class UploadWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val app = applicationContext as AutoCrmApp
        // Without a session there is nobody to upload as. Not a failure: the rows stay, and
        // signing in enqueues this again.
        app.sessionStore.token() ?: return Result.success()

        val dao = app.database.pendingUploads()
        val uploader = Uploader(app.api, dao)

        var retryLater = false
        while (true) {
            val due = dao.dueForUpload(System.currentTimeMillis())
            if (due.isEmpty()) break

            for (row in due) {
                when (val outcome = uploader.upload(row)) {
                    is Uploader.Outcome.Uploaded -> Unit
                    is Uploader.Outcome.Blocked -> dao.markBlocked(row.id, outcome.reason)
                    is Uploader.Outcome.Retry -> {
                        val attempts = row.attempts + 1
                        dao.markRetryable(
                            id = row.id,
                            error = outcome.reason,
                            nextAttemptAt = System.currentTimeMillis() + PendingUpload.backoffFor(attempts),
                        )
                        retryLater = true
                    }
                }
            }
            // A pass that only produced retries would otherwise spin: stop and let
            // WorkManager's own backoff bring us back.
            if (retryLater) break
        }

        // Files of confirmed uploads are removed here rather than at completion, so the
        // queue screen can show a batch finishing before the evidence disappears from it.
        dao.sweepCompleted { path -> File(path).delete() }

        return if (retryLater) Result.retry() else Result.success()
    }

    companion object {
        private const val UNIQUE_WORK = "autocrm-upload-queue"

        /**
         * Enqueued after every capture and at app start. `KEEP` rather than `REPLACE`: a
         * burst of twenty photos must not cancel and restart the drain twenty times.
         */
        fun enqueue(context: Context) {
            val request = OneTimeWorkRequestBuilder<UploadWorker>()
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

        /** "Try again now" from the queue screen: replaces whatever is scheduled. */
        fun enqueueNow(context: Context) {
            val request = OneTimeWorkRequestBuilder<UploadWorker>()
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .build(),
                )
                .build()
            WorkManager.getInstance(context)
                .enqueueUniqueWork(UNIQUE_WORK, ExistingWorkPolicy.REPLACE, request)
        }
    }
}
