package com.omnipresent.ui

import android.app.Activity
import android.content.pm.ActivityInfo
import androidx.compose.foundation.background
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.omnipresent.network.UdpClient
import com.omnipresent.protocol.TrackpadMessage
import com.omnipresent.protocol.trackpadMessage
import kotlinx.coroutines.launch
import kotlin.math.abs

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

    val context = LocalContext.current
    DisposableEffect(Unit) {
        val activity = context as? Activity
        val originalOrientation = activity?.requestedOrientation ?: ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE

        onDispose {
            udpClient.close()
            activity?.requestedOrientation = originalOrientation
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF121212))
            // 1. GESTOR DE CLICS (Un solo dedo y doble clic)
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.LEFT_CLICK, phase = TrackpadMessage.PhaseType.START))
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.LEFT_CLICK, phase = TrackpadMessage.PhaseType.END))
                        }
                    },
                    onDoubleTap = {
                        coroutineScope.launch {
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.DOUBLE_CLICK, phase = TrackpadMessage.PhaseType.START))
                            udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.DOUBLE_CLICK, phase = TrackpadMessage.PhaseType.END))
                        }
                    }
                )
            }
            // 2. GESTOR DE MOVIMIENTO Y SCROLL (Captura continua)
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    var maxPointers = 0
                    var gestureMoved = false

                    while (true) {
                        val event = awaitPointerEvent()
                        val activePointers = event.changes.filter { it.pressed }
                        val numFingers = activePointers.size

                        if (numFingers > maxPointers) {
                            maxPointers = numFingers
                        }

                        if (numFingers > 0) {
                            var avgDx = 0f
                            var avgDy = 0f

                            for (pointer in activePointers) {
                                val change = pointer.positionChange()
                                avgDx += change.x
                                avgDy += change.y
                                // Quitamos el pointer.consume() de aquí para no bloquear el clic
                            }

                            avgDx /= numFingers
                            avgDy /= numFingers

                            // Si superamos el umbral de movimiento, o ya estábamos moviéndonos
                            if (gestureMoved || abs(avgDx) > 1.5f || abs(avgDy) > 1.5f) {
                                gestureMoved = true

                                // AHORA SÍ consumimos el evento porque confirmamos que es un arrastre/scroll
                                for (pointer in activePointers) {
                                    pointer.consume()
                                }

                                var action = TrackpadMessage.ActionType.NO_ACTION

                                if (numFingers == 2) {
                                    action = if (abs(avgDy) > abs(avgDx)) {
                                        TrackpadMessage.ActionType.VERTICAL_SCROLL
                                    } else {
                                        TrackpadMessage.ActionType.HORIZONTAL_SCROLL
                                    }
                                }

                                coroutineScope.launch {
                                    udpClient.send(
                                        createMessage(
                                            token = token,
                                            dx = avgDx,
                                            dy = avgDy,
                                            action = action,
                                            phase = TrackpadMessage.PhaseType.UPDATE
                                        )
                                    )
                                }
                            }
                        } else {
                            // Cero dedos en pantalla

                            // Si tocaron 2 dedos y NO se movieron, es un Clic Derecho
                            if (maxPointers == 2 && !gestureMoved) {
                                coroutineScope.launch {
                                    udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.RIGHT_CLICK, phase = TrackpadMessage.PhaseType.START))
                                    udpClient.send(createMessage(token, action = TrackpadMessage.ActionType.RIGHT_CLICK, phase = TrackpadMessage.PhaseType.END))
                                }
                            }

                            // Reiniciar variables para el próximo toque
                            maxPointers = 0
                            gestureMoved = false
                        }
                    }
                }
            }
    ) {
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