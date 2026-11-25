package com.vex.connect.navigation

import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Devices
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.SwapVert
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.devices.DevicesScreen
import com.vex.connect.ui.home.HomeScreen
import com.vex.connect.ui.pair.PairScreen
import com.vex.connect.ui.settings.SettingsScreen
import com.vex.connect.ui.transfers.TransfersScreen

/** The pairing flow is a full screen above the tabs (no bottom bar). */
const val PAIR_ROUTE = "pair"

/** The four top-level destinations, in bottom-bar order. */
enum class Destination(val route: String, val label: String, val icon: ImageVector) {
    Home("home", "Home", Icons.Rounded.Home),
    Devices("devices", "Devices", Icons.Rounded.Devices),
    Transfers("transfers", "Transfers", Icons.Rounded.SwapVert),
    Settings("settings", "Settings", Icons.Rounded.Settings),
}

@Composable
fun MainScaffold(factory: AppViewModelFactory) {
    val nav = rememberNavController()
    val backStack by nav.currentBackStackEntryAsState()
    val current = backStack?.destination

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        bottomBar = {
            if (current?.route != PAIR_ROUTE) NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer) {
                Destination.entries.forEach { dest ->
                    val selected = current?.hierarchy?.any { it.route == dest.route } == true
                    NavigationBarItem(
                        selected = selected,
                        onClick = {
                            nav.navigate(dest.route) {
                                popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                                launchSingleTop = true
                                restoreState = true
                            }
                        },
                        icon = { Icon(dest.icon, contentDescription = dest.label) },
                        label = { Text(dest.label) },
                        colors = NavigationBarItemDefaults.colors(
                            indicatorColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
                            selectedIconColor = MaterialTheme.colorScheme.primary,
                            selectedTextColor = MaterialTheme.colorScheme.primary,
                        ),
                    )
                }
            }
        },
    ) { inner: PaddingValues ->
        NavHost(
            navController = nav,
            startDestination = Destination.Home.route,
            enterTransition = { fadeIn() },
            exitTransition = { fadeOut() },
            popEnterTransition = { fadeIn() },
            popExitTransition = { fadeOut() },
        ) {
            composable(Destination.Home.route) { HomeScreen(factory, inner, onPair = { nav.navigate(PAIR_ROUTE) }) }
            composable(Destination.Devices.route) { DevicesScreen(factory, inner, onPair = { nav.navigate(PAIR_ROUTE) }) }
            composable(PAIR_ROUTE) { PairScreen(factory, inner, onClose = { nav.popBackStack() }) }
            composable(Destination.Transfers.route) { TransfersScreen(factory, inner) }
            composable(Destination.Settings.route) { SettingsScreen(factory, inner) }
        }
    }
}
