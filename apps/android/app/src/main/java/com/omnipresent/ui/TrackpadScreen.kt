package com.omnipresent.ui

import android.app.Activity
import android.content.pm.ActivityInfo
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
import androidx.compose.ui.platform.LocalViewConfiguration
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

/**
 * Accumulated pixel delta required to exit UNDECIDED and commit to a gesture.
 * Compared against the running sum of per-frame deltas, NOT against individual
 * frame deltas. This lets slow, precise movements lock intent reliably.
 */
private const val GESTURE_SLOP_ACCUMULATED = 8f

/** Minimum accumulated delta to fire a 3-finger swipe action. */
private const val THREE_FINGER_SWIPE_THRESHOLD = 30f

/**
 * Represents what the user appears to be doing once the slop threshold is
 * crossed. Using a plain enum (not AtomicReference) is correct here because
 * [awaitEachGesture] runs entirely on a single coroutine — no cross-thread
 * mutation ever occurs.
 */
private enum class GestureIntent {
    UNDECIDED,
    CURSOR_MOVE,
    SCROLL,
    THREE_SWIPE,
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
    val doubleTapTimeoutMs = LocalViewConfiguration.current.doubleTapTimeoutMillis

    // Re-create the UDP client when the target changes, not on every recomposition.
    val udpClient = remember(ip, port) { UdpClient(ip, port) }
    var showMenu by remember { mutableStateOf(false) }

    // Channel decouples the UI-thread gesture handler from blocking network I/O.
    val messageChannel = remember { Channel<TrackpadMessage>(Channel.UNLIMITED) }

    // Plain IntArray avoids recomposition triggers while still being a stable
    // heap reference that survives across lambda captures.
    val seqCounter = remember { intArrayOf(0) }

    // ── Network dispatcher ────────────────────────────────────────────────────
    LaunchedEffect(messageChannel) {
        for (msg in messageChannel) {
            udpClient.send(msg)
        }
    }

    // ── Fullscreen landscape setup ────────────────────────────────────────────
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

