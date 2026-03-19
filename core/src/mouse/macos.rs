use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};

// High-level Enigo library (v0.6.1) and Axis enum
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::thread;
use std::time::Duration;

pub struct MacOsMouseStrategy {
    enigo: Enigo,
    scroll_accumulator_y: f32,
    scroll_accumulator_x: f32,
}

impl MacOsMouseStrategy {
    pub fn new() -> Self {
        // Initialize Enigo with default configuration
        let enigo = Enigo::new(&Settings::default()).expect("FATAL: Failed to initialize Enigo");

        Self {
            enigo,
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
        }
    }

    /// Press Control, then tap the arrow key, and finally release Control
    fn send_shortcut(&mut self, key: Key) {
        let _ = self.enigo.key(Key::Control, Direction::Press);
        thread::sleep(Duration::from_millis(20));
        let _ = self.enigo.key(key, Direction::Click);
        thread::sleep(Duration::from_millis(20));
        let _ = self.enigo.key(Key::Control, Direction::Release);
    }
}

impl MouseStrategy for MacOsMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        // Coordinate::Rel moves the mouse relative to the current position.
        // Enigo automatically handles whether a button is pressed for dragging.
        let _ = self
            .enigo
            .move_mouse(delta_x as i32, delta_y as i32, Coordinate::Rel);
    }

    fn execute_action(&mut self, action: ActionType, phase: PhaseType, dx: f32, dy: f32) {
        match action {
            // Left click handling (Start = Press, End = Release, Other = quick Click)
            ActionType::LeftClick => match phase {
                PhaseType::Start => {
                    let _ = self.enigo.button(Button::Left, Direction::Press);
                }
                PhaseType::End => {
                    let _ = self.enigo.button(Button::Left, Direction::Release);
                }
                _ => {
                    let _ = self.enigo.button(Button::Left, Direction::Click);
                }
            },

            // Right click handling
            ActionType::RightClick => match phase {
                PhaseType::Start => {
                    let _ = self.enigo.button(Button::Right, Direction::Press);
                }
                PhaseType::End => {
                    let _ = self.enigo.button(Button::Right, Direction::Release);
                }
                _ => {
                    let _ = self.enigo.button(Button::Right, Direction::Click);
                }
            },

            // Scroll handling (with Axis correction for Enigo 0.6.1)
            ActionType::VerticalScroll | ActionType::HorizontalScroll => {
                self.scroll_accumulator_y += dy;
                self.scroll_accumulator_x += dx;

                let threshold = 5.0; // Scroll sensitivity

                // Evaluate and perform vertical scroll
                if self.scroll_accumulator_y.abs() >= threshold {
                    let scroll_y = (self.scroll_accumulator_y / threshold).trunc() as i32;
                    self.scroll_accumulator_y %= threshold;

                    // If scroll feels inverted due to macOS "Natural Scrolling",
                    // replace 'scroll_y' with '-scroll_y'
                    let _ = self.enigo.scroll(-scroll_y, Axis::Vertical);
                }

                // Evaluate and perform horizontal scroll
                if self.scroll_accumulator_x.abs() >= threshold {
                    let scroll_x = (self.scroll_accumulator_x / threshold).trunc() as i32;
                    self.scroll_accumulator_x %= threshold;

                    let _ = self.enigo.scroll(-scroll_x, Axis::Horizontal);
                }
            }

            // Map swipes to Control + Arrow shortcuts (Mission Control / Desktops)
            ActionType::SwipeUp => self.send_shortcut(Key::UpArrow),
            ActionType::SwipeDown => self.send_shortcut(Key::DownArrow),

            // Invert Left/Right intentionally to match natural hand gesture
            ActionType::SwipeLeft => self.send_shortcut(Key::RightArrow),
            ActionType::SwipeRight => self.send_shortcut(Key::LeftArrow),

            ActionType::NoAction => {
                self.scroll_accumulator_y = 0.0;
                self.scroll_accumulator_x = 0.0;
            }

            // Catch-all in case ActionType grows in the future
            _ => {}
        }
    }
}
