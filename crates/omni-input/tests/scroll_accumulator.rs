use omni_input::scroll::{ScrollAccumulator, millilines_from_lines};
use omni_protocol::input::{MILLILINES_PER_LINE, ScrollDelta};

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
    assert_eq!(acc.take_lines(ScrollDelta::new(0, 400)), (0, 1));
}

#[test]
fn what_is_left_over_stays_for_the_next_event() {
    let mut acc = ScrollAccumulator::default();
    acc.take_lines(ScrollDelta::new(0, MILLILINES_PER_LINE + 900));
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
    assert_eq!(acc.take_lines(ScrollDelta::new(0, -900)), (0, 0));
    assert_eq!(acc.take_lines(ScrollDelta::new(0, 0)), (0, 0));
}

#[test]
fn the_two_axes_are_kept_apart() {
    let mut acc = ScrollAccumulator::default();
    acc.take_lines(ScrollDelta::new(900, 0));
    assert_eq!(acc.take_lines(ScrollDelta::new(0, 900)), (0, 0));
    assert_eq!(acc.take_lines(ScrollDelta::new(100, 100)), (1, 1));
}

#[test]
fn each_axis_can_be_scaled_by_its_own_amount() {
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
    let (_, lines_y) = acc.take_lines(ScrollDelta::new(0, i32::MAX));
    assert!(lines_y > 0);
}
