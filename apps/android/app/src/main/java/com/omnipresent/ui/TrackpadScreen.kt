package com.omnipresent.ui

import android.app.Activity
import android.content.pm.ActivityInfo
import android.os.SystemClock
import android.view.HapticFeedbackConstants
import android.view.View
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
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
import androidx.compose.ui.input.pointer.*
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.util.fastForEach
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.omnipresent.network.UdpClient
import com.omnipresent.protocol.TrackpadMessage
import com.omnipresent.protocol.trackpadMessage
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.abs

private const val GESTURE_SLOP_ACCUMULATED = 8f
private const val THREE_FINGER_SWIPE_THRESHOLD = 30f

private enum class GestureIntent {
    UNDECIDED,
    CURSOR_MOVE,
    SCROLL,
    THREE_SWIPE,
}

// Represents the drag lifecycle state
private enum class DragState {
    IDLE,
    WAITING_FOR_LONG_PRESS,
    DRAGGING
}

@Composable
fun TrackpadScreen(
    ip: String,
    port: Int,
    token: Int,
    onExit: () -> Unit,
    onScanNewQr: () -> Unit,
) {
    val context = LocalContext.current
    val view = LocalView.current // Required for haptic feedback

    val udpClient = remember(ip, port) { UdpClient(ip, port) }
    var showMenu by remember { mutableStateOf(false) }

    val messageChannel = remember { Channel<TrackpadMessage>(Channel.UNLIMITED) }
    val seqCounter = remember { intArrayOf(0) }

    // Sends messages asynchronously to avoid blocking UI thread
    LaunchedEffect(messageChannel) {
        for (msg in messageChannel) {
            udpClient.send(msg)
        }
    }

    // Forces landscape mode and hides system UI for full trackpad experience
    DisposableEffect(Unit) {
        val activity = context as? Activity
        val window = activity?.window
        val originalOrientation =
            activity?.requestedOrientation ?: ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED

        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE
        window?.let {
            WindowCompat.getInsetsController(it, it.decorView).apply {
                systemBarsBehavior =
                    WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                hide(WindowInsetsCompat.Type.systemBars())
            }
        }

        onDispose {
            messageChannel.close()
            udpClient.close()
            activity?.requestedOrientation = originalOrientation
            window?.let {
                WindowCompat.getInsetsController(it, it.decorView)
                    .show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF121212))
            .pointerInput(token) {
                awaitEachGesture {
                    handleGesture(
                        token = token,
                        view = view,
                        getSeq = { seqCounter[0]++ },
                        send = { messageChannel.trySend(it) },
                    )
                }
            },
    ) {
        Box(
            modifier = Modifier
                .safeDrawingPadding()
                .padding(16.dp)
                .align(Alignment.TopStart),
        ) {
            IconButton(
                onClick = { showMenu = true },
                colors = IconButtonDefaults.iconButtonColors(
                    containerColor = Color.White.copy(alpha = 0.1f),
                    contentColor = Color.White,
                ),
            ) {
                Icon(Icons.Default.Menu, contentDescription = "Menu")
            }

            DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
                DropdownMenuItem(
                    text = { Text("Scan QR") },
                    onClick = { showMenu = false; onScanNewQr() },
                    leadingIcon = { Icon(Icons.Default.QrCodeScanner, contentDescription = null) },
                )
                DropdownMenuItem(
                    text = { Text("Exit") },
                    onClick = { showMenu = false; onExit() },
                    leadingIcon = { Icon(Icons.Default.ExitToApp, contentDescription = null) },
                )
            }
        }

        Text(
            text = "Trackpad Active",
            style = MaterialTheme.typography.bodyLarge,
            color = Color.White.copy(alpha = 0.2f),
            modifier = Modifier.align(Alignment.Center),
        )
    }
}

