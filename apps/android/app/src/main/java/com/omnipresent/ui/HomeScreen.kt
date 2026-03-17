package com.omnipresent.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun HomeScreen(onScanClick: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Column(
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

            // Scan QR Button
            Button(
                onClick = onScanClick,
                modifier = Modifier
                    .padding(16.dp)
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
        }
    }
}