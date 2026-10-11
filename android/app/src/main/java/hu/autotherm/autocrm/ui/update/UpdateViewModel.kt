package hu.autotherm.autocrm.ui.update

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import hu.autotherm.autocrm.data.api.AppVersion
import hu.autotherm.autocrm.data.api.AutoCrmApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File

/**
 * The update check: on launch, ask the server what it publishes, and offer it if it is
 * newer than what this phone runs.
 *
 * Runs before sign-in on purpose. A build too old for the current API has no working login,
 * so if the check waited for a session it would strand exactly the phones that need it.
 */
class UpdateViewModel(
    private val api: AutoCrmApi,
    private val context: Context,
) : ViewModel() {

    data class State(
        /** The release to install, once known. Null means up to date or nothing published. */
        val available: AppVersion? = null,
        /** True when the server says this build may not be used any more. */
        val mandatory: Boolean = false,
        val checking: Boolean = false,
        val downloading: Boolean = false,
        /** 0..100, or null before the first byte. */
        val progress: Int? = null,
        /** The verified APK, ready for the installer. */
        val ready: File? = null,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    /** Checks once per screen. A phone that is up to date never sees anything. */
    fun check(force: Boolean = false) {
        if (_state.value.checking || _state.value.downloading) return
        if (_state.value.available != null && !force) return
        viewModelScope.launch {
            _state.value = _state.value.copy(checking = true, error = null)
            try {
                val current = ApkUpdater.currentVersionCode(context)
                val published = api.appVersion()
                val newer = published != null && published.versionCode > current
                _state.value = _state.value.copy(
                    checking = false,
                    available = if (newer) published else null,
                    mandatory = newer && published!!.mandatory,
                )
            } catch (e: Exception) {
                // An unreachable server is not an error the user needs to see: the app works,
                // it is just not updated yet. Silent by design.
                _state.value = _state.value.copy(checking = false)
            }
        }
    }

    /** Downloads the published APK and verifies it. [onReady] fires when it is installable. */
    fun download(onReady: () -> Unit) {
        val version = _state.value.available ?: return
        if (_state.value.downloading) return
        viewModelScope.launch {
            _state.value = _state.value.copy(downloading = true, progress = 0, error = null)
            val result = ApkUpdater.download(
                context = context,
                expectedSize = version.sizeBytes,
                expectedSha256 = version.sha256,
                open = { api.openApk(version.apkUrl) },
                onProgress = { percent ->
                    viewModelScope.launch {
                        _state.value = _state.value.copy(progress = percent)
                    }
                },
            )
            when (result) {
                is ApkUpdater.Download.Done -> {
                    _state.value = _state.value.copy(
                        downloading = false,
                        progress = 100,
                        ready = result.file,
                        error = null,
                    )
                    onReady()
                }
                is ApkUpdater.Download.Corrupt -> {
                    _state.value = _state.value.copy(
                        downloading = false,
                        error = "A letöltött fájl nem egyezik a szerverével. Nem telepítjük.",
                    )
                }
                is ApkUpdater.Download.Failed -> {
                    _state.value = _state.value.copy(
                        downloading = false,
                        error = result.reason,
                    )
                }
            }
        }
    }

    fun dismiss() {
        val mandatory = _state.value.mandatory
        if (!mandatory) {
            _state.value = _state.value.copy(available = null, error = null)
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }

    /** Removes a downloaded APK (after an install, or when the user gives up). */
    fun clearApk() {
        viewModelScope.launch(Dispatchers.IO) {
            ApkUpdater.pendingApk(context).delete()
        }
        _state.value = _state.value.copy(ready = null, progress = null)
    }
}