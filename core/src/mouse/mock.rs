use log::info;

use crate::mouse::strategy::MouseStrategy;

pub struct MockMouseStrategy;

impl MockMouseStrategy {
    pub fn new() -> Self {
        Self {}
    }
}

impl MouseStrategy for MockMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        info!("[MOCK] Moviendo cursor -> X: {}, Y: {}", delta_x, delta_y);
    }

    fn execute_click(&mut self, action_id: i32, phase_id: i32) {
        info!(
            "[MOCK] Ejecutando acción {} en fase {}",
            action_id, phase_id
        );
    }
}
