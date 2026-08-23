use crate::domain::{ClipboardData, ClipboardError};
use crate::port::ClipboardPort;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Application service that coordinates clipboard operations and enforces invariants.
pub struct ClipboardManager<P: ClipboardPort> {
    port: P,
    enabled: AtomicBool,
    last_state: Mutex<ClipboardState>,
}

#[derive(Default)]
struct ClipboardState {
    /// The last clipboard content successfully read or written to prevent loops.
    last_synced: Option<ClipboardData>,
}

impl<P: ClipboardPort> ClipboardManager<P> {
    /// Creates a new manager. `enabled` dictates the initial opt-in status.
    pub fn new(port: P, enabled: bool) -> Self {
        Self {
            port,
            enabled: AtomicBool::new(enabled),
            last_state: Mutex::new(ClipboardState::default()),
        }
    }

    /// Returns a reference to the underlying clipboard port adapter.
    pub fn port(&self) -> &P {
        &self.port
    }

    /// Whether clipboard sharing is currently on.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Dynamically toggles clipboard sharing at runtime.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled && let Ok(mut state) = self.last_state.lock() {
            // Forget history so re-enabling re-syncs from a clean slate.
            state.last_synced = None;
        }
    }

    /// Checks if the local clipboard has changed.
    /// Returns `Some(ClipboardData)` if a new local copy event is detected.
    /// Returns `None` if no change has occurred, or if sharing is disabled.
    pub fn poll_local_change(&self) -> Result<Option<ClipboardData>, ClipboardError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(ClipboardError::Disabled);
        }

        let current = self.port.read()?;

        let mut state = self
            .last_state
            .lock()
            .map_err(|_| ClipboardError::Platform("Lock poisoned".to_string()))?;

        match (&current, &state.last_synced) {
            (Some(cur_data), Some(last_data)) if cur_data == last_data => {
                // Ignore matching payloads (prevent feedback loop / redundant transmissions)
                Ok(None)
            }
            (Some(cur_data), _) => {
                // Record it either way, so a payload we choose not to send is not
                // re-examined on every poll.
                state.last_synced = Some(cur_data.clone());
                // Don't propagate anything malformed or over the size cap.
                if let Err(e) = cur_data.validate() {
                    tracing::warn!(%e, "skipping local clipboard payload");
                    return Ok(None);
                }
                Ok(Some(cur_data.clone()))
            }
            (None, _) => {
                // Clipboard is empty or format unsupported
                Ok(None)
            }
        }
    }

    /// Injects a remote clipboard update into the local OS clipboard.
    /// Updates the internal state to prevent echoing this write back.
    pub fn handle_remote_update(&self, data: ClipboardData) -> Result<(), ClipboardError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(ClipboardError::Disabled);
        }

        // Reject malformed or oversized payloads before touching the OS clipboard.
        data.validate()?;

        // Write to system clipboard
        self.port.write(&data)?;

        // Update last synced to match what we just wrote
        let mut state = self
            .last_state
            .lock()
            .map_err(|_| ClipboardError::Platform("Lock poisoned".to_string()))?;
        state.last_synced = Some(data);

        Ok(())
    }
}
