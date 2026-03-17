package com.omnipresent.ui

import android.app.Activity
import android.content.pm.ActivityInfo
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExitToApp
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.PointerEvent
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.omnipresent.network.UdpClient
import com.omnipresent.protocol.TrackpadMessage
import com.omnipresent.protocol.trackpadMessage
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicReference
import kotlin.math.abs

// Gesture intent resolution state machine
private enum class GestureIntent {
    UNDECIDED,
    CURSOR_MOVE,
    SCROLL,
    THREE_SWIPE,
    LONG_PRESS_DRAG
}

private const val GESTURE_SLOP = 3f

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
    val longPressTimeoutMs = viewConfig.longPressTimeoutMillis
    val doubleTapTimeoutMs = viewConfig.doubleTapTimeoutMillis

    val context = LocalContext.current
    DisposableEffect(Unit) {
        val activity = context as? Activity
        val window = activity?.window
        val originalOrientation = activity?.requestedOrientation
            ?: ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED

        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE

        if (window != null) {
            val ctrl = WindowCompat.getInsetsController(window, window.decorView)
            ctrl.systemBarsBehavior =
                WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            ctrl.hide(WindowInsetsCompat.Type.systemBars())
        }

        onDispose {
            udpClient.close()
            activity?.requestedOrientation = originalOrientation
            if (window != null) {
                WindowCompat.getInsetsController(window, window.decorView)
                    .show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF121212))
            .pointerInput(Unit) {
                awaitEachGesture {

                    // Atomic lock to decide gesture intent (thread-safe, non-suspending)
                    val intentRef = AtomicReference(GestureIntent.UNDECIDED)

                    fun tryLockIntent(newIntent: GestureIntent): Boolean =
                        intentRef.compareAndSet(GestureIntent.UNDECIDED, newIntent)

                    var maxPointers = 0
                    var swipeDx = 0f
                    var swipeDy = 0f
                    var swipeDispatched = false

                    // Phase 1: initial touch
                    awaitFirstDown(requireUnconsumed = false)
                    maxPointers = 1

                    // Long press detection (converted into drag)
                    val longPressJob = coroutineScope.launch {
                        delay(longPressTimeoutMs)
                        if (tryLockIntent(GestureIntent.LONG_PRESS_DRAG)) {
                            udpClient.send(
                                createMessage(
                                    token,
                                    action = TrackpadMessage.ActionType.LEFT_CLICK,
                                    phase = TrackpadMessage.PhaseType.START
                                )
                            )
                        }
                    }

                    try {
                        while (true) {
                            val event: PointerEvent = awaitPointerEvent(PointerEventPass.Main)
                            val active = event.changes.filter { it.pressed }
                            val numFingers = active.size

                            if (numFingers > maxPointers) maxPointers = numFingers
                            if (numFingers == 0) break

                            var avgDx = 0f
                            var avgDy = 0f
                            active.forEach { p ->
                                val d = p.positionChange()
                                avgDx += d.x
                                avgDy += d.y
                            }
                            avgDx /= numFingers
                            avgDy /= numFingers

                            val hasMeaningfulMove =
                                abs(avgDx) > GESTURE_SLOP || abs(avgDy) > GESTURE_SLOP

                            // Decide gesture type based on fingers and movement
                            if (hasMeaningfulMove && intentRef.get() == GestureIntent.UNDECIDED) {
                                val newIntent = when (numFingers) {
                                    2 -> GestureIntent.SCROLL
                                    3 -> GestureIntent.THREE_SWIPE
                                    else -> GestureIntent.CURSOR_MOVE
                                }
                                if (tryLockIntent(newIntent)) {
                                    longPressJob.cancel()
                                }
                            }

                            when (intentRef.get()) {
                                // Cursor movement or drag
                                GestureIntent.CURSOR_MOVE,
                                GestureIntent.LONG_PRESS_DRAG -> {
                                    active.forEach { it.consume() }
                                    coroutineScope.launch {
                                        udpClient.send(
                                            createMessage(
                                                token,
                                                dx = avgDx, dy = avgDy,
                                                action = TrackpadMessage.ActionType.NO_ACTION,
                                                phase = TrackpadMessage.PhaseType.UPDATE
                                            )
                                        )
                                    }
                                }

                                // Two-finger scroll
                                GestureIntent.SCROLL -> {
                                    active.forEach { it.consume() }
                                    val scrollAction =
                                        if (abs(avgDy) > abs(avgDx))
                                            TrackpadMessage.ActionType.VERTICAL_SCROLL
                                        else
                                            TrackpadMessage.ActionType.HORIZONTAL_SCROLL

                                    coroutineScope.launch {
                                        udpClient.send(
                                            createMessage(
                                                token,
                                                dx = avgDx, dy = avgDy,
                                                action = scrollAction,
                                                phase = TrackpadMessage.PhaseType.UPDATE
                                            )
                                        )
                                    }
                                }

                                // Three-finger swipe (discrete action)
                                GestureIntent.THREE_SWIPE -> {
                                    active.forEach { it.consume() }
                                    swipeDx += avgDx
                                    swipeDy += avgDy

                                    if (!swipeDispatched &&
                                        (abs(swipeDx) > 30f || abs(swipeDy) > 30f)
                                    ) {
                                        swipeDispatched = true
                                        val swipeAction =
                                            if (abs(swipeDx) > abs(swipeDy))
                                                if (swipeDx > 0) TrackpadMessage.ActionType.SWIPE_RIGHT
                                                else TrackpadMessage.ActionType.SWIPE_LEFT
                                            else
                                                if (swipeDy > 0) TrackpadMessage.ActionType.SWIPE_DOWN
                                                else TrackpadMessage.ActionType.SWIPE_UP

                                        coroutineScope.launch {
                                            udpClient.send(
                                                createMessage(
                                                    token,
                                                    action = swipeAction,
                                                    phase = TrackpadMessage.PhaseType.START
                                                )
                                            )
                                        }
                                    }
                                }

                                GestureIntent.UNDECIDED -> {}
                            }
                        }
                    } finally {
                        longPressJob.cancel()
                    }

                    // Phase 3: resolve tap gestures (no movement)
                    when {
                        intentRef.get() == GestureIntent.LONG_PRESS_DRAG -> {
                            coroutineScope.launch {
                                udpClient.send(
                                    createMessage(
                                        token,
                                        action = TrackpadMessage.ActionType.LEFT_CLICK,
                                        phase = TrackpadMessage.PhaseType.END
                                    )
                                )
                            }
                        }

                        intentRef.get() == GestureIntent.UNDECIDED && maxPointers == 2 -> {
                            coroutineScope.launch {
                                udpClient.sendClick(token, TrackpadMessage.ActionType.RIGHT_CLICK)
                            }
                        }

                        intentRef.get() == GestureIntent.UNDECIDED && maxPointers == 1 -> {
                            val secondDown = withTimeoutOrNull(doubleTapTimeoutMs) {
                                awaitFirstDown(requireUnconsumed = false)
                            }
                            coroutineScope.launch {
                                if (secondDown != null) {
                                    udpClient.sendClick(token, TrackpadMessage.ActionType.DOUBLE_CLICK)
                                } else {
                                    udpClient.sendClick(token, TrackpadMessage.ActionType.LEFT_CLICK)
                                }
                            }
                        }
                    }
                }
            }
    ) {
        Box(
            modifier = Modifier
                .safeDrawingPadding()
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
                    text = { Text("Scan QR") },
                    onClick = { showMenu = false; onScanNewQr() },
                    leadingIcon = { Icon(Icons.Default.QrCodeScanner, contentDescription = null) }
                )
                DropdownMenuItem(
                    text = { Text("Exit") },
                    onClick = { showMenu = false; onExit() },
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

// Sends a full click (press + release)
private suspend fun UdpClient.sendClick(token: Int, action: TrackpadMessage.ActionType) {
    send(createMessage(token, action = action, phase = TrackpadMessage.PhaseType.START))
    send(createMessage(token, action = action, phase = TrackpadMessage.PhaseType.END))
}

// Builds a trackpad message
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