package hu.autotherm.autocrm.data.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase

/**
 * The phone owns two things: photos that have not reached the server yet, and
 * handover-inspection drafts that have not been signed on the server yet.
 * Everything it shows is read from the API and may be stale; caching orders
 * locally would mean deciding what happens when the cached copy and the server
 * disagree, which is a synchronisation problem nobody asked for.
 */
@Database(
    entities = [PendingUpload::class, InspectionDraft::class],
    version = 2,
    exportSchema = true,
)
abstract class AutoCrmDatabase : RoomDatabase() {
    abstract fun pendingUploads(): PendingUploadDao
    abstract fun inspectionDrafts(): InspectionDraftDao

    companion object {
        val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    """
                    CREATE TABLE inspection_drafts (
                        local_uuid TEXT NOT NULL PRIMARY KEY,
                        order_id INTEGER NOT NULL,
                        order_number TEXT NOT NULL,
                        kind TEXT NOT NULL,
                        server_id INTEGER,
                        state TEXT NOT NULL,
                        payload_json TEXT NOT NULL,
                        error TEXT,
                        updated_at INTEGER NOT NULL
                    )
                    """.trimIndent(),
                )
            }
        }

        @Volatile
        private var instance: AutoCrmDatabase? = null

        fun get(context: Context): AutoCrmDatabase = instance ?: synchronized(this) {
            instance ?: Room.databaseBuilder(
                context.applicationContext,
                AutoCrmDatabase::class.java,
                "autocrm.db",
            )
                .addMigrations(MIGRATION_1_2)
                // No fallbackToDestructiveMigration. A destructive migration here throws
                // away photos that have not been uploaded, which is precisely the failure
                // this database exists to prevent. A future schema change writes a real
                // migration or it does not ship.
                .build()
                .also { instance = it }
        }
    }
}
