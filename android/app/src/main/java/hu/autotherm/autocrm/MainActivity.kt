package hu.autotherm.autocrm

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.compose.ui.platform.LocalContext
import hu.autotherm.autocrm.data.notifications.NotificationPollWorker
import hu.autotherm.autocrm.ui.hr.LeaveScreen
import hu.autotherm.autocrm.ui.search.SearchScreen
import hu.autotherm.autocrm.ui.search.SearchViewModel
import hu.autotherm.autocrm.util.formatTime
import hu.autotherm.autocrm.ui.hr.LeaveViewModel
import hu.autotherm.autocrm.ui.notifications.NotificationsScreen
import hu.autotherm.autocrm.ui.notifications.NotificationsViewModel
import kotlinx.coroutines.delay
import androidx.fragment.app.FragmentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.ui.Alignment
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material.icons.automirrored.filled.TrendingUp
import androidx.compose.material.icons.filled.BarChart
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.CloudUpload
import androidx.compose.material.icons.filled.Contacts
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Badge
import androidx.compose.material.icons.filled.ManageAccounts
import androidx.compose.material.icons.filled.PhotoCamera
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.LocalParking
import hu.autotherm.autocrm.ui.common.AppLock
import hu.autotherm.autocrm.ui.common.AppLockGate
import hu.autotherm.autocrm.ui.share.SharedFile
import hu.autotherm.autocrm.ui.share.ShareScreen
import hu.autotherm.autocrm.ui.share.ShareViewModel
import hu.autotherm.autocrm.ui.yard.YardScreen
import hu.autotherm.autocrm.widget.TasksWidget
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.NavigationDrawerItemDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import hu.autotherm.autocrm.data.auth.SessionStore
import hu.autotherm.autocrm.data.prefs.ThemePrefs
import hu.autotherm.autocrm.ui.capture.CaptureScreen
import hu.autotherm.autocrm.ui.capture.CaptureViewModel
import hu.autotherm.autocrm.ui.admin.UsersScreen
import hu.autotherm.autocrm.ui.admin.UsersViewModel
import hu.autotherm.autocrm.ui.hr.HrEditScreen
import hu.autotherm.autocrm.ui.hr.HrEditViewModel
import hu.autotherm.autocrm.ui.hr.HrListScreen
import hu.autotherm.autocrm.ui.hr.HrListViewModel
import hu.autotherm.autocrm.ui.directory.DirectoryScreen
import hu.autotherm.autocrm.ui.directory.DirectoryViewModel
import hu.autotherm.autocrm.ui.emails.EmailComposeScreen
import hu.autotherm.autocrm.ui.emails.EmailComposeViewModel
import hu.autotherm.autocrm.ui.emails.EmailDetailScreen
import hu.autotherm.autocrm.ui.emails.EmailDetailViewModel
import hu.autotherm.autocrm.ui.emails.EmailListScreen
import hu.autotherm.autocrm.ui.emails.EmailListViewModel
import hu.autotherm.autocrm.ui.inspection.InspectionHomeScreen
import hu.autotherm.autocrm.ui.inspection.InspectionHomeViewModel
import hu.autotherm.autocrm.ui.inspection.InspectionServerScreen
import hu.autotherm.autocrm.ui.inspection.InspectionServerViewModel
import hu.autotherm.autocrm.ui.inspection.WalkaroundScreen
import hu.autotherm.autocrm.ui.inspection.WalkaroundViewModel
import hu.autotherm.autocrm.ui.leads.LeadDetailScreen
import hu.autotherm.autocrm.ui.leads.LeadDetailViewModel
import hu.autotherm.autocrm.ui.leads.LeadEditScreen
import hu.autotherm.autocrm.ui.leads.LeadEditViewModel
import hu.autotherm.autocrm.ui.leads.LeadListScreen
import hu.autotherm.autocrm.ui.leads.LeadListViewModel
import hu.autotherm.autocrm.ui.login.ChangePasswordScreen
import hu.autotherm.autocrm.ui.login.ChangePasswordViewModel
import hu.autotherm.autocrm.ui.login.LoginScreen
import hu.autotherm.autocrm.ui.login.LoginViewModel
import hu.autotherm.autocrm.ui.orders.OrderDetailScreen
import hu.autotherm.autocrm.ui.orders.OrderDetailViewModel
import hu.autotherm.autocrm.ui.orders.OrderEditScreen
import hu.autotherm.autocrm.ui.orders.OrderEditViewModel
import hu.autotherm.autocrm.ui.orders.OrderListScreen
import hu.autotherm.autocrm.ui.orders.OrderListViewModel
import hu.autotherm.autocrm.ui.partners.PartnerDetailScreen
import hu.autotherm.autocrm.ui.partners.PartnerDetailViewModel
import hu.autotherm.autocrm.ui.partners.PartnerEditScreen
import hu.autotherm.autocrm.ui.partners.PartnerEditViewModel
import hu.autotherm.autocrm.ui.photos.OrderPhotoViewModel
import hu.autotherm.autocrm.ui.picker.OrderPickerScreen
import hu.autotherm.autocrm.ui.picker.OrderPickerViewModel
import hu.autotherm.autocrm.ui.queue.QueueScreen
import hu.autotherm.autocrm.ui.queue.QueueViewModel
import hu.autotherm.autocrm.ui.reports.ReportsScreen
import hu.autotherm.autocrm.ui.reports.ReportsViewModel
import hu.autotherm.autocrm.ui.settings.AppearanceScreen
import hu.autotherm.autocrm.ui.settings.AppearanceViewModel
import hu.autotherm.autocrm.ui.server.ServerSetupScreen
import hu.autotherm.autocrm.ui.server.ServerSetupViewModel
import hu.autotherm.autocrm.ui.tasks.TasksScreen
import hu.autotherm.autocrm.ui.tasks.TasksViewModel
import hu.autotherm.autocrm.ui.theme.AutoCrmTheme
import hu.autotherm.autocrm.ui.theme.Steel500
import kotlinx.coroutines.launch

