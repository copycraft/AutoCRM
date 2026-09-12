package hu.autotherm.autocrm.data.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

/**
 * One table, on purpose.
 *
 * The phone is not a second copy of the CRM: everything it shows is read from the API and
 * may be stale, and the only thing it *owns* is photos that have not reached the server
 * yet. Caching orders locally would mean deciding what happens when the cached copy and the
 * server disagree, which is a synchronisation problem nobody asked for.
 */
@Database(
    entities = [PendingUpload::class],
    version = 1,
    exportSchema = true,
)
abstract class AutoCrmDatabase : RoomDatabase() {
    abstract fun pendingUploads(): PendingUploadDao

    companion object {
        @Volatile
        private var instance: AutoCrmDatabase? = null

        fun get(context: Context): AutoCrmDatabase = instance ?: synchronized(this) {
            instance ?: Room.databaseBuilder(
                context.applicationContext,
                AutoCrmDatabase::class.java,
                "autocrm.db",
            )
                // No fallbackToDestructiveMigration. A destructive migration here throws
                // away photos that have not been uploaded, which is precisely the failure
                // this database exists to prevent. A future schema change writes a real
                // migration or it does not ship.
                .build()
                .also { instance = it }
        }
    }
}
