use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};

const SCROLL_THRESHOLD: f32 = 15.0;

pub struct LinuxMouseStrategy {
    device: VirtualDevice,
    scroll_accumulator_y: f32,
    scroll_accumulator_x: f32,
    last_sequence: u32, // NEW: Tracks packet order
}

impl LinuxMouseStrategy {
    pub fn new() -> Self {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);

        keys.insert(KeyCode::KEY_LEFTMETA);
        keys.insert(KeyCode::KEY_LEFTCTRL);
        keys.insert(KeyCode::KEY_LEFTALT);
        keys.insert(KeyCode::KEY_LEFT);
        keys.insert(KeyCode::KEY_RIGHT);

        let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
        rel_axes.insert(RelativeAxisCode::REL_X);
        rel_axes.insert(RelativeAxisCode::REL_Y);
        rel_axes.insert(RelativeAxisCode::REL_WHEEL);
        rel_axes.insert(RelativeAxisCode::REL_HWHEEL);

        let device = VirtualDeviceBuilder::new()
            .expect("Failed to initialize uinput builder")
            .name("Omnipresent Virtual Trackpad")
            .with_keys(&keys)
            .expect("Failed to add key capabilities to device")
            .with_relative_axes(&rel_axes)
            .expect("Failed to add relative axes")
            .build()
            .expect("FATAL: Failed to create uinput device");

        Self {
            device,
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
            last_sequence: 0,
        }
    }

    /// NEW: UDP Anti-Jitter Filter
    /// Call this before processing movement or actions.
    /// Returns `true` if the packet is fresh, `false` if it arrived late and should be dropped.
    pub fn accept_sequence(&mut self, seq: u32) -> bool {
        // Using wrapping subtraction handles the case where the u32 counter rolls over to 0.
        let diff = self.last_sequence.wrapping_sub(seq);

        // If the sequence is older than the last one (but by less than 1000 to account for wrap-around)
        // it means the UDP packet arrived out of order. Drop it.
        if diff > 0 && diff < 1000 {
            return false;
        }

        self.last_sequence = seq;
        true
    }

    // Handles click behavior depending on gesture phase
    fn handle_click_phase(&mut self, button: KeyCode, phase: PhaseType) {
        match phase {
            PhaseType::Start => {
                let _ = self
                    .device
                    .emit(&[InputEvent::new(EventType::KEY.0, button.0, 1)]);
            }
            PhaseType::End => {
                let _ = self
                    .device
                    .emit(&[InputEvent::new(EventType::KEY.0, button.0, 0)]);
            }
            _ => {
                let _ = self.device.emit(&[
                    InputEvent::new(EventType::KEY.0, button.0, 1),
                    InputEvent::new(EventType::KEY.0, button.0, 0),
                ]);
            }
        }
    }
}

impl MouseStrategy for LinuxMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        let dx = delta_x.round() as i32;
        let dy = delta_y.round() as i32;

        let mut events = Vec::new();

        if dx != 0 {
            events.push(InputEvent::new(
                EventType::RELATIVE.0,
                RelativeAxisCode::REL_X.0,
                dx,
            ));
        }

        if dy != 0 {
            events.push(InputEvent::new(
                EventType::RELATIVE.0,
                RelativeAxisCode::REL_Y.0,
                dy,
            ));
        }

        if !events.is_empty() {
            let _ = self.device.emit(&events);
        }
    }

    fn execute_action(&mut self, action: ActionType, phase: PhaseType, dx: f32, dy: f32) {
        use std::thread;
        use std::time::Duration;

        match action {
            ActionType::RightClick => self.handle_click_phase(KeyCode::BTN_RIGHT, phase),

            ActionType::LeftClick => self.handle_click_phase(KeyCode::BTN_LEFT, phase),

            ActionType::DoubleClick => match phase {
                PhaseType::Start => {
                    // First click: press
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        1,
                    )]);
                    // IMPORTANT: 60ms delay ensures Linux debouncer registers the click
                    thread::sleep(Duration::from_millis(60));

                    // First click: release
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        0,
                    )]);
                    thread::sleep(Duration::from_millis(60));

                    // Second click: press and HOLD (Starts Drag)
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        1,
                    )]);
                }
                PhaseType::End => {
                    // Second click: release (Ends Drag)
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        0,
                    )]);
                }
                _ => {
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        0,
                    )]);
                }
            },

            ActionType::VerticalScroll => {
                self.scroll_accumulator_y += dy;
                if self.scroll_accumulator_y.abs() >= SCROLL_THRESHOLD {
                    let scroll_direction = if self.scroll_accumulator_y > 0.0 {
                        1
                    } else {
                        -1
                    };
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_WHEEL.0,
                        scroll_direction,
                    )]);
                    self.scroll_accumulator_y %= SCROLL_THRESHOLD;
                }
            }

            ActionType::HorizontalScroll => {
                self.scroll_accumulator_x += dx;
                if self.scroll_accumulator_x.abs() >= SCROLL_THRESHOLD {
                    let scroll_direction = if self.scroll_accumulator_x > 0.0 {
                        -1
                    } else {
                        1
                    };
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_HWHEEL.0,
                        scroll_direction,
                    )]);
                    self.scroll_accumulator_x %= SCROLL_THRESHOLD;
                }
            }

            ActionType::SwipeUp => {
                let _ = self.device.emit(&[InputEvent::new(
                    EventType::KEY.0,
                    KeyCode::KEY_LEFTMETA.0,
                    1,
                )]);
                thread::sleep(Duration::from_millis(20));
                let _ = self.device.emit(&[InputEvent::new(
                    EventType::KEY.0,
                    KeyCode::KEY_LEFTMETA.0,
                    0,
                )]);
            }

            ActionType::SwipeLeft => {
                let _ = self.device.emit(&[
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTCTRL.0, 1),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTALT.0, 1),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_RIGHT.0, 1),
                ]);
                thread::sleep(Duration::from_millis(20));
                let _ = self.device.emit(&[
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_RIGHT.0, 0),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTALT.0, 0),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTCTRL.0, 0),
                ]);
            }

            ActionType::SwipeRight => {
                let _ = self.device.emit(&[
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTCTRL.0, 1),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTALT.0, 1),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFT.0, 1),
                ]);
                thread::sleep(Duration::from_millis(20));
                let _ = self.device.emit(&[
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFT.0, 0),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTALT.0, 0),
                    InputEvent::new(EventType::KEY.0, KeyCode::KEY_LEFTCTRL.0, 0),
                ]);
            }

            ActionType::SwipeDown => {
                let _ = self.device.emit(&[InputEvent::new(
                    EventType::KEY.0,
                    KeyCode::KEY_LEFTMETA.0,
                    1,
                )]);
                thread::sleep(Duration::from_millis(20));
                let _ = self.device.emit(&[InputEvent::new(
                    EventType::KEY.0,
                    KeyCode::KEY_LEFTMETA.0,
                    0,
                )]);
            }

            ActionType::NoAction => {
                self.scroll_accumulator_y = 0.0;
                self.scroll_accumulator_x = 0.0;
            }
        }
    }
}
