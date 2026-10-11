package hu.autotherm.autocrm.ui.update

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.InputStream
import java.security.MessageDigest

/**
 * Sideloaded self-update: download the release APK the backend serves, check it, and hand
 * it to the system installer.
 *
 * There is no silent install and there cannot be one for an ordinary app: Android routes
 * every package change through the system installer, and from Android 8 the app must also
 * hold "install unknown apps" (REQUEST_INSTALL_PACKAGES), which the user grants once in
 * Settings. What this can do is make that exactly one tap: check on launch, offer the
 * dialog, download in the background, verify the hash, open the installer.
 *
 * The signature is the real safety net and it is Android's, not ours: the system refuses to
 * install an APK signed with a different key than the installed app, so a tampered download
 * cannot take over the phone. The sha256 check below is belt and braces — it catches a
 * truncated or half-written file before the installer is ever shown, and a bad hash means we
 * never hand a corrupt file to the system.
 */
object ApkUpdater {

    const val APK_MIME = "application/vnd.android.package-archive"

    /** Where a downloaded APK waits. Same directory as the upload queue: app-private. */
    fun pendingApk(context: Context): File =
        File(File(context.filesDir, "pending").apply { mkdirs() }, "autocrm-update.apk")

    /** The versionCode of the running app. */
    fun currentVersionCode(context: Context): Long {
        val info = context.packageManager.getPackageInfo(context.packageName, 0)
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            info.longVersionCode
        } else {
            @Suppress("DEPRECATION")
            info.versionCode.toLong()
        }
    }

    /**
     * May the app open the installer right now? False when the user has not yet allowed
     * AutoCRM to install packages (Android 8+), which is a per-app setting in Settings.
     */
    fun canInstall(context: Context): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.packageManager.canRequestPackageInstalls()
        } else {
            true
        }

    /** The Settings page where "install unknown apps" is granted for this app. */
    fun installPermissionSettings(context: Context): Intent =
        Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
            data = Uri.fromParts("package", context.packageName, null)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

    /**
     * sha256 of the file, lowercase hex, streamed in 64 KB blocks — the same convention the
     * photo queue uses for uploads, deliberately: one hashing rule in the app.
     */
    fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { stream ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = stream.read(buffer)
                if (read <= 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    /** What a download did, for the dialog to say something true. */
    sealed class Download {
        data class Done(val file: File) : Download()
        /** The bytes do not match what the server promised. Never installed. */
        data class Corrupt(val expected: String, val actual: String) : Download()
        data class Failed(val reason: String) : Download()
    }

    /**
     * Streams [open] into the pending file, reporting whole-percent progress against
     * [expectedSize], and verifies the hash before returning. It downloads to `.part` and
     * renames only after the hash matched, so a killed download never leaves a file that
     * looks installable.
     */
    suspend fun download(
        context: Context,
        expectedSize: Long,
        expectedSha256: String,
        open: suspend () -> InputStream,
        onProgress: (Int) -> Unit,
    ): Download = withContext(Dispatchers.IO) {
        val target = pendingApk(context)
        val temp = File(target.absolutePath + ".part")
        var lastPercent = -1
        try {
            open().use { input ->
                temp.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    var total = 0L
                    while (true) {
                        val read = input.read(buffer)
                        if (read <= 0) break
                        output.write(buffer, 0, read)
                        total += read
                        val percent = if (expectedSize > 0) {
                            ((total * 100) / expectedSize).toInt().coerceIn(0, 100)
                        } else {
                            0
                        }
                        if (percent != lastPercent) {
                            lastPercent = percent
                            onProgress(percent)
                        }
                    }
                }
            }
            onProgress(100)
            val actual = sha256(temp)
            if (!actual.equals(expectedSha256, ignoreCase = true)) {
                temp.delete()
                return@withContext Download.Corrupt(expectedSha256, actual)
            }
            if (target.exists()) target.delete()
            if (!temp.renameTo(target)) {
                temp.copyTo(target, overwrite = true)
                temp.delete()
            }
            Download.Done(target)
        } catch (e: Exception) {
            temp.delete()
            Download.Failed(e.message ?: "a letöltés nem sikerült")
        }
    }

    /**
     * The intent that opens the system installer for [apk].
     *
     * `ACTION_VIEW` with the package mime type is the documented way to hand a downloaded
     * package to Android, and it keeps the app's data: the installed package is replaced by
     * one signed with the same key, so an update must never cost anyone their unsent photos
     * or their session. `FLAG_GRANT_READ_URI_PERMISSION` is what lets the installer (a
     * different process) read the file out of our private storage.
     */
    fun installIntent(activity: Activity, apk: File): Intent =
        Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(
                FileProvider.getUriForFile(
                    activity,
                    "${activity.packageName}.fileprovider",
                    apk,
                ),
                APK_MIME,
            )
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

    /** Opens the installer. False when the device has nothing that can install packages. */
    fun openInstaller(activity: Activity, apk: File): Boolean = try {
        activity.startActivity(installIntent(activity, apk))
        true
    } catch (e: Exception) {
        false
    }
}