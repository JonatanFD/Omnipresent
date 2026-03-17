use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};

const SCROLL_THRESHOLD: f32 = 15.0;

pub struct LinuxMouseStrategy {
    device: VirtualDevice,
    scroll_accumulator_y: f32,
    scroll_accumulator_x: f32,
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
            .expect("Error al iniciar el constructor uinput")
            .name("Omnipresent Virtual Trackpad")
            .with_keys(&keys)
            .expect("Error agregando botones al dispositivo")
            .with_relative_axes(&rel_axes)
            .expect("Error agregando ejes de movimiento")
            .build()
            .expect("FATAL: Fallo al crear el dispositivo uinput.");

        Self {
            device,
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
        }
    }

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
                    // 1. Primer tap: Presionar
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        1,
                    )]);
                    thread::sleep(Duration::from_millis(30)); // Aumentamos un poco la pausa

                    // 2. Primer tap: Soltar
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        0,
                    )]);
                    thread::sleep(Duration::from_millis(30));

                    // 3. Segundo tap: Presionar y MANTENER
                    let _ = self.device.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        KeyCode::BTN_LEFT.0,
                        1,
                    )]);
                }
                PhaseType::End => {
                    // Si el End llega casi instantáneamente, a veces Linux no registra
                    // que hubo tiempo suficiente entre la "presión" y la "liberación"
                    // para el segundo clic. Le damos un ligerísimo margen.
                    thread::sleep(Duration::from_millis(20));

                    // 4. Segundo tap: Soltar
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
                // 🚀 Accumulate finger movement
                self.scroll_accumulator_y += dy;

                // Only emit a wheel "tick" when the threshold is exceeded
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

                    // Reset the accumulator but keep the remainder to avoid losing smoothness
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
