package hu.autotherm.autocrm.data.cache

import android.content.Context
import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.Upsert
import java.util.concurrent.atomic.AtomicInteger

/**
 * The last good answer to a read, so screens still work with no signal.
 *
 * Kept in its own database, apart from the photo queue and the inspection drafts: those
 * hold work that exists nowhere else and must never be lost, this holds copies of what the
 * server already has and can be thrown away at any time (so it migrates destructively).
 */
@Entity(
    tableName = "cached_responses",
    primaryKeys = ["key", "user_id"],
    indices = [Index("fetched_at")],
)
data class CachedResponse(
    /** The full request URL: server, path and query. */
    @androidx.room.ColumnInfo(name = "key") val key: String,
    @androidx.room.ColumnInfo(name = "user_id") val userId: Long,
    @androidx.room.ColumnInfo(name = "body") val body: String,
    @androidx.room.ColumnInfo(name = "fetched_at") val fetchedAt: Long,
)

@Dao
interface CachedResponseDao {
    @Query("SELECT * FROM cached_responses WHERE `key` = :key AND user_id = :userId")
    suspend fun find(key: String, userId: Long): CachedResponse?

    @Upsert
    suspend fun put(row: CachedResponse)

    @Query("DELETE FROM cached_responses")
    suspend fun clear()

    @Query("DELETE FROM cached_responses WHERE fetched_at < :before")
    suspend fun deleteOlderThan(before: Long)

    /** Keeps the newest [keep] rows. */
    @Query(
        "DELETE FROM cached_responses WHERE rowid NOT IN " +
            "(SELECT rowid FROM cached_responses ORDER BY fetched_at DESC LIMIT :keep)",
    )
    suspend fun trimTo(keep: Int)
}

@Database(entities = [CachedResponse::class], version = 1, exportSchema = false)
abstract class ResponseCacheDatabase : RoomDatabase() {
    abstract fun responses(): CachedResponseDao
}

/**
 * Per-user cache of GET responses. Another user on the same phone never sees it (rows are
 * keyed by user), sign-out clears it, and it stays small: old rows and rows beyond a cap
 * are dropped as new ones arrive.
 */
class ResponseCache(private val dao: CachedResponseDao, private val now: () -> Long = System::currentTimeMillis) {

    constructor(context: Context) : this(
        Room.databaseBuilder(context.applicationContext, ResponseCacheDatabase::class.java, "response-cache.db")
            // Disposable by design: a schema change just starts the cache over.
            .fallbackToDestructiveMigration()
            .build()
            .responses(),
    )

    private val writes = AtomicInteger()

    suspend fun get(key: String, userId: Long): CachedResponse? = dao.find(key, userId)

    suspend fun put(key: String, userId: Long, body: String) {
        val time = now()
        dao.put(CachedResponse(key, userId, body, time))
        // Housekeeping now and then, not on every write.
        if (writes.incrementAndGet() % HOUSEKEEPING_EVERY == 0) {
            dao.deleteOlderThan(time - MAX_AGE_MS)
            dao.trimTo(MAX_ROWS)
        }
    }

    suspend fun clear() = dao.clear()

    companion object {
        const val MAX_ROWS = 400
        const val MAX_AGE_MS = 14L * 24 * 60 * 60 * 1000
        private const val HOUSEKEEPING_EVERY = 25

        /**
         * Which reads are worth keeping. Left out on purpose: sign-in and session calls,
         * the staff directory and user list (personal data that should not sit on a phone
         * longer than it must), the notification feed (stale news is worse than none), search
         * (it asks a new question each time), and anything carrying a signed, expiring link
         * (photos and documents), which would be dead by the time it was read back.
         */
        fun isCacheable(path: String): Boolean {
            if (!path.startsWith("/api/")) return false
            val rest = path.removePrefix("/api")
            val excluded = listOf("/auth", "/hr", "/users", "/notifications", "/search", "/uploads")
            if (excluded.any { rest == it || rest.startsWith("$it/") }) return false
            if (rest.endsWith("/images") || rest.endsWith("/documents") || rest.contains("/documents/")) return false
            return true
        }
    }
}
