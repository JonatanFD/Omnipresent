//! Turning the protocol's scroll unit into whole units a platform can inject.
//!
//! Every sink faces the same problem: the wire carries milli-lines, a fine unit
//! that a trackpad can express, but the OS wants whole lines (macOS) or whole
//! wheel notches (Windows, Linux). Dividing and throwing the remainder away
//! would make slow scrolling vanish entirely, so the remainder is kept and added
//! to the next event.

use omni_protocol::input::{MILLILINES_PER_LINE, ScrollDelta};

/// Converts the protocol's milli-lines into whole lines, remembering what was
/// left over so small movements add up instead of disappearing.
#[derive(Debug, Default)]
pub struct ScrollAccumulator {
    remainder_x: i32,
    remainder_y: i32,
}

impl ScrollAccumulator {
    /// Adds `delta` to what is pending and returns the whole lines to inject,
    /// horizontal first. Anything short of a full line stays for the next call.
    pub fn take_lines(&mut self, delta: ScrollDelta) -> (i32, i32) {
        // Saturating, because the pending amount comes partly from the peer: a
        // broken or hostile one must not be able to overflow this.
        self.remainder_x = self.remainder_x.saturating_add(delta.dx);
        self.remainder_y = self.remainder_y.saturating_add(delta.dy);

        // Division truncates towards zero, so a negative amount yields negative
        // lines and what is left over keeps its sign — the pending amount never
        // flips direction between calls.
        let lines_x = self.remainder_x / MILLILINES_PER_LINE;
        let lines_y = self.remainder_y / MILLILINES_PER_LINE;
        self.remainder_x -= lines_x * MILLILINES_PER_LINE;
        self.remainder_y -= lines_y * MILLILINES_PER_LINE;

        (lines_x, lines_y)
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
}
