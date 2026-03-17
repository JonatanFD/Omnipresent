package com.omnipresent.ui

import android.app.Activity
import android.content.pm.ActivityInfo
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExitToApp
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.QrCodeScanner // 🚀 IMPORTANTE: Nuevo ícono
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.PointerEvent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.unit.dp
import com.omnipresent.network.UdpClient
import com.omnipresent.protocol.TrackpadMessage
import com.omnipresent.protocol.trackpadMessage
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.abs

private enum class GestureIntent {
    UNDECIDED,
    CURSOR_MOVE,
    SCROLL,
    THREE_SWIPE,
    TAP,
    RIGHT_CLICK,
    LONG_PRESS_DRAG
}

@Composable
fun TrackpadScreen(
    ip: String,
    port: Int,
    token: Int,
    onExit: () -> Unit,
    onScanNewQr: () -> Unit
) {
    val coroutineScope = rememberCoroutineScope()
    val udpClient = remember { UdpClient(ip, port) }
    var showMenu by remember { mutableStateOf(false) }

    val viewConfig = LocalViewConfiguration.current
    val doubleTapTimeoutMs = viewConfig.doubleTapTimeoutMillis
    val longPressTimeoutMs = viewConfig.longPressTimeoutMillis

    val context = LocalContext.current
    DisposableEffect(Unit) {
        val activity = context as? Activity
        val originalOrientation = activity?.requestedOrientation
            ?: ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE
        onDispose {
            udpClient.close() // Socket closes automatically when leaving this screen
            activity?.requestedOrientation = originalOrientation
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF121212))
            .pointerInput(Unit) {
                awaitEachGesture {

                    var intent = GestureIntent.UNDECIDED
                    var maxPointers = 0
                    var totalSwipeDx = 0f
                    var totalSwipeDy = 0f
                    var swipeDispatched = false

                    awaitFirstDown(requireUnconsumed = false)
                    maxPointers = 1

                    // Long press detector
                    val longPressJob = coroutineScope.launch {
                        delay(longPressTimeoutMs)
                        if (intent == GestureIntent.UNDECIDED && maxPointers == 1) {
                            intent = GestureIntent.LONG_PRESS_DRAG
                            udpClient.send(
                                createMessage(token,
                                    action = TrackpadMessage.ActionType.LEFT_CLICK,
                                    phase = TrackpadMessage.PhaseType.START)
                            )
                        }
                    }

                    // Main event loop
                    try {
                        while (true) {
                            val event: PointerEvent = awaitPointerEvent()
                            val active = event.changes.filter { it.pressed }
                            val numFingers = active.size

                            if (numFingers > maxPointers) maxPointers = numFingers
                            if (numFingers == 0) break

                            var avgDx = 0f
                            var avgDy = 0f
                            for (p in active) {
                                val ch = p.positionChange()
                                avgDx += ch.x
                                avgDy += ch.y
                            }
                            avgDx /= numFingers
                            avgDy /= numFingers

                            val hasMeaningfulMove = abs(avgDx) > 1.5f || abs(avgDy) > 1.5f

                            // Lock intent once a meaningful move is detected
                            if (intent == GestureIntent.UNDECIDED && hasMeaningfulMove) {
                                longPressJob.cancel()
                                intent = when (numFingers) {
                                    1 -> GestureIntent.CURSOR_MOVE
                                    2 -> GestureIntent.SCROLL
                                    3 -> GestureIntent.THREE_SWIPE
                                    else -> GestureIntent.CURSOR_MOVE
                                }
                            }

                            // Dispatch based on locked intent
                            when (intent) {
                                GestureIntent.CURSOR_MOVE, GestureIntent.LONG_PRESS_DRAG -> {
                                    active.forEach { it.consume() }
                                    coroutineScope.launch {
                                        udpClient.send(
                                            createMessage(token,
                                                dx = avgDx, dy = avgDy,
                                                action = TrackpadMessage.ActionType.NO_ACTION,
                                                phase = TrackpadMessage.PhaseType.UPDATE)
                                        )
                                    }
                                }

                                GestureIntent.SCROLL -> {
                                    active.forEach { it.consume() }
                                    val scrollAction = if (abs(avgDy) > abs(avgDx))
                                        TrackpadMessage.ActionType.VERTICAL_SCROLL
                                    else
                                        TrackpadMessage.ActionType.HORIZONTAL_SCROLL

                                    coroutineScope.launch {
                                        udpClient.send(
                                            createMessage(token,
                                                dx = avgDx, dy = avgDy,
                                                action = scrollAction,
                                                phase = TrackpadMessage.PhaseType.UPDATE)
                                        )
                                    }
                                }

                                GestureIntent.THREE_SWIPE -> {
                                    active.forEach { it.consume() }
                                    totalSwipeDx += avgDx
                                    totalSwipeDy += avgDy

                                    if (!swipeDispatched &&
                                        (abs(totalSwipeDx) > 30f || abs(totalSwipeDy) > 30f)
                                    ) {
                                        swipeDispatched = true
                                        val swipeAction = if (abs(totalSwipeDx) > abs(totalSwipeDy)) {
                                            if (totalSwipeDx > 0) TrackpadMessage.ActionType.SWIPE_RIGHT
                                            else TrackpadMessage.ActionType.SWIPE_LEFT
                                        } else {
                                            if (totalSwipeDy > 0) TrackpadMessage.ActionType.SWIPE_DOWN
                                            else TrackpadMessage.ActionType.SWIPE_UP
                                        }
                                        coroutineScope.launch {
                                            udpClient.send(
                                                createMessage(token,
                                                    action = swipeAction,
                                                    phase = TrackpadMessage.PhaseType.START)
                                            )
                                        }
                                    }
                                }
                                else -> {}
                            }
                        }
                    } finally {
                        longPressJob.cancel()
                    }

                    // Resolve final intent when fingers are lifted
                    when (intent) {
                        GestureIntent.LONG_PRESS_DRAG -> {
                            coroutineScope.launch {
                                udpClient.send(
                                    createMessage(token,
                                        action = TrackpadMessage.ActionType.LEFT_CLICK,
                                        phase = TrackpadMessage.PhaseType.END)
                                )
                            }
                        }
                        GestureIntent.UNDECIDED -> {
                            when (maxPointers) {
                                2 -> {
                                    coroutineScope.launch {
                                        udpClient.sendClick(token, TrackpadMessage.ActionType.RIGHT_CLICK)
                                    }
                                }
                                1 -> {
                                    val secondDown = withTimeoutOrNull(doubleTapTimeoutMs) {
                                        awaitFirstDown(requireUnconsumed = false)
                                    }

                                    if (secondDown != null) {
                                        waitForUpOrCancellation()
                                        coroutineScope.launch {
                                            udpClient.sendClick(token, TrackpadMessage.ActionType.DOUBLE_CLICK)
                                        }
                                    } else {
                                        coroutineScope.launch {
                                            udpClient.sendClick(token, TrackpadMessage.ActionType.LEFT_CLICK)
                                        }
                                    }
                                }
                                else -> {}
                            }
                        }
                        else -> {}
                    }
                }
            }
    ) {
        // --- TOP-LEFT MENU UI ---
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
                // 🚀 NUEVO: Botón para escanear de nuevo
                DropdownMenuItem(
                    text = { Text("Scan QR") },
                    onClick = {
                        showMenu = false
                        onScanNewQr()
                    },
                    leadingIcon = { Icon(Icons.Default.QrCodeScanner, contentDescription = null) }
                )
                DropdownMenuItem(
                    text = { Text("Exit") },
                    onClick = {
                        showMenu = false
                        onExit()
                    },
                    leadingIcon = { Icon(Icons.Default.ExitToApp, contentDescription = null) }
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

private suspend fun UdpClient.sendClick(token: Int, action: TrackpadMessage.ActionType) {
    send(createMessage(token, action = action, phase = TrackpadMessage.PhaseType.START))
    send(createMessage(token, action = action, phase = TrackpadMessage.PhaseType.END))
}

private fun createMessage(
    token: Int,
    dx: Float = 0f,
    dy: Float = 0f,
    action: TrackpadMessage.ActionType = TrackpadMessage.ActionType.NO_ACTION,
    phase: TrackpadMessage.PhaseType = TrackpadMessage.PhaseType.START
): TrackpadMessage = trackpadMessage {
    this.authToken = token
    this.deltaX = dx
    this.deltaY = dy
    this.action = action
    this.phase = phase
    this.timestamp = System.currentTimeMillis()
    this.deviceName = android.os.Build.MODEL
}