package com.omnipresent.ui

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Coffee
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun HomeScreen(
    isDarkTheme: Boolean,
    onThemeToggle: () -> Unit,
    onFindServerClick: () -> Unit,
    onScanClick: () -> Unit,
    canReconnect: Boolean,
    onReconnectClick: () -> Unit
) {
    val context = LocalContext.current

    Box(
        modifier = Modifier.fillMaxSize()
    ) {
        // Theme toggle button in top right
        IconButton(
            onClick = onThemeToggle,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(16.dp)
        ) {
            Icon(
                imageVector = if (isDarkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                contentDescription = "Toggle Theme"
            )
        }

        Column(
            modifier = Modifier.align(Alignment.Center),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            // Main App Title
            Text(
                text = "Omnipresent",
                style = MaterialTheme.typography.displayLarge,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.primary
            )

            // Subtitle / Description
            Text(
                text = "Remote Trackpad & Gestures",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(modifier = Modifier.height(48.dp))

            // Find Server Button
            Button(
                onClick = onFindServerClick,
                modifier = Modifier
                    .padding(vertical = 8.dp)
                    .height(56.dp)
                    .fillMaxWidth(0.7f)
            ) {
                Icon(
                    imageVector = Icons.Default.Search,
                    contentDescription = "Find Server Icon"
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = "Find Server",
                    style = MaterialTheme.typography.titleMedium
                )
            }

            // Scan QR Button
            Button(
                onClick = onScanClick,
                modifier = Modifier
                    .padding(vertical = 8.dp)
                    .height(56.dp) // Taller for better touch target
                    .fillMaxWidth(0.7f) // Takes up 70% of screen width
            ) {
                Icon(
                    imageVector = Icons.Default.QrCodeScanner,
                    contentDescription = "Scan QR Code Icon"
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = "Scan QR",
                    style = MaterialTheme.typography.titleMedium
                )
            }

            if (canReconnect) {
                Button(
                    onClick = onReconnectClick,
                    modifier = Modifier
                        .padding(vertical = 8.dp)
                        .height(56.dp)
                        .fillMaxWidth(0.7f),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.secondary
                    )
                ) {
                    Icon(
                        imageVector = Icons.Default.Refresh,
                        contentDescription = "Reconnect"
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                    Text(
                        text = "Reconnect",
                        style = MaterialTheme.typography.titleMedium
                    )
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            // Ko-fi Button
            FilledTonalButton(
                onClick = {
                    val intent = Intent(Intent.ACTION_VIEW, Uri.parse("https://ko-fi.com/U7U51VV3PC"))
                    context.startActivity(intent)
                },
                modifier = Modifier
                    .padding(vertical = 8.dp)
                    .height(48.dp)
                    .fillMaxWidth(0.5f)
            ) {
                Icon(
                    imageVector = Icons.Default.Coffee,
                    contentDescription = "Ko-fi"
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text("Support me on Ko-fi")
            }
        }
    }
}