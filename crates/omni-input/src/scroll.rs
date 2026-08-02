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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_notch_is_one_line() {
        let mut acc = ScrollAccumulator::default();
        assert_eq!(
            acc.take_lines(ScrollDelta::new(0, MILLILINES_PER_LINE)),
            (0, 1)
        );
    }

    #[test]
    fn movement_shorter_than_a_line_yields_nothing_yet() {
        let mut acc = ScrollAccumulator::default();
        assert_eq!(acc.take_lines(ScrollDelta::new(0, 400)), (0, 0));
    }

    #[test]
    fn movement_shorter_than_a_line_adds_up_across_events() {
        let mut acc = ScrollAccumulator::default();
        acc.take_lines(ScrollDelta::new(0, 400));
        acc.take_lines(ScrollDelta::new(0, 400));
        // 1200 milli-lines have now arrived: one whole line comes out.
        assert_eq!(acc.take_lines(ScrollDelta::new(0, 400)), (0, 1));
    }

    #[test]
    fn what_is_left_over_stays_for_the_next_event() {
        let mut acc = ScrollAccumulator::default();
        acc.take_lines(ScrollDelta::new(0, MILLILINES_PER_LINE + 900));
        // 900 were held back, so only 100 more are needed for the next line.
        assert_eq!(acc.take_lines(ScrollDelta::new(0, 100)), (0, 1));
    }

    #[test]
    fn scrolling_down_adds_up_the_same_way() {
        let mut acc = ScrollAccumulator::default();
        acc.take_lines(ScrollDelta::new(0, -400));
        acc.take_lines(ScrollDelta::new(0, -400));
        assert_eq!(acc.take_lines(ScrollDelta::new(0, -400)), (0, -1));
    }

    #[test]
    fn reversing_direction_cancels_the_pending_amount() {
        let mut acc = ScrollAccumulator::default();
        acc.take_lines(ScrollDelta::new(0, 900));
        // The user scrolled back the other way before completing a line, so
        // nothing should come out in either direction.
        assert_eq!(acc.take_lines(ScrollDelta::new(0, -900)), (0, 0));
        assert_eq!(acc.take_lines(ScrollDelta::new(0, 0)), (0, 0));
    }

    #[test]
    fn the_two_axes_are_kept_apart() {
        let mut acc = ScrollAccumulator::default();
        acc.take_lines(ScrollDelta::new(900, 0));
        // The vertical axis has seen nothing, so it must not borrow the
        // horizontal remainder.
        assert_eq!(acc.take_lines(ScrollDelta::new(0, 900)), (0, 0));
        assert_eq!(acc.take_lines(ScrollDelta::new(100, 100)), (1, 1));
    }

    #[test]
    fn each_axis_can_be_scaled_by_its_own_amount() {
        // Windows sizes vertical scrolling in lines and horizontal in
        // characters, and the two settings are independent.
        let mut acc = ScrollAccumulator::default();
        let (x, y) = acc.take_scaled(
            ScrollDelta::new(3 * MILLILINES_PER_LINE, 3 * MILLILINES_PER_LINE),
            1,
            MILLILINES_PER_LINE,
            3 * MILLILINES_PER_LINE,
        );
        assert_eq!((x, y), (3, 1));
    }

    #[test]
    fn a_whole_line_becomes_a_thousand_millilines() {
        assert_eq!(millilines_from_lines(1.0), MILLILINES_PER_LINE);
    }

    #[test]
    fn a_fraction_of_a_line_keeps_its_precision() {
        // A trackpad reports fractions; they must survive the trip.
        assert_eq!(millilines_from_lines(0.25), 250);
    }

    #[test]
    fn scrolling_the_other_way_stays_negative() {
        assert_eq!(millilines_from_lines(-2.5), -2_500);
    }

    #[test]
    fn no_movement_stays_no_movement() {
        assert_eq!(millilines_from_lines(0.0), 0);
    }

    #[test]
    fn a_movement_too_small_to_measure_is_not_lost() {
        // Rounding would make this nothing, and a slow swipe is made of many of
        // them — so it would scroll nowhere at all.
        assert_eq!(millilines_from_lines(0.000_1), 1);
        assert_eq!(millilines_from_lines(-0.000_1), -1);
    }

    #[test]
    fn an_absurd_line_count_is_clamped_rather_than_wrapping() {
        assert_eq!(millilines_from_lines(f64::MAX), i32::MAX);
        assert_eq!(millilines_from_lines(f64::MIN), i32::MIN);
    }

    #[test]
    fn a_value_that_is_not_a_number_is_treated_as_no_movement() {
        assert_eq!(millilines_from_lines(f64::NAN), 0);
    }

    #[test]
    fn an_absurd_delta_cannot_overflow_the_remainder() {
        let mut acc = ScrollAccumulator::default();
        acc.take_lines(ScrollDelta::new(0, i32::MAX));
        // A peer sending nonsense must not panic the sink; the next ordinary
        // event still behaves.
        let (_, lines_y) = acc.take_lines(ScrollDelta::new(0, i32::MAX));
        assert!(lines_y > 0);
    }

    #[test]
    fn scaling_up_multiplies_before_it_divides() {
        // A Windows notch is 120 raw units and stands for however many lines
        // that machine's own setting says — three, by default.
        let mut scaled = Scaled::default();
        assert_eq!(scaled.take(120, 3 * MILLILINES_PER_LINE, 120), 3_000);
    }

    #[test]
    fn a_fraction_of_a_notch_is_not_thrown_away() {
        // High-resolution wheels and precision touchpads report far less than a
        // whole notch at a time. Dividing first would floor every one of them to
        // zero and scrolling would do nothing at all.
        let mut scaled = Scaled::default();
        assert_eq!(scaled.take(40, 3 * MILLILINES_PER_LINE, 120), 1_000);
    }

    #[test]
    fn amounts_too_small_to_convert_add_up_instead_of_vanishing() {
        let mut scaled = Scaled::default();
        // A twelfth of a notch: less than one whole output unit each time.
        assert_eq!(scaled.take(10, 1, 120), 0);
        assert_eq!(scaled.take(10, 1, 120), 0);
        // Twelve of them make a notch.
        for _ in 0..9 {
            assert_eq!(scaled.take(10, 1, 120), 0);
        }
        assert_eq!(scaled.take(10, 1, 120), 1);
    }

    #[test]
    fn a_notch_survives_the_trip_between_two_machines_set_up_the_same_way() {
        // The contract the scroll unit exists for: what the wire carries is a
        // distance in lines, so a machine that captures a notch as "three lines"
        // must have the machine at the other end scroll three lines — one notch
        // there too, not three. Getting this wrong is invisible in one direction
        // and multiplies the distance by three in the other.
        const WHEEL_DELTA: i32 = 120;
        const LINES_PER_NOTCH: i32 = 3;

        let mut captured = Scaled::default();
        let millilines = captured.take(WHEEL_DELTA, LINES_PER_NOTCH * MILLILINES_PER_LINE, 120);
        assert_eq!(millilines, 3_000, "one notch is three lines here");

        let mut injected = Scaled::default();
        let wheel = injected.take(
            millilines,
            WHEEL_DELTA,
            LINES_PER_NOTCH * MILLILINES_PER_LINE,
        );
        assert_eq!(wheel, WHEEL_DELTA, "and lands as exactly one notch again");
    }

    #[test]
    fn a_slower_machine_scrolls_less_for_the_same_notch() {
        // The distance is decided where the wheel turned. A machine set to one
        // line per notch asks for a third of what a machine set to three does,
        // and the receiving end honours that rather than its own preference.
        let mut slow = Scaled::default();
        let mut fast = Scaled::default();

        assert_eq!(slow.take(120, MILLILINES_PER_LINE, 120), 1_000);
        assert_eq!(fast.take(120, 3 * MILLILINES_PER_LINE, 120), 3_000);
    }

    #[test]
    fn a_denominator_of_zero_yields_nothing_rather_than_dividing_by_it() {
        let mut scaled = Scaled::default();
        assert_eq!(scaled.take(1_000, 1, 0), 0);
        assert_eq!(scaled.take(1_000, 1, -5), 0);
    }

    #[test]
    fn scaling_cannot_be_overflowed_by_an_absurd_value() {
        let mut scaled = Scaled::default();
        scaled.take(i32::MAX, i32::MAX, 1);
        // Still answering sensibly rather than having panicked or wrapped.
        assert!(scaled.take(1, 1, 1) >= 0);
    }
}
