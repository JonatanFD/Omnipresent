//! Turning the protocol's scroll unit into whole units a platform can inject —
//! and back again when capturing.
//!
//! Every adapter faces the same problem in one direction or the other. The wire
//! carries milli-lines, a fine unit that a trackpad can express, but each OS
//! measures scrolling in something coarser of its own: whole lines (macOS),
//! wheel clicks (Linux), or hundred-and-twentieths of a wheel notch (Windows).
//! Converting by dividing and throwing the remainder away would make slow
//! scrolling vanish entirely, so the remainder is kept and added to the next
//! event.
//!
//! [`Scaled`] is that arithmetic on its own: multiply by one number, divide by
//! another, keep what did not divide evenly. [`ScrollAccumulator`] is the
//! two-axis convenience over it for the platforms whose unit *is* the line.

use omni_protocol::input::{MILLILINES_PER_LINE, ScrollDelta};

/// Scales a stream of whole numbers by a fraction, remembering what did not
/// divide evenly so that small movements add up instead of disappearing.
///
/// One of these tracks one axis. Feeding it values in one unit and reading them
/// back in another is what lets a device that reports fractions — a trackpad, a
/// high-resolution wheel — travel through a coarser unit without being rounded
/// away to nothing.
#[derive(Debug, Default)]
pub struct Scaled {
    /// What is left over from earlier calls, in numerator units.
    pending: i32,
}

impl Scaled {
    /// An accumulator with nothing pending. Callable in a `const` context so a
    /// platform adapter can keep one in a `static` alongside its other hook state.
    pub const fn new() -> Self {
        Self { pending: 0 }
    }

    /// Adds `value * numerator` to what is pending and returns how many whole
    /// `denominator`s that makes. Anything short of one stays for the next call.
    ///
    /// A `denominator` of zero or less has no meaning; it yields nothing rather
    /// than dividing by it.
    pub fn take(&mut self, value: i32, numerator: i32, denominator: i32) -> i32 {
        if denominator <= 0 {
            return 0;
        }
        // In 64 bits, then clamped: part of `value` comes from the peer, and a
        // broken or hostile one must not be able to overflow this.
        let scaled = i64::from(value) * i64::from(numerator);
        let pending = i64::from(self.pending)
            .saturating_add(scaled)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;

        // Division truncates towards zero, so a negative amount yields a
        // negative result and the leftover keeps its sign — the pending amount
        // never flips direction between calls.
        let whole = pending / denominator;
        self.pending = pending - whole * denominator;
        whole
    }
}

/// Converts the protocol's milli-lines into whole units of a platform's own
/// scroll measure, one accumulator per axis so neither borrows the other's
/// leftover.
#[derive(Debug, Default)]
pub struct ScrollAccumulator {
    x: Scaled,
    y: Scaled,
}

impl ScrollAccumulator {
    /// Adds `delta` to what is pending and returns the whole lines to inject,
    /// horizontal first. Anything short of a full line stays for the next call.
    pub fn take_lines(&mut self, delta: ScrollDelta) -> (i32, i32) {
        self.take_scaled(delta, 1, MILLILINES_PER_LINE, MILLILINES_PER_LINE)
    }

    /// The general form: returns `delta * numerator` divided by each axis's
    /// denominator, keeping the remainders.
    ///
    /// The two axes take separate denominators because an OS may size them
    /// differently — Windows scrolls vertically by a number of *lines* and
    /// horizontally by a number of *characters*, each its own setting.
    pub fn take_scaled(
        &mut self,
        delta: ScrollDelta,
        numerator: i32,
        denominator_x: i32,
        denominator_y: i32,
    ) -> (i32, i32) {
        (
            self.x.take(delta.dx, numerator, denominator_x),
            self.y.take(delta.dy, numerator, denominator_y),
        )
    }
}

/// Converts a platform's fractional line count into the protocol's milli-lines.
///
/// macOS reports scrolling as a fractional number of lines, so a source there
/// has a `f64` to hand over. A movement the user made is never rounded away to
/// nothing, and a nonsensical value is clamped instead of wrapping around.
pub fn millilines_from_lines(lines: f64) -> i32 {
    // Casting a float to an integer in Rust clamps to the integer's range and
    // turns "not a number" into zero, which is exactly what is wanted here.
    let millilines = (lines * f64::from(MILLILINES_PER_LINE)).round() as i32;
    if millilines != 0 || lines == 0.0 || lines.is_nan() {
        return millilines;
    }
    // Too small to round to even one milli-line. Report the smallest movement
    // there is instead of nothing, so a slow swipe still gets somewhere.
    if lines.is_sign_negative() { -1 } else { 1 }
}