// A FragmentActivity (a ComponentActivity underneath) because the system's biometric
// prompt hosts itself in a fragment (app lock, 0049).
class MainActivity : FragmentActivity() {

    /** Set by a tapped system notification; the scaffold navigates to it once and clears it. */
    private var pendingRoute by mutableStateOf<String?>(null)

    /** Files another app shared with us ("Megosztás → AutoCRM"), waiting for a target. */
    private var pendingShare by mutableStateOf<List<SharedFile>?>(null)

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        pendingRoute = intent.getStringExtra(NotificationPollWorker.EXTRA_ROUTE)
        sharedFiles(intent)?.let { pendingShare = it }
    }

    override fun onStart() {
        super.onStart()
        AppLock.onForeground(this)
        lifecycleScope.launch { TasksWidget.refresh(this@MainActivity) }
    }

    override fun onStop() {
        super.onStop()
        AppLock.onBackground()
    }

    /** The files of a SEND / SEND_MULTIPLE intent, or null for anything else. */
    @Suppress("DEPRECATION")
    private fun sharedFiles(intent: Intent?): List<SharedFile>? {
        intent ?: return null
        val uris: List<android.net.Uri> = when (intent.action) {
            Intent.ACTION_SEND -> listOfNotNull(
                if (Build.VERSION.SDK_INT >= 33) {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM, android.net.Uri::class.java)
                } else {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM)
                },
            )
            Intent.ACTION_SEND_MULTIPLE ->
                if (Build.VERSION.SDK_INT >= 33) {
                    intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, android.net.Uri::class.java).orEmpty()
                } else {
                    intent.getParcelableArrayListExtra<android.net.Uri>(Intent.EXTRA_STREAM).orEmpty()
                }
            else -> return null
        }
        if (uris.isEmpty()) return null
        return uris.take(20).map { SharedFile(it, contentResolver.getType(it) ?: intent.type) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        pendingRoute = intent?.getStringExtra(NotificationPollWorker.EXTRA_ROUTE)
        pendingShare = sharedFiles(intent)
        val app = application as AutoCrmApp
        setContent {
            val themeMode by app.themePrefs.mode.collectAsState(initial = ThemePrefs.MODE_SYSTEM)
            val amoled by app.themePrefs.amoled.collectAsState(initial = false)
            val dark = when (themeMode) {
                ThemePrefs.MODE_LIGHT -> false
                ThemePrefs.MODE_DARK -> true
                else -> isSystemInDarkTheme()
            }
            AutoCrmTheme(darkTheme = dark, amoled = amoled) {
                val server by app.serverStore.baseUrl.collectAsState(initial = null)
                val account by app.sessionStore.account.collectAsState(initial = null)
                var editingServer by rememberSaveable { mutableStateOf(false) }

                when {
                    // Nothing can be asked of the user before the app knows where to ask it.
                    server == null || editingServer -> ServerSetupScreen(
                        viewModel = viewModel { ServerSetupViewModel(app.serverStore, app.api, app.sessionStore) },
                        onSaved = { editingServer = false },
                        onCancel = if (server != null) ({ editingServer = false }) else null,
                    )

                    // `null` covers both "not signed in" and "DataStore has not answered
                    // yet". The login screen flashing for one frame is better than a screen
                    // full of 401s, which is what the alternative produces.
                    account == null -> LoginScreen(
                        viewModel = viewModel { LoginViewModel(app.api, app.sessionStore) },
                        serverAddress = server,
                        onChangeServer = { editingServer = true },
                        onSignedIn = { /* the account flow re-composes this away */ },
                    )

                    // Forced rotation: the backend 422s everything except me/password/
                    // logout until this is done, so it gates the whole app like login.
                    // (Delegated val: no smart cast, hence !!.)
                    account!!.mustChangePassword -> ChangePasswordScreen(
                        viewModel = viewModel { ChangePasswordViewModel(app.api, app.sessionStore) },
                        onChanged = { /* me() already re-saved; the flow moves on */ },
                        onSignedOut = { /* session cleared; the flow falls back to login */ },
                    )

                    else -> AppLockGate {
                        val share = pendingShare
                        if (share != null) {
                            ShareScreen(
                                files = share,
                                viewModel = viewModel(key = "share") { ShareViewModel(app) },
                                onDone = { target ->
                                    pendingShare = null
                                    if (target != null) {
                                        pendingRoute = if (target.kind == "lead") "lead/${target.id}" else "order/${target.id}"
                                    }
                                },
                            )
                        } else {
                            AppScaffold(
                                app,
                                account!!,
                                pendingRoute = pendingRoute,
                                onRouteHandled = { pendingRoute = null },
                            )
                        }
                    }
                }
            }
        }
    }
}

