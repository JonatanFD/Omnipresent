use log::{debug, info};

pub struct InputController {
    // Here we will later add the struct from the library you choose (e.g., enigo)
}

impl InputController {
    pub fn new() -> Self {
        Self {
            // Initialization
        }
    }

    pub fn move_mouse(&mut self, delta_x: f32, delta_y: f32) {
        // Logic to move the mouse in the OS
        debug!("Moving mouse: X:{}, Y:{}", delta_x, delta_y);
    }

    pub fn execute_action(&mut self, action: i32, phase: i32) {
        // Logic for clicks, scroll, etc.
        info!("Executing action: {}, phase: {}", action, phase);
    }
}
