package hu.autotherm.autocrm

import android.app.Application
import android.os.StrictMode
import androidx.work.Configuration
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.db.AutoCrmDatabase
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.data.prefs.ServerStore
import hu.autotherm.autocrm.data.upload.UploadQueue
import hu.autotherm.autocrm.data.upload.UploadWorker

/**
 * Constructor injection by hand, no DI framework.
 *
 * There are six things to wire and one place that needs them. Hilt would add an annotation
 * processor, a compile step and a layer of indirection to save writing the six lines below
 * — the same trade the backend refuses when it keeps a Postgres-backed job queue instead of
 * a job-queue service.
 */
class AutoCrmApp : Application(), Configuration.Provider {

    val sessionStore: SessionStore by lazy { SessionStore(this) }
    val capturePrefs: CapturePrefs by lazy { CapturePrefs(this) }
    val database: AutoCrmDatabase by lazy { AutoCrmDatabase.get(this) }
    val serverStore: ServerStore by lazy { ServerStore(this) }
    val api: AutoCrmApi by lazy { AutoCrmApi(serverStore, sessionStore) }
    val uploadQueue: UploadQueue by lazy { UploadQueue(this, database.pendingUploads()) }

    /**
     * For the handful of writes that belong to the app rather than to a screen — setting the
     * sticky order from the order detail, for instance. Deliberately not a ViewModel scope:
     * the write must outlive the composable that started it.
     */
    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    fun selectOrderForCapture(order: CapturePrefs.CurrentOrder) {
        appScope.launch { capturePrefs.setCurrentOrder(order) }
    }

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().build()

    override fun onCreate() {
        super.onCreate()
        // Debug builds fail loudly on main-thread I/O. Without this, reading a response body
        // on the main thread reached a real phone before anyone noticed: the symptom was a
        // generic error on every screen, which reads like a server problem.
        if (BuildConfig.DEBUG) {
            StrictMode.setThreadPolicy(
                StrictMode.ThreadPolicy.Builder()
                    .detectNetwork()
                    .detectDiskReads()
                    .detectDiskWrites()
                    .penaltyLog()
                    .build(),
            )
        }
        // WorkManager initialises on demand: the manifest removes the startup provider and
        // this class supplies the configuration, so the first getInstance() builds it.
        // Calling initialize() here as well would throw if anything reached it first.
        //
        // A phone that was out of coverage all night has photos waiting. Poking the queue at
        // launch means the fitter sees them drain rather than wondering.
        UploadWorker.enqueue(this)
    }
}
