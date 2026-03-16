package com.omnipresent.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExitToApp
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import com.omnipresent.network.UdpClient
import com.omnipresent.protocol.TrackpadMessage
import com.omnipresent.protocol.trackpadMessage
import kotlinx.coroutines.launch

@Composable
fun TrackpadScreen(
    ip: String,
    port: Int,
    token: Int,
    onExit: () -> Unit
) {
    val coroutineScope = rememberCoroutineScope()
    val udpClient = remember { UdpClient(ip, port) }
    var showMenu by remember { mutableStateOf(false) }

    DisposableEffect(Unit) {
        onDispose {
            udpClient.close()
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF121212))
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.LEFT_CLICK))
                        }
                    },
                    onDoubleTap = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.DOUBLE_CLICK))
                        }
                    },
                    onLongPress = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.RIGHT_CLICK))
                        }
                    }
                )
            }
            .pointerInput(Unit) {
                detectDragGestures(
                    onDragStart = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, phase = TrackpadMessage.PhaseType.START))
                        }
                    },
                    onDragEnd = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, phase = TrackpadMessage.PhaseType.END))
                        }
                    },
                    onDragCancel = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, phase = TrackpadMessage.PhaseType.END))
                        }
                    },
                    onDrag = { change, dragAmount ->
                        change.consume()
                        coroutineScope.launch {
                            // Detect if two fingers are down for scrolling
                            val isScrolling = change.pressed && change.id.value > 0 // Simplified check
                            val action = if (isScrolling) TrackpadMessage.ActionType.VERTICAL_SCROLL else TrackpadMessage.ActionType.NO_ACTION
                            
                            udpClient.send(
                                createMessage(
                                    token,
                                    dx = dragAmount.x,
                                    dy = dragAmount.y,
                                    action = action,
                                    phase = TrackpadMessage.PhaseType.UPDATE
                                )
                            )
                        }
                    }
                )
            }
    ) {
        // Submenu (IconButton) on top-left
        Box(
            modifier = Modifier
                .statusBarsPadding()
                .padding(16.dp)
                .align(Alignment.TopStart)
        ) {
            IconButton(
                onClick = { showMenu = true },
                colors = IconButtonDefaults.iconButtonColors(
                    containerColor = Color.White.copy(alpha = 0.1f),
                    contentColor = Color.White
                )
            ) {
                Icon(Icons.Default.Menu, contentDescription = "Menu")
            }

            DropdownMenu(
                expanded = showMenu,
                onDismissRequest = { showMenu = false }
            ) {
                DropdownMenuItem(
                    text = { Text("Exit") },
                    onClick = {
                        showMenu = false
                        onExit()
                    },
                    leadingIcon = {
                        Icon(Icons.Default.ExitToApp, contentDescription = null)
                    }
                )
            }
        }

        Text(
            text = "Trackpad Active",
            style = MaterialTheme.typography.bodyLarge,
            color = Color.White.copy(alpha = 0.2f),
            modifier = Modifier.align(Alignment.Center)
        )
    }
}

private fun createMessage(
    token: Int,
    dx: Float = 0f,
    dy: Float = 0f,
    action: TrackpadMessage.ActionType = TrackpadMessage.ActionType.NO_ACTION,
    phase: TrackpadMessage.PhaseType = TrackpadMessage.PhaseType.START
): TrackpadMessage {
    return trackpadMessage {
        this.authToken = token
        this.deltaX = dx
        this.deltaY = dy
        this.action = action
        this.phase = phase
        this.timestamp = System.currentTimeMillis()
        this.deviceName = android.os.Build.MODEL
    }
}
