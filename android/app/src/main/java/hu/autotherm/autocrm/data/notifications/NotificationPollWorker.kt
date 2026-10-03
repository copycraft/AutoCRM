package hu.autotherm.autocrm.data.notifications

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.MainActivity
import hu.autotherm.autocrm.R
import hu.autotherm.autocrm.data.api.ApiException
import hu.autotherm.autocrm.data.api.NotificationItem
import java.util.concurrent.TimeUnit

/**
 * Asks the server what is new, every ~15 minutes (the floor for periodic work on Android),
 * and shows a system notification for anything the phone has not shown yet.
 *
 * This is polling, not push: a new lead can wait up to a quarter of an hour to reach the
 * phone. Instant delivery needs Firebase Cloud Messaging, which needs a Firebase project the
 * shop would have to create; the feed and the cursor here are what FCM would plug into.
 */
class NotificationPollWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val app = applicationContext as AutoCrmApp
        val account = app.sessionStore.currentAccount() ?: return Result.success()
        if (account.mustChangePassword) return Result.success()

        val cursorStore = app.notificationCursor
        val cursor = cursorStore.get(account.userId)
        val feed = try {
            app.api.notifications(afterId = cursor, limit = 50).items
        } catch (e: ApiException.Network) {
            return Result.retry()
        } catch (e: Exception) {
            // Forbidden, a server fault, a bad body: the next period tries again.
            return Result.success()
        }
        val plan = NotificationPlanner.plan(cursor, feed)
        show(applicationContext, plan.toShow)
        cursorStore.set(account.userId, plan.newCursor)
        return Result.success()
    }

    companion object {
        private const val UNIQUE_WORK = "autocrm-notification-poll"
        const val CHANNEL_LEADS = "leads"
        const val EXTRA_ROUTE = "route"

        /** Above this many at once, one summary replaces the individual notifications. */
        private const val MAX_INDIVIDUAL = 3
        private const val SUMMARY_ID = 1

        /** Creates the channel (idempotent) and schedules the periodic poll. */
        fun schedule(context: Context) {
            createChannel(context)
            val request = PeriodicWorkRequestBuilder<NotificationPollWorker>(15, TimeUnit.MINUTES)
                .setConstraints(
                    Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build(),
                )
                .build()
            WorkManager.getInstance(context)
                .enqueueUniquePeriodicWork(UNIQUE_WORK, ExistingPeriodicWorkPolicy.KEEP, request)
        }

        private fun createChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val channel = NotificationChannel(
                CHANNEL_LEADS,
                "Új érdeklődések",
                NotificationManager.IMPORTANCE_HIGH,
            ).apply { description = "Új érdeklődés érkezett a weboldalról" }
            context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }

        fun canNotify(context: Context): Boolean =
            Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED

        private fun open(context: Context, requestCode: Int, route: String?): PendingIntent {
            val intent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
                if (route != null) putExtra(EXTRA_ROUTE, route)
            }
            return PendingIntent.getActivity(
                context, requestCode, intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }

        /** Without the permission nothing is shown; the in-app list carries the same items. */
        @android.annotation.SuppressLint("MissingPermission")
        fun show(context: Context, items: List<NotificationItem>) {
            if (items.isEmpty() || !canNotify(context)) return
            val manager = NotificationManagerCompat.from(context)
            if (items.size > MAX_INDIVIDUAL) {
                manager.notify(
                    SUMMARY_ID,
                    NotificationCompat.Builder(context, CHANNEL_LEADS)
                        .setSmallIcon(R.mipmap.ic_launcher)
                        .setContentTitle("${items.size} új értesítés")
                        .setContentText("Koppintson az áttekintéshez")
                        .setContentIntent(open(context, SUMMARY_ID, "notifications"))
                        .setAutoCancel(true)
                        .build(),
                )
                return
            }
            for (n in items) {
                // Ids start above the summary's, and are unique per feed item.
                val id = (n.id + 1000).toInt()
                manager.notify(
                    id,
                    NotificationCompat.Builder(context, CHANNEL_LEADS)
                        .setSmallIcon(R.mipmap.ic_launcher)
                        .setContentTitle(n.title)
                        .setContentText(n.body)
                        .setStyle(NotificationCompat.BigTextStyle().bigText(n.body))
                        .setContentIntent(
                            open(context, id, NotificationPlanner.routeForLink(n.link) ?: "notifications"),
                        )
                        .setAutoCancel(true)
                        .build(),
                )
            }
        }
    }
}
