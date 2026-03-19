package com.omnipresent.ui

import android.content.Context
import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.omnipresent.network.DiscoveredServer

@Composable
fun AppNavigation(
    isDarkTheme: Boolean,
    onThemeToggle: () -> Unit
) {
    val navController = rememberNavController()
    val context = LocalContext.current
    val prefs = context.getSharedPreferences("app_prefs", Context.MODE_PRIVATE)

    NavHost(navController = navController, startDestination = "home") {
        composable("discovery") {
            DiscoveryScreen(
                onServerFound = { server ->
                    prefs.edit()
                        .putString("saved_ip", server.ip)
                        .putInt("saved_port", server.port)
                        .putInt("saved_token", server.token)
                        .apply()

                    navController.navigate("trackpad/${server.ip}/${server.port}/${server.token}") {
                        popUpTo("discovery") { inclusive = true }
                    }
                },
                onDiscoveryFailed = {
                    navController.navigate("home") {
                        popUpTo("discovery") { inclusive = true }
                    }
                }
            )
        }

        composable("home") {
            val savedIp = prefs.getString("saved_ip", null)
            val savedPort = prefs.getInt("saved_port", -1)
            val savedToken = prefs.getInt("saved_token", -1)

            HomeScreen(
                isDarkTheme = isDarkTheme,
                onThemeToggle = onThemeToggle,
                onFindServerClick = {
                    navController.navigate("discovery")
                },
                onScanClick = {
                    navController.navigate("scanner")
                },
                canReconnect = savedIp != null && savedPort != -1 && savedToken != -1,
                onReconnectClick = {
                    if (savedIp != null && savedPort != -1 && savedToken != -1) {
                        navController.navigate("trackpad/$savedIp/$savedPort/$savedToken")
                    }
                }
            )
        }

        composable("scanner") {
            ScannerScreen(onQrScanned = { qrUri ->
                val uri = Uri.parse(qrUri)
                val ip = uri.host ?: ""
                val port = uri.port
                val token = uri.getQueryParameter("token")?.toIntOrNull() ?: 0

                if (ip.isNotEmpty() && port != -1) {
                    prefs.edit()
                        .putString("saved_ip", ip)
                        .putInt("saved_port", port)
                        .putInt("saved_token", token)
                        .apply()

                    navController.navigate("trackpad/$ip/$port/$token") {
                        popUpTo("home")
                    }
                }
            })
        }

        composable(
            route = "trackpad/{ip}/{port}/{token}",
            arguments = listOf(
                navArgument("ip") { type = NavType.StringType },
                navArgument("port") { type = NavType.IntType },
                navArgument("token") { type = NavType.IntType }
            )
        ) { backStackEntry ->
            val ip = backStackEntry.arguments?.getString("ip") ?: ""
            val port = backStackEntry.arguments?.getInt("port") ?: 0
            val token = backStackEntry.arguments?.getInt("token") ?: 0

            TrackpadScreen(
                ip = ip,
                port = port,
                token = token,
                onExit = {
                    // Returns to the home screen
                    navController.popBackStack("home", inclusive = false)
                },
                onScanNewQr = {
                    // Navigates to scanner and clears the trackpad from the backstack
                    navController.navigate("scanner") {
                        popUpTo("home")
                    }
                }
            )
        }
    }
}