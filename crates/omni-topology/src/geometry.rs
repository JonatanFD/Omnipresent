//! Plain geometric value objects: screen sizes, points, and edges.

use omni_protocol::input::MouseDelta;
use serde::{Deserialize, Serialize};

/// The pixel dimensions of a machine's screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

impl Screen {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Rescales a mouse movement captured on one screen for use on another.
///
/// A screen's size is reported in whatever unit its OS uses, and those units are
/// not the same size: macOS reports points, so a Retina Mac calls itself
/// 1512 wide, while a DPI-aware Windows machine reports real pixels and calls a
/// 4K panel 3840 wide. Moving the cursor by "10" therefore covers very different
/// fractions of the screen depending on whose screen it is on, and the cursor
/// visibly changed speed the moment it crossed — slow on one machine, racing on
/// the other.
///
/// Matching the *fraction of the screen* covered is what makes a hand movement
/// feel the same on both. A move that would cross the whole of `from` crosses
/// the whole of `to`.
///
/// A movement that was not zero never scales down to zero: at a big enough
/// difference in size the slowest movements would otherwise round away and the
/// cursor would refuse to budge.
pub fn scale_delta(delta: MouseDelta, from: Screen, to: Screen) -> MouseDelta {
    MouseDelta::new(
        scale_axis(delta.dx, from.width, to.width),
        scale_axis(delta.dy, from.height, to.height),
    )
}

/// Rescales one axis, rounding to nearest and never losing a movement entirely.
fn scale_axis(delta: i32, from: u32, to: u32) -> i32 {
    if delta == 0 || from == 0 || to == 0 || from == to {
        return delta;
    }
    let scaled = (delta as i64 * to as i64) / from as i64;
    if scaled == 0 {
        // Too small to survive the division: keep the smallest movement there is,
        // in the direction it was going.
        return delta.signum();
    }
    scaled as i32
}

/// A cursor position, local to one machine's screen. Always kept within that
/// screen's bounds: `0..width` by `0..height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

impl Point {
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

/// One of the four screen edges. Used both to describe how machines are arranged
/// (which neighbor sits past which edge) and to report where the cursor leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// The edge you enter from when crossing onto a neighbor. Crossing off the
    /// right edge of one screen puts you on the left edge of the next.
    pub const fn opposite(self) -> Edge {
        match self {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }

    /// Whether crossing this edge is horizontal movement (left/right). The
    /// perpendicular axis — the one mapped onto the neighbor — is then vertical.
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_edges_pair_up() {
        assert_eq!(Edge::Left.opposite(), Edge::Right);
        assert_eq!(Edge::Right.opposite(), Edge::Left);
        assert_eq!(Edge::Top.opposite(), Edge::Bottom);
        assert_eq!(Edge::Bottom.opposite(), Edge::Top);
    }

    #[test]
    fn left_and_right_are_horizontal() {
        assert!(Edge::Left.is_horizontal());
        assert!(Edge::Right.is_horizontal());
        assert!(!Edge::Top.is_horizontal());
        assert!(!Edge::Bottom.is_horizontal());
    }

    #[test]
    fn a_movement_between_identical_screens_is_unchanged() {
        let screen = Screen::new(1920, 1080);
        let delta = MouseDelta::new(7, -3);

        assert_eq!(scale_delta(delta, screen, screen), delta);
    }

    #[test]
    fn a_movement_covers_the_same_fraction_of_either_screen() {
        // A Retina Mac reports points, a 4K PC reports pixels. Crossing the whole
        // of one must cross the whole of the other, or the cursor changes speed
        // the moment it crosses.
        let mac = Screen::new(1512, 982);
        let pc = Screen::new(3840, 2160);

        assert_eq!(
            scale_delta(MouseDelta::new(1512, 0), mac, pc),
            MouseDelta::new(3840, 0)
        );
        assert_eq!(
            scale_delta(MouseDelta::new(3840, 0), pc, mac),
            MouseDelta::new(1512, 0)
        );
    }

    #[test]
    fn each_axis_scales_by_its_own_side() {
        let from = Screen::new(1000, 100);
        let to = Screen::new(2000, 50);

        assert_eq!(
            scale_delta(MouseDelta::new(10, 10), from, to),
            MouseDelta::new(20, 5)
        );
    }

    #[test]
    fn direction_survives_scaling() {
        let from = Screen::new(1000, 1000);
        let to = Screen::new(500, 500);

        assert_eq!(
            scale_delta(MouseDelta::new(-100, 100), from, to),
            MouseDelta::new(-50, 50)
        );
    }

    #[test]
    fn the_smallest_movement_is_never_scaled_away() {
        // One unit going onto a much smaller screen would round to zero, and the
        // cursor would sit still however far the mouse was moved.
        let big = Screen::new(3840, 2160);
        let small = Screen::new(1280, 720);

        assert_eq!(
            scale_delta(MouseDelta::new(1, -1), big, small),
            MouseDelta::new(1, -1)
        );
    }

    #[test]
    fn no_movement_stays_no_movement() {
        let from = Screen::new(1000, 1000);
        let to = Screen::new(3000, 3000);

        assert_eq!(
            scale_delta(MouseDelta::default(), from, to),
            MouseDelta::default()
        );
    }

    #[test]
    fn a_screen_with_no_size_is_left_alone_rather_than_dividing_by_zero() {
        let unknown = Screen::new(0, 0);
        let real = Screen::new(1920, 1080);
        let delta = MouseDelta::new(5, 5);

        assert_eq!(scale_delta(delta, unknown, real), delta);
        assert_eq!(scale_delta(delta, real, unknown), delta);
    }
}
