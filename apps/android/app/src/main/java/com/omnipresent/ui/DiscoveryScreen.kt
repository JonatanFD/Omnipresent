package com.omnipresent.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.omnipresent.network.DiscoveredServer
import com.omnipresent.network.DiscoveryClient
import kotlinx.coroutines.delay

@Composable
fun DiscoveryScreen(
    onServerFound: (DiscoveredServer) -> Unit,
    onDiscoveryFailed: () -> Unit
) {
    var statusText by remember { mutableStateOf("Looking for server on network...") }
    val discoveryClient = remember { DiscoveryClient() }

    LaunchedEffect(Unit) {
        val server = discoveryClient.discoverServer()
        if (server != null) {
            statusText = "Server found! Connecting..."
            delay(500) // Brief delay so user sees the message
            onServerFound(server)
        } else {
            statusText = "Could not find server."
            delay(1000) // Brief delay before transitioning
            onDiscoveryFailed()
        }
    }

    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            CircularProgressIndicator(
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(64.dp)
            )
            
            Spacer(modifier = Modifier.height(32.dp))
            
            Text(
                text = statusText,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onBackground
            )
        }
    }
}
