package hu.autotherm.autocrm.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import kotlinx.coroutines.flow.Flow

@Dao
interface PendingUploadDao {

    /**
     * `IGNORE`, not `REPLACE`: the unique index is `(order_id, sha256)`, so a second insert
     * of the same bytes for the same order is the same photo. Replacing would reset the
     * attempt count and the ticket of a row already in flight.
     *
     * Returns -1 when the row was already there.
     */
    @Insert(onConflict = OnConflictStrategy.IGNORE)
    suspend fun insert(upload: PendingUpload): Long

    @Query("SELECT * FROM pending_uploads WHERE id = :id")
    suspend fun byId(id: Long): PendingUpload?

    @Query("SELECT * FROM pending_uploads WHERE order_id = :orderId AND sha256 = :sha256")
    suspend fun byContent(orderId: Long, sha256: String): PendingUpload?

    /**
     * The worker's work list. `uploading` rows are included because a process death leaves
     * them stranded in that state; whoever picks them up next re-checks with the server,
     * which is idempotent.
     *
     * Inspection rows are excluded here, in SQL, not after the LIMIT: they belong to the
     * inspection sync, and a walkaround's 20+ older inspection photos would otherwise fill
     * every batch and starve the production photos queued behind them.
     */
    @Query(
        """
        SELECT * FROM pending_uploads
         WHERE state IN ('pending', 'uploading')
           AND category != 'inspection'
           AND next_attempt_at <= :now
         ORDER BY created_at, id
         LIMIT :limit
        """,
    )
    suspend fun dueForUpload(now: Long, limit: Int = 20): List<PendingUpload>

    @Query("SELECT count(*) FROM pending_uploads WHERE state IN ('pending', 'uploading')")
    fun outstandingCount(): Flow<Int>

    @Query("SELECT count(*) FROM pending_uploads WHERE state = 'blocked'")
    fun blockedCount(): Flow<Int>

    @Query("SELECT * FROM pending_uploads WHERE state != 'done' ORDER BY created_at DESC, id DESC")
    fun watchQueue(): Flow<List<PendingUpload>>

    @Query(
        """
        SELECT * FROM pending_uploads
         WHERE order_id = :orderId AND state != 'done'
         ORDER BY created_at DESC, id DESC
        """,
    )
    fun watchForOrder(orderId: Long): Flow<List<PendingUpload>>

    @Query("UPDATE pending_uploads SET state = :state WHERE id = :id")
    suspend fun setState(id: Long, state: String)

    @Query(
        """
        UPDATE pending_uploads
           SET ticket = :ticket, ticket_expires_at = :expiresAt, state = 'uploading'
         WHERE id = :id
        """,
    )
    suspend fun setTicket(id: Long, ticket: String, expiresAt: Long?)

    @Query(
        """
        UPDATE pending_uploads
           SET state = 'done', uploaded_image_id = :imageId, last_error = NULL
         WHERE id = :id
        """,
    )
    suspend fun markDone(id: Long, imageId: Long?)

    /**
     * A failure that may pass: record it, back off, leave it queued.
     *
     * The ticket is kept: it is only stored after a successful PUT, so it means "the bytes
     * are already in storage" and the next attempt goes straight to complete while it is
     * still valid (see [PendingUpload.isTicketUsable]).
     */
    @Query(
        """
        UPDATE pending_uploads
           SET state = 'pending', attempts = attempts + 1,
               last_error = :error, next_attempt_at = :nextAttemptAt
         WHERE id = :id
        """,
    )
    suspend fun markRetryable(id: Long, error: String, nextAttemptAt: Long)

    /** A failure that will not pass. Kept visible; never deleted behind the fitter's back. */
    @Query("UPDATE pending_uploads SET state = 'blocked', last_error = :error WHERE id = :id")
    suspend fun markBlocked(id: Long, error: String)

    /** "Try again" on the queue screen: clears the backoff on everything stuck. */
    @Query(
        """
        UPDATE pending_uploads
           SET state = 'pending', attempts = 0, next_attempt_at = 0, last_error = NULL,
               ticket = NULL, ticket_expires_at = NULL
         WHERE state IN ('blocked', 'pending')
        """,
    )
    suspend fun retryAll()

    /** A manual retry starts clean, with a fresh ticket. */
    @Query(
        """
        UPDATE pending_uploads
           SET state = 'pending', attempts = 0, next_attempt_at = 0,
               ticket = NULL, ticket_expires_at = NULL
         WHERE id = :id
        """,
    )
    suspend fun retryOne(id: Long)

    @Query("DELETE FROM pending_uploads WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("SELECT * FROM pending_uploads WHERE state = 'done'")
    suspend fun completed(): List<PendingUpload>

    /**
     * Frees a row and its file together. Nothing else deletes either: a file without a row
     * is orphaned bytes, a row without a file is an upload that can never succeed.
     */
    @Transaction
    suspend fun sweepCompleted(deleteFile: (String) -> Unit) {
        for (row in completed()) {
            deleteFile(row.filePath)
            delete(row.id)
        }
    }
}
