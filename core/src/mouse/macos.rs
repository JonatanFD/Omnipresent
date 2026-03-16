use crate::{
    mouse::strategy::MouseStrategy,
    network::{ActionType, PhaseType},
};

pub struct MacOsMouseStrategy;

impl MacOsMouseStrategy {
    pub fn new() -> Self {
        Self {}
    }
}

impl MouseStrategy for MacOsMouseStrategy {
    fn move_cursor(&mut self, _delta_x: f32, _delta_y: f32) {
        println!("[MACOS] (Not implemented yet)");
    }
    fn execute_click(&mut self, _action_id: ActionType, _phase_id: PhaseType) {}
}
