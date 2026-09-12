package hu.autotherm.autocrm

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Inventory2
import androidx.compose.material.icons.filled.People
import androidx.compose.material.icons.filled.TrackChanges
import androidx.compose.material.icons.filled.Upload
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import hu.autotherm.autocrm.ui.emails.EmailDetailScreen
import hu.autotherm.autocrm.ui.emails.EmailDetailViewModel
import hu.autotherm.autocrm.ui.emails.EmailListScreen
import hu.autotherm.autocrm.ui.emails.EmailListViewModel
import hu.autotherm.autocrm.ui.leads.LeadDetailScreen
import hu.autotherm.autocrm.ui.leads.LeadDetailViewModel
import hu.autotherm.autocrm.ui.leads.LeadListScreen
import hu.autotherm.autocrm.ui.leads.LeadListViewModel
import hu.autotherm.autocrm.ui.login.LoginScreen
import hu.autotherm.autocrm.ui.login.LoginViewModel
import hu.autotherm.autocrm.ui.orders.OrderDetailScreen
import hu.autotherm.autocrm.ui.orders.OrderDetailViewModel
import hu.autotherm.autocrm.ui.orders.OrderListScreen
import hu.autotherm.autocrm.ui.orders.OrderListViewModel
import hu.autotherm.autocrm.ui.partners.PartnerDetailScreen
import hu.autotherm.autocrm.ui.partners.PartnerDetailViewModel
import hu.autotherm.autocrm.ui.partners.PartnerListScreen
import hu.autotherm.autocrm.ui.partners.PartnerListViewModel
import hu.autotherm.autocrm.ui.photos.OrderPhotoViewModel
import hu.autotherm.autocrm.ui.queue.QueueScreen
import hu.autotherm.autocrm.ui.queue.QueueViewModel
import hu.autotherm.autocrm.ui.server.ServerSetupScreen
import hu.autotherm.autocrm.ui.server.ServerSetupViewModel
import hu.autotherm.autocrm.ui.theme.AutoCrmTheme

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val app = application as AutoCrmApp
        setContent {
            AutoCrmTheme {
                val server by app.serverStore.baseUrl.collectAsState(initial = null)
                val account by app.sessionStore.account.collectAsState(initial = null)
                var editingServer by rememberSaveable { mutableStateOf(false) }

                when {
                    // Nothing can be asked of the user before the app knows where to ask it.
                    server == null || editingServer -> ServerSetupScreen(
                        viewModel = viewModel { ServerSetupViewModel(app.serverStore, app.api) },
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

                    else -> AppScaffold(app)
                }
            }
        }
    }
}

/**
 * The same sections as the web client's sidebar, minus the ones that need a wide screen.
 *
 * Orders first, because that is where the work is. There is no camera tab: photographing
 * happens inside a job, through the phone's own camera app — a photo belongs to a vehicle,
 * and choosing the job afterwards is how photos end up on the wrong one.
 */
private sealed class Tab(val route: String, val label: String, val icon: ImageVector) {
    data object Orders : Tab("orders", "Munkák", Icons.Filled.Inventory2)
    data object Leads : Tab("leads", "Leadek", Icons.Filled.TrackChanges)
    data object Partners : Tab("partners", "Ügyfelek", Icons.Filled.People)
    data object Emails : Tab("emails", "E-mailek", Icons.Filled.Email)
    data object Queue : Tab("queue", "Sor", Icons.Filled.Upload)
}

private val TABS = listOf(Tab.Orders, Tab.Leads, Tab.Partners, Tab.Emails, Tab.Queue)

@Composable
private fun AppScaffold(app: AutoCrmApp) {
    val navController = rememberNavController()
    val outstanding by app.uploadQueue.outstanding.collectAsState(initial = 0)
    val blocked by app.uploadQueue.blocked.collectAsState(initial = 0)

    Scaffold(
        bottomBar = {
            val entry by navController.currentBackStackEntryAsState()
            NavigationBar {
                TABS.forEach { tab ->
                    val selected = entry?.destination?.hierarchy?.any { it.route == tab.route } == true
                    NavigationBarItem(
                        selected = selected,
                        onClick = { navController.navigateToTab(tab.route) },
                        icon = {
                            if (tab is Tab.Queue && (outstanding > 0 || blocked > 0)) {
                                BadgedBox(badge = { Badge { Text("${outstanding + blocked}") } }) {
                                    Icon(tab.icon, contentDescription = tab.label)
                                }
                            } else {
                                Icon(tab.icon, contentDescription = tab.label)
                            }
                        },
                        label = { Text(tab.label) },
                    )
                }
            }
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            NavHost(navController = navController, startDestination = Tab.Orders.route) {
                composable(Tab.Orders.route) {
                    OrderListScreen(
                        viewModel = viewModel { OrderListViewModel(app.api) },
                        onOpen = { id -> navController.navigate("order/$id") },
                    )
                }
                // "order/{id}", not "orders/{id}": a detail route sharing a prefix with a tab
                // route makes the bottom bar's selected-state matching ambiguous.
                composable("order/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    OrderDetailScreen(
                        orderId = id,
                        viewModel = viewModel { OrderDetailViewModel(app.api, app.sessionStore) },
                        photoViewModel = viewModel { OrderPhotoViewModel(app.uploadQueue, app.capturePrefs) },
                        onOpenOrder = { other -> navController.navigate("order/$other") },
                    )
                }
                composable(Tab.Leads.route) {
                    LeadListScreen(
                        viewModel = viewModel { LeadListViewModel(app.api) },
                        onOpen = { id -> navController.navigate("lead/$id") },
                    )
                }
                composable("lead/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    LeadDetailScreen(
                        leadId = id,
                        viewModel = viewModel { LeadDetailViewModel(app.api) },
                        onOpenOrder = { orderId -> navController.navigate("order/$orderId") },
                    )
                }
                composable(Tab.Partners.route) {
                    PartnerListScreen(
                        viewModel = viewModel { PartnerListViewModel(app.api) },
                        onOpenPartner = { id -> navController.navigate("partner/$id") },
                    )
                }
                composable("partner/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    PartnerDetailScreen(
                        partnerId = id,
                        viewModel = viewModel { PartnerDetailViewModel(app.api) },
                        onOpenOrder = { orderId -> navController.navigate("order/$orderId") },
                        onOpenLead = { leadId -> navController.navigate("lead/$leadId") },
                    )
                }
                composable(Tab.Emails.route) {
                    EmailListScreen(
                        viewModel = viewModel { EmailListViewModel(app.api) },
                        onOpen = { id -> navController.navigate("email/$id") },
                    )
                }
                composable("email/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    EmailDetailScreen(
                        emailId = id,
                        viewModel = viewModel { EmailDetailViewModel(app.api) },
                        onOpenOrder = { orderId -> navController.navigate("order/$orderId") },
                    )
                }
                composable(Tab.Queue.route) {
                    QueueScreen(viewModel = viewModel { QueueViewModel(app.uploadQueue) })
                }
            }
        }
    }
}

/** Tab switching that does not stack twenty copies of a list on the back stack. */
private fun NavHostController.navigateToTab(route: String) {
    navigate(route) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}
