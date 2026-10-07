package hu.autotherm.autocrm.widget

import android.content.Context
import android.content.Intent
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.updateAll
import androidx.glance.background
import androidx.glance.layout.Column
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import hu.autotherm.autocrm.AutoCrmApp
import hu.autotherm.autocrm.MainActivity
import hu.autotherm.autocrm.data.api.Task
import hu.autotherm.autocrm.data.notifications.NotificationPollWorker
import java.time.LocalDate

/**
 * Home-screen widget (0049): the signed-in person's open tasks due today or earlier, and how
 * many more are coming. Tapping it opens the task list. Refreshed by the system about every
 * half hour, and whenever the app is opened.
 */
class TasksWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val app = context.applicationContext as AutoCrmApp
        val signedIn = app.sessionStore.currentAccount() != null
        val tasks: List<Task>? = if (signedIn) {
            try {
                app.api.tasksMine().filter { !it.isDone }
            } catch (_: Exception) {
                null
            }
        } else {
            null
        }
        val today = LocalDate.now().toString()
        val due = tasks.orEmpty().filter { it.dueDate != null && it.dueDate <= today }
        val later = tasks.orEmpty().size - due.size

        provideContent {
            GlanceTheme {
                Column(
                    GlanceModifier
                        .fillMaxSize()
                        .background(GlanceTheme.colors.widgetBackground)
                        .padding(12.dp)
                        .clickable(
                            actionStartActivity(
                                Intent(context, MainActivity::class.java)
                                    .putExtra(NotificationPollWorker.EXTRA_ROUTE, "tasks")
                                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP),
                            ),
                        ),
                ) {
                    Text(
                        "Mai feladataim",
                        style = TextStyle(fontWeight = FontWeight.Bold, fontSize = 15.sp, color = GlanceTheme.colors.onSurface),
                    )
                    Spacer(GlanceModifier.height(6.dp))
                    when {
                        !signedIn -> Text("Jelentkezz be az AutoCRM-be.", style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant))
                        tasks == null -> Text("Nincs kapcsolat.", style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant))
                        due.isEmpty() -> Text("Mára nincs teendő.", style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant))
                        else -> due.take(5).forEach { task ->
                            Text(
                                (if (task.dueDate != null && task.dueDate < today) "! " else "• ") + task.title,
                                maxLines = 1,
                                style = TextStyle(fontSize = 13.sp, color = GlanceTheme.colors.onSurface),
                            )
                        }
                    }
                    if (due.size > 5) {
                        Text("…és még ${due.size - 5}", style = TextStyle(fontSize = 12.sp, color = GlanceTheme.colors.onSurfaceVariant))
                    }
                    if (later > 0) {
                        Spacer(GlanceModifier.height(4.dp))
                        Text("Később: $later", style = TextStyle(fontSize = 12.sp, color = GlanceTheme.colors.onSurfaceVariant))
                    }
                }
            }
        }
    }

    companion object {
        /** Refresh every placed widget now (the app was opened, a task was ticked). */
        suspend fun refresh(context: Context) {
            try {
                TasksWidget().updateAll(context)
            } catch (_: Exception) {
                // No widget placed, or the host is busy: nothing to do.
            }
        }
    }
}

class TasksWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = TasksWidget()
}