/**
 * Every section in one drawer: orders and leads for the work, the Névjegyzék for
 * people, emails, the fitter's own tasks, the Monday numbers, and the upload queue.
 * Detail and editor routes are not destinations — they open on top and highlight
 * their parent section.
 */
private sealed class Destination(val route: String, val label: String, val icon: ImageVector) {
    /** One box for orders, partners, leads, contacts, emails (and staff, with HR access). */
    data object Search : Destination("search", "Keresés", Icons.Filled.Search)
    data object Orders : Destination("orders", "Munkák", Icons.Filled.Build)
    /** Capture-first photography: pick the van, shoot, pick the next one. */
    data object Capture : Destination("capture", "Fotózás", Icons.Filled.PhotoCamera)
    data object Leads : Destination("leads", "Leadek", Icons.AutoMirrored.Filled.TrendingUp)
    data object Directory : Destination("directory", "Névjegyzék", Icons.Filled.Contacts)
    data object Emails : Destination("emails", "E-mailek", Icons.Filled.Email)
    data object Tasks : Destination("tasks", "Feladatok", Icons.Filled.Checklist)
    data object Reports : Destination("reports", "Jelentések", Icons.Filled.BarChart)
    /** Where each vehicle stands in the yard and the bays (0048). */
    data object Yard : Destination("yard", "Udvar", Icons.Filled.LocalParking)
    /** Staff directory: only for admins and users an admin granted HR access. */
    data object Hr : Destination("hr", "HR", Icons.Filled.Badge)
    /** Every account and its rights: admins only. */
    data object Users : Destination("users", "Felhasználók", Icons.Filled.ManageAccounts)
    /** What happened that needs attention: new website leads. */
    data object Notifications : Destination("notifications", "Értesítések", Icons.Filled.Notifications)
    data object Queue : Destination("queue", "Sor", Icons.Filled.CloudUpload)
    data object Settings : Destination("settings", "Beállítások", Icons.Filled.Settings)
}

