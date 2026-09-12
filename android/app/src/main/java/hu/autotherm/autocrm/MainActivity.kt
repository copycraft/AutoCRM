package hu.autotherm.autocrm

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.Inventory2
import androidx.compose.material.icons.filled.People
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
import hu.autotherm.autocrm.data.prefs.CapturePrefs
import hu.autotherm.autocrm.ui.capture.CaptureScreen
import hu.autotherm.autocrm.ui.capture.CaptureViewModel
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
import hu.autotherm.autocrm.ui.picker.OrderPickerScreen
import hu.autotherm.autocrm.ui.picker.OrderPickerViewModel
import hu.autotherm.autocrm.ui.queue.QueueScreen
import hu.autotherm.autocrm.ui.queue.QueueViewModel
import hu.autotherm.autocrm.ui.theme.AutoCrmTheme

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val app = application as AutoCrmApp
        setContent {
            AutoCrmTheme {
                val account by app.sessionStore.account.collectAsState(initial = null)
                // `null` covers both "not signed in" and "DataStore has not answered yet".
                // The login screen flashing for one frame is better than a screen full of
                // 401s, which is what the alternative produces.
                if (account == null) {
                    LoginScreen(
                        viewModel = viewModel { LoginViewModel(app.api, app.sessionStore) },
                        onSignedIn = { /* the account flow re-composes this away */ },
                    )
                } else {
                    AppScaffold(app)
                }
            }
        }
    }
}

private sealed class Tab(val route: String, val label: String, val icon: ImageVector) {
    data object Capture : Tab("capture", "Fotó", Icons.Filled.CameraAlt)
    data object Orders : Tab("orders", "Munkák", Icons.Filled.Inventory2)
    data object People : Tab("people", "Ügyfelek", Icons.Filled.People)
    data object Queue : Tab("queue", "Sor", Icons.Filled.Upload)
}

private val TABS = listOf(Tab.Capture, Tab.Orders, Tab.People, Tab.Queue)

/**
 * Four tabs, and capture is the first one.
 *
 * The order is the argument: this app opens on the camera because that is the job it does
 * that the web client cannot, and every extra tap between launching it and taking a photo is
 * a photo that ends up in a WhatsApp group instead.
 */
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
            NavHost(navController = navController, startDestination = Tab.Capture.route) {
                composable(Tab.Capture.route) {
                    CaptureScreen(
                        viewModel = viewModel { CaptureViewModel(app.uploadQueue, app.capturePrefs) },
                        onPickOrder = { navController.navigate("picker") },
                        onOpenQueue = { navController.navigateToTab(Tab.Queue.route) },
                    )
                }
                composable("picker") {
                    OrderPickerScreen(
                        viewModel = viewModel { OrderPickerViewModel(app.api, app.capturePrefs) },
                        onChosen = { navController.popBackStack() },
                    )
                }
                composable(Tab.Orders.route) {
                    OrderListScreen(
                        viewModel = viewModel { OrderListViewModel(app.api) },
                        onOpen = { id -> navController.navigate("orders/$id") },
                    )
                }
                composable("orders/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    OrderDetailScreen(
                        orderId = id,
                        viewModel = viewModel { OrderDetailViewModel(app.api, app.sessionStore) },
                        onPhotograph = { detail ->
                            // "Photograph this one" sets the sticky order, so the capture tab
                            // is already pointing at the right van when it opens.
                            app.selectOrderForCapture(
                                CapturePrefs.CurrentOrder(
                                    id = detail.order.id,
                                    number = detail.order.number,
                                    title = detail.order.title,
                                    plate = detail.order.vehiclePlate,
                                ),
                            )
                            navController.navigateToTab(Tab.Capture.route)
                        },
                    )
                }
                composable(Tab.People.route) {
                    PartnerListScreen(
                        viewModel = viewModel { PartnerListViewModel(app.api) },
                        onOpenPartner = { id -> navController.navigate("partners/$id") },
                        onOpenLeads = { navController.navigate("leads") },
                    )
                }
                composable("partners/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    PartnerDetailScreen(
                        partnerId = id,
                        viewModel = viewModel { PartnerDetailViewModel(app.api) },
                        onOpenOrder = { orderId -> navController.navigate("orders/$orderId") },
                        onOpenLead = { leadId -> navController.navigate("leads/$leadId") },
                    )
                }
                composable("leads") {
                    LeadListScreen(
                        viewModel = viewModel { LeadListViewModel(app.api) },
                        onOpen = { id -> navController.navigate("leads/$id") },
                    )
                }
                composable("leads/{id}") { entry ->
                    val id = entry.arguments?.getString("id")?.toLongOrNull() ?: return@composable
                    LeadDetailScreen(
                        leadId = id,
                        viewModel = viewModel { LeadDetailViewModel(app.api) },
                        onOpenOrder = { orderId -> navController.navigate("orders/$orderId") },
                    )
                }
                composable(Tab.Queue.route) {
                    QueueScreen(viewModel = viewModel { QueueViewModel(app.uploadQueue) })
                }
            }
        }
    }
}

/** Tab switching that does not stack twenty copies of the camera on the back stack. */
private fun NavHostController.navigateToTab(route: String) {
    navigate(route) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}

