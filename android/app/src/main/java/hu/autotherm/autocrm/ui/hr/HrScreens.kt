package hu.autotherm.autocrm.ui.hr

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import coil.compose.AsyncImage
import hu.autotherm.autocrm.data.api.AutoCrmApi
import hu.autotherm.autocrm.data.api.Employee
import hu.autotherm.autocrm.ui.common.AutoCrmTextField
import hu.autotherm.autocrm.ui.common.Card
import hu.autotherm.autocrm.ui.common.DialogShell
import hu.autotherm.autocrm.ui.common.EmailInfo
import hu.autotherm.autocrm.ui.common.EmptyState
import hu.autotherm.autocrm.ui.common.ErrorState
import hu.autotherm.autocrm.ui.common.ListSkeleton
import hu.autotherm.autocrm.ui.common.PhoneInfo
import hu.autotherm.autocrm.ui.common.PrimaryButton
import hu.autotherm.autocrm.ui.common.ScreenTopBar
import hu.autotherm.autocrm.ui.common.SearchField
import hu.autotherm.autocrm.ui.common.StatusBadge
import hu.autotherm.autocrm.ui.common.Tone
import hu.autotherm.autocrm.ui.common.describeError
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import java.io.ByteArrayOutputStream

/** The server caps the photo at 8 MB; larger originals are recompressed here first. */
private const val MAX_PHOTO_BYTES = 8 * 1024 * 1024

private fun initials(name: String): String =
    name.split(Regex("\\s+")).filter { it.isNotBlank() }.take(2)
        .joinToString("") { it.first().uppercase() }

/** The profile picture, or the initials on a grey disc when there is none. */
@Composable
fun EmployeeAvatar(name: String, photoUrl: String?, size: Dp, modifier: Modifier = Modifier) {
    val base = modifier.size(size).clip(CircleShape)
    if (photoUrl != null) {
        // Tap the photo to see the face, not a 64 dp circle (a new starter's badge photo).
        var enlarged by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
        AsyncImage(
            model = photoUrl,
            contentDescription = name,
            contentScale = ContentScale.Crop,
            modifier = base.clickable { enlarged = true },
        )
        if (enlarged) {
            androidx.compose.ui.window.Dialog(onDismissRequest = { enlarged = false }) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier.clickable { enlarged = false },
                ) {
                    AsyncImage(
                        model = photoUrl,
                        contentDescription = name,
                        contentScale = ContentScale.Fit,
                        modifier = Modifier.fillMaxWidth().clip(androidx.compose.foundation.shape.RoundedCornerShape(16.dp)),
                    )
                    Text(
                        name,
                        style = MaterialTheme.typography.titleMedium,
                        color = androidx.compose.ui.graphics.Color.White,
                        modifier = Modifier.padding(top = 12.dp),
                    )
                }
            }
        }
    } else {
        Box(base.background(Steel200), contentAlignment = Alignment.Center) {
            Text(initials(name), style = MaterialTheme.typography.titleMedium, color = Steel500)
        }
    }
}

// ── List ────────────────────────────────────────────────────────────────────────────

class HrListViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loading: Boolean = true,
        val refreshing: Boolean = false,
        val query: String = "",
        val includeArchived: Boolean = false,
        val employees: List<Employee> = emptyList(),
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun setQuery(q: String) {
        _state.value = _state.value.copy(query = q)
    }

    fun setIncludeArchived(on: Boolean) {
        _state.value = _state.value.copy(includeArchived = on)
        load()
    }

    fun load() {
        val s = _state.value
        viewModelScope.launch {
            val keepRows = s.employees.isNotEmpty()
            _state.value = _state.value.copy(loading = !keepRows, refreshing = keepRows, error = null)
            try {
                val rows = api.employees(s.query.trim().ifBlank { null }, s.includeArchived)
                _state.value = _state.value.copy(loading = false, refreshing = false, employees = rows)
            } catch (e: Throwable) {
                _state.value = _state.value.copy(loading = false, refreshing = false, error = describeError(e))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HrListScreen(
    viewModel: HrListViewModel,
    onMenu: () -> Unit,
    onLeave: () -> Unit,
    onNew: () -> Unit,
    onEdit: (Long) -> Unit,
) {
    val state by viewModel.state.collectAsState()
    LaunchedEffect(Unit) { viewModel.load() }

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = "HR",
                subtitle = if (state.employees.isNotEmpty()) "${state.employees.size} munkatárs" else null,
                onMenu = onMenu,
                refreshing = state.refreshing,
                onRefresh = viewModel::load,
                actions = { TextButton(onClick = onLeave) { Text("Távollétek") } },
            )
        },
        floatingActionButton = {
            FloatingActionButton(onClick = onNew) {
                Icon(Icons.Filled.Add, contentDescription = "Új munkatárs")
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 12.dp)) {
            SearchField(
                value = state.query,
                onValueChange = viewModel::setQuery,
                label = "Keresés névre, e-mailre, telefonszámra",
                modifier = Modifier.fillMaxWidth(),
                onSearch = viewModel::load,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = state.includeArchived, onCheckedChange = viewModel::setIncludeArchived)
                Text("Kilépettek is", style = MaterialTheme.typography.bodyMedium)
            }
            when {
                state.loading -> ListSkeleton()
                state.error != null && state.employees.isEmpty() ->
                    ErrorState(state.error!!, onRetry = viewModel::load)
                state.employees.isEmpty() -> EmptyState(
                    if (state.query.isBlank()) "Még nincs munkatárs rögzítve." else "Nincs találat.",
                )
                else -> hu.autotherm.autocrm.ui.common.AppPullToRefresh(isRefreshing = state.refreshing, onRefresh = viewModel::load) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(state.employees, key = { it.id }) { e ->
                            Card(modifier = Modifier.animateItem(), onClick = { onEdit(e.id) }) {
                                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                                    EmployeeAvatar(e.fullName, e.photoUrl, 64.dp)
                                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                        Row(
                                            Modifier.fillMaxWidth(),
                                            horizontalArrangement = Arrangement.SpaceBetween,
                                        ) {
                                            Text(
                                                e.fullName,
                                                style = MaterialTheme.typography.titleMedium,
                                                modifier = Modifier.weight(1f, fill = false),
                                            )
                                            if (e.archivedAt != null) StatusBadge("Kilépett", Tone.Steel)
                                        }
                                        PhoneInfo("Céges telefonszám", e.companyPhone)
                                        PhoneInfo("Személyes telefonszám", e.personalPhone)
                                        EmailInfo("E-mail", e.email)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Edit ────────────────────────────────────────────────────────────────────────────

class HrEditViewModel(private val api: AutoCrmApi) : ViewModel() {

    data class State(
        val loaded: Boolean = false,
        val fullName: String = "",
        val email: String = "",
        val companyPhone: String = "",
        val personalPhone: String = "",
        /** The photo already on the server (a signed link), if any. */
        val photoUrl: String? = null,
        /** A newly picked photo, uploaded on save. */
        val newPhoto: ByteArray? = null,
        val removePhoto: Boolean = false,
        val archived: Boolean = false,
        val busy: Boolean = false,
        val error: String? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun set(next: (State) -> State) {
        _state.value = next(_state.value)
    }

    fun load(id: Long) {
        viewModelScope.launch {
            try {
                val e = api.employee(id)
                _state.value = State(
                    loaded = true,
                    fullName = e.fullName,
                    email = e.email.orEmpty(),
                    companyPhone = e.companyPhone.orEmpty(),
                    personalPhone = e.personalPhone.orEmpty(),
                    photoUrl = e.photoUrl,
                    archived = e.archivedAt != null,
                )
            } catch (t: Throwable) {
                _state.value = _state.value.copy(error = describeError(t))
            }
        }
    }

    fun save(id: Long?, onSaved: () -> Unit) {
        val s = _state.value
        if (s.busy || s.fullName.isBlank()) return
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            // Explicit nulls: a blank field clears the stored value on PATCH.
            fun text(v: String) = v.trim().takeIf { it.isNotEmpty() }?.let(::JsonPrimitive) ?: JsonNull
            val body = buildJsonObject {
                put("full_name", JsonPrimitive(s.fullName.trim()))
                put("email", text(s.email))
                put("company_phone", text(s.companyPhone))
                put("personal_phone", text(s.personalPhone))
            }
            try {
                val saved = if (id == null) api.createEmployee(body) else api.patchEmployee(id, body)
                when {
                    s.newPhoto != null -> api.setEmployeePhoto(saved.id, s.newPhoto, "image/jpeg")
                    s.removePhoto && s.photoUrl != null -> api.removeEmployeePhoto(saved.id)
                }
                _state.value = _state.value.copy(busy = false)
                onSaved()
            } catch (t: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(t))
            }
        }
    }

    fun toggleArchived(id: Long, onDone: () -> Unit) {
        val s = _state.value
        if (s.busy) return
        viewModelScope.launch {
            _state.value = s.copy(busy = true, error = null)
            try {
                api.setEmployeeArchived(id, !s.archived)
                _state.value = _state.value.copy(busy = false)
                onDone()
            } catch (t: Throwable) {
                _state.value = _state.value.copy(busy = false, error = describeError(t))
            }
        }
    }
}

/**
 * A picked image as JPEG bytes under the server's size limit. Phone originals are usually
 * well under it; a bigger one is decoded at half size and recompressed. Null when the
 * file is not an image.
 */
private fun readPhoto(context: android.content.Context, uri: Uri): ByteArray? {
    val original = context.contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: return null
    if (original.size <= MAX_PHOTO_BYTES && original.size > 0) {
        // Validate it decodes; the server would refuse junk anyway, but say so here.
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(original, 0, original.size, bounds)
        return if (bounds.outWidth > 0) original else null
    }
    var sample = 2
    while (sample <= 16) {
        val bitmap = BitmapFactory.decodeByteArray(
            original, 0, original.size, BitmapFactory.Options().apply { inSampleSize = sample },
        ) ?: return null
        val out = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.JPEG, 90, out)
        bitmap.recycle()
        if (out.size() <= MAX_PHOTO_BYTES) return out.toByteArray()
        sample *= 2
    }
    return null
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HrEditScreen(
    employeeId: Long?,
    viewModel: HrEditViewModel,
    onBack: () -> Unit,
    onSaved: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var confirmArchive by remember { mutableStateOf(false) }
    LaunchedEffect(employeeId) { if (employeeId != null) viewModel.load(employeeId) }

    // The system photo picker: no storage permission, the user exposes only the one picture.
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val bytes = withContext(Dispatchers.IO) { runCatching { readPhoto(context, uri) }.getOrNull() }
            if (bytes == null) {
                viewModel.set { it.copy(error = "A kép nem olvasható, vagy túl nagy.") }
            } else {
                viewModel.set { it.copy(newPhoto = bytes, removePhoto = false, error = null) }
            }
        }
    }
    val preview = remember(state.newPhoto) {
        state.newPhoto?.let { BitmapFactory.decodeByteArray(it, 0, it.size)?.asImageBitmap() }
    }
    val hasPhoto = preview != null || (state.photoUrl != null && !state.removePhoto)

    Scaffold(
        topBar = {
            ScreenTopBar(
                title = if (employeeId == null) "Új munkatárs" else "Munkatárs szerkesztése",
                onBack = onBack,
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp).verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            state.error?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                if (preview != null) {
                    Image(
                        bitmap = preview,
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.size(88.dp).clip(CircleShape),
                    )
                } else {
                    EmployeeAvatar(
                        state.fullName,
                        state.photoUrl.takeIf { !state.removePhoto },
                        88.dp,
                    )
                }
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    OutlinedButton(onClick = {
                        picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                    }) { Text(if (hasPhoto) "Fénykép cseréje" else "Fénykép kiválasztása") }
                    if (hasPhoto) {
                        TextButton(onClick = {
                            viewModel.set { it.copy(newPhoto = null, removePhoto = true) }
                        }) { Text("Fénykép törlése") }
                    }
                }
            }
            AutoCrmTextField(
                value = state.fullName,
                onValueChange = { v -> viewModel.set { it.copy(fullName = v) } },
                label = "Név *",
                // "Kovács Anna": every part of a name starts with a capital.
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                    capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Words,
                ),
                modifier = Modifier.fillMaxWidth(),
            )
            AutoCrmTextField(
                value = state.companyPhone,
                onValueChange = { v -> viewModel.set { it.copy(companyPhone = v) } },
                label = "Céges telefonszám",
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Phone),
            )
            AutoCrmTextField(
                value = state.personalPhone,
                onValueChange = { v -> viewModel.set { it.copy(personalPhone = v) } },
                label = "Személyes telefonszám",
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Phone),
            )
            AutoCrmTextField(
                value = state.email,
                onValueChange = { v -> viewModel.set { it.copy(email = v) } },
                label = "E-mail",
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Email),
            )
            PrimaryButton(
                text = if (state.busy) "Mentés…" else "Mentés",
                onClick = { viewModel.save(employeeId, onSaved) },
                enabled = !state.busy && state.fullName.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            )
            if (employeeId != null && state.loaded) {
                TextButton(
                    onClick = {
                        if (state.archived) viewModel.toggleArchived(employeeId, onSaved)
                        else confirmArchive = true
                    },
                    enabled = !state.busy,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(if (state.archived) "Visszavétel" else "Kilépettként jelöl") }
            }
        }
    }

    if (confirmArchive && employeeId != null) {
        DialogShell(
            title = "Kilépettként jelöli?",
            onDismiss = { confirmArchive = false },
            actions = {
                TextButton(onClick = { confirmArchive = false }) { Text("Mégse") }
                TextButton(onClick = {
                    confirmArchive = false
                    viewModel.toggleArchived(employeeId, onSaved)
                }) { Text("Megerősítés") }
            },
        ) {
            Text(
                "A munkatárs eltűnik az alapértelmezett listából, de az adatai megmaradnak.",
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}
