package com.omnipresent.ui

import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument

@Composable
fun AppNavigation() {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            HomeScreen(onScanClick = {
                navController.navigate("scanner")
            })
        }
        composable("scanner") {
            ScannerScreen(onQrScanned = { qrUri ->
                val uri = Uri.parse(qrUri)
                val ip = uri.host ?: ""
                val port = uri.port
                val token = uri.getQueryParameter("token")?.toIntOrNull() ?: 0
                
                if (ip.isNotEmpty() && port != -1) {
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
                    navController.popBackStack("home", inclusive = false)
                }
            )
        }
    }
}