    // ── UI ────────────────────────────────────────────────────────────────────
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF121212))
            // KEY on `token` so the gesture handler is reinstalled on session change.
            // Keying on `Unit` would keep a stale closure if the session rotates.
            .pointerInput(token) {
                awaitEachGesture {
                    handleGesture(
                        token = token,
                        doubleTapTimeoutMs = doubleTapTimeoutMs,
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

// ─────────────────────────────────────────────────────────────────────────────
// Core gesture handler
//
// Extracted into its own AwaitPointerEventScope extension for two reasons:
//   1. The TrackpadScreen composable stays readable.
//   2. The logic can be unit-tested independently of the composable.
// ─────────────────────────────────────────────────────────────────────────────

private suspend fun AwaitPointerEventScope.handleGesture(
    token: Int,
    doubleTapTimeoutMs: Long,
    getSeq: () -> Int,
    send: (TrackpadMessage) -> Unit,
) {
    // ── Phase 1: wait for the first finger ───────────────────────────────────
    awaitFirstDown(requireUnconsumed = false)

    var intent = GestureIntent.UNDECIDED
    var maxPointers = 1

    // ── Slop accumulator — intent detection only ──────────────────────────────
    // We sum per-frame deltas until we cross GESTURE_SLOP_ACCUMULATED, then lock
    // the intent. Using an accumulator instead of a per-frame check means slow,
    // precise movements will always lock intent — they just take a few more frames.
    // The individual delta size is completely irrelevant for locking intent.
    var slopAccumX = 0f
    var slopAccumY = 0f

    // ── 3-finger swipe accumulator ────────────────────────────────────────────
    var swipeAccumX = 0f
    var swipeAccumY = 0f
    var swipeDispatched = false

    // ── Phase 2: movement tracking ───────────────────────────────────────────
    while (true) {
        val event = awaitPointerEvent() // PointerEventPass.Main is the default
        val changes = event.changes

        // Count live pointers without allocating a filtered list.
        var activeCount = 0
        changes.fastForEach { if (it.pressed) activeCount++ }

        if (activeCount == 0) break // All fingers lifted — exit movement loop.
        if (activeCount > maxPointers) maxPointers = activeCount

        // Average delta across all active pointers (allocation-free Compose API).
        val pan = event.calculatePan()
        val dx = pan.x
        val dy = pan.y

        // Accumulate for slop check. Even 0.1f/frame adds up correctly across
        // many frames, so slow precise movements eventually lock intent.
        slopAccumX += dx
        slopAccumY += dy

        // Lock intent once the accumulated travel clears the threshold.
        // Evaluated BEFORE dispatching so the locking frame sends its delta too.
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
            // ── 1-finger move → cursor ───────────────────────────────────────
            // KEY FIX: dispatch every non-zero frame delta, no per-frame threshold.
            // The host cursor must accumulate every tiny nudge for precise control.
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

            // ── 2-finger move → scroll ───────────────────────────────────────
            // Same principle: no per-frame size gate once intent is locked.
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

            // ── 3-finger move → directional swipe (fire-once) ────────────────
            // Uses its own accumulator for direction; individual delta size
            // doesn't matter — we just need enough travel to pick an axis.
            GestureIntent.THREE_SWIPE -> {
                changes.fastForEach { if (it.pressed) it.consume() }
                swipeAccumX += dx
                swipeAccumY += dy

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

    // ── Phase 3: tap resolution ───────────────────────────────────────────────
    when {
        // ── 2-finger tap → Right Click ────────────────────────────────────────
        intent == GestureIntent.UNDECIDED && maxPointers == 2 -> {
            sendClick(token, getSeq, send, TrackpadMessage.ActionType.RIGHT_CLICK)
        }

        // ── 1-finger tap → Left Click + optional Double-Tap-Drag ─────────────
        intent == GestureIntent.UNDECIDED && maxPointers == 1 -> {
            // ① Immediate click — zero latency for normal taps.
            sendClick(token, getSeq, send, TrackpadMessage.ActionType.LEFT_CLICK)

            // ② Watch for a follow-up press that would start a drag.
            val dragDown = withTimeoutOrNull(doubleTapTimeoutMs) {
                awaitFirstDown(requireUnconsumed = false)
            } ?: return // Timeout: this was just a normal click, we're done.

            // ③ Second finger-down within the window → Double-tap drag begins.
            send(
                buildMessage(
                    token, getSeq(), 0f, 0f,
                    TrackpadMessage.ActionType.DOUBLE_CLICK,
                    TrackpadMessage.PhaseType.START
                )
            )

            while (true) {
                val dragEvent = awaitPointerEvent()
                val dragChanges = dragEvent.changes

                var dragActive = 0
                dragChanges.fastForEach { if (it.pressed) dragActive++ }
                if (dragActive == 0) break

                val dragPan = dragEvent.calculatePan()
                dragChanges.fastForEach { if (it.pressed) it.consume() }

                if (abs(dragPan.x) > 0f || abs(dragPan.y) > 0f) {
                    send(
                        buildMessage(
                            token, getSeq(), dragPan.x, dragPan.y,
                            TrackpadMessage.ActionType.NO_ACTION,
                            TrackpadMessage.PhaseType.UPDATE
                        )
                    )
                }
            }

            // ④ Finger lifted: release the held button.
            send(
                buildMessage(
                    token, getSeq(), 0f, 0f,
                    TrackpadMessage.ActionType.DOUBLE_CLICK,
                    TrackpadMessage.PhaseType.END
                )
            )
        }
    }
}

/** Sends a discrete click as a START immediately followed by an END. */
private fun sendClick(
    token: Int,
    getSeq: () -> Int,
    send: (TrackpadMessage) -> Unit,
    action: TrackpadMessage.ActionType,
) {
    send(buildMessage(token, getSeq(), 0f, 0f, action, TrackpadMessage.PhaseType.START))
    send(buildMessage(token, getSeq(), 0f, 0f, action, TrackpadMessage.PhaseType.END))
}

/** Constructs a [TrackpadMessage] from its constituent fields. */
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