private val DESTINATIONS = listOf(
    Destination.Search,
    Destination.Orders,
    Destination.Capture,
    Destination.Leads,
    Destination.Directory,
    Destination.Emails,
    Destination.Tasks,
    Destination.Reports,
    Destination.Yard,
    Destination.Notifications,
    Destination.Hr,
    Destination.Users,
    Destination.Queue,
    Destination.Settings,
)

/** Which drawer section a route (including detail/editor routes) belongs to. */
private fun parentOf(route: String?): Destination = when {
    route == null -> Destination.Orders
    route.startsWith("order") -> Destination.Orders
    route.startsWith("capture") -> Destination.Capture
    route.startsWith("lead") -> Destination.Leads
    route.startsWith("partner") || route.startsWith("directory") -> Destination.Directory
    route.startsWith("email") -> Destination.Emails
    route.startsWith("task") -> Destination.Tasks
    route.startsWith("report") -> Destination.Reports
    route.startsWith("yard") -> Destination.Yard
    route.startsWith("search") -> Destination.Search
    route.startsWith("notifications") -> Destination.Notifications
    route.startsWith("hr") -> Destination.Hr
    route.startsWith("users") -> Destination.Users
    route.startsWith("queue") -> Destination.Queue
    // Inspection walkaround/history always opens from an order.
    route.startsWith("inspection") -> Destination.Orders
    else -> Destination.Orders
}

