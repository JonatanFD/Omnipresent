//! The ports through which the rest of the system reaches the OS input
//! subsystem. Domain code depends only on these traits; the platform-specific
//! code that actually talks to the OS lives in adapters behind them.

use omni_protocol::InputEvent;

/// Everything the desktop covers, across every display attached.
///
/// The whole desktop rather than the main display, because the cursor moves
/// freely over all of them: treating only the main one as real meant a cursor
/// moved onto a second monitor was reported as pinned to the main screen's edge,
/// and the next nudge in that direction looked exactly like the user pushing
/// past the edge — so control jumped to the peer that sat there.
///
/// The origin is where the OS puts the desktop's top-left corner, which is not
/// the main display's corner when a monitor sits above or to the left of it: on
/// Windows those coordinates are negative. The rest of the system works in a
/// 0-based space of `width` × `height`, and the adapters add or subtract the
/// origin at the boundary, so nothing above here has to think about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopBounds {
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: u32,
    pub height: u32,
}

impl DesktopBounds {
    pub const fn new(origin_x: i32, origin_y: i32, width: u32, height: u32) -> Self {
        Self {
            origin_x,
            origin_y,
            width,
            height,
        }
    }

    /// Converts an OS screen coordinate into the 0-based desktop space.
    pub const fn to_desktop(self, x: i32, y: i32) -> (i32, i32) {
        (x - self.origin_x, y - self.origin_y)
    }

    /// Converts a 0-based desktop coordinate back into OS screen coordinates.
    pub const fn to_screen(self, x: i32, y: i32) -> (i32, i32) {
        (x + self.origin_x, y + self.origin_y)
    }

    /// The desktop's size, which is what the virtual layout is built from.
    pub const fn size(self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Captures input events from the local OS — the only way the system reads the
/// keyboard and mouse. Implemented per platform (macOS, Linux).
pub trait InputSource {
    /// What can go wrong reading from the OS.
    type Error;

    /// Returns the next captured event, or `None` if none is available right now.
    /// Non-blocking: the event loop polls it repeatedly.
    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error>;

    /// Tells the source whether its events are currently routed to a remote
    /// machine. While suppressed, captured events keep flowing through `poll`
    /// but are withheld from the local OS, so input never acts on two machines
    /// at once. Sources with nothing to withhold (like the in-memory test
    /// source) ignore this.
    fn set_suppressed(&mut self, _suppressed: bool) {}
}

/// Injects input events into the local OS, used when this machine is the Target
/// and is receiving a peer's keyboard and mouse. Implemented per platform.
pub trait InputSink {
    /// What can go wrong writing to the OS.
    type Error;

    /// Synthesizes one event into the local OS as if it came from real hardware.
    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error>;

    /// Moves the local cursor to an absolute position in desktop space, used
    /// when the cursor enters this machine on an edge crossing. Sinks that
    /// cannot position absolutely approximate or ignore it.
    fn warp(&mut self, _x: i32, _y: i32) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_screen_desktop_needs_no_translation() {
        let bounds = DesktopBounds::new(0, 0, 1920, 1080);

        assert_eq!(bounds.to_desktop(100, 50), (100, 50));
        assert_eq!(bounds.to_screen(100, 50), (100, 50));
        assert_eq!(bounds.size(), (1920, 1080));
    }

    #[test]
    fn a_monitor_left_of_the_main_one_puts_the_origin_negative() {
        // A 1920-wide monitor to the left of a 1920-wide main display: the
        // desktop starts at -1920 and is 3840 across.
        let bounds = DesktopBounds::new(-1920, 0, 3840, 1080);

        // The far left of the desktop is 0 to us, -1920 to the OS.
        assert_eq!(bounds.to_desktop(-1920, 0), (0, 0));
        assert_eq!(bounds.to_screen(0, 0), (-1920, 0));
        // And the main display's own corner sits halfway along.
        assert_eq!(bounds.to_desktop(0, 0), (1920, 0));
    }

    #[test]
    fn translating_both_ways_gets_the_original_back() {
        let bounds = DesktopBounds::new(-1920, -200, 3840, 1480);

        for point in [(0, 0), (-1920, -200), (1919, 1079), (500, -100)] {
            let (dx, dy) = bounds.to_desktop(point.0, point.1);
            assert_eq!(bounds.to_screen(dx, dy), point);
        }
    }
}
