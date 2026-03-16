use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Mouse, Settings};

pub struct LinuxMouseStrategy {
    enigo: Enigo,
}

impl LinuxMouseStrategy {
    pub fn new() -> Self {
        Self {
            enigo: Enigo::new(&Settings::default()).expect("Fallo al inicializar Enigo"),
        }
    }

    fn handle_click_phase(&mut self, button: Button, phase: PhaseType) {
        let direction = match phase {
            PhaseType::Start => Direction::Press,
            PhaseType::End => Direction::Release,
            // None o Update se comportan como un click normal
            _ => Direction::Click,
        };

        let _ = self.enigo.button(button, direction);
    }
}

impl MouseStrategy for LinuxMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        let dx = delta_x.round() as i32;
        let dy = delta_y.round() as i32;
        let _ = self.enigo.move_mouse(dx, dy, Coordinate::Rel);
    }

    fn execute_click(&mut self, action: ActionType, phase: PhaseType) {
        // ¡Mira lo limpio que queda esto ahora!
        match action {
            ActionType::RightClick => self.handle_click_phase(Button::Right, phase),
            ActionType::LeftClick => self.handle_click_phase(Button::Left, phase),
            ActionType::DoubleClick => {
                let _ = self.enigo.button(Button::Left, Direction::Click);
                let _ = self.enigo.button(Button::Left, Direction::Click);
            }
            ActionType::HorizontalScroll => {
                let _ = self.enigo.scroll(1, Axis::Horizontal);
            }
            ActionType::VerticalScroll => {
                let _ = self.enigo.scroll(1, Axis::Vertical);
            }
            ActionType::SwipeLeft
            | ActionType::SwipeRight
            | ActionType::SwipeUp
            | ActionType::SwipeDown => {
                // Aquí puedes implementar lógica para swipes en el futuro
                println!("Swipe detectado, no implementado aún.");
            }
            ActionType::NoAction => {} // No hacemos nada
        }
    }
}
