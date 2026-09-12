package hu.autotherm.autocrm.data.db

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One photo waiting to reach the server.
 *
 * This table is the answer to the question the viability review called the most
 * consequential unknown in the whole MEO workflow: what happens to a photo taken in a yard
 * with no signal. It is written before the camera shutter sound finishes and deleted only
 * when the server has confirmed the row exists. Between those two points the phone can be
 * force-stopped, run out of battery, spend a week out of coverage or be rebooted, and the
 * photo is still here.
 *
 * The file itself lives in the app's own `files/pending/` directory, not in the shared
 * gallery: a photo the fitter can delete from Google Photos while it is still queued is a
 * photo that vanishes halfway through the evidence chain.
 */
@Entity(
    tableName = "pending_uploads",
    indices = [
        Index(value = ["state", "next_attempt_at"]),
        // The server enforces (order_id, content_hash) uniqueness; matching it here means a
        // double tap on the shutter enqueues one row, not two that race each other.
        Index(value = ["order_id", "sha256"], unique = true),
    ],
)
data class PendingUpload(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,

    @ColumnInfo(name = "order_id") val orderId: Long,
    /** Denormalised so the queue screen reads without a network call or a join. */
    @ColumnInfo(name = "order_number") val orderNumber: String,
    @ColumnInfo(name = "category") val category: String,

    /** Absolute path in app-private storage. Deleted with the row, never before. */
    @ColumnInfo(name = "file_path") val filePath: String,
    @ColumnInfo(name = "content_type") val contentType: String,
    @ColumnInfo(name = "byte_size") val byteSize: Long,
    /** Lowercase hex, computed once at capture; the upload ticket is bound to it. */
    @ColumnInfo(name = "sha256") val sha256: String,
    @ColumnInfo(name = "filename") val filename: String,

    @ColumnInfo(name = "state") val state: String = STATE_PENDING,
    @ColumnInfo(name = "attempts") val attempts: Int = 0,
    /**
     * Set when a ticket has been issued but the PUT has not been confirmed. Keeping it lets
     * a resumed upload skip straight to completing rather than asking for a second ticket
     * for bytes that are already in the object store.
     */
    @ColumnInfo(name = "ticket") val ticket: String? = null,
    @ColumnInfo(name = "ticket_expires_at") val ticketExpiresAt: Long? = null,

    @ColumnInfo(name = "last_error") val lastError: String? = null,
    /** Epoch millis. The worker ignores rows scheduled for later. */
    @ColumnInfo(name = "next_attempt_at") val nextAttemptAt: Long = 0,
    @ColumnInfo(name = "created_at") val createdAt: Long = System.currentTimeMillis(),
    @ColumnInfo(name = "uploaded_image_id") val uploadedImageId: Long? = null,
) {
    companion object {
        /** Waiting for the worker. */
        const val STATE_PENDING = "pending"
        /** The worker has it right now. */
        const val STATE_UPLOADING = "uploading"

        /**
         * The server refused it in a way that will not change: a 422, a deleted order, a
         * category the order no longer accepts. Left in the table on purpose — silently
         * dropping a photo is the one behaviour this whole design exists to prevent. The
         * fitter sees it on the queue screen and decides.
         */
        const val STATE_BLOCKED = "blocked"

        /**
         * Confirmed by the server. Rows sit here briefly so the queue screen can show
         * "12 uploaded" before a sweep deletes them and their files.
         */
        const val STATE_DONE = "done"

        /** Backoff between attempts, capped. Index is the attempt count. */
        private val BACKOFF_MS = longArrayOf(
            0,
            10_000,      // 10s — a passing tunnel
            60_000,      // 1m
            5 * 60_000,  // 5m
            30 * 60_000, // 30m
            2 * 60 * 60_000, // 2h — the van is somewhere without coverage
        )

        fun backoffFor(attempts: Int): Long =
            BACKOFF_MS[attempts.coerceIn(0, BACKOFF_MS.lastIndex)]
    }

    val isTicketUsable: Boolean
        get() = ticket != null && (ticketExpiresAt ?: 0) > System.currentTimeMillis() + 60_000
}
