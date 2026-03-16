use crate::mouse::strategy::MouseStrategy;

pub struct LinuxMouseStrategy {
    // The library client would go here (e.g. enigo::Enigo)
}

impl LinuxMouseStrategy {
    pub fn new() -> Self {
        Self {}
    }
}

impl MouseStrategy for LinuxMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        // Real OS logic
    }

    fn execute_click(&mut self, action_id: i32, phase_id: i32) {
        // Real OS logic
    }
}