@Composable
private fun AppScaffold(
    app: AutoCrmApp,
    account: SessionStore.Account,
    pendingRoute: String?,
    onRouteHandled: () -> Unit,
) {
    val navController = rememberNavController()
    val context = LocalContext.current
    val offlineSince by app.api.offlineSince.collectAsState()
    var unreadNotifications by rememberSaveable { mutableStateOf(0L) }
    // Android 13+ asks before an app may show notifications. Asked once the user is in; a
    // refusal is fine, the same items are in the Értesítések list.
    val askPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            askPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
    // The unread count on the drawer item, refreshed while the app is on screen.
    val lifecycleOwner = LocalLifecycleOwner.current
    LaunchedEffect(lifecycleOwner) {
        lifecycleOwner.lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                runCatching { app.api.notifications(unreadOnly = true, limit = 1) }
                    .onSuccess { unreadNotifications = it.unread }
                delay(60_000)
            }
        }
    }
    // A tapped system notification opens what it is about.
    LaunchedEffect(pendingRoute) {
        val route = pendingRoute ?: return@LaunchedEffect
        if (route == Destination.Notifications.route) navController.navigateToTop(route)
        else runCatching { navController.navigate(route) }
        onRouteHandled()
    }
    val drawerState = rememberDrawerState(initialValue = DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val outstanding by app.uploadQueue.outstanding.collectAsState(initial = 0)
    val blocked by app.uploadQueue.blocked.collectAsState(initial = 0)
    val canEdit = account.canEdit
    val openDrawer: () -> Unit = { scope.launch { drawerState.open() } }
    // Who the server says this is can change under a running session (an admin grants or
    // takes away HR access, or re-roles an account). Refresh it on open so the drawer
    // follows without a sign-in; offline, the stored account stands.
    LaunchedEffect(Unit) {
        runCatching { app.api.me() }.onSuccess { app.sessionStore.refreshUser(it.user) }
    }
    var confirmLogout by rememberSaveable { mutableStateOf(false) }
    // A cancelled logout must not stay armed: reopening the drawer starts over.
    LaunchedEffect(drawerState.isClosed) {
        if (drawerState.isClosed) confirmLogout = false
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet {
                Column(Modifier.padding(16.dp)) {
                    Text("AUTOTHERM", style = MaterialTheme.typography.titleLarge)
                    Text(
                        account.displayName,
                        style = MaterialTheme.typography.bodyLarge,
                    )
                    Text(account.email, style = MaterialTheme.typography.labelMedium, color = Steel500)
                }
                HorizontalDivider()
                val entry by navController.currentBackStackEntryAsState()
                val current = parentOf(entry?.destination?.route)
                // Hidden like every other capability gate: the server refuses
                // uploads from roles without media rights anyway.
                DESTINATIONS.filter { dest ->
                    when (dest) {
                        Destination.Capture -> account.canUploadMedia
                        Destination.Hr -> account.hrAccess
                        Destination.Users -> account.isAdmin
                        else -> true
                    }
                }.forEach { dest ->
                    NavigationDrawerItem(
                        label = { Text(dest.label) },
                        selected = current == dest,
                        onClick = {
                            scope.launch { drawerState.close() }
                            navController.navigateToTop(dest.route)
                        },
                        icon = {
                            Icon(dest.icon, contentDescription = null)
                        },
                        badge = {
                            if (dest is Destination.Queue && (outstanding > 0 || blocked > 0)) {
                                Text("${outstanding + blocked}")
                            }
                            if (dest is Destination.Notifications && unreadNotifications > 0) {
                                Text("$unreadNotifications")
                            }
                        },
                        modifier = Modifier.padding(NavigationDrawerItemDefaults.ItemPadding),
                    )
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                NavigationDrawerItem(
                    label = { Text(if (confirmLogout) "Biztos? Koppints újra" else "Kijelentkezés") },
                    selected = false,
                    onClick = {
                        if (confirmLogout) {
                            scope.launch { drawerState.close() }
                            // Best-effort server logout; the local session clears regardless
                            // so a dead token never bricks the app.
                            scope.launch {
                                runCatching { app.api.logout() }
                                app.sessionStore.clear()
                                // What the next user of this phone must not be able to read.
                                app.responseCache.clear()
                            }
                        } else confirmLogout = true
                    },
                    icon = { Icon(Icons.AutoMirrored.Filled.Logout, contentDescription = null) },
                    modifier = Modifier.padding(NavigationDrawerItemDefaults.ItemPadding),
                )
                Spacer(Modifier.height(8.dp))
            }
        },
    ) {
        Column(Modifier.fillMaxSize()) {
        // Shown while the screen below is read from the phone's copy, not the server.
        offlineSince?.let { since ->
            Row(
                Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.secondaryContainer)
                    .padding(horizontal = 16.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(Icons.Filled.CloudOff, contentDescription = null)
                Text(
                    "Nincs kapcsolat: a legutóbbi frissítés adatai (${formatTime(since)})",
                    style = MaterialTheme.typography.labelMedium,
                )
            }
        }
        NavHost(
            navController = navController,
            startDestination = Destination.Orders.route,
            modifier = Modifier.weight(1f),
            // One motion language for the whole app: new screens slide in from the
            // right and fade, leaving screens hold still and fade. Fast enough to
            // feel instant on a shop floor (220ms), present enough to feel spatial.
            enterTransition = {
                slideInHorizontally(tween(220)) { it / 4 } + fadeIn(tween(220))
            },
            exitTransition = {
                fadeOut(tween(180))
            },
            popEnterTransition = {
                fadeIn(tween(180))
            },
            popExitTransition = {
                slideOutHorizontally(tween(220)) { it / 4 } + fadeOut(tween(220))
            },
        ) {
            composable(Destination.Orders.route) {
                OrderListScreen(
                    viewModel = viewModel { OrderListViewModel(app.api) },
                    onOpen = { id -> navController.navigate("order/$id") },
                    onMenu = openDrawer,
                    canEdit = canEdit,
                    onNewOrder = { navController.navigate("order/new/0") },
                )
            }
            composable(
                "order/new/{partnerId}",
                arguments = listOf(navArgument("partnerId") { type = NavType.LongType }),
            ) { entry ->
                val partnerId = entry.arguments?.getLong("partnerId")?.takeIf { it > 0 }
                OrderEditScreen(
                    orderId = null,
                    partnerId = partnerId,
                    viewModel = viewModel { OrderEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = { id ->
                        navController.navigate("order/$id") {
                            popUpTo("order/new/{partnerId}") { inclusive = true }
                        }
                    },
                )
            }
            // "order/{id}", not "orders/{id}": a detail route sharing a prefix with a tab
            // route makes the selected-state matching ambiguous.
                composable("order/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    OrderDetailScreen(
                        orderId = id,
                        viewModel = viewModel { OrderDetailViewModel(app.api, app.sessionStore, app.lookupsCache) },
                        photoViewModel = viewModel { OrderPhotoViewModel(app.uploadQueue, app.capturePrefs, app.api, app.lookupsCache) },
                        onOpenOrder = { other -> navController.navigate("order/$other") },
                        onBack = { navController.popBackStack() },
                        onEditOrder = { navController.navigate("order/$id/edit") },
                        onComposeEmail = { navController.navigate("email/new?orderId=$id") },
                        onInspections = { navController.navigate("inspections/$id") },
                    )
                }
            composable(Destination.Capture.route) {
                OrderPickerScreen(
                    viewModel = viewModel { OrderPickerViewModel(app.api, app.capturePrefs) },
                    onMenu = openDrawer,
                    onChoose = { id -> navController.navigate("capture/$id") },
                )
            }
            composable("capture/{id}") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                CaptureScreen(
                    orderId = id,
                    viewModel = viewModel { CaptureViewModel(app.api) },
                    photoViewModel = viewModel { OrderPhotoViewModel(app.uploadQueue, app.capturePrefs, app.api, app.lookupsCache) },
                    onBack = { navController.popBackStack() },
                )
            }
            composable("order/{id}/edit") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                OrderEditScreen(
                    orderId = id,
                    partnerId = null,
                    viewModel = viewModel { OrderEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = {
                        navController.navigate("order/$id") {
                            popUpTo("order/{id}/edit") { inclusive = true }
                        }
                    },
                )
            }
            composable(Destination.Leads.route) {
                LeadListScreen(
                    viewModel = viewModel { LeadListViewModel(app.api) },
                    onOpen = { id -> navController.navigate("lead/$id") },
                    onMenu = openDrawer,
                    canEdit = canEdit,
                    onNewLead = { navController.navigate("lead/new/0") },
                )
            }
            composable(
                "lead/new/{partnerId}",
                arguments = listOf(navArgument("partnerId") { type = NavType.LongType }),
            ) { entry ->
                val partnerId = entry.arguments?.getLong("partnerId")?.takeIf { it > 0 }
                LeadEditScreen(
                    leadId = null,
                    partnerId = partnerId,
                    viewModel = viewModel { LeadEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = { id ->
                        navController.navigate("lead/$id") {
                            popUpTo("lead/new/{partnerId}") { inclusive = true }
                        }
                    },
                )
            }
            composable("lead/{id}") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                LeadDetailScreen(
                    leadId = id,
                    viewModel = viewModel { LeadDetailViewModel(app.api, app.lookupsCache, app.sessionStore) },
                    onOpenOrder = { orderId -> navController.navigate("order/$orderId") },
                    onBack = { navController.popBackStack() },
                    canEdit = canEdit,
                    onEditLead = { navController.navigate("lead/$id/edit") },
                    onConverted = { orderId ->
                        navController.navigate("order/$orderId") {
                            popUpTo("lead/{id}") { inclusive = true }
                        }
                    },
                )
            }
            composable("lead/{id}/edit") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                LeadEditScreen(
                    leadId = id,
                    partnerId = null,
                    viewModel = viewModel { LeadEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = {
                        navController.navigate("lead/$id") {
                            popUpTo("lead/{id}/edit") { inclusive = true }
                        }
                    },
                )
            }
            composable(Destination.Directory.route) {
                DirectoryScreen(
                    viewModel = viewModel { DirectoryViewModel(app.api) },
                    canEdit = canEdit,
                    onMenu = openDrawer,
                    onOpenPartner = { id -> navController.navigate("partner/$id") },
                    onNewPartner = { navController.navigate("partner/new") },
                    onEditPartner = { id -> navController.navigate("partner/$id/edit") },
                )
            }
            composable("partner/new") {
                PartnerEditScreen(
                    partnerId = null,
                    viewModel = viewModel { PartnerEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = { id ->
                        navController.navigate("partner/$id") {
                            popUpTo("partner/new") { inclusive = true }
                        }
                    },
                )
            }
            composable("partner/{id}") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                PartnerDetailScreen(
                    partnerId = id,
                    viewModel = viewModel { PartnerDetailViewModel(app.api) },
                    onOpenOrder = { orderId -> navController.navigate("order/$orderId") },
                    onOpenLead = { leadId -> navController.navigate("lead/$leadId") },
                    onBack = { navController.popBackStack() },
                    canEdit = canEdit,
                    onEditPartner = { navController.navigate("partner/$id/edit") },
                    onNewOrder = { pid -> navController.navigate("order/new/$pid") },
                    onNewLead = { pid -> navController.navigate("lead/new/$pid") },
                )
            }
            composable("partner/{id}/edit") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                PartnerEditScreen(
                    partnerId = id,
                    viewModel = viewModel { PartnerEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = {
                        navController.navigate("partner/$id") {
                            popUpTo("partner/{id}/edit") { inclusive = true }
                        }
                    },
                )
            }
            composable(Destination.Emails.route) {
                EmailListScreen(
                    viewModel = viewModel { EmailListViewModel(app.api) },
                    onOpen = { id -> navController.navigate("email/$id") },
                    onMenu = openDrawer,
                    canEdit = canEdit,
                    onCompose = { navController.navigate("email/new") },
                )
            }
            composable(
                "email/new?orderId={orderId}&leadId={leadId}",
                arguments = listOf(
                    navArgument("orderId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("leadId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
            ) { entry ->
                val orderId = entry.arguments?.getString("orderId")?.toLongOrNull()
                val leadId = entry.arguments?.getString("leadId")?.toLongOrNull()
                EmailComposeScreen(
                    orderId = orderId,
                    leadId = leadId,
                    viewModel = viewModel { EmailComposeViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSent = { id ->
                        navController.popBackStack()
                        navController.navigate("email/$id")
                    },
                )
            }
            composable("email/{id}") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                EmailDetailScreen(
                    emailId = id,
                    viewModel = viewModel { EmailDetailViewModel(app.api) },
                    onOpenOrder = { orderId -> navController.navigate("order/$orderId") },
                    onBack = { navController.popBackStack() },
                )
            }
            composable(Destination.Tasks.route) {
                TasksScreen(
                    viewModel = viewModel { TasksViewModel(app.api) },
                    onMenu = openDrawer,
                    onOpenTask = { entity, id ->
                        when (entity) {
                            "order" -> navController.navigate("order/$id")
                            "lead" -> navController.navigate("lead/$id")
                            "partner" -> navController.navigate("partner/$id")
                        }
                    },
                )
            }
            composable(Destination.Reports.route) {
                ReportsScreen(
                    viewModel = viewModel { ReportsViewModel(app.api) },
                    onMenu = openDrawer,
                    onOpenOrder = { id -> navController.navigate("order/$id") },
                )
            }
            composable(Destination.Queue.route) {
                QueueScreen(
                    viewModel = viewModel { QueueViewModel(app.uploadQueue, app.api, app.sessionStore, app.lookupsCache) },
                    onMenu = openDrawer,
                )
            }
            // The server refuses these to everyone else; the guard also covers a route that
            // is open while a grant is taken away (the account refresh below follows).
            composable(Destination.Search.route) {
                SearchScreen(
                    viewModel = viewModel { SearchViewModel(app.api) },
                    onMenu = openDrawer,
                    onOpen = { route -> navController.navigate(route) },
                )
            }
            composable(Destination.Notifications.route) {
                NotificationsScreen(
                    viewModel = viewModel { NotificationsViewModel(app.api) },
                    onMenu = openDrawer,
                    onRoute = { route -> navController.navigate(route) },
                )
            }
            composable("hr/leave") {
                if (account.hrAccess) LeaveScreen(
                    viewModel = viewModel { LeaveViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                )
            }
            composable(Destination.Hr.route) {
                if (account.hrAccess) HrListScreen(
                    viewModel = viewModel { HrListViewModel(app.api) },
                    onMenu = openDrawer,
                    onLeave = { navController.navigate("hr/leave") },
                    onNew = { navController.navigate("hr/new") },
                    onEdit = { id -> navController.navigate("hr/$id/edit") },
                )
            }
            composable("hr/new") {
                if (account.hrAccess) HrEditScreen(
                    employeeId = null,
                    viewModel = viewModel { HrEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = { navController.popBackStack() },
                )
            }
            composable(
                "hr/{id}/edit",
                arguments = listOf(navArgument("id") { type = NavType.LongType }),
            ) { entry ->
                val id = entry.arguments?.getLong("id") ?: return@composable
                if (account.hrAccess) HrEditScreen(
                    employeeId = id,
                    viewModel = viewModel { HrEditViewModel(app.api) },
                    onBack = { navController.popBackStack() },
                    onSaved = { navController.popBackStack() },
                )
            }
            composable(Destination.Users.route) {
                if (account.isAdmin) UsersScreen(
                    viewModel = viewModel { UsersViewModel(app.api) },
                    myUserId = account.userId,
                    onMenu = openDrawer,
                )
            }
            composable(Destination.Yard.route) {
                YardScreen(
                    canMove = account.canChangeStage,
                    onOpenOrder = { id -> navController.navigate("order/$id") },
                    onMenu = openDrawer,
                )
            }
            composable(Destination.Settings.route) {
                AppearanceScreen(
                    viewModel = viewModel { AppearanceViewModel(app.themePrefs) },
                    onMenu = openDrawer,
                )
            }
            // ── Handover inspections: phone-only creation, read everywhere ──
            composable("inspections/{orderId}") { entry ->
                val orderId = entry.arguments?.getString("orderId")?.toLongOrNull()
                    ?: return@composable
                InspectionHomeScreen(
                    orderId = orderId,
                    viewModel = viewModel { InspectionHomeViewModel(app) },
                    canInspect = account.canChangeStage,
                    onBack = { navController.popBackStack() },
                    onStart = { kind -> navController.navigate("inspection/new/$orderId/$kind") },
                    onResume = { uuid -> navController.navigate("inspection/walk/$uuid") },
                    onOpenServer = { id -> navController.navigate("inspection/$id") },
                )
            }
            composable("inspection/new/{orderId}/{kind}") { entry ->
                val orderId = entry.arguments?.getString("orderId")?.toLongOrNull()
                    ?: return@composable
                val kind = entry.arguments?.getString("kind")?.takeIf {
                    it == "checkout" || it == "checkin"
                } ?: return@composable
                WalkaroundScreen(
                    uuid = null,
                    orderId = orderId,
                    kind = kind,
                    viewModel = viewModel { WalkaroundViewModel(app) },
                    onExit = { navController.popBackStack() },
                )
            }
            composable("inspection/walk/{uuid}") { entry ->
                val uuid = entry.arguments?.getString("uuid") ?: return@composable
                WalkaroundScreen(
                    uuid = uuid,
                    orderId = 0,
                    kind = "checkout",
                    viewModel = viewModel { WalkaroundViewModel(app) },
                    onExit = { navController.popBackStack() },
                )
            }
            composable("inspection/{id}") { entry ->
                val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                InspectionServerScreen(
                    inspectionId = id,
                    viewModel = viewModel { InspectionServerViewModel(app) },
                    canAnnotate = account.canChangeStage,
                    onBack = { navController.popBackStack() },
                )
            }
        }
        }
    }
}

/** Drawer switching that does not stack twenty copies of a list on the back stack. */
private fun NavHostController.navigateToTop(route: String) {
    navigate(route) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}
