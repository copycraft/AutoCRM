package hu.autotherm.autocrm.data.upload

import android.content.Context
import android.net.Uri
import androidx.core.content.FileProvider
import hu.autotherm.autocrm.data.db.PendingUpload
import hu.autotherm.autocrm.data.db.PendingUploadDao
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.withContext
import java.io.File

/**
 * The front door to the queue: everything that captures a photo goes through [enqueue], and
 * everything that displays the queue reads the flows here.
 *
 * The ordering inside [enqueue] is the point. The file is moved into app-private storage
 * and the row is written **before** the function returns, so a crash on the next line loses
 * nothing. Only then is the worker poked.
 */
class UploadQueue(
    private val context: Context,
    private val dao: PendingUploadDao,
) {
    /** Where queued photos live until the server confirms them. */
    private val pendingDir: File
        get() = File(context.filesDir, "pending").apply { mkdirs() }

    val outstanding: Flow<Int> = dao.outstandingCount()
    val blocked: Flow<Int> = dao.blockedCount()
    val queue: Flow<List<PendingUpload>> = dao.watchQueue()

    fun forOrder(orderId: Long): Flow<List<PendingUpload>> = dao.watchForOrder(orderId)

    /** What [enqueue] did, so the capture screen can say something truthful. */
    sealed class Enqueued {
        data class Queued(val id: Long) : Enqueued()
        /** These exact bytes are already queued for this order — the same photo picked twice. */
        data object Duplicate : Enqueued()
        /** The picked item could not be read: a cloud-only photo, or a revoked permission. */
        data object Unreadable : Enqueued()
    }

    /**
     * Takes ownership of [captured] and queues it.
     *
     * [captured] is moved, not copied: the camera writes straight into app-private storage,
     * so there is no window in which the only copy sits in a public directory the fitter
     * could clear from the gallery.
     */
    suspend fun enqueue(
        captured: File,
        orderId: Long,
        orderNumber: String,
        category: String,
        contentType: String = "image/jpeg",
    ): Enqueued = withContext(Dispatchers.IO) {
        val sha = captured.sha256Hex()

        // Compute the hash before the move so a duplicate never displaces the queued file.
        dao.byContent(orderId, sha)?.let {
            captured.delete()
            return@withContext Enqueued.Duplicate
        }

        // Keyed by order as well as content: the same photo queued for two orders must
        // not share one file, or completing the first row's sweep deletes the second
        // row's bytes out from under it. Rows already queued keep their stored path.
        val destination = File(pendingDir, "$orderId-$sha.jpg")
        if (captured.absolutePath != destination.absolutePath) {
            if (!captured.renameTo(destination)) {
                captured.copyTo(destination, overwrite = true)
                captured.delete()
            }
        }

        val row = PendingUpload(
            orderId = orderId,
            orderNumber = orderNumber,
            category = category,
            filePath = destination.absolutePath,
            contentType = contentType,
            byteSize = destination.length(),
            sha256 = sha,
            filename = "${orderNumber}-${category}-${sha.take(8)}.jpg",
        )
        val id = dao.insert(row)
        UploadWorker.enqueue(context)
        if (id == -1L) Enqueued.Duplicate else Enqueued.Queued(id)
    }

    suspend fun retryAll() {
        dao.retryAll()
        UploadWorker.enqueueNow(context)
    }

    suspend fun retry(id: Long) {
        dao.retryOne(id)
        UploadWorker.enqueueNow(context)
    }

    /**
     * Removes a photo the fitter has decided is wrong — the only path that deletes queued
     * bytes, and it exists because the alternative is a permanently blocked row the app
     * nags about. Nothing automatic ever calls this.
     */
    suspend fun discard(id: Long) = withContext(Dispatchers.IO) {
        val row = dao.byId(id) ?: return@withContext
        File(row.filePath).delete()
        dao.delete(id)
    }

    /**
     * Queues a photo the user picked from the gallery, or one the system camera app just
     * wrote. The bytes are copied into the queue's own directory rather than referenced
     * where they lie: a gallery item can be deleted by the person who took it while it is
     * still waiting to upload, and then the evidence is gone.
     */
    suspend fun enqueueFromUri(
        uri: Uri,
        orderId: Long,
        orderNumber: String,
        category: String,
    ): Enqueued = withContext(Dispatchers.IO) {
        val contentType = context.contentResolver.getType(uri) ?: "image/jpeg"
        val staged = File(pendingDir, "staged-${System.currentTimeMillis()}-${uri.hashCode()}")
        context.contentResolver.openInputStream(uri)?.use { input ->
            staged.outputStream().use { output -> input.copyTo(output) }
        } ?: return@withContext Enqueued.Unreadable

        if (staged.length() == 0L) {
            staged.delete()
            return@withContext Enqueued.Unreadable
        }
        enqueue(staged, orderId, orderNumber, category, contentType)
    }

    /**
     * A file for the system camera app to write into, already inside app-private storage,
     * with the content URI to hand it. The camera app gets write access to this one path
     * and nothing else (see res/xml/file_paths.xml).
     */
    fun newCameraTarget(): Pair<File, Uri> {
        val file = File(pendingDir, "camera-${System.currentTimeMillis()}.jpg")
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        return file to uri
    }
}
