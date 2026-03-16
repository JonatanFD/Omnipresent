use crate::mouse::strategy::MouseStrategy;

pub struct WindowsMouseStrategy;

impl WindowsMouseStrategy {
    pub fn new() -> Self {
        Self {}
    }
}

impl MouseStrategy for WindowsMouseStrategy {
    fn move_cursor(&mut self, _delta_x: f32, _delta_y: f32) {
        println!("[WINDOWS] (Not implemented yet)");
    }
    fn execute_click(&mut self, _action_id: i32, _phase_id: i32) {}
}
