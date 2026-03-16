use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};

pub struct LinuxMouseStrategy {
    device: VirtualDevice,
}

impl LinuxMouseStrategy {
    pub fn new() -> Self {
        // 1. Declaramos qué botones físicos tendrá nuestro "Mouse Virtual"
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);

        // 2. Declaramos qué ejes de movimiento tendrá (X, Y y Ruedas de Scroll)
        let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
        rel_axes.insert(RelativeAxisCode::REL_X);
        rel_axes.insert(RelativeAxisCode::REL_Y);
        rel_axes.insert(RelativeAxisCode::REL_WHEEL); // Scroll vertical
        rel_axes.insert(RelativeAxisCode::REL_HWHEEL); // Scroll horizontal

        // 3. Le pedimos al Kernel que cree el dispositivo USB falso
        let device = VirtualDeviceBuilder::new()
            .expect("Error al iniciar el constructor uinput")
            .name("Omnipresent Virtual Trackpad") // ¡Aparecerá así en tu configuración de Linux!
            .with_keys(&keys)
            .expect("Error agregando botones al dispositivo")
            .with_relative_axes(&rel_axes)
            .expect("Error agregando ejes de movimiento")
            .build()
            .expect("FATAL: Fallo al crear el dispositivo uinput. ¿Lo ejecutaste con sudo?");

        Self { device }
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
                // Click completo (presionar y soltar)
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

    fn execute_click(&mut self, action: ActionType, phase: PhaseType) {
        match action {
            ActionType::RightClick => self.handle_click_phase(KeyCode::BTN_RIGHT, phase),
            ActionType::LeftClick => self.handle_click_phase(KeyCode::BTN_LEFT, phase),
            ActionType::DoubleClick => {
                let _ = self.device.emit(&[
                    InputEvent::new(EventType::KEY.0, KeyCode::BTN_LEFT.0, 1),
                    InputEvent::new(EventType::KEY.0, KeyCode::BTN_LEFT.0, 0),
                    InputEvent::new(EventType::KEY.0, KeyCode::BTN_LEFT.0, 1),
                    InputEvent::new(EventType::KEY.0, KeyCode::BTN_LEFT.0, 0),
                ]);
            }
            ActionType::VerticalScroll => {
                let _ = self.device.emit(&[InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_WHEEL.0,
                    -1,
                )]);
            }
            ActionType::HorizontalScroll => {
                let _ = self.device.emit(&[InputEvent::new(
                    EventType::RELATIVE.0,
                    RelativeAxisCode::REL_HWHEEL.0,
                    1,
                )]);
            }
            ActionType::SwipeLeft
            | ActionType::SwipeRight
            | ActionType::SwipeUp
            | ActionType::SwipeDown => {
                println!("Swipe detectado, no implementado aún.");
            }
            ActionType::NoAction => {}
        }
    }
}