private suspend fun AwaitPointerEventScope.handleGesture(
    token: Int,
    view: View,
    getSeq: () -> Int,
    send: (TrackpadMessage) -> Unit,
) {
    val downEvent = awaitFirstDown(requireUnconsumed = false)
    val downTime = SystemClock.uptimeMillis() // Timestamp of first touch

    var intent = GestureIntent.UNDECIDED
    var dragState = DragState.WAITING_FOR_LONG_PRESS // Assume drag until proven otherwise
    var maxPointers = 1

    var slopAccumX = 0f
    var slopAccumY = 0f

    var swipeAccumX = 0f
    var swipeAccumY = 0f
    var swipeDispatched = false

    // Time threshold to detect long press for drag
    val LONG_PRESS_TIMEOUT_MS = 200L

    while (true) {
        // Wait conditionally depending on long press detection
        val event = if (dragState == DragState.WAITING_FOR_LONG_PRESS) {
            val timeRemaining = LONG_PRESS_TIMEOUT_MS - (SystemClock.uptimeMillis() - downTime)
            if (timeRemaining > 0) {
                withTimeoutOrNull(timeRemaining) { awaitPointerEvent() }
            } else {
                null // Timeout expired
            }
        } else {
            awaitPointerEvent()
        }

        // Timeout case: finger stayed still → trigger drag
        if (event == null) {
            if (dragState == DragState.WAITING_FOR_LONG_PRESS && maxPointers == 1 && intent == GestureIntent.UNDECIDED) {
                dragState = DragState.DRAGGING
                intent = GestureIntent.CURSOR_MOVE

                // Haptic feedback simulates physical click
                view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)

                // Send drag start (left click down)
                send(buildMessage(token, getSeq(), 0f, 0f, TrackpadMessage.ActionType.LEFT_CLICK, TrackpadMessage.PhaseType.START))
            }
            continue
        }

        val changes = event.changes
        var activeCount = 0
        changes.fastForEach { if (it.pressed) activeCount++ }

        // All fingers lifted → finish gesture
        if (activeCount == 0) {
            if (dragState == DragState.DRAGGING) {
                // Release click if dragging
                send(buildMessage(token, getSeq(), 0f, 0f, TrackpadMessage.ActionType.LEFT_CLICK, TrackpadMessage.PhaseType.END))
            }
            break
        }

        if (activeCount > maxPointers) {
            maxPointers = activeCount
            // Cancel drag if multi-touch detected
            if (dragState == DragState.WAITING_FOR_LONG_PRESS) {
                dragState = DragState.IDLE
            }
        }

        val pan = event.calculatePan()
        val dx = pan.x
        val dy = pan.y

        slopAccumX += dx
        slopAccumY += dy

        // Cancel drag if user moves too early
        if (dragState == DragState.WAITING_FOR_LONG_PRESS &&
            (abs(slopAccumX) > GESTURE_SLOP_ACCUMULATED || abs(slopAccumY) > GESTURE_SLOP_ACCUMULATED)) {
            dragState = DragState.IDLE
        }

        // Determine gesture intent once threshold is exceeded
        if (intent == GestureIntent.UNDECIDED &&
            (abs(slopAccumX) > GESTURE_SLOP_ACCUMULATED ||
                    abs(slopAccumY) > GESTURE_SLOP_ACCUMULATED)
        ) {
            intent = when (activeCount) {
                2 -> GestureIntent.SCROLL
                3 -> GestureIntent.THREE_SWIPE
                else -> GestureIntent.CURSOR_MOVE
            }
        }

        when (intent) {
            GestureIntent.CURSOR_MOVE -> {
                changes.fastForEach { if (it.pressed) it.consume() }
                if (dx != 0f || dy != 0f) {
                    send(
                        buildMessage(
                            token, getSeq(), dx, dy,
                            TrackpadMessage.ActionType.NO_ACTION,
                            TrackpadMessage.PhaseType.UPDATE
                        )
                    )
                }
            }

            GestureIntent.SCROLL -> {
                changes.fastForEach { if (it.pressed) it.consume() }
                if (dx != 0f || dy != 0f) {
                    val action = if (abs(dy) >= abs(dx))
                        TrackpadMessage.ActionType.VERTICAL_SCROLL
                    else
                        TrackpadMessage.ActionType.HORIZONTAL_SCROLL
                    send(
                        buildMessage(
                            token, getSeq(), dx, dy,
                            action, TrackpadMessage.PhaseType.UPDATE
                        )
                    )
                }
            }

            GestureIntent.THREE_SWIPE -> {
                changes.fastForEach { if (it.pressed) it.consume() }
                swipeAccumX += dx
                swipeAccumY += dy

                // Dispatch only once when threshold is exceeded
                if (!swipeDispatched &&
                    (abs(swipeAccumX) > THREE_FINGER_SWIPE_THRESHOLD ||
                            abs(swipeAccumY) > THREE_FINGER_SWIPE_THRESHOLD)
                ) {
                    swipeDispatched = true
                    val action = when {
                        abs(swipeAccumX) > abs(swipeAccumY) ->
                            if (swipeAccumX > 0) TrackpadMessage.ActionType.SWIPE_RIGHT
                            else TrackpadMessage.ActionType.SWIPE_LEFT
                        else ->
                            if (swipeAccumY > 0) TrackpadMessage.ActionType.SWIPE_DOWN
                            else TrackpadMessage.ActionType.SWIPE_UP
                    }
                    send(
                        buildMessage(
                            token, getSeq(), 0f, 0f,
                            action, TrackpadMessage.PhaseType.START
                        )
                    )
                }
            }

            GestureIntent.UNDECIDED -> Unit
        }
    }

    // Tap resolution phase
    when {
        // 2-finger tap → right click
        intent == GestureIntent.UNDECIDED && maxPointers == 2 -> {
            sendClick(token, getSeq, send, TrackpadMessage.ActionType.RIGHT_CLICK)
        }

        // 1-finger tap → left click (only if no drag happened)
        intent == GestureIntent.UNDECIDED && maxPointers == 1 && dragState != DragState.DRAGGING -> {
            sendClick(token, getSeq, send, TrackpadMessage.ActionType.LEFT_CLICK)
        }
    }
}

private fun sendClick(
    token: Int,
    getSeq: () -> Int,
    send: (TrackpadMessage) -> Unit,
    action: TrackpadMessage.ActionType,
) {
    send(buildMessage(token, getSeq(), 0f, 0f, action, TrackpadMessage.PhaseType.START))
    send(buildMessage(token, getSeq(), 0f, 0f, action, TrackpadMessage.PhaseType.END))
}

private fun buildMessage(
    token: Int,
    seq: Int,
    dx: Float,
    dy: Float,
    action: TrackpadMessage.ActionType,
    phase: TrackpadMessage.PhaseType,
): TrackpadMessage = trackpadMessage {
    authToken = token
    sequenceNumber = seq
    deltaX = dx
    deltaY = dy
    this.action = action
    this.phase = phase
    timestamp = System.currentTimeMillis()
}