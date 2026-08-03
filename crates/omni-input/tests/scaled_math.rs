use omni_input::scroll::Scaled;
use omni_protocol::input::MILLILINES_PER_LINE;

#[test]
fn scaling_up_multiplies_before_it_divides() {
    let mut scaled = Scaled::default();
    assert_eq!(scaled.take(120, 3 * MILLILINES_PER_LINE, 120), 3_000);
}

#[test]
fn a_fraction_of_a_notch_is_not_thrown_away() {
    let mut scaled = Scaled::default();
    assert_eq!(scaled.take(40, 3 * MILLILINES_PER_LINE, 120), 1_000);
}

#[test]
fn amounts_too_small_to_convert_add_up_instead_of_vanishing() {
    let mut scaled = Scaled::default();
    assert_eq!(scaled.take(10, 1, 120), 0);
    assert_eq!(scaled.take(10, 1, 120), 0);
    for _ in 0..9 {
        assert_eq!(scaled.take(10, 1, 120), 0);
    }
    assert_eq!(scaled.take(10, 1, 120), 1);
}

#[test]
fn a_notch_survives_the_trip_between_two_machines_set_up_the_same_way() {
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
    assert!(scaled.take(1, 1, 1) >= 0);
}
