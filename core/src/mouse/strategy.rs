use crate::network::{ActionType, PhaseType};

pub trait MouseStrategy: Send {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32);
    fn execute_click(&mut self, action: ActionType, phase: PhaseType);
}
