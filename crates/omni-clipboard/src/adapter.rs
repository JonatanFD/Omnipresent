use crate::domain::{ClipboardData, ClipboardError, ClipboardImage};
use crate::port::ClipboardPort;
use arboard::Clipboard;
use std::borrow::Cow;
use std::sync::Mutex;

/// Production adapter implementing `ClipboardPort` using the `arboard` crate.
///
/// Text and image (RGBA) sync are validated on **Windows and macOS**, where
/// `arboard` reads and writes both formats over the native pasteboard. **Linux
/// is not supported yet:** image sync there is unverified and the X11/Wayland
/// backends are not exercised — see `docs/STATUS.md` ("Not yet done").
///
/// The `arboard` handle is kept for the lifetime of the adapter instead of
/// being rebuilt on every `read`/`write`: the daemon polls every 500 ms when
/// sharing is on, and `Clipboard::new()` opens a fresh connection to the
/// display server (X11/Wayland) or takes the global clipboard lock (Windows)
/// each time. Reusing one handle turns a per-poll cost into a one-off.
///
/// If the display is not available at construction (a headless box, or one
/// whose session starts after the daemon), the handle is left `None` and
/// retried on the next `read`/`write` — so the adapter self-heals once the
/// display comes up instead of caching the failure forever.
pub struct ArboardAdapter {
    clipboard: Mutex<Option<Clipboard>>,
}

impl ArboardAdapter {
    pub fn new() -> Self {
        match Clipboard::new() {
            Ok(c) => Self {
                clipboard: Mutex::new(Some(c)),
            },
            Err(e) => {
                tracing::warn!(%e, "clipboard backend unavailable at startup; will retry on first use");
                Self {
                    clipboard: Mutex::new(None),
                }
            }
        }
    }

    /// Returns a locked handle, opening the clipboard lazily if it was not
    /// available at construction. The first successful open is cached so
    /// subsequent calls are free.
    fn acquire(&self) -> Result<std::sync::MutexGuard<'_, Option<Clipboard>>, ClipboardError> {
        let mut guard = self
            .clipboard
            .lock()
            .map_err(|_| ClipboardError::Platform("clipboard lock poisoned".to_string()))?;
        if guard.is_none() {
            match Clipboard::new() {
                Ok(c) => *guard = Some(c),
                Err(e) => return Err(ClipboardError::Platform(e.to_string())),
            }
        }
        Ok(guard)
    }
}

impl Default for ArboardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardPort for ArboardAdapter {
    fn read(&self) -> Result<Option<ClipboardData>, ClipboardError> {
        let mut guard = self.acquire()?;
        let clipboard = guard.as_mut().expect("acquire guaranteed a handle");

        // 1. Try reading text
        if let Ok(text) = clipboard.get_text()
            && !text.is_empty()
        {
            return Ok(Some(ClipboardData::Text(text)));
        }

        // 2. Try reading image
        if let Ok(image) = clipboard.get_image() {
            return Ok(Some(ClipboardData::Image(ClipboardImage {
                width: image.width as u32,
                height: image.height as u32,
                bytes: image.bytes.into_owned(),
            })));
        }

        Ok(None)
    }

    fn write(&self, data: &ClipboardData) -> Result<(), ClipboardError> {
        let mut guard = self.acquire()?;
        let clipboard = guard.as_mut().expect("acquire guaranteed a handle");

        match data {
            ClipboardData::Text(text) => {
                clipboard
                    .set_text(text.clone())
                    .map_err(|e| ClipboardError::Platform(e.to_string()))?;
            }
            ClipboardData::Image(image) => {
                image.validate()?;
                let img_data = arboard::ImageData {
                    width: image.width as usize,
                    height: image.height as usize,
                    bytes: Cow::Borrowed(&image.bytes),
                };
                clipboard
                    .set_image(img_data)
                    .map_err(|e| ClipboardError::Platform(e.to_string()))?;
            }
        }
        Ok(())
    }
}
