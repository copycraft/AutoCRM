package hu.autotherm.autocrm.data.db

import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

/**
 * A handover inspection owned by the phone until the server confirms it.
 *
 * The walkaround happens where the cars are, which is where the signal is not.
 * So the whole draft — readings, zone order, photo references, damages, verdicts,
 * signatures — lives here as one JSON payload, while the photo bytes ride the
 * normal `pending_uploads` queue with category `inspection`. The sync worker
 * creates the server inspection, attaches everything, signs, and only then
 * deletes this row. Like pending photos, a draft is never dropped silently:
 * a failed sync stays visible with its error.
 */
@Entity(tableName = "inspection_drafts")
data class InspectionDraft(
    @PrimaryKey
    @ColumnInfo(name = "local_uuid")
    val localUuid: String,
    @ColumnInfo(name = "order_id") val orderId: Long,
    @ColumnInfo(name = "order_number") val orderNumber: String,
    @ColumnInfo(name = "kind") val kind: String,
    /** Server row once created (at first sync when started offline). */
    @ColumnInfo(name = "server_id") val serverId: Long? = null,
    @ColumnInfo(name = "state") val state: String = STATE_DRAFT,
    /** Full walkaround state, see `DraftPayload` in data.inspection. */
    @ColumnInfo(name = "payload_json") val payloadJson: String,
    @ColumnInfo(name = "error") val error: String? = null,
    @ColumnInfo(name = "updated_at") val updatedAt: Long = System.currentTimeMillis(),
) {
    companion object {
        /** Being walked right now, or waiting for signal. */
        const val STATE_DRAFT = "draft"
        /** The worker has it right now. */
        const val STATE_SYNCING = "syncing"
        /** Signed on the server. Kept briefly so the list can confirm, then swept. */
        const val STATE_SYNCED = "synced"
    }
}

@Dao
interface InspectionDraftDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(draft: InspectionDraft)

    @Query("SELECT * FROM inspection_drafts WHERE local_uuid = :uuid")
    suspend fun byUuid(uuid: String): InspectionDraft?

    @Query("SELECT * FROM inspection_drafts WHERE order_id = :orderId ORDER BY updated_at DESC")
    fun watchForOrder(orderId: Long): Flow<List<InspectionDraft>>

    @Query("SELECT * FROM inspection_drafts WHERE state IN ('draft', 'syncing') ORDER BY updated_at")
    suspend fun pendingSync(): List<InspectionDraft>

    @Query("SELECT count(*) FROM inspection_drafts WHERE state != 'synced'")
    fun openCount(): Flow<Int>

    @Query("UPDATE inspection_drafts SET state = :state, error = :error WHERE local_uuid = :uuid")
    suspend fun setState(uuid: String, state: String, error: String? = null)

    @Query("UPDATE inspection_drafts SET server_id = :serverId WHERE local_uuid = :uuid")
    suspend fun setServerId(uuid: String, serverId: Long)

    @Query("UPDATE inspection_drafts SET payload_json = :json WHERE local_uuid = :uuid")
    suspend fun setPayload(uuid: String, json: String)

    @Query("DELETE FROM inspection_drafts WHERE local_uuid = :uuid")
    suspend fun delete(uuid: String)
}
